use crate::core::find_repo_root;
use crate::core::ObjectHash;
use crate::network::transport;
use crate::network::AirgapValidator;
use crate::response::FetchResponse;
use crate::storage::ObjectStore;

pub fn execute(remote: String, branch: Option<String>) -> Result<FetchResponse, String> {
    let repo_root = find_repo_root()?;
    let remote_url = get_remote_url(&repo_root, &remote)?;

    let validator = AirgapValidator::new()?;
    validator.validate_transport(&remote_url)?;

    let remote_path = transport::resolve_url(&remote_url)?;

    let local_store = ObjectStore::new(&repo_root);
    let remote_store = ObjectStore::new(&remote_path);

    // Determine which branches to fetch
    let remote_branches = transport::list_remote_branches(&remote_path)?;
    let branches_to_fetch: Vec<(String, String)> = if let Some(ref b) = branch {
        let hash = transport::read_remote_ref(&remote_path, b)?;
        vec![(b.clone(), hash)]
    } else {
        remote_branches
    };

    if branches_to_fetch.is_empty() {
        return Ok(FetchResponse {
            remote: remote.clone(),
            branches_updated: vec![],
            objects_transferred: 0,
            message: format!("No branches found on remote '{}'", remote),
        });
    }

    // Collect known objects locally for negotiation
    let known = transport::collect_known_hashes(&repo_root);

    // Walk each branch's commit graph and transfer objects
    let mut total_transferred = 0;
    let mut updated_branches = Vec::new();

    for (branch_name, hash) in &branches_to_fetch {
        let commit_hash = ObjectHash::from_hex(hash.clone());

        // Walk the remote's commit graph to find all needed objects
        let needed = transport::walk_commit_graph(&remote_store, &commit_hash, &known)?;

        // Transfer objects
        let transferred = transport::transfer_objects(&remote_store, &local_store, &needed)?;
        total_transferred += transferred;

        // Update remote-tracking ref
        transport::update_remote_tracking_ref(&repo_root, &remote, branch_name, hash)?;

        updated_branches.push(format!(
            "{} -> {}/{}",
            &hash[..16.min(hash.len())],
            remote,
            branch_name
        ));
    }

    let message = if total_transferred > 0 {
        format!(
            "From {}\n  {} objects transferred, {} branches updated",
            remote_url,
            total_transferred,
            updated_branches.len()
        )
    } else {
        format!("From {}\n  Already up to date", remote_url)
    };

    Ok(FetchResponse {
        remote,
        branches_updated: updated_branches,
        objects_transferred: total_transferred,
        message,
    })
}

fn get_remote_url(repo_root: &std::path::Path, remote_name: &str) -> Result<String, String> {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::fs;

    #[derive(Debug, Deserialize, Serialize)]
    struct Remote {
        url: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct RemoteConfig {
        remotes: HashMap<String, Remote>,
    }

    let config_path = repo_root.join(".lit").join("remotes");

    if !config_path.exists() {
        return Err(format!(
            "No remotes configured. Use 'lit remote add {} <url>'",
            remote_name
        ));
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read remotes config: {}", e))?;

    let config: RemoteConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse remotes config: {}", e))?;

    config
        .remotes
        .get(remote_name)
        .map(|r| r.url.clone())
        .ok_or_else(|| format!("Remote '{}' not found", remote_name))
}
