use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash,
};
use crate::response::CommitResponse;
use crate::storage::ObjectStore;

/// Reword the commit message of the most recent commit (or a specified commit).
pub fn execute(
    new_message: String,
    _target: Option<String>,
) -> Result<CommitResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let head_hash = read_head(&repo_root)?;
    let head_obj = store.read(&ObjectHash::from_hex(head_hash.clone()))?;

    let old_commit = match head_obj {
        Object::Commit(c) => c,
        _ => return Err("HEAD is not a commit".into()),
    };

    let parents = old_commit.parents.clone();
    let parent_str = parents.first().map(|p| p.to_string());
    let author_name = old_commit.author.clone();
    let tree_hash = old_commit.tree.clone();

    // Create new commit with updated message, same tree and parents
    let commit = Commit::new(
        tree_hash.clone(),
        parents,
        author_name.clone(),
        new_message.clone(),
    );
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
        message: new_message,
        timestamp,
    })
}
