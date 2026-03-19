use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Blob, Commit, Object, ObjectHash,
    Tree,
};
use crate::response::SnapshotResponse;
use crate::storage::{Index, ObjectStore};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn execute(
    message: String,
    author: Option<String>,
    metadata: Option<serde_json::Value>,
) -> Result<SnapshotResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);
    let mut index = Index::load(&repo_root)?;

    // Stage all files (like `lit add .`)
    let files_added = add_all_files(&repo_root, &store, &mut index)?;
    index.save(&repo_root)?;

    if index.entries.is_empty() {
        return Err("Nothing to snapshot (no files in working directory)".into());
    }

    // Get author
    let author_name = if let Some(a) = author {
        a
    } else {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "Unknown".to_string())
    };

    // Build tree from index
    let tree_hash = build_tree_from_index(&index, &store)?;

    // Get parent commit(s)
    let parents = match read_head(&repo_root) {
        Ok(parent_hash) => vec![ObjectHash::from_hex(parent_hash)],
        Err(_) => vec![],
    };
    let parent_str = parents.first().map(|p| p.to_string());

    // Create commit object with optional metadata
    let mut commit = Commit::new(
        tree_hash.clone(),
        parents,
        author_name.clone(),
        message.clone(),
    );
    commit.metadata = metadata;

    let timestamp = commit.timestamp;
    let commit_object = Object::Commit(commit);
    let commit_hash = store.write(&commit_object)?;

    // Update the current branch reference
    let branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    write_ref(
        &repo_root,
        &format!("heads/{}", branch),
        commit_hash.as_str(),
    )?;

    Ok(SnapshotResponse {
        hash: commit_hash.to_string(),
        short_hash: commit_hash.short().to_string(),
        tree: tree_hash.to_string(),
        parent: parent_str,
        author: author_name,
        message,
        timestamp,
        files_added,
    })
}

fn add_all_files(
    repo_root: &Path,
    store: &ObjectStore,
    index: &mut Index,
) -> Result<usize, crate::errors::LitError> {
    let mut count = 0usize;
    for entry in WalkDir::new(repo_root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !name.starts_with('.') && name != "target" && name != "node_modules"
    }) {
        let entry = entry.map_err(|e| format!("Failed to read directory: {}", e))?;
        if entry.file_type().is_file() {
            let path = entry.path();
            if path.starts_with(repo_root.join(".lit")) {
                continue;
            }
            let content =
                fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let blob = Blob::new(content);
            let hash = store.write(&Object::Blob(blob))?;
            let rel_path = path
                .strip_prefix(repo_root)
                .map_err(|e| format!("Path error: {}", e))?
                .to_string_lossy()
                .replace('\\', "/");
            index.add(rel_path, hash.to_string(), "100644".to_string());
            count += 1;
        }
    }
    Ok(count)
}

fn build_tree_from_index(index: &Index, store: &ObjectStore) -> Result<ObjectHash, crate::errors::LitError> {
    let mut tree_map: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    for entry in index.sorted_entries() {
        let parts: Vec<&str> = entry.path.split('/').collect();
        if parts.len() == 1 {
            tree_map.entry("".to_string()).or_default().push((
                parts[0].to_string(),
                entry.hash.clone(),
                entry.mode.clone(),
            ));
        } else {
            let dir = parts[0].to_string();
            tree_map.entry(dir).or_default().push((
                parts[1..].join("/"),
                entry.hash.clone(),
                entry.mode.clone(),
            ));
        }
    }

    let mut root_tree = Tree::new();

    if let Some(root_files) = tree_map.get("") {
        for (name, hash, mode) in root_files {
            root_tree.add_entry(
                mode.clone(),
                name.clone(),
                ObjectHash::from_hex(hash.clone()),
                "blob".to_string(),
            );
        }
    }

    for dir in tree_map.keys() {
        if !dir.is_empty() {
            let mut subtree = Tree::new();
            if let Some(files) = tree_map.get(dir) {
                for (name, hash, mode) in files {
                    subtree.add_entry(
                        mode.clone(),
                        name.clone(),
                        ObjectHash::from_hex(hash.clone()),
                        "blob".to_string(),
                    );
                }
            }
            let subtree_hash = store.write(&Object::Tree(subtree))?;
            root_tree.add_entry(
                "040000".to_string(),
                dir.clone(),
                subtree_hash,
                "tree".to_string(),
            );
        }
    }

    store.write(&Object::Tree(root_tree)).map_err(Into::into)
}
