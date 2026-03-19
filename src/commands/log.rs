use crate::core::{find_repo_root, get_current_branch, read_head, Object, ObjectHash};
use crate::response::{CommitEntry, LogResponse};
use crate::storage::ObjectStore;

pub fn execute(count: usize, _oneline: bool) -> Result<LogResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Get current HEAD
    let head_hash = match read_head(&repo_root) {
        Ok(hash) => hash,
        Err(_) => {
            return Ok(LogResponse {
                branch: get_current_branch(&repo_root).ok(),
                commits: vec![],
            });
        }
    };

    let current_branch = get_current_branch(&repo_root).ok();

    // Walk commit history
    let mut commits = Vec::new();
    let mut current = ObjectHash::from_hex(head_hash.clone());

    for _ in 0..count {
        match store.read(&current) {
            Ok(Object::Commit(commit)) => {
                let is_head = current.to_string() == head_hash;
                commits.push(CommitEntry {
                    hash: current.to_string(),
                    short_hash: current.short().to_string(),
                    author: commit.author.clone(),
                    timestamp: commit.timestamp,
                    message: commit.message.clone(),
                    is_head,
                });

                if commit.parents.is_empty() {
                    break;
                }

                current = commit.parents[0].clone();
            }
            Ok(_) => return Err("Expected commit object".into()),
            Err(_) => break,
        }
    }

    Ok(LogResponse {
        branch: current_branch,
        commits,
    })
}
