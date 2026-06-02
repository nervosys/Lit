use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash, Tree,
};
use crate::response::CommitResponse;
use crate::storage::{Index, ObjectStore};
use std::collections::HashMap;

/// Amend the most recent commit with currently staged changes.
/// If no new changes are staged, re-uses the existing tree.
/// Optionally updates the commit message.
pub fn execute(
    message: Option<String>,
    author: Option<String>,
) -> Result<CommitResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);
    let index = Index::load(&repo_root)?;

    // Read parent commit
    let head_hash = read_head(&repo_root)?;
    let head_obj = store.read(&ObjectHash::from_hex(head_hash.clone()))?;

    let old_commit = match head_obj {
        Object::Commit(c) => c,
        _ => return Err("HEAD is not a commit".into()),
    };

    // Build new tree from index (if staging area has content) or re-use old tree
    let tree_hash = if index.entries.is_empty() {
        old_commit.tree.clone()
    } else {
        build_tree_from_index(&index, &store)?
    };

    // Use new message or keep old one
    let msg = message.unwrap_or_else(|| old_commit.message.clone());

    // Use new author or keep old one
    let author_name = author.unwrap_or_else(|| old_commit.author.clone());

    // Keep the same parents as the original commit
    let parents = old_commit.parents.clone();
    let parent_str = parents.first().map(|p| p.to_string());

    // Create amended commit
    let commit = Commit::new(tree_hash.clone(), parents, author_name.clone(), msg.clone());
    let timestamp = commit.timestamp;
    let commit_object = Object::Commit(commit);
    let commit_hash = store.write(&commit_object)?;

    // Update branch ref
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
        message: msg,
        timestamp,
    })
}

fn build_tree_from_index(
    index: &Index,
    store: &ObjectStore,
) -> Result<ObjectHash, crate::errors::LitError> {
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
    for (dir, files) in &tree_map {
        if dir.is_empty() {
            for (name, hash, mode) in files {
                root_tree.add_entry(
                    mode.clone(),
                    name.clone(),
                    ObjectHash::from_hex(hash.clone()),
                    "blob".to_string(),
                );
            }
        } else {
            let mut sub_tree = Tree::new();
            for (name, hash, mode) in files {
                sub_tree.add_entry(
                    mode.clone(),
                    name.clone(),
                    ObjectHash::from_hex(hash.clone()),
                    "blob".to_string(),
                );
            }
            let sub_hash = store.write(&Object::Tree(sub_tree))?;
            root_tree.add_entry(
                "040000".to_string(),
                dir.clone(),
                sub_hash,
                "tree".to_string(),
            );
        }
    }
    Ok(store.write(&Object::Tree(root_tree))?)
}
