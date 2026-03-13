use crate::core::merge::MergeStrategy;
use crate::core::{find_repo_root, get_current_branch, write_ref, Commit, Object, ObjectHash};
use crate::response::ResolveResponse;
use crate::storage::{Index, ObjectStore};
use std::fs;
use std::str::FromStr;

/// Resolve merge conflicts programmatically
///
/// Modes:
///   - `lit resolve <file> --strategy=ours` — take our version
///   - `lit resolve <file> --strategy=theirs` — take their version
///   - `lit resolve --all --strategy=ours` — resolve all with strategy
///   - `lit resolve --continue` — finalize merge after manual resolution
pub fn execute(
    file: Option<String>,
    strategy: Option<String>,
    all: bool,
    finish: bool,
) -> Result<ResolveResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let merge_dir = repo_root.join(".lit").join("merge");
    if !merge_dir.exists() {
        return Err("No merge in progress. Nothing to resolve.".to_string());
    }

    if finish {
        return finalize_merge(&repo_root, &store);
    }

    let strategy = match &strategy {
        Some(s) => MergeStrategy::from_str(s)?,
        None => return Err("--strategy is required for resolve (ours or theirs)".to_string()),
    };

    // Read conflict state
    let conflicts_path = merge_dir.join("conflicts.json");
    let conflicts_data = fs::read_to_string(&conflicts_path)
        .map_err(|e| format!("Failed to read conflict state: {}", e))?;
    let file_results: Vec<crate::core::merge::FileMergeResult> =
        serde_json::from_str(&conflicts_data)
            .map_err(|e| format!("Failed to parse conflict state: {}", e))?;

    let merge_head_str = fs::read_to_string(merge_dir.join("MERGE_HEAD"))
        .map_err(|e| format!("Failed to read MERGE_HEAD: {}", e))?
        .trim()
        .to_string();
    let merge_head = ObjectHash::from_hex(merge_head_str);

    let head_hash_str = crate::core::read_head(&repo_root)?;
    let head_hash = ObjectHash::from_hex(head_hash_str);

    // Get trees for ours/theirs
    let ours_tree = get_commit_tree(&store, &head_hash)?;
    let theirs_tree = get_commit_tree(&store, &merge_head)?;
    let ours_files = crate::core::diff::collect_tree_files(&ours_tree, &store, "")?;
    let theirs_files = crate::core::diff::collect_tree_files(&theirs_tree, &store, "")?;

    let ours_map: std::collections::HashMap<String, ObjectHash> = ours_files.into_iter().collect();
    let theirs_map: std::collections::HashMap<String, ObjectHash> =
        theirs_files.into_iter().collect();

    let mut resolved_files = Vec::new();
    let mut index = Index::load(&repo_root)?;

    let conflicting: Vec<&crate::core::merge::FileMergeResult> = file_results
        .iter()
        .filter(|r| r.status == crate::core::merge::FileMergeStatus::Conflict)
        .collect();

    for conflict in &conflicting {
        let should_resolve = all || file.as_deref() == Some(&conflict.path);
        if !should_resolve {
            continue;
        }

        let chosen_hash = match strategy {
            MergeStrategy::Ours => ours_map.get(&conflict.path),
            MergeStrategy::Theirs => theirs_map.get(&conflict.path),
            MergeStrategy::Recursive => {
                return Err(
                    "Cannot use 'recursive' strategy for resolve. Use 'ours' or 'theirs'."
                        .to_string(),
                );
            }
        };

        if let Some(hash) = chosen_hash {
            // Update index with resolved version
            let entry = crate::storage::index::IndexEntry {
                path: conflict.path.clone(),
                hash: hash.to_string(),
                mode: "100644".to_string(),
            };
            index.entries.insert(conflict.path.clone(), entry);

            // Write file to working tree
            if let Ok(Object::Blob(blob)) = store.read(hash) {
                let file_path = repo_root.join(&conflict.path);
                if let Some(parent) = file_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                fs::write(&file_path, &blob.content)
                    .map_err(|e| format!("Failed to write {}: {}", conflict.path, e))?;
            }

            resolved_files.push(conflict.path.clone());
        }
    }

    if resolved_files.is_empty() {
        return Err(if let Some(f) = &file {
            format!("No conflict found for file '{}'", f)
        } else {
            "No conflicts to resolve".to_string()
        });
    }

    // Save updated index
    index.save(&repo_root)?;

    // Update conflict state — remove resolved conflicts
    let remaining: Vec<&crate::core::merge::FileMergeResult> = file_results
        .iter()
        .filter(|r| {
            r.status == crate::core::merge::FileMergeStatus::Conflict
                && !resolved_files.contains(&r.path)
        })
        .collect();

    if remaining.is_empty() {
        // All conflicts resolved — clean up and offer to finalize
        let _ = fs::remove_file(&conflicts_path);
        Ok(ResolveResponse {
            resolved_files,
            remaining_conflicts: 0,
            merge_complete: false,
            message: "All conflicts resolved. Run 'lit resolve --continue' to finalize the merge."
                .to_string(),
        })
    } else {
        // Write updated conflict state
        let updated_data = serde_json::to_string_pretty(&remaining)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        fs::write(&conflicts_path, updated_data)
            .map_err(|e| format!("Failed to update conflict state: {}", e))?;

        Ok(ResolveResponse {
            resolved_files,
            remaining_conflicts: remaining.len(),
            merge_complete: false,
            message: format!("{} conflict(s) remaining", remaining.len()),
        })
    }
}

/// Finalize a merge after all conflicts are resolved
fn finalize_merge(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<ResolveResponse, String> {
    let merge_dir = repo_root.join(".lit").join("merge");

    // Check no remaining conflicts
    let conflicts_path = merge_dir.join("conflicts.json");
    if conflicts_path.exists() {
        let data = fs::read_to_string(&conflicts_path)
            .map_err(|e| format!("Failed to read conflicts: {}", e))?;
        let remaining: Vec<crate::core::merge::FileMergeResult> =
            serde_json::from_str(&data).unwrap_or_default();
        let conflict_count = remaining
            .iter()
            .filter(|r| r.status == crate::core::merge::FileMergeStatus::Conflict)
            .count();
        if conflict_count > 0 {
            return Err(format!(
                "{} unresolved conflict(s) remain. Resolve them first.",
                conflict_count
            ));
        }
    }

    // Read MERGE_HEAD
    let merge_head_str = fs::read_to_string(merge_dir.join("MERGE_HEAD"))
        .map_err(|e| format!("Failed to read MERGE_HEAD: {}", e))?
        .trim()
        .to_string();
    let merge_head = ObjectHash::from_hex(merge_head_str);

    let head_hash_str = crate::core::read_head(repo_root)?;
    let head_hash = ObjectHash::from_hex(head_hash_str);

    // Build tree from current index
    let index = Index::load(repo_root)?;
    let tree_hash = build_tree_from_index(&index, store)?;

    // Create merge commit
    let author = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "Unknown".to_string());

    let commit = Commit::new(
        tree_hash,
        vec![head_hash, merge_head],
        author,
        "Merge commit (conflicts resolved)".to_string(),
    );

    let commit_obj = Object::Commit(commit);
    let commit_hash = store.write(&commit_obj)?;

    // Update branch ref
    let current_branch = get_current_branch(repo_root).unwrap_or_else(|_| "main".to_string());
    write_ref(
        repo_root,
        &format!("heads/{}", current_branch),
        commit_hash.as_str(),
    )?;

    // Clean up merge state
    let _ = fs::remove_dir_all(&merge_dir);

    Ok(ResolveResponse {
        resolved_files: vec![],
        remaining_conflicts: 0,
        merge_complete: true,
        message: format!("Merge complete: {}", commit_hash.short()),
    })
}

fn get_commit_tree(
    store: &ObjectStore,
    commit_hash: &ObjectHash,
) -> Result<crate::core::Tree, String> {
    let commit = match store.read(commit_hash)? {
        Object::Commit(c) => c,
        _ => return Err(format!("Expected commit object for {}", commit_hash)),
    };
    match store.read(&commit.tree)? {
        Object::Tree(t) => Ok(t),
        _ => Err(format!("Expected tree object for {}", commit.tree)),
    }
}

/// Build tree from index (same logic as commit command)
fn build_tree_from_index(index: &Index, store: &ObjectStore) -> Result<ObjectHash, String> {
    use crate::core::Tree;
    use std::collections::HashMap;

    let mut tree_map: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    for entry in index.sorted_entries() {
        let parts: Vec<&str> = entry.path.split('/').collect();
        if parts.len() == 1 {
            tree_map.entry("".to_string()).or_default().push((
                parts[0].to_string(),
                entry.hash.clone(),
                entry.mode.clone(),
            ));
        } else {
            let dir = parts[0].to_string();
            tree_map.entry(dir).or_default().push((
                parts[1..].join("/"),
                entry.hash.clone(),
                entry.mode.clone(),
            ));
        }
    }

    let mut root_tree = Tree::new();

    if let Some(root_files) = tree_map.get("") {
        for (name, hash, mode) in root_files {
            root_tree.add_entry(
                mode.clone(),
                name.clone(),
                ObjectHash::from_hex(hash.clone()),
                "blob".to_string(),
            );
        }
    }

    for dir in tree_map.keys() {
        if !dir.is_empty() {
            let mut subtree = Tree::new();
            if let Some(files) = tree_map.get(dir) {
                for (name, hash, mode) in files {
                    subtree.add_entry(
                        mode.clone(),
                        name.clone(),
                        ObjectHash::from_hex(hash.clone()),
                        "blob".to_string(),
                    );
                }
            }
            let subtree_object = Object::Tree(subtree);
            let subtree_hash = store.write(&subtree_object)?;
            root_tree.add_entry(
                "040000".to_string(),
                dir.clone(),
                subtree_hash,
                "tree".to_string(),
            );
        }
    }

    let tree_object = Object::Tree(root_tree);
    store.write(&tree_object)
}
