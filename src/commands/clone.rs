use crate::core::ObjectHash;
use crate::network::transport::{self, RemoteRepo};
use crate::network::AirgapValidator;
use crate::response::CloneResponse;
use crate::storage::ObjectStore;
use std::fs;
use std::path::Path;

pub fn execute(url: String, directory: Option<String>) -> Result<CloneResponse, String> {
    let validator = AirgapValidator::new()?;
    validator.validate_transport(&url)?;

    let remote_repo = RemoteRepo::open(&url)?;

    // Determine target directory
    let dir_name = if let Some(d) = directory {
        d
    } else {
        // Extract a sensible name from the URL
        let name = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("repo")
            .trim_end_matches(".git")
            .trim_end_matches(".lit");
        if name.is_empty() {
            "repo".to_string()
        } else {
            name.to_string()
        }
    };

    let target = std::env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?
        .join(&dir_name);

    if target.exists() {
        return Err(format!("Directory '{}' already exists", dir_name));
    }

    // Initialize the new repo
    fs::create_dir_all(&target).map_err(|e| format!("Failed to create directory: {}", e))?;

    let original_dir =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    std::env::set_current_dir(&target).map_err(|e| format!("Failed to change directory: {}", e))?;

    let init_result = crate::commands::init::execute(false, None);
    std::env::set_current_dir(&original_dir)
        .map_err(|e| format!("Failed to restore directory: {}", e))?;
    init_result?;

    // Set up remote
    let remotes_config = serde_json::json!({
        "remotes": {
            "origin": {
                "url": url
            }
        }
    });
    fs::write(
        target.join(".lit").join("remotes"),
        serde_json::to_string_pretty(&remotes_config)
            .map_err(|e| format!("Failed to serialize remotes: {}", e))?,
    )
    .map_err(|e| format!("Failed to write remotes config: {}", e))?;

    // Get remote branches and download objects
    let local_store = ObjectStore::new(&target);
    let remote_branches = remote_repo.list_branches()?;

    let wants: Vec<String> = remote_branches.iter().map(|(_, h)| h.clone()).collect();
    let needed = remote_repo.negotiate_download(&local_store, &wants)?;
    let total_transferred = remote_repo.download_objects(&local_store, &needed)?;

    // Create remote-tracking refs
    for (branch_name, hash) in &remote_branches {
        transport::update_remote_tracking_ref(&target, "origin", branch_name, hash)?;
    }

    // Determine default branch from remote HEAD
    let remote_head = remote_repo.read_head()?;
    let default_branch = if remote_head.starts_with("ref: refs/heads/") {
        remote_head
            .strip_prefix("ref: refs/heads/")
            .unwrap()
            .to_string()
    } else {
        "main".to_string()
    };

    // Set up local branch matching default
    if let Some((_, hash)) = remote_branches
        .iter()
        .find(|(name, _)| name == &default_branch)
    {
        crate::core::refs::write_ref(&target, &format!("heads/{}", default_branch), hash)?;
        fs::write(
            target.join(".lit").join("HEAD"),
            format!("ref: refs/heads/{}\n", default_branch),
        )
        .map_err(|e| format!("Failed to write HEAD: {}", e))?;

        checkout_tree(&target, &ObjectHash::from_hex(hash.clone()), &local_store)?;
    }

    Ok(CloneResponse {
        url: url.clone(),
        directory: dir_name.clone(),
        branches_cloned: remote_branches.iter().map(|(n, _)| n.clone()).collect(),
        objects_transferred: total_transferred,
        message: format!(
            "Cloned into '{}'\n  {} objects, {} branches",
            dir_name,
            total_transferred,
            remote_branches.len()
        ),
    })
}

/// Checkout working tree from a commit
fn checkout_tree(
    repo_path: &Path,
    commit_hash: &ObjectHash,
    store: &ObjectStore,
) -> Result<(), String> {
    use crate::core::Object;

    let commit_obj = store.read(commit_hash)?;
    let commit = match commit_obj {
        Object::Commit(c) => c,
        _ => return Err("Expected commit object".to_string()),
    };

    let tree_obj = store.read(&commit.tree)?;
    let tree = match tree_obj {
        Object::Tree(t) => t,
        _ => return Err("Expected tree object".to_string()),
    };

    checkout_tree_recursive(repo_path, &tree, store, repo_path)
}

fn checkout_tree_recursive(
    base_path: &Path,
    tree: &crate::core::Tree,
    store: &ObjectStore,
    _repo_path: &Path,
) -> Result<(), String> {
    use crate::core::Object;

    for entry in &tree.entries {
        let entry_path = base_path.join(&entry.name);

        let obj = store.read(&entry.hash)?;
        match obj {
            Object::Blob(blob) => {
                if let Some(parent) = entry_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create directory: {}", e))?;
                }
                fs::write(&entry_path, &blob.content)
                    .map_err(|e| format!("Failed to write file '{}': {}", entry.name, e))?;
            }
            Object::Tree(subtree) => {
                fs::create_dir_all(&entry_path)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
                checkout_tree_recursive(&entry_path, &subtree, store, _repo_path)?;
            }
            _ => {}
        }
    }
    Ok(())
}
