use crate::core::diff::{collect_tree_files, diff_blobs, diff_trees, DiffStat, FileDiff};
use crate::core::{find_repo_root, Object, ObjectHash};
use crate::response::DiffResponse;
use crate::storage::{Index, ObjectStore};
use std::collections::HashMap;
use std::fs;

/// Execute the diff command
///
/// Modes:
///   - No args: working tree vs index (unstaged changes)
///   - --staged: index vs HEAD (staged changes)
///   - Two refs: commit-to-commit or branch-to-branch
pub fn execute(
    staged: bool,
    stat: bool,
    word_diff: bool,
    ref1: Option<String>,
    ref2: Option<String>,
) -> Result<DiffResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let file_diffs = if let Some(r1) = ref1 {
        let r2 = ref2.unwrap_or_else(|| "HEAD".to_string());
        // Commit-to-commit or branch-to-branch diff
        diff_refs(&repo_root, &store, &r1, &r2)?
    } else if staged {
        // Index vs HEAD
        diff_staged(&repo_root, &store)?
    } else {
        // Working tree vs index
        diff_working(&repo_root, &store)?
    };

    let stats: Vec<DiffStat> = file_diffs
        .iter()
        .map(|d| DiffStat {
            path: d.path.clone(),
            additions: d.additions,
            deletions: d.deletions,
            status: d.status,
        })
        .collect();

    let total_additions: usize = stats.iter().map(|s| s.additions).sum();
    let total_deletions: usize = stats.iter().map(|s| s.deletions).sum();
    let files_changed = file_diffs.len();

    Ok(DiffResponse {
        files: file_diffs,
        stats,
        stat_only: stat,
        word_diff,
        files_changed,
        total_additions,
        total_deletions,
    })
}

/// Diff working tree against index (unstaged changes)
fn diff_working(
    repo_root: &std::path::Path,
    _store: &ObjectStore,
) -> Result<Vec<FileDiff>, String> {
    let index = Index::load(repo_root)?;
    let mut diffs = Vec::new();

    for (path, entry) in &index.entries {
        let file_path = repo_root.join(path);

        if !file_path.exists() {
            // File deleted from working tree
            let old_content = read_blob_by_hash_str(_store, &entry.hash)?;
            diffs.push(diff_blobs(
                path,
                Some(&old_content),
                None,
                Some(entry.hash.clone()),
                None,
            ));
            continue;
        }

        let current_content =
            fs::read(&file_path).map_err(|e| format!("Failed to read {}: {}", path, e))?;

        let old_content = read_blob_by_hash_str(_store, &entry.hash)?;

        if current_content != old_content {
            diffs.push(diff_blobs(
                path,
                Some(&old_content),
                Some(&current_content),
                Some(entry.hash.clone()),
                None,
            ));
        }
    }

    diffs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(diffs)
}

/// Diff index (staged changes) against HEAD
fn diff_staged(repo_root: &std::path::Path, store: &ObjectStore) -> Result<Vec<FileDiff>, String> {
    let index = Index::load(repo_root)?;

    // Get HEAD tree files
    let head_files = get_head_tree_files(repo_root, store).unwrap_or_default();

    let mut diffs = Vec::new();

    // Files in index
    for (path, entry) in &index.entries {
        let new_content = read_blob_by_hash_str(store, &entry.hash)?;

        if let Some(old_hash) = head_files.get(path) {
            // File exists in HEAD — check if changed
            let old_hash_str = old_hash.to_string();
            if entry.hash != old_hash_str {
                let old_content = read_blob_by_hash_str(store, &old_hash_str)?;
                diffs.push(diff_blobs(
                    path,
                    Some(&old_content),
                    Some(&new_content),
                    Some(old_hash_str),
                    Some(entry.hash.clone()),
                ));
            }
        } else {
            // New file
            diffs.push(diff_blobs(
                path,
                None,
                Some(&new_content),
                None,
                Some(entry.hash.clone()),
            ));
        }
    }

    // Files in HEAD but not in index (would be deletions if we tracked that)
    // Note: lit's current index model only tracks staged files, not deletions

    diffs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(diffs)
}

/// Diff two refs (commits or branches)
fn diff_refs(
    repo_root: &std::path::Path,
    store: &ObjectStore,
    ref1: &str,
    ref2: &str,
) -> Result<Vec<FileDiff>, String> {
    let hash1 = resolve_ref(repo_root, ref1)?;
    let hash2 = resolve_ref(repo_root, ref2)?;

    let tree1 = get_commit_tree(store, &hash1)?;
    let tree2 = get_commit_tree(store, &hash2)?;

    diff_trees(&tree1, &tree2, store)
}

/// Resolve a ref string to an ObjectHash — supports branch names, HEAD, and raw hashes
fn resolve_ref(repo_root: &std::path::Path, reference: &str) -> Result<ObjectHash, String> {
    if reference == "HEAD" {
        let head = crate::core::read_head(repo_root)?;
        return Ok(ObjectHash::from_hex(head));
    }

    // Try as branch ref
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("heads/{}", reference)) {
        return Ok(ObjectHash::from_hex(hash));
    }

    // Try as tag ref
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("tags/{}", reference)) {
        return Ok(ObjectHash::from_hex(hash));
    }

    // Try as raw hash
    Ok(ObjectHash::from_hex(reference.to_string()))
}

/// Get the tree object from a commit
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

/// Get all files from HEAD's tree
fn get_head_tree_files(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<HashMap<String, ObjectHash>, String> {
    let head_hash = crate::core::read_head(repo_root)?;
    let commit_hash = ObjectHash::from_hex(head_hash);
    let tree = get_commit_tree(store, &commit_hash)?;
    let files = collect_tree_files(&tree, store, "")?;
    Ok(files.into_iter().collect())
}

/// Read a blob by its hash string
fn read_blob_by_hash_str(store: &ObjectStore, hash: &str) -> Result<Vec<u8>, String> {
    let obj_hash = ObjectHash::from_hex(hash.to_string());
    match store.read(&obj_hash)? {
        Object::Blob(b) => Ok(b.content),
        _ => Err(format!("Expected blob object for hash {}", hash)),
    }
}
