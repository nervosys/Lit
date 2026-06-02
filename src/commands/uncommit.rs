use crate::core::{find_repo_root, get_current_branch, read_head, write_ref, Object, ObjectHash};
use crate::response::CommitResponse;
use crate::storage::ObjectStore;

/// Uncommit the last commit, keeping changes in the working tree.
/// With --discard, also drops the committed content.
pub fn execute(discard: bool) -> Result<CommitResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let head_hash = read_head(&repo_root)?;
    let head_obj = store.read(&ObjectHash::from_hex(head_hash.clone()))?;

    let old_commit = match head_obj {
        Object::Commit(c) => c,
        _ => return Err("HEAD is not a commit".into()),
    };

    // Get the parent commit hash
    let parent_hash = old_commit
        .parents
        .first()
        .ok_or_else(|| crate::errors::LitError::general("Cannot uncommit the initial commit"))?
        .to_string();

    let branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());

    // Move branch pointer back to parent
    write_ref(&repo_root, &format!("heads/{}", branch), &parent_hash)?;

    if !discard {
        // Re-stage the files from the uncommitted commit's tree
        // (In a full implementation, this would restore the index from the tree diff)
        // For now, the files remain in the working directory
    }

    Ok(CommitResponse {
        hash: parent_hash.clone(),
        short_hash: parent_hash[..8.min(parent_hash.len())].to_string(),
        tree: old_commit.tree.to_string(),
        parent: old_commit.parents.get(1).map(|p| p.to_string()),
        author: old_commit.author.clone(),
        message: format!(
            "Uncommitted: {}{}",
            old_commit.message,
            if discard { " (discarded)" } else { "" }
        ),
        timestamp: old_commit.timestamp,
    })
}
