use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash, Tree,
};
use crate::response::RebaseResponse;
use crate::storage::ObjectStore;
use serde::{Deserialize, Serialize};
use std::fs;

/// Rebase todo entry for interactive rebase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebaseTodoEntry {
    pub action: String,
    pub hash: String,
    pub short_hash: String,
    pub message: String,
}

pub fn execute(
    base: String,
    interactive: bool,
    onto: Option<String>,
    abort: bool,
    cont: bool,
) -> Result<RebaseResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    if abort {
        return rebase_abort(&repo_root);
    }

    if cont {
        return rebase_continue(&repo_root);
    }

    if interactive {
        return rebase_interactive(&repo_root, &base);
    }

    rebase_noninteractive(&repo_root, &base, onto)
}

fn rebase_noninteractive(
    repo_root: &std::path::Path,
    base: &str,
    onto: Option<String>,
) -> Result<RebaseResponse, crate::errors::LitError> {
    let store = ObjectStore::new(repo_root);

    let base_hash = resolve_rev(repo_root, base)?;
    let head_hash = read_head(repo_root)?;
    let current_branch = get_current_branch(repo_root)?;

    let onto_hash = match &onto {
        Some(o) => resolve_rev(repo_root, o)?,
        None => base_hash.clone(),
    };

    // Collect commits to replay: HEAD back to (but not including) base
    let commits_to_replay = collect_commits_since(&store, &head_hash, &base_hash)?;

    if commits_to_replay.is_empty() {
        return Ok(RebaseResponse {
            rebased_commits: 0,
            onto: onto_hash[..16.min(onto_hash.len())].to_string(),
            branch: current_branch,
            message: "Already up to date, nothing to rebase".to_string(),
            todo: None,
        });
    }

    // Save rebase state
    save_rebase_state(repo_root, &current_branch, &head_hash, &onto_hash)?;

    // Replay commits one by one onto the new base
    let mut current_parent = onto_hash.clone();
    let mut replayed = 0;

    for (_commit_hash, commit) in commits_to_replay.iter().rev() {
        let new_parent_obj = ObjectHash::from_hex(current_parent.clone());

        // Apply this commit's changes on top of current_parent
        let new_tree = apply_commit_onto(&store, commit, &new_parent_obj)?;

        let new_commit = Commit::new(
            new_tree,
            vec![new_parent_obj],
            commit.author.clone(),
            commit.message.clone(),
        );

        let new_hash = store.write(&Object::Commit(new_commit))?;
        current_parent = new_hash.to_string();
        replayed += 1;
    }

    // Update branch ref
    write_ref(
        repo_root,
        &format!("heads/{}", current_branch),
        &current_parent,
    )?;

    // Clean up rebase state
    cleanup_rebase_state(repo_root)?;

    Ok(RebaseResponse {
        rebased_commits: replayed,
        onto: onto_hash[..16.min(onto_hash.len())].to_string(),
        branch: current_branch,
        message: format!(
            "Successfully rebased {} commit(s) onto {}",
            replayed,
            &onto_hash[..16.min(onto_hash.len())]
        ),
        todo: None,
    })
}

fn rebase_interactive(repo_root: &std::path::Path, base: &str) -> Result<RebaseResponse, crate::errors::LitError> {
    let store = ObjectStore::new(repo_root);

    let base_hash = resolve_rev(repo_root, base)?;
    let head_hash = read_head(repo_root)?;
    let current_branch = get_current_branch(repo_root)?;

    let commits = collect_commits_since(&store, &head_hash, &base_hash)?;

    if commits.is_empty() {
        return Ok(RebaseResponse {
            rebased_commits: 0,
            onto: base_hash[..16.min(base_hash.len())].to_string(),
            branch: current_branch,
            message: "Nothing to rebase".to_string(),
            todo: None,
        });
    }

    // Build interactive todo list (reversed so oldest first)
    let todo: Vec<RebaseTodoEntry> = commits
        .iter()
        .rev()
        .map(|(hash, commit)| RebaseTodoEntry {
            action: "pick".to_string(),
            hash: hash.clone(),
            short_hash: hash[..16.min(hash.len())].to_string(),
            message: commit.message.clone(),
        })
        .collect();

    // Save the todo to disk for the agent to modify and then `--continue`
    save_rebase_state(repo_root, &current_branch, &head_hash, &base_hash)?;
    let todo_path = repo_root.join(".lit").join("rebase").join("todo.json");
    let todo_json = serde_json::to_string_pretty(&todo)
        .map_err(|e| format!("Failed to serialize rebase todo: {}", e))?;
    fs::write(&todo_path, &todo_json).map_err(|e| format!("Failed to write rebase todo: {}", e))?;

    Ok(RebaseResponse {
        rebased_commits: 0,
        onto: base_hash[..16.min(base_hash.len())].to_string(),
        branch: current_branch,
        message: format!(
            "Interactive rebase started with {} commit(s). Edit .lit/rebase/todo.json and run `lit rebase --continue`",
            todo.len()
        ),
        todo: Some(serde_json::to_value(&todo).unwrap_or_default()),
    })
}

fn rebase_continue(repo_root: &std::path::Path) -> Result<RebaseResponse, crate::errors::LitError> {
    let rebase_dir = repo_root.join(".lit").join("rebase");
    if !rebase_dir.exists() {
        return Err("No rebase in progress".into());
    }

    let store = ObjectStore::new(repo_root);

    let branch = fs::read_to_string(rebase_dir.join("branch"))
        .map_err(|e| format!("Failed to read rebase state: {}", e))?
        .trim()
        .to_string();

    let onto_hash = fs::read_to_string(rebase_dir.join("onto"))
        .map_err(|e| format!("Failed to read rebase state: {}", e))?
        .trim()
        .to_string();

    let todo_path = rebase_dir.join("todo.json");
    let todo_json =
        fs::read_to_string(&todo_path).map_err(|e| format!("Failed to read rebase todo: {}", e))?;
    let todo: Vec<RebaseTodoEntry> = serde_json::from_str(&todo_json)
        .map_err(|e| format!("Failed to parse rebase todo: {}", e))?;

    let mut current_parent = onto_hash.clone();
    let mut replayed = 0;

    for entry in &todo {
        match entry.action.as_str() {
            "pick" | "p" => {
                let hash = ObjectHash::from_hex(entry.hash.clone());
                let commit = match store.read(&hash)? {
                    Object::Commit(c) => c,
                    _ => return Err(format!("'{}' is not a commit", entry.hash).into()),
                };

                let parent_obj = ObjectHash::from_hex(current_parent.clone());
                let new_tree = apply_commit_onto(&store, &commit, &parent_obj)?;

                let new_commit = Commit::new(
                    new_tree,
                    vec![parent_obj],
                    commit.author.clone(),
                    commit.message.clone(),
                );

                let new_hash = store.write(&Object::Commit(new_commit))?;
                current_parent = new_hash.to_string();
                replayed += 1;
            }
            "drop" | "d" => {
                // Skip this commit
                continue;
            }
            "reword" | "r" => {
                let hash = ObjectHash::from_hex(entry.hash.clone());
                let commit = match store.read(&hash)? {
                    Object::Commit(c) => c,
                    _ => return Err(format!("'{}' is not a commit", entry.hash).into()),
                };

                let parent_obj = ObjectHash::from_hex(current_parent.clone());
                let new_tree = apply_commit_onto(&store, &commit, &parent_obj)?;

                // Use the message from the todo entry (agent may have modified it)
                let new_commit = Commit::new(
                    new_tree,
                    vec![parent_obj],
                    commit.author.clone(),
                    entry.message.clone(),
                );

                let new_hash = store.write(&Object::Commit(new_commit))?;
                current_parent = new_hash.to_string();
                replayed += 1;
            }
            other => {
                return Err(format!("Unknown rebase action: '{}'", other).into());
            }
        }
    }

    write_ref(repo_root, &format!("heads/{}", branch), &current_parent)?;

    cleanup_rebase_state(repo_root)?;

    Ok(RebaseResponse {
        rebased_commits: replayed,
        onto: onto_hash[..16.min(onto_hash.len())].to_string(),
        branch,
        message: format!("Successfully rebased {} commit(s)", replayed),
        todo: None,
    })
}

fn rebase_abort(repo_root: &std::path::Path) -> Result<RebaseResponse, crate::errors::LitError> {
    let rebase_dir = repo_root.join(".lit").join("rebase");
    if !rebase_dir.exists() {
        return Err("No rebase in progress".into());
    }

    let branch = fs::read_to_string(rebase_dir.join("branch"))
        .map_err(|e| format!("Failed to read rebase state: {}", e))?
        .trim()
        .to_string();

    let orig_head = fs::read_to_string(rebase_dir.join("orig_head"))
        .map_err(|e| format!("Failed to read rebase state: {}", e))?
        .trim()
        .to_string();

    // Restore original HEAD
    write_ref(repo_root, &format!("heads/{}", branch), &orig_head)?;

    cleanup_rebase_state(repo_root)?;

    Ok(RebaseResponse {
        rebased_commits: 0,
        onto: String::new(),
        branch,
        message: "Rebase aborted, HEAD restored to original position".to_string(),
        todo: None,
    })
}

fn collect_commits_since(
    store: &ObjectStore,
    head: &str,
    base: &str,
) -> Result<Vec<(String, Commit)>, String> {
    let mut commits = Vec::new();
    let mut current = head.to_string();

    loop {
        if current == base {
            break;
        }

        let hash = ObjectHash::from_hex(current.clone());
        let commit = match store.read(&hash)? {
            Object::Commit(c) => c,
            _ => return Err("Not a commit in history".into()),
        };

        let parent = commit.parents.first().map(|p| p.to_string());
        commits.push((current, commit));

        match parent {
            Some(p) => current = p,
            None => break,
        }
    }

    Ok(commits)
}

fn apply_commit_onto(
    store: &ObjectStore,
    commit: &Commit,
    new_parent: &ObjectHash,
) -> Result<ObjectHash, crate::errors::LitError> {
    // Get parent's tree (the new base)
    let parent_commit = match store.read(new_parent)? {
        Object::Commit(c) => c,
        _ => return Err("Parent is not a commit".into()),
    };

    // Get the original parent's tree
    let orig_parent_tree = if let Some(orig_parent) = commit.parents.first() {
        match store.read(orig_parent)? {
            Object::Commit(c) => load_tree_files(store, &c.tree)?,
            _ => std::collections::HashMap::new(),
        }
    } else {
        std::collections::HashMap::new()
    };

    let commit_tree_files = load_tree_files(store, &commit.tree)?;
    let new_base_files = load_tree_files(store, &parent_commit.tree)?;

    let mut result_tree = Tree::new();

    // Start with new base, apply diff from orig_parent â†’ commit
    for (path, (hash, mode)) in &new_base_files {
        let in_orig = orig_parent_tree.get(path);
        let in_commit = commit_tree_files.get(path);

        match (in_orig, in_commit) {
            // File modified in commit â†’ use commit version
            (Some(ov), Some(cv)) if ov.0 != cv.0 => {
                result_tree.add_entry(
                    cv.1.clone(),
                    path.clone(),
                    ObjectHash::from_hex(cv.0.clone()),
                    "blob".to_string(),
                );
            }
            // File deleted in commit â†’ skip
            (Some(_), None) => continue,
            // No change â†’ keep new base version
            _ => {
                result_tree.add_entry(
                    mode.clone(),
                    path.clone(),
                    ObjectHash::from_hex(hash.clone()),
                    "blob".to_string(),
                );
            }
        }
    }

    // Files added in commit
    for (path, (hash, mode)) in &commit_tree_files {
        if !orig_parent_tree.contains_key(path) && !new_base_files.contains_key(path) {
            result_tree.add_entry(
                mode.clone(),
                path.clone(),
                ObjectHash::from_hex(hash.clone()),
                "blob".to_string(),
            );
        }
    }

    store.write(&Object::Tree(result_tree)).map_err(Into::into)
}

fn load_tree_files(
    store: &ObjectStore,
    tree_hash: &ObjectHash,
) -> Result<std::collections::HashMap<String, (String, String)>, crate::errors::LitError> {
    let tree = match store.read(tree_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".into()),
    };
    let mut files = std::collections::HashMap::new();
    for entry in &tree.entries {
        files.insert(
            entry.name.clone(),
            (entry.hash.to_string(), entry.mode.clone()),
        );
    }
    Ok(files)
}

fn save_rebase_state(
    repo_root: &std::path::Path,
    branch: &str,
    orig_head: &str,
    onto: &str,
) -> Result<(), crate::errors::LitError> {
    let rebase_dir = repo_root.join(".lit").join("rebase");
    fs::create_dir_all(&rebase_dir)
        .map_err(|e| format!("Failed to create rebase directory: {}", e))?;
    fs::write(rebase_dir.join("branch"), branch)
        .map_err(|e| format!("Failed to save rebase state: {}", e))?;
    fs::write(rebase_dir.join("orig_head"), orig_head)
        .map_err(|e| format!("Failed to save rebase state: {}", e))?;
    fs::write(rebase_dir.join("onto"), onto)
        .map_err(|e| format!("Failed to save rebase state: {}", e))?;
    Ok(())
}

fn cleanup_rebase_state(repo_root: &std::path::Path) -> Result<(), crate::errors::LitError> {
    let rebase_dir = repo_root.join(".lit").join("rebase");
    if rebase_dir.exists() {
        fs::remove_dir_all(&rebase_dir)
            .map_err(|e| format!("Failed to clean up rebase state: {}", e))?;
    }
    Ok(())
}

fn resolve_rev(repo_root: &std::path::Path, target: &str) -> Result<String, crate::errors::LitError> {
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
    if target == "HEAD" {
        return Ok(read_head(repo_root)?);
    }
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("heads/{}", target)) {
        return Ok(hash);
    }
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("tags/{}", target)) {
        return Ok(hash);
    }
    if target.len() >= 16 && target.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(target.to_string());
    }
    Err(format!("Cannot resolve '{}' to a commit", target).into())
}
