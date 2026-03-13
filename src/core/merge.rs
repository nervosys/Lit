/// 3-Way Merge Engine
///
/// Implements recursive 3-way merge with structured conflict output.
/// Supports strategies: recursive (default), ours, theirs.
use crate::core::diff::{myers_diff, DiffOp};
use crate::core::{Object, ObjectHash, Tree, TreeEntry};
use crate::storage::ObjectStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Merge strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    Recursive,
    Ours,
    Theirs,
}

impl std::str::FromStr for MergeStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "recursive" => Ok(MergeStrategy::Recursive),
            "ours" => Ok(MergeStrategy::Ours),
            "theirs" => Ok(MergeStrategy::Theirs),
            _ => Err(format!(
                "Unknown merge strategy: '{}'. Valid: recursive, ours, theirs",
                s
            )),
        }
    }
}

/// Result of a merge operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// The merged tree (if merge succeeded or has conflicts with partial results)
    pub tree: Option<ObjectHash>,
    /// Whether the merge was a fast-forward
    pub fast_forward: bool,
    /// Whether conflicts were detected
    pub has_conflicts: bool,
    /// Per-file merge results
    pub file_results: Vec<FileMergeResult>,
    /// Strategy used
    pub strategy: String,
}

/// Per-file merge result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMergeResult {
    pub path: String,
    pub status: FileMergeStatus,
    /// Conflict regions (only present if status == Conflict)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<ConflictRegion>,
}

/// File-level merge status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileMergeStatus {
    Clean,
    Conflict,
    Added,
    Deleted,
    /// Both sides modified but auto-resolved
    AutoResolved,
}

/// A single conflict region in a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictRegion {
    pub start_line: usize,
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
    pub base: Vec<String>,
}

/// Find the common ancestor (merge base) between two commits
pub fn find_merge_base(
    store: &ObjectStore,
    commit_a: &ObjectHash,
    commit_b: &ObjectHash,
) -> Result<Option<ObjectHash>, String> {
    // BFS from both commits, first intersection is the merge base
    let mut visited_a: HashMap<String, usize> = HashMap::new();
    let mut visited_b: HashMap<String, usize> = HashMap::new();
    let mut queue_a = vec![commit_a.clone()];
    let mut queue_b = vec![commit_b.clone()];
    let mut depth = 0usize;

    // Alternating BFS from both sides
    while !queue_a.is_empty() || !queue_b.is_empty() {
        // Process queue A
        let mut next_a = Vec::new();
        for hash in &queue_a {
            let key = hash.to_string();
            if visited_b.contains_key(&key) {
                return Ok(Some(hash.clone()));
            }
            if visited_a.contains_key(&key) {
                continue;
            }
            visited_a.insert(key, depth);

            if let Ok(Object::Commit(commit)) = store.read(hash) {
                for parent in &commit.parents {
                    if !visited_a.contains_key(&parent.to_string()) {
                        next_a.push(parent.clone());
                    }
                }
            }
        }
        queue_a = next_a;

        // Process queue B
        let mut next_b = Vec::new();
        for hash in &queue_b {
            let key = hash.to_string();
            if visited_a.contains_key(&key) {
                return Ok(Some(hash.clone()));
            }
            if visited_b.contains_key(&key) {
                continue;
            }
            visited_b.insert(key, depth);

            if let Ok(Object::Commit(commit)) = store.read(hash) {
                for parent in &commit.parents {
                    if !visited_b.contains_key(&parent.to_string()) {
                        next_b.push(parent.clone());
                    }
                }
            }
        }
        queue_b = next_b;
        depth += 1;

        // Safety limit
        if depth > 10000 {
            return Err("Merge base search exceeded depth limit".to_string());
        }
    }

    Ok(None) // No common ancestor
}

/// Check if commit_a is an ancestor of commit_b (for fast-forward detection)
pub fn is_ancestor(
    store: &ObjectStore,
    ancestor: &ObjectHash,
    descendant: &ObjectHash,
) -> Result<bool, String> {
    if ancestor.to_string() == descendant.to_string() {
        return Ok(true);
    }

    let mut queue = vec![descendant.clone()];
    let mut visited = std::collections::HashSet::new();

    while let Some(hash) = queue.pop() {
        let key = hash.to_string();
        if key == ancestor.to_string() {
            return Ok(true);
        }
        if !visited.insert(key) {
            continue;
        }
        if let Ok(Object::Commit(commit)) = store.read(&hash) {
            for parent in &commit.parents {
                queue.push(parent.clone());
            }
        }
    }

    Ok(false)
}

/// Perform a 3-way merge between two trees with a common base
pub fn merge_trees(
    store: &ObjectStore,
    base_tree: Option<&Tree>,
    ours_tree: &Tree,
    theirs_tree: &Tree,
    strategy: MergeStrategy,
) -> Result<MergeResult, String> {
    // Collect files from all three trees
    let base_files = match base_tree {
        Some(t) => crate::core::diff::collect_tree_files(t, store, "")?,
        None => vec![],
    };
    let ours_files = crate::core::diff::collect_tree_files(ours_tree, store, "")?;
    let theirs_files = crate::core::diff::collect_tree_files(theirs_tree, store, "")?;

    let base_map: HashMap<String, ObjectHash> = base_files.into_iter().collect();
    let ours_map: HashMap<String, ObjectHash> = ours_files.into_iter().collect();
    let theirs_map: HashMap<String, ObjectHash> = theirs_files.into_iter().collect();

    // Collect all file paths
    let mut all_paths: Vec<String> = ours_map
        .keys()
        .chain(theirs_map.keys())
        .chain(base_map.keys())
        .cloned()
        .collect();
    all_paths.sort();
    all_paths.dedup();

    let mut file_results = Vec::new();
    let mut merged_entries: Vec<(String, ObjectHash)> = Vec::new();
    let mut has_conflicts = false;

    for path in &all_paths {
        let base_hash = base_map.get(path);
        let ours_hash = ours_map.get(path);
        let theirs_hash = theirs_map.get(path);

        let result = merge_file_entry(store, path, base_hash, ours_hash, theirs_hash, strategy)?;

        if result.status == FileMergeStatus::Conflict {
            has_conflicts = true;
        }

        // Determine which hash to use in the merged tree
        match result.status {
            FileMergeStatus::Clean | FileMergeStatus::AutoResolved => {
                // Use theirs if only theirs changed, ours otherwise
                if let Some(h) = ours_hash {
                    if base_hash.map(|b| b == h).unwrap_or(false) {
                        // Ours unchanged from base, use theirs
                        if let Some(th) = theirs_hash {
                            merged_entries.push((path.clone(), th.clone()));
                        }
                    } else {
                        merged_entries.push((path.clone(), h.clone()));
                    }
                } else if let Some(th) = theirs_hash {
                    merged_entries.push((path.clone(), th.clone()));
                }
            }
            FileMergeStatus::Added => {
                if let Some(h) = ours_hash.or(theirs_hash) {
                    merged_entries.push((path.clone(), h.clone()));
                }
            }
            FileMergeStatus::Deleted => {
                // Don't include in merged tree
            }
            FileMergeStatus::Conflict => {
                // For conflicts with strategy override, pick accordingly
                match strategy {
                    MergeStrategy::Ours => {
                        if let Some(h) = ours_hash {
                            merged_entries.push((path.clone(), h.clone()));
                        }
                    }
                    MergeStrategy::Theirs => {
                        if let Some(h) = theirs_hash {
                            merged_entries.push((path.clone(), h.clone()));
                        }
                    }
                    MergeStrategy::Recursive => {
                        // Keep ours version in tree, conflicts recorded separately
                        if let Some(h) = ours_hash {
                            merged_entries.push((path.clone(), h.clone()));
                        }
                    }
                }
            }
        }

        file_results.push(result);
    }

    // Build merged tree
    let tree_hash = if !has_conflicts || strategy != MergeStrategy::Recursive {
        Some(build_flat_tree(store, &merged_entries)?)
    } else {
        None // Don't create tree when there are unresolved conflicts
    };

    Ok(MergeResult {
        tree: tree_hash,
        fast_forward: false,
        has_conflicts,
        file_results,
        strategy: format!("{:?}", strategy).to_lowercase(),
    })
}

/// Merge a single file entry using 3-way merge
fn merge_file_entry(
    store: &ObjectStore,
    path: &str,
    base_hash: Option<&ObjectHash>,
    ours_hash: Option<&ObjectHash>,
    theirs_hash: Option<&ObjectHash>,
    strategy: MergeStrategy,
) -> Result<FileMergeResult, String> {
    match (base_hash, ours_hash, theirs_hash) {
        // File exists in base but not in both branches — both deleted
        (Some(_), None, None) => Ok(FileMergeResult {
            path: path.to_string(),
            status: FileMergeStatus::Deleted,
            conflicts: vec![],
        }),

        // File only in ours (added by us, not in base or theirs)
        (None, Some(_), None) => Ok(FileMergeResult {
            path: path.to_string(),
            status: FileMergeStatus::Added,
            conflicts: vec![],
        }),

        // File only in theirs (added by them)
        (None, None, Some(_)) => Ok(FileMergeResult {
            path: path.to_string(),
            status: FileMergeStatus::Added,
            conflicts: vec![],
        }),

        // File added by both — check if same content
        (None, Some(o), Some(t)) => {
            if o == t {
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Clean,
                    conflicts: vec![],
                })
            } else {
                handle_content_conflict(store, path, None, Some(o), Some(t), strategy)
            }
        }

        // File in base and ours, deleted by theirs
        (Some(b), Some(o), None) => {
            if o == b {
                // We didn't change it, they deleted it — accept deletion
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Deleted,
                    conflicts: vec![],
                })
            } else {
                // We modified, they deleted — conflict
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Conflict,
                    conflicts: vec![ConflictRegion {
                        start_line: 1,
                        ours: vec!["(file modified)".to_string()],
                        theirs: vec!["(file deleted)".to_string()],
                        base: vec!["(file existed)".to_string()],
                    }],
                })
            }
        }

        // File in base and theirs, deleted by ours
        (Some(b), None, Some(t)) => {
            if t == b {
                // They didn't change it, we deleted it — accept deletion
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Deleted,
                    conflicts: vec![],
                })
            } else {
                // They modified, we deleted — conflict
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Conflict,
                    conflicts: vec![ConflictRegion {
                        start_line: 1,
                        ours: vec!["(file deleted)".to_string()],
                        theirs: vec!["(file modified)".to_string()],
                        base: vec!["(file existed)".to_string()],
                    }],
                })
            }
        }

        // File in all three — standard 3-way merge
        (Some(b), Some(o), Some(t)) => {
            if o == t {
                // Both made same change (or neither changed)
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Clean,
                    conflicts: vec![],
                })
            } else if o == b {
                // Only theirs changed — take theirs
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Clean,
                    conflicts: vec![],
                })
            } else if t == b {
                // Only ours changed — take ours
                Ok(FileMergeResult {
                    path: path.to_string(),
                    status: FileMergeStatus::Clean,
                    conflicts: vec![],
                })
            } else {
                // Both changed differently — need content-level merge
                handle_content_conflict(store, path, Some(b), Some(o), Some(t), strategy)
            }
        }

        // No file anywhere (shouldn't happen)
        (None, None, None) => Ok(FileMergeResult {
            path: path.to_string(),
            status: FileMergeStatus::Clean,
            conflicts: vec![],
        }),
    }
}

/// Handle content-level conflict between two versions
fn handle_content_conflict(
    store: &ObjectStore,
    path: &str,
    base_hash: Option<&ObjectHash>,
    ours_hash: Option<&ObjectHash>,
    theirs_hash: Option<&ObjectHash>,
    strategy: MergeStrategy,
) -> Result<FileMergeResult, String> {
    // For ours/theirs strategies, no conflict
    match strategy {
        MergeStrategy::Ours => {
            return Ok(FileMergeResult {
                path: path.to_string(),
                status: FileMergeStatus::AutoResolved,
                conflicts: vec![],
            });
        }
        MergeStrategy::Theirs => {
            return Ok(FileMergeResult {
                path: path.to_string(),
                status: FileMergeStatus::AutoResolved,
                conflicts: vec![],
            });
        }
        MergeStrategy::Recursive => {}
    }

    // Read content from all versions
    let base_content = match base_hash {
        Some(h) => read_blob_text(store, h)?,
        None => String::new(),
    };
    let ours_content = match ours_hash {
        Some(h) => read_blob_text(store, h)?,
        None => String::new(),
    };
    let theirs_content = match theirs_hash {
        Some(h) => read_blob_text(store, h)?,
        None => String::new(),
    };

    // Try line-level 3-way merge
    let base_lines: Vec<&str> = base_content.lines().collect();
    let ours_lines: Vec<&str> = ours_content.lines().collect();
    let theirs_lines: Vec<&str> = theirs_content.lines().collect();

    let merge_result = three_way_merge(&base_lines, &ours_lines, &theirs_lines);

    if merge_result.conflicts.is_empty() {
        Ok(FileMergeResult {
            path: path.to_string(),
            status: FileMergeStatus::AutoResolved,
            conflicts: vec![],
        })
    } else {
        Ok(FileMergeResult {
            path: path.to_string(),
            status: FileMergeStatus::Conflict,
            conflicts: merge_result.conflicts,
        })
    }
}

/// Result of a 3-way line merge
struct ThreeWayResult {
    _merged_lines: Vec<String>,
    conflicts: Vec<ConflictRegion>,
}

/// Perform line-level 3-way merge
fn three_way_merge(base: &[&str], ours: &[&str], theirs: &[&str]) -> ThreeWayResult {
    // Diff base→ours and base→theirs using raw diff ops (no context padding)
    let ours_ops = myers_diff(base, ours);
    let theirs_ops = myers_diff(base, theirs);

    // Track which base lines each side actually modifies (removes or replaces)
    let mut ours_changed_lines: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut theirs_changed_lines: std::collections::HashSet<usize> =
        std::collections::HashSet::new();

    {
        let mut base_idx = 0usize;
        for op in &ours_ops {
            match op {
                DiffOp::Equal(_) => {
                    base_idx += 1;
                }
                DiffOp::Delete(_) => {
                    ours_changed_lines.insert(base_idx);
                    base_idx += 1;
                }
                DiffOp::Insert(_) => {
                    ours_changed_lines.insert(base_idx);
                }
            }
        }
    }

    {
        let mut base_idx = 0usize;
        for op in &theirs_ops {
            match op {
                DiffOp::Equal(_) => {
                    base_idx += 1;
                }
                DiffOp::Delete(_) => {
                    theirs_changed_lines.insert(base_idx);
                    base_idx += 1;
                }
                DiffOp::Insert(_) => {
                    theirs_changed_lines.insert(base_idx);
                }
            }
        }
    }

    // Detect overlapping changes (conflicts)
    let mut conflicts = Vec::new();
    let overlapping: std::collections::HashSet<usize> = ours_changed_lines
        .intersection(&theirs_changed_lines)
        .cloned()
        .collect();

    if !overlapping.is_empty() {
        let mut sorted_overlaps: Vec<usize> = overlapping.into_iter().collect();
        sorted_overlaps.sort();

        let mut regions: Vec<(usize, usize)> = Vec::new();
        let mut start = sorted_overlaps[0];
        let mut end = sorted_overlaps[0];

        for &line in &sorted_overlaps[1..] {
            if line <= end + 1 {
                end = line;
            } else {
                regions.push((start, end));
                start = line;
                end = line;
            }
        }
        regions.push((start, end));

        for (start, end) in regions {
            let base_region: Vec<String> = (start..=end)
                .filter_map(|i| base.get(i).map(|s| s.to_string()))
                .collect();
            let ours_region: Vec<String> = (start..=end)
                .filter_map(|i| ours.get(i).map(|s| s.to_string()))
                .collect();
            let theirs_region: Vec<String> = (start..=end)
                .filter_map(|i| theirs.get(i).map(|s| s.to_string()))
                .collect();

            conflicts.push(ConflictRegion {
                start_line: start + 1,
                ours: ours_region,
                theirs: theirs_region,
                base: base_region,
            });
        }
    }

    // Build merged output
    let mut merged_lines = Vec::new();
    let max_len = base.len().max(ours.len()).max(theirs.len());

    for i in 0..max_len {
        if ours_changed_lines.contains(&i) && !theirs_changed_lines.contains(&i) {
            if let Some(line) = ours.get(i) {
                merged_lines.push(line.to_string());
            }
        } else if theirs_changed_lines.contains(&i) && !ours_changed_lines.contains(&i) {
            if let Some(line) = theirs.get(i) {
                merged_lines.push(line.to_string());
            }
        } else if !ours_changed_lines.contains(&i) && !theirs_changed_lines.contains(&i) {
            if let Some(line) = base.get(i) {
                merged_lines.push(line.to_string());
            }
        }
    }

    ThreeWayResult {
        _merged_lines: merged_lines,
        conflicts,
    }
}

/// Read a blob as UTF-8 text
fn read_blob_text(store: &ObjectStore, hash: &ObjectHash) -> Result<String, String> {
    match store.read(hash)? {
        Object::Blob(b) => String::from_utf8(b.content)
            .map_err(|_| format!("File {} is binary, cannot merge", hash)),
        _ => Err(format!("Expected blob object for hash {}", hash)),
    }
}

/// Build a flat tree from (path, hash) pairs
fn build_flat_tree(
    store: &ObjectStore,
    entries: &[(String, ObjectHash)],
) -> Result<ObjectHash, String> {
    // Group by top-level directory
    let mut root_entries: Vec<TreeEntry> = Vec::new();
    let mut subdirs: HashMap<String, Vec<(String, ObjectHash)>> = HashMap::new();

    for (path, hash) in entries {
        if let Some(sep) = path.find('/') {
            let dir = &path[..sep];
            let rest = &path[sep + 1..];
            subdirs
                .entry(dir.to_string())
                .or_default()
                .push((rest.to_string(), hash.clone()));
        } else {
            root_entries.push(TreeEntry {
                mode: "100644".to_string(),
                name: path.clone(),
                hash: hash.clone(),
                object_type: "blob".to_string(),
            });
        }
    }

    // Recursively build subtrees
    for (dir, sub_entries) in &subdirs {
        let subtree_hash = build_flat_tree(store, sub_entries)?;
        root_entries.push(TreeEntry {
            mode: "040000".to_string(),
            name: dir.clone(),
            hash: subtree_hash,
            object_type: "tree".to_string(),
        });
    }

    root_entries.sort_by(|a, b| a.name.cmp(&b.name));

    let tree = Tree {
        entries: root_entries,
    };
    let tree_obj = Object::Tree(tree);
    store.write(&tree_obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_merge_strategy_from_str() {
        assert_eq!(
            MergeStrategy::from_str("recursive").unwrap(),
            MergeStrategy::Recursive
        );
        assert_eq!(
            MergeStrategy::from_str("ours").unwrap(),
            MergeStrategy::Ours
        );
        assert_eq!(
            MergeStrategy::from_str("theirs").unwrap(),
            MergeStrategy::Theirs
        );
        assert!(MergeStrategy::from_str("invalid").is_err());
    }

    #[test]
    fn test_three_way_merge_no_conflict() {
        let base = vec!["line1", "line2", "line3"];
        let ours = vec!["line1", "MODIFIED", "line3"];
        let theirs = vec!["line1", "line2", "line3"];
        let result = three_way_merge(&base, &ours, &theirs);
        assert!(result.conflicts.is_empty());
    }

    #[test]
    fn test_three_way_merge_both_sides_different_regions() {
        let base = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
        let ours = vec!["A", "b", "c", "d", "e", "f", "g", "h", "i", "j"];
        let theirs = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "J"];
        let result = three_way_merge(&base, &ours, &theirs);
        assert!(result.conflicts.is_empty());
        assert_eq!(
            result._merged_lines,
            vec!["A", "b", "c", "d", "e", "f", "g", "h", "i", "J"]
        );
    }

    #[test]
    fn test_three_way_merge_conflict() {
        let base = vec!["line1", "line2", "line3"];
        let ours = vec!["line1", "OURS", "line3"];
        let theirs = vec!["line1", "THEIRS", "line3"];
        let result = three_way_merge(&base, &ours, &theirs);
        assert!(!result.conflicts.is_empty());
    }
}
