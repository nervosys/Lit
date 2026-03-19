use crate::core::find_repo_root;
use crate::network::transport::RemoteRepo;
use crate::network::AirgapValidator;
use crate::response::FetchResponse;
use crate::storage::ObjectStore;

pub fn execute(remote: String, branch: Option<String>) -> Result<FetchResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let remote_url = get_remote_url(&repo_root, &remote)?;

    let validator = AirgapValidator::new()?;
    validator.validate_transport(&remote_url)?;

    let remote_repo = RemoteRepo::open(&remote_url)?;
    let local_store = ObjectStore::new(&repo_root);

    // Determine which branches to fetch
    let branches_to_fetch: Vec<(String, String)> = if let Some(ref b) = branch {
        let hash = remote_repo.read_branch_ref(b)?;
        vec![(b.clone(), hash)]
    } else {
        remote_repo.list_branches()?
    };

    if branches_to_fetch.is_empty() {
        return Ok(FetchResponse {
            remote: remote.clone(),
            branches_updated: vec![],
            objects_transferred: 0,
            message: format!("No branches found on remote '{}'", remote),
        });
    }

    // Negotiate and download objects
    let wants: Vec<String> = branches_to_fetch.iter().map(|(_, h)| h.clone()).collect();
    let needed = remote_repo.negotiate_download(&local_store, &wants)?;
    let total_transferred = remote_repo.download_objects(&local_store, &needed)?;

    // Update remote-tracking refs
    let mut updated_branches = Vec::new();
    for (branch_name, hash) in &branches_to_fetch {
        crate::network::transport::update_remote_tracking_ref(
            &repo_root,
            &remote,
            branch_name,
            hash,
        )?;
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

fn get_remote_url(repo_root: &std::path::Path, remote_name: &str) -> Result<String, crate::errors::LitError> {
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
        ).into());
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read remotes config: {}", e))?;

    let config: RemoteConfig = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse remotes config: {}", e))?;

    config
        .remotes
        .get(remote_name)
        .map(|r| r.url.clone())
        .ok_or_else(|| format!("Remote '{}' not found", remote_name).into())
}
