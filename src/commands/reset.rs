use crate::core::{
    find_repo_root, get_current_branch, read_head, set_head_detached, write_ref, Object,
    ObjectHash,
};
use crate::response::ResetResponse;
use crate::storage::{Index, ObjectStore};
use std::fs;

pub fn execute(target: String, soft: bool, hard: bool) -> Result<ResetResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Resolve target to a commit hash
    let commit_hash = execute_resolve(&repo_root, &target)?;

    // Verify it's a commit
    let hash_obj = ObjectHash::from_hex(commit_hash.clone());
    let commit = match store.read(&hash_obj)? {
        Object::Commit(c) => c,
        _ => return Err(format!("'{}' is not a commit", target).into()),
    };

    let mode = if soft {
        "soft"
    } else if hard {
        "hard"
    } else {
        "mixed"
    };

    // Soft: move HEAD only
    move_head(&repo_root, &commit_hash)?;

    if mode != "soft" {
        // Mixed + Hard: reset index to match commit's tree
        reset_index_to_tree(&repo_root, &store, &commit.tree)?;
    }

    if mode == "hard" {
        // Hard: also reset working tree
        reset_working_tree(&repo_root, &store, &commit.tree)?;
    }

    Ok(ResetResponse {
        target: commit_hash[..16.min(commit_hash.len())].to_string(),
        mode: mode.to_string(),
        message: format!(
            "HEAD is now at {} {}",
            &commit_hash[..16.min(commit_hash.len())],
            commit.message
        ),
    })
}

pub fn execute_resolve(repo_root: &std::path::Path, target: &str) -> Result<String, crate::errors::LitError> {
    // Try HEAD~N syntax
    if target.starts_with("HEAD~") || target.starts_with("HEAD^") {
        let count: usize = target[5..].parse().unwrap_or(1);
        let mut current = read_head(repo_root)?;
        let store = ObjectStore::new(repo_root);

        for _ in 0..count {
            let hash = ObjectHash::from_hex(current);
            let commit = match store.read(&hash)? {
                Object::Commit(c) => c,
                _ => return Err("Not a commit in history".into()),
            };
            current = commit
                .parents
                .first()
                .ok_or("No parent commit")?
                .to_string();
        }
        return Ok(current);
    }

    // Try HEAD
    if target == "HEAD" {
        return Ok(read_head(repo_root)?);
    }

    // Try as branch ref
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("heads/{}", target)) {
        return Ok(hash);
    }

    // Try as tag ref
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("tags/{}", target)) {
        return Ok(hash);
    }

    // Treat as raw hash
    if target.len() >= 16 && target.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(target.to_string());
    }

    Err(format!("Cannot resolve '{}' to a commit", target).into())
}

fn move_head(repo_root: &std::path::Path, commit_hash: &str) -> Result<(), crate::errors::LitError> {
    match get_current_branch(repo_root) {
        Ok(branch) => Ok(write_ref(repo_root, &format!("heads/{}", branch), commit_hash)?),
        Err(_) => Ok(set_head_detached(repo_root, commit_hash)?),
    }
}

fn reset_index_to_tree(
    repo_root: &std::path::Path,
    store: &ObjectStore,
    tree_hash: &ObjectHash,
) -> Result<(), crate::errors::LitError> {
    let tree = match store.read(tree_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".into()),
    };

    let mut index = Index::new();
    for entry in &tree.entries {
        index.add(
            entry.name.clone(),
            entry.hash.to_string(),
            entry.mode.clone(),
        );
    }
    index.save(repo_root).map_err(Into::into)
}

fn reset_working_tree(
    repo_root: &std::path::Path,
    store: &ObjectStore,
    tree_hash: &ObjectHash,
) -> Result<(), crate::errors::LitError> {
    let tree = match store.read(tree_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".into()),
    };

    // Remove tracked files that aren't in the tree
    // (simple approach: write all files from tree)
    for entry in &tree.entries {
        if entry.object_type == "blob" {
            let blob = match store.read(&entry.hash)? {
                Object::Blob(b) => b,
                _ => continue,
            };
            let full_path = repo_root.join(&entry.name);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }
            fs::write(&full_path, &blob.content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }
    Ok(())
}
