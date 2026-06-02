use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash,
};
use crate::response::CommitResponse;
use crate::storage::ObjectStore;

/// Squash the last N commits into a single commit.
pub fn execute(
    count: usize,
    message: Option<String>,
) -> Result<CommitResponse, crate::errors::LitError> {
    if count < 2 {
        return Err("Squash requires at least 2 commits".into());
    }

    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let head_hash = read_head(&repo_root)?;

    // Walk back `count` commits to find the base
    let mut current_hash = head_hash.clone();
    let mut messages: Vec<String> = Vec::new();
    let mut final_tree = String::new();
    let mut final_author = String::new();

    for i in 0..count {
        let obj = store.read(&ObjectHash::from_hex(current_hash.clone()))?;
        match obj {
            Object::Commit(c) => {
                if i == 0 {
                    final_tree = c.tree.to_string();
                    final_author = c.author.clone();
                }
                messages.push(c.message.clone());
                if let Some(parent) = c.parents.first() {
                    current_hash = parent.to_string();
                } else if i < count - 1 {
                    return Err(format!(
                        "Only {} commits available, cannot squash {}",
                        i + 1,
                        count
                    )
                    .into());
                }
            }
            _ => return Err("Unexpected non-commit object in history".into()),
        }
    }

    messages.reverse();

    // The parent of the squashed commit is the parent of the oldest squashed commit
    let base_obj = store.read(&ObjectHash::from_hex(current_hash.clone()))?;
    let parents = match base_obj {
        Object::Commit(_) => vec![crate::core::ObjectHash::from_hex(current_hash.clone())],
        _ => vec![],
    };
    let parent_str = Some(current_hash);

    let squash_message = message.unwrap_or_else(|| {
        let mut msg = String::from("Squashed commits:\n\n");
        for m in &messages {
            msg.push_str(&format!("* {}\n", m));
        }
        msg
    });

    let tree_hash = crate::core::ObjectHash::from_hex(final_tree.clone());
    let commit = Commit::new(
        tree_hash,
        parents,
        final_author.clone(),
        squash_message.clone(),
    );
    let timestamp = commit.timestamp;
    let commit_object = Object::Commit(commit);
    let commit_hash = store.write(&commit_object)?;

    let branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    write_ref(
        &repo_root,
        &format!("heads/{}", branch),
        commit_hash.as_str(),
    )?;

    Ok(CommitResponse {
        hash: commit_hash.to_string(),
        short_hash: commit_hash.short().to_string(),
        tree: final_tree,
        parent: parent_str,
        author: final_author,
        message: squash_message,
        timestamp,
    })
}
