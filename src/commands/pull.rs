use crate::commands::{fetch, merge};
use crate::core::find_repo_root;
use crate::response::PullResponse;

pub fn execute(remote: String, branch: String) -> Result<PullResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    // Step 1: Fetch
    let fetch_result = fetch::execute(remote.clone(), Some(branch.clone()))?;

    // Step 2: Merge the remote-tracking branch into current
    let tracking_ref = format!("remotes/{}/{}", remote, branch);
    let remote_hash = crate::core::refs::read_ref(&repo_root, &tracking_ref)
        .map_err(|_| format!("No tracking ref for '{}/{}'", remote, branch))?;

    // Check if current branch tip matches — already up to date
    let _current_branch =
        crate::core::refs::get_current_branch(&repo_root).unwrap_or_else(|_| "HEAD".to_string());
    let local_hash = crate::core::refs::read_head(&repo_root).ok();

    if local_hash.as_deref() == Some(&remote_hash) {
        return Ok(PullResponse {
            remote: remote.clone(),
            branch: branch.clone(),
            objects_fetched: fetch_result.objects_transferred,
            fast_forward: false,
            has_conflicts: false,
            merge_message: "Already up to date".to_string(),
            message: format!(
                "From {}\n  Already up to date ({} objects fetched)",
                remote, fetch_result.objects_transferred
            ),
        });
    }

    // Step 3: Merge
    // Use the tracking ref name as the merge source
    let merge_branch = format!("{}/{}", remote, branch);
    let merge_result = merge::execute(merge_branch, None)?;

    Ok(PullResponse {
        remote: remote.clone(),
        branch: branch.clone(),
        objects_fetched: fetch_result.objects_transferred,
        fast_forward: merge_result.fast_forward,
        has_conflicts: merge_result.has_conflicts,
        merge_message: merge_result.message.clone(),
        message: format!(
            "From {}\n  {} objects fetched\n  {}",
            remote, fetch_result.objects_transferred, merge_result.message
        ),
    })
}
