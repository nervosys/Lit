use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash, Tree,
};
use crate::response::CommitResponse;
use crate::storage::{Index, ObjectStore};
use std::collections::HashMap;

pub fn execute(message: String, author: Option<String>) -> Result<CommitResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);
    let index = Index::load(&repo_root)?;

    if index.entries.is_empty() {
        return Err("Nothing to commit (staging area is empty)".into());
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
        Err(_) => vec![], // First commit
    };

    let parent_str = parents.first().map(|p| p.to_string());

    // Create commit object
    let commit = Commit::new(
        tree_hash.clone(),
        parents,
        author_name.clone(),
        message.clone(),
    );
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

    Ok(CommitResponse {
        hash: commit_hash.to_string(),
        short_hash: commit_hash.short().to_string(),
        tree: tree_hash.to_string(),
        parent: parent_str,
        author: author_name,
        message,
        timestamp,
    })
}

fn build_tree_from_index(index: &Index, store: &ObjectStore) -> Result<ObjectHash, crate::errors::LitError> {
    // Group files by directory
    let mut tree_map: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    for entry in index.sorted_entries() {
        let parts: Vec<&str> = entry.path.split('/').collect();

        if parts.len() == 1 {
            // Root level file
            tree_map.entry("".to_string()).or_default().push((
                parts[0].to_string(),
                entry.hash.clone(),
                entry.mode.clone(),
            ));
        } else {
            // Nested file
            let dir = parts[0].to_string();
            tree_map.entry(dir).or_default().push((
                parts[1..].join("/"),
                entry.hash.clone(),
                entry.mode.clone(),
            ));
        }
    }

    // Build root tree
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

    // Add subdirectories
    for dir in tree_map.keys() {
        if !dir.is_empty() {
            // Create subtree (simplified - doesn't handle deep nesting)
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

            let subtree_object = Object::Tree(subtree);
            let subtree_hash = store.write(&subtree_object)?;

            root_tree.add_entry(
                "040000".to_string(),
                dir.clone(),
                subtree_hash,
                "tree".to_string(),
            );
        }
    }

    // Write root tree
    let tree_object = Object::Tree(root_tree);
    store.write(&tree_object).map_err(Into::into)
}
