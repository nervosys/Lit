use crate::core::{find_repo_root, read_ref, update_head, write_ref, Object, ObjectHash, Tree};
use crate::response::CheckoutResponse;
use crate::storage::{Index, ObjectStore};
use std::fs;
use std::path::Path;

pub fn execute(
    target: String,
    create_new: bool,
) -> Result<CheckoutResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    if create_new {
        use crate::core::read_head;
        let head_hash = read_head(&repo_root)?;
        write_ref(&repo_root, &format!("heads/{}", target), &head_hash)?;
        update_head(&repo_root, &target)?;
        checkout_commit(&repo_root, &ObjectHash::from_hex(head_hash))?;
        Ok(CheckoutResponse {
            target,
            is_new_branch: true,
            is_detached: false,
        })
    } else if let Ok(commit_hash) = read_ref(&repo_root, &format!("heads/{}", target)) {
        update_head(&repo_root, &target)?;
        checkout_commit(&repo_root, &ObjectHash::from_hex(commit_hash))?;
        Ok(CheckoutResponse {
            target,
            is_new_branch: false,
            is_detached: false,
        })
    } else {
        let hash = ObjectHash::from_hex(target.clone());
        checkout_commit(&repo_root, &hash)?;
        use crate::core::set_head_detached;
        set_head_detached(&repo_root, &target)?;
        Ok(CheckoutResponse {
            target,
            is_new_branch: false,
            is_detached: true,
        })
    }
}

fn checkout_commit(
    repo_root: &Path,
    commit_hash: &ObjectHash,
) -> Result<(), crate::errors::LitError> {
    let store = ObjectStore::new(repo_root);

    // Read commit
    let commit = match store.read(commit_hash)? {
        Object::Commit(c) => c,
        _ => return Err("Not a commit".into()),
    };

    // Read tree
    let tree = match store.read(&commit.tree)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".into()),
    };

    // Update the working directory and the index in one walk.
    //
    // These were two walks, and only this one recursed. The index was rebuilt
    // from the root tree's entries alone, keyed by entry name, so a
    // subdirectory was recorded as though it were a file — `src` with the
    // subtree's hash — and the blobs beneath it got no entry at all. `status`
    // then called `fs::read` on a directory and failed outright, so any
    // repository with a subdirectory was broken by a checkout. Populating the
    // index from the same recursion that writes the files keeps the two in
    // step by construction.
    let mut index = Index::new();
    checkout_tree(repo_root, &tree, &store, "", &mut index)?;
    index.save(repo_root)?;

    Ok(())
}

/// Write a tree to the working directory, recording each blob in `index`
/// under its full path.
fn checkout_tree(
    repo_root: &Path,
    tree: &Tree,
    store: &ObjectStore,
    prefix: &str,
    index: &mut Index,
) -> Result<(), crate::errors::LitError> {
    for entry in &tree.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };

        let full_path = repo_root.join(&path);

        match entry.object_type.as_str() {
            "blob" => {
                // Write file
                let blob = match store.read(&entry.hash)? {
                    Object::Blob(b) => b,
                    _ => return Err("Expected blob".into()),
                };

                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create directory: {}", e))?;
                }

                fs::write(&full_path, &blob.content)
                    .map_err(|e| format!("Failed to write file: {}", e))?;

                index.add(path, entry.hash.to_string(), entry.mode.clone());
            }
            "tree" => {
                // Recursively checkout subtree
                let subtree = match store.read(&entry.hash)? {
                    Object::Tree(t) => t,
                    _ => return Err("Expected tree".into()),
                };

                checkout_tree(repo_root, &subtree, store, &path, index)?;
            }
            _ => {}
        }
    }

    Ok(())
}
