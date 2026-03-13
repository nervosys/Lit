use crate::core::{find_repo_root, read_head, Object, ObjectHash};
use crate::response::StashResponse;
use crate::storage::{Index, ObjectStore};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StashEntry {
    /// Commit HEAD was pointing to when stash was created
    pub head_commit: String,
    /// Branch name (if on a branch)
    pub branch: Option<String>,
    /// Hash of the tree object representing index state
    pub index_tree: String,
    /// Hash of the tree object representing working tree state
    pub worktree_tree: String,
    /// Message describing the stash
    pub message: String,
    /// Timestamp
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StashList {
    entries: Vec<StashEntry>,
}

impl StashList {
    fn load(repo_root: &std::path::Path) -> Result<Self, String> {
        let path = repo_root.join(".lit").join("stash");
        if !path.exists() {
            return Ok(StashList {
                entries: Vec::new(),
            });
        }
        let data = fs::read_to_string(&path).map_err(|e| format!("Failed to read stash: {}", e))?;
        serde_json::from_str(&data).map_err(|e| format!("Failed to parse stash: {}", e))
    }

    fn save(&self, repo_root: &std::path::Path) -> Result<(), String> {
        let path = repo_root.join(".lit").join("stash");
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize stash: {}", e))?;
        fs::write(&path, data).map_err(|e| format!("Failed to write stash: {}", e))
    }
}

pub fn execute(command: Option<crate::StashCommands>) -> Result<StashResponse, String> {
    let repo_root = find_repo_root()?;

    match command {
        None | Some(crate::StashCommands::Push { message: None }) => stash_push(&repo_root, None),
        Some(crate::StashCommands::Push { message }) => stash_push(&repo_root, message),
        Some(crate::StashCommands::Pop) => stash_pop(&repo_root),
        Some(crate::StashCommands::Apply { index }) => stash_apply(&repo_root, index),
        Some(crate::StashCommands::List) => stash_list(&repo_root),
        Some(crate::StashCommands::Drop { index }) => stash_drop(&repo_root, index),
    }
}

fn build_tree_from_working_dir(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<ObjectHash, String> {
    use crate::core::Tree;

    let mut tree = Tree::new();
    collect_files_to_tree(repo_root, repo_root, &mut tree, store)?;

    let tree_obj = Object::Tree(tree);
    store.write(&tree_obj)
}

fn collect_files_to_tree(
    repo_root: &std::path::Path,
    dir: &std::path::Path,
    tree: &mut crate::core::Tree,
    store: &ObjectStore,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip .lit directory
        if name == ".lit" {
            continue;
        }

        if path.is_file() {
            let content = fs::read(&path).map_err(|e| format!("Failed to read file: {}", e))?;
            let blob = crate::core::Blob::new(content);
            let blob_hash = store.write(&Object::Blob(blob))?;
            let rel_path = path
                .strip_prefix(repo_root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            tree.add_entry(
                "100644".to_string(),
                rel_path,
                blob_hash,
                "blob".to_string(),
            );
        }
    }
    Ok(())
}

fn build_tree_from_index(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<ObjectHash, String> {
    let index = Index::load(repo_root)?;
    let mut tree = crate::core::Tree::new();

    for entry in index.sorted_entries() {
        tree.add_entry(
            entry.mode.clone(),
            entry.path.clone(),
            ObjectHash::from_hex(entry.hash.clone()),
            "blob".to_string(),
        );
    }

    let tree_obj = Object::Tree(tree);
    store.write(&tree_obj)
}

fn stash_push(
    repo_root: &std::path::Path,
    message: Option<String>,
) -> Result<StashResponse, String> {
    let store = ObjectStore::new(repo_root);
    let head_commit = read_head(repo_root)?;
    let branch = crate::core::get_current_branch(repo_root).ok();

    // Save the current index tree
    let index_tree_hash = build_tree_from_index(repo_root, &store)?;

    // Save the current working tree
    let worktree_tree_hash = build_tree_from_working_dir(repo_root, &store)?;

    let msg = message
        .unwrap_or_else(|| format!("WIP on {}", branch.as_deref().unwrap_or("detached HEAD")));

    let entry = StashEntry {
        head_commit: head_commit.clone(),
        branch: branch.clone(),
        index_tree: index_tree_hash.to_string(),
        worktree_tree: worktree_tree_hash.to_string(),
        message: msg.clone(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    let mut stash_list = StashList::load(repo_root)?;
    stash_list.entries.push(entry);
    stash_list.save(repo_root)?;

    // Restore working tree to HEAD state
    restore_to_commit(repo_root, &head_commit)?;

    let index = stash_list.entries.len() - 1;
    Ok(StashResponse::Push {
        index,
        message: format!("Saved working directory and index state: {}", msg),
    })
}

fn stash_pop(repo_root: &std::path::Path) -> Result<StashResponse, String> {
    let mut stash_list = StashList::load(repo_root)?;

    if stash_list.entries.is_empty() {
        return Err("No stash entries".to_string());
    }

    let entry = stash_list.entries.pop().unwrap();
    let index = stash_list.entries.len();
    stash_list.save(repo_root)?;

    // Restore working tree from stash
    apply_stash_entry(repo_root, &entry)?;

    Ok(StashResponse::Pop {
        index,
        message: format!("Restored stash@{{{}}}: {}", index, entry.message),
    })
}

fn stash_apply(repo_root: &std::path::Path, idx: Option<usize>) -> Result<StashResponse, String> {
    let stash_list = StashList::load(repo_root)?;

    if stash_list.entries.is_empty() {
        return Err("No stash entries".to_string());
    }

    let index = idx.unwrap_or(stash_list.entries.len() - 1);

    if index >= stash_list.entries.len() {
        return Err(format!("stash@{{{}}} does not exist", index));
    }

    let entry = &stash_list.entries[index];
    apply_stash_entry(repo_root, entry)?;

    Ok(StashResponse::Apply {
        index,
        message: format!("Applied stash@{{{}}}: {}", index, entry.message),
    })
}

fn stash_list(repo_root: &std::path::Path) -> Result<StashResponse, String> {
    let stash_list = StashList::load(repo_root)?;

    let entries: Vec<crate::response::StashEntryInfo> = stash_list
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| crate::response::StashEntryInfo {
            index: i,
            message: e.message.clone(),
            branch: e.branch.clone(),
            timestamp: e.timestamp,
        })
        .collect();

    Ok(StashResponse::List { entries })
}

fn stash_drop(repo_root: &std::path::Path, idx: Option<usize>) -> Result<StashResponse, String> {
    let mut stash_list = StashList::load(repo_root)?;

    if stash_list.entries.is_empty() {
        return Err("No stash entries".to_string());
    }

    let index = idx.unwrap_or(stash_list.entries.len() - 1);

    if index >= stash_list.entries.len() {
        return Err(format!("stash@{{{}}} does not exist", index));
    }

    stash_list.entries.remove(index);
    stash_list.save(repo_root)?;

    Ok(StashResponse::Drop {
        index,
        message: format!("Dropped stash@{{{}}}", index),
    })
}

fn apply_stash_entry(repo_root: &std::path::Path, entry: &StashEntry) -> Result<(), String> {
    let store = ObjectStore::new(repo_root);
    let worktree_hash = ObjectHash::from_hex(entry.worktree_tree.clone());

    let tree = match store.read(&worktree_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Invalid stash: not a tree".to_string()),
    };

    // Restore files from the stashed working tree
    for te in &tree.entries {
        if te.object_type == "blob" {
            let blob = match store.read(&te.hash)? {
                Object::Blob(b) => b,
                _ => continue,
            };

            let full_path = repo_root.join(&te.name);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }

            fs::write(&full_path, &blob.content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    // Restore index from stashed index tree
    let index_hash = ObjectHash::from_hex(entry.index_tree.clone());
    let index_tree = match store.read(&index_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Invalid stash: index not a tree".to_string()),
    };

    let mut index = Index::new();
    for te in &index_tree.entries {
        index.add(te.name.clone(), te.hash.to_string(), te.mode.clone());
    }
    index.save(repo_root)?;

    Ok(())
}

fn restore_to_commit(repo_root: &std::path::Path, commit_hash: &str) -> Result<(), String> {
    let store = ObjectStore::new(repo_root);
    let hash = ObjectHash::from_hex(commit_hash.to_string());

    let commit = match store.read(&hash)? {
        Object::Commit(c) => c,
        _ => return Err("Not a commit".to_string()),
    };

    let tree = match store.read(&commit.tree)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".to_string()),
    };

    // Update working directory files
    for te in &tree.entries {
        if te.object_type == "blob" {
            let blob = match store.read(&te.hash)? {
                Object::Blob(b) => b,
                _ => continue,
            };
            let full_path = repo_root.join(&te.name);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }
            fs::write(&full_path, &blob.content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }

    // Reset index
    let mut index = Index::new();
    for te in &tree.entries {
        index.add(te.name.clone(), te.hash.to_string(), te.mode.clone());
    }
    index.save(repo_root)?;

    Ok(())
}
