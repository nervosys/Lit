/// Diff Engine — Myers diff algorithm with structured output
///
/// Provides line-level diffing between blobs and tree-level diffing
/// between commits/branches. All output is structured for agent consumption.
use serde::{Serialize, Deserialize};

/// A single diff hunk representing a contiguous region of changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_count: usize,
    pub new_start: usize,
    pub new_count: usize,
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

/// The type of a diff line
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineKind {
    Context,
    Add,
    Remove,
}

/// A file-level diff result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub hunks: Vec<DiffHunk>,
    pub old_hash: Option<String>,
    pub new_hash: Option<String>,
    pub is_binary: bool,
    pub additions: usize,
    pub deletions: usize,
}

/// File status in a diff
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

/// Stat summary for a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStat {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
    pub status: FileStatus,
}

/// Context lines around hunks (default: 3)
const CONTEXT_LINES: usize = 3;

/// Myers diff algorithm — computes the shortest edit script between two sequences
///
/// Returns a list of edit operations (keep, insert, delete) that transform `old` into `new`.
pub fn myers_diff<'a>(old: &'a [&str], new: &'a [&str]) -> Vec<DiffOp<'a>> {
    let n = old.len();
    let m = new.len();

    if n == 0 && m == 0 {
        return vec![];
    }
    if n == 0 {
        return new.iter().map(|l| DiffOp::Insert(l)).collect();
    }
    if m == 0 {
        return old.iter().map(|l| DiffOp::Delete(l)).collect();
    }

    let max = n + m;
    // V array indexed from -max to +max; offset by max
    let size = 2 * max + 1;
    let mut v = vec![0usize; size];
    let mut trace: Vec<Vec<usize>> = Vec::new();

    'outer: for d in 0..=(max as isize) {
        trace.push(v.clone());
        let mut new_v = v.clone();

        let k_min = -d;
        let k_max = d;
        let mut k = k_min;
        while k <= k_max {
            let idx = (k + max as isize) as usize;
            let mut x = if k == -d
                || (k != d
                    && v[((k - 1) + max as isize) as usize] < v[((k + 1) + max as isize) as usize])
            {
                v[((k + 1) + max as isize) as usize]
            } else {
                v[((k - 1) + max as isize) as usize] + 1
            };

            let mut y = (x as isize - k) as usize;

            // Follow diagonal (matching lines)
            while x < n && y < m && old[x] == new[y] {
                x += 1;
                y += 1;
            }

            new_v[idx] = x;

            if x >= n && y >= m {
                v = new_v;
                trace.push(v.clone());
                break 'outer;
            }

            k += 2;
        }
        v = new_v;
    }

    // Backtrack to find the actual edit path
    backtrack(&trace, n, m, max, old, new)
}

/// Edit operation from Myers diff
#[derive(Debug, Clone)]
pub enum DiffOp<'a> {
    Equal(&'a str),
    Insert(&'a str),
    Delete(&'a str),
}

fn backtrack<'a>(
    trace: &[Vec<usize>],
    n: usize,
    m: usize,
    max: usize,
    old: &'a [&str],
    new: &'a [&str],
) -> Vec<DiffOp<'a>> {
    let mut ops = Vec::new();
    let mut x = n;
    let mut y = m;

    for d in (0..trace.len().saturating_sub(1)).rev() {
        let v = &trace[d];
        let k = x as isize - y as isize;

        let (prev_x, prev_y) = if d == 0 {
            // At d=0, the starting position is always (0, 0)
            (0usize, 0usize)
        } else {
            let prev_k = if k == -(d as isize)
                || (k != d as isize
                    && v[((k - 1) + max as isize) as usize] < v[((k + 1) + max as isize) as usize])
            {
                k + 1
            } else {
                k - 1
            };

            let px = v[(prev_k + max as isize) as usize];
            let py = (px as isize - prev_k) as usize;
            (px, py)
        };

        // Diagonal moves (equal lines)
        while x > prev_x && y > prev_y {
            x -= 1;
            y -= 1;
            ops.push(DiffOp::Equal(old[x]));
        }

        if d > 0 {
            let prev_k = if k == -(d as isize)
                || (k != d as isize
                    && v[((k - 1) + max as isize) as usize] < v[((k + 1) + max as isize) as usize])
            {
                k + 1
            } else {
                k - 1
            };
            let prev_x = v[(prev_k + max as isize) as usize];

            if x == prev_x {
                // Insert
                y -= 1;
                ops.push(DiffOp::Insert(new[y]));
            } else {
                // Delete
                x -= 1;
                ops.push(DiffOp::Delete(old[x]));
            }
        }
    }

    ops.reverse();
    ops
}

/// Convert diff operations into hunks with context lines
pub fn ops_to_hunks(ops: &[DiffOp], context: usize) -> Vec<DiffHunk> {
    if ops.is_empty() {
        return vec![];
    }

    // First, convert ops to annotated lines with positions
    let mut lines: Vec<(usize, usize, DiffLineKind, String)> = Vec::new();
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    for op in ops {
        match op {
            DiffOp::Equal(s) => {
                lines.push((old_line, new_line, DiffLineKind::Context, s.to_string()));
                old_line += 1;
                new_line += 1;
            }
            DiffOp::Delete(s) => {
                lines.push((old_line, new_line, DiffLineKind::Remove, s.to_string()));
                old_line += 1;
            }
            DiffOp::Insert(s) => {
                lines.push((old_line, new_line, DiffLineKind::Add, s.to_string()));
                new_line += 1;
            }
        }
    }

    // Find change regions and group into hunks with context
    let change_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, (_, _, kind, _))| *kind != DiffLineKind::Context)
        .map(|(i, _)| i)
        .collect();

    if change_indices.is_empty() {
        return vec![];
    }

    // Group changes that are within `context * 2` lines of each other
    let mut groups: Vec<(usize, usize)> = Vec::new();
    let mut group_start = change_indices[0];
    let mut group_end = change_indices[0];

    for &idx in &change_indices[1..] {
        if idx <= group_end + context * 2 + 1 {
            group_end = idx;
        } else {
            groups.push((group_start, group_end));
            group_start = idx;
            group_end = idx;
        }
    }
    groups.push((group_start, group_end));

    // Build hunks from groups
    let mut hunks = Vec::new();
    for (start, end) in groups {
        let hunk_start = start.saturating_sub(context);
        let hunk_end = (end + context + 1).min(lines.len());

        let mut hunk_lines = Vec::new();
        let mut old_start = 0;
        let mut new_start = 0;
        let mut old_count = 0;
        let mut new_count = 0;
        let mut first = true;

        for line in lines.iter().take(hunk_end).skip(hunk_start) {
            let (ol, nl, kind, content) = line;
            if first {
                old_start = *ol;
                new_start = *nl;
                first = false;
            }
            match kind {
                DiffLineKind::Context => {
                    old_count += 1;
                    new_count += 1;
                }
                DiffLineKind::Add => {
                    new_count += 1;
                }
                DiffLineKind::Remove => {
                    old_count += 1;
                }
            }
            hunk_lines.push(DiffLine {
                kind: *kind,
                content: content.clone(),
            });
        }

        hunks.push(DiffHunk {
            old_start: old_start + 1, // 1-indexed
            old_count,
            new_start: new_start + 1, // 1-indexed
            new_count,
            lines: hunk_lines,
        });
    }

    hunks
}

/// Diff two text strings, returning structured hunks
pub fn diff_text(old: &str, new: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    let ops = myers_diff(&old_lines, &new_lines);
    ops_to_hunks(&ops, CONTEXT_LINES)
}

/// Diff two blobs, returning a FileDiff
pub fn diff_blobs(
    path: &str,
    old_content: Option<&[u8]>,
    new_content: Option<&[u8]>,
    old_hash: Option<String>,
    new_hash: Option<String>,
) -> FileDiff {
    let status = match (old_content, new_content) {
        (None, Some(_)) => FileStatus::Added,
        (Some(_), None) => FileStatus::Deleted,
        _ => FileStatus::Modified,
    };

    // Check if either side is binary
    let old_text = old_content.and_then(|c| std::str::from_utf8(c).ok());
    let new_text = new_content.and_then(|c| std::str::from_utf8(c).ok());

    let is_binary = (old_content.is_some() && old_text.is_none())
        || (new_content.is_some() && new_text.is_none());

    if is_binary {
        return FileDiff {
            path: path.to_string(),
            status,
            hunks: vec![],
            old_hash,
            new_hash,
            is_binary: true,
            additions: 0,
            deletions: 0,
        };
    }

    let old_str = old_text.unwrap_or("");
    let new_str = new_text.unwrap_or("");

    let hunks = diff_text(old_str, new_str);

    let mut additions = 0;
    let mut deletions = 0;
    for hunk in &hunks {
        for line in &hunk.lines {
            match line.kind {
                DiffLineKind::Add => additions += 1,
                DiffLineKind::Remove => deletions += 1,
                DiffLineKind::Context => {}
            }
        }
    }

    FileDiff {
        path: path.to_string(),
        status,
        hunks,
        old_hash,
        new_hash,
        is_binary: false,
        additions,
        deletions,
    }
}

/// Collect all file paths from a tree recursively
pub fn collect_tree_files(
    tree: &crate::core::Tree,
    store: &crate::storage::ObjectStore,
    prefix: &str,
) -> Result<Vec<(String, crate::core::ObjectHash)>, String> {
    let mut files = Vec::new();

    for entry in &tree.entries {
        let path = if prefix.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", prefix, entry.name)
        };

        match entry.object_type.as_str() {
            "blob" => {
                files.push((path, entry.hash.clone()));
            }
            "tree" => {
                let subtree = match store.read(&entry.hash)? {
                    crate::core::Object::Tree(t) => t,
                    _ => return Err(format!("Expected tree at {}", path)),
                };
                files.extend(collect_tree_files(&subtree, store, &path)?);
            }
            _ => {}
        }
    }

    Ok(files)
}

/// Diff two trees by their commit hashes, returning per-file diffs
pub fn diff_trees(
    old_tree: &crate::core::Tree,
    new_tree: &crate::core::Tree,
    store: &crate::storage::ObjectStore,
) -> Result<Vec<FileDiff>, String> {
    let old_files = collect_tree_files(old_tree, store, "")?;
    let new_files = collect_tree_files(new_tree, store, "")?;

    let mut old_map: std::collections::HashMap<String, crate::core::ObjectHash> =
        old_files.into_iter().collect();
    let new_map: std::collections::HashMap<String, crate::core::ObjectHash> =
        new_files.into_iter().collect();

    let mut diffs = Vec::new();

    // Files in new tree
    for (path, new_hash) in &new_map {
        if let Some(old_hash) = old_map.remove(path) {
            // File exists in both — check if modified
            if old_hash != *new_hash {
                let old_blob = read_blob(store, &old_hash)?;
                let new_blob = read_blob(store, new_hash)?;
                diffs.push(diff_blobs(
                    path,
                    Some(&old_blob),
                    Some(&new_blob),
                    Some(old_hash.to_string()),
                    Some(new_hash.to_string()),
                ));
            }
        } else {
            // File only in new tree — added
            let new_blob = read_blob(store, new_hash)?;
            diffs.push(diff_blobs(
                path,
                None,
                Some(&new_blob),
                None,
                Some(new_hash.to_string()),
            ));
        }
    }

    // Files only in old tree — deleted
    for (path, old_hash) in &old_map {
        let old_blob = read_blob(store, old_hash)?;
        diffs.push(diff_blobs(
            path,
            Some(&old_blob),
            None,
            Some(old_hash.to_string()),
            None,
        ));
    }

    // Sort by path for deterministic output
    diffs.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(diffs)
}

fn read_blob(
    store: &crate::storage::ObjectStore,
    hash: &crate::core::ObjectHash,
) -> Result<Vec<u8>, String> {
    match store.read(hash)? {
        crate::core::Object::Blob(b) => Ok(b.content),
        _ => Err(format!("Expected blob object for hash {}", hash)),
    }
}

/// A segment of a word-level diff within a single line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordDiffSegment {
    pub kind: DiffLineKind,
    pub text: String,
}

/// Tokenize a line into words and whitespace, preserving all characters
fn tokenize_words(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut chars = line.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if ch.is_alphanumeric() || ch == '_' {
            // Word token
            let mut end = start;
            while let Some(&(i, c)) = chars.peek() {
                if c.is_alphanumeric() || c == '_' {
                    end = i + c.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(&line[start..end]);
        } else {
            // Non-word token (punctuation, whitespace, etc.) — each char is its own token
            tokens.push(&line[start..start + ch.len_utf8()]);
            chars.next();
        }
    }
    tokens
}

/// Compute word-level diff between two lines that were changed
///
/// Returns a sequence of segments marking equal, added, and removed words.
pub fn word_diff_line(old_line: &str, new_line: &str) -> Vec<WordDiffSegment> {
    let old_tokens = tokenize_words(old_line);
    let new_tokens = tokenize_words(new_line);

    let ops = myers_diff(&old_tokens, &new_tokens);
    let mut segments: Vec<WordDiffSegment> = Vec::new();

    for op in &ops {
        let (kind, text) = match op {
            DiffOp::Equal(s) => (DiffLineKind::Context, *s),
            DiffOp::Delete(s) => (DiffLineKind::Remove, *s),
            DiffOp::Insert(s) => (DiffLineKind::Add, *s),
        };
        // Merge adjacent segments of the same kind
        if let Some(last) = segments.last_mut() {
            if last.kind == kind {
                last.text.push_str(text);
                continue;
            }
        }
        segments.push(WordDiffSegment {
            kind,
            text: text.to_string(),
        });
    }
    segments
}

/// Produce word-level diff segments for paired Remove/Add lines in a hunk
///
/// Returns a new list of DiffLines where consecutive Remove/Add pairs
/// are annotated with `word_segments`.
pub fn annotate_hunk_word_diff(hunk: &DiffHunk) -> Vec<(DiffLine, Option<Vec<WordDiffSegment>>)> {
    let mut result: Vec<(DiffLine, Option<Vec<WordDiffSegment>>)> = Vec::new();
    let lines = &hunk.lines;
    let mut i = 0;

    while i < lines.len() {
        if lines[i].kind == DiffLineKind::Remove {
            // Collect consecutive removes
            let remove_start = i;
            while i < lines.len() && lines[i].kind == DiffLineKind::Remove {
                i += 1;
            }
            let remove_end = i;
            // Collect consecutive adds
            let add_start = i;
            while i < lines.len() && lines[i].kind == DiffLineKind::Add {
                i += 1;
            }
            let add_end = i;

            let removes = &lines[remove_start..remove_end];
            let adds = &lines[add_start..add_end];
            let pairs = removes.len().min(adds.len());

            // Pair up removes and adds for word-level diff
            for j in 0..pairs {
                let segs = word_diff_line(&removes[j].content, &adds[j].content);
                result.push((removes[j].clone(), Some(segs.clone())));
                result.push((adds[j].clone(), Some(segs)));
            }
            // Remaining unpaired removes
            for remove in removes.iter().skip(pairs) {
                result.push((remove.clone(), None));
            }
            // Remaining unpaired adds
            for add in adds.iter().skip(pairs) {
                result.push((add.clone(), None));
            }
        } else {
            result.push((lines[i].clone(), None));
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_diff() {
        let hunks = diff_text("", "");
        assert!(hunks.is_empty());
    }

    #[test]
    fn test_add_lines() {
        let hunks = diff_text("", "hello\nworld\n");
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].lines.len(), 2);
        assert!(hunks[0].lines.iter().all(|l| l.kind == DiffLineKind::Add));
    }

    #[test]
    fn test_delete_lines() {
        let hunks = diff_text("hello\nworld\n", "");
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0]
            .lines
            .iter()
            .all(|l| l.kind == DiffLineKind::Remove));
    }

    #[test]
    fn test_modify_line() {
        let hunks = diff_text("hello\nworld\n", "hello\nearth\n");
        assert_eq!(hunks.len(), 1);
        let changes: Vec<_> = hunks[0]
            .lines
            .iter()
            .filter(|l| l.kind != DiffLineKind::Context)
            .collect();
        assert_eq!(changes.len(), 2); // one remove, one add
    }

    #[test]
    fn test_identical_text() {
        let hunks = diff_text("hello\nworld\n", "hello\nworld\n");
        assert!(hunks.is_empty());
    }

    #[test]
    fn test_additions_count() {
        let diff = diff_blobs("test.txt", Some(b"a\nb\n"), Some(b"a\nb\nc\n"), None, None);
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);
        assert_eq!(diff.status, FileStatus::Modified);
    }

    #[test]
    fn test_new_file_status() {
        let diff = diff_blobs("test.txt", None, Some(b"hello\n"), None, None);
        assert_eq!(diff.status, FileStatus::Added);
    }

    #[test]
    fn test_deleted_file_status() {
        let diff = diff_blobs("test.txt", Some(b"hello\n"), None, None, None);
        assert_eq!(diff.status, FileStatus::Deleted);
    }

    #[test]
    fn test_binary_detection() {
        let diff = diff_blobs("img.png", Some(b"\x89PNG\r\n\x1a\n\x00"), None, None, None);
        assert!(diff.is_binary);
    }

    #[test]
    fn test_multi_hunk() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n";
        let new = "1\n2\n3\n4\n5\n6\n7\nEIGHT\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\nTWENTY\n";
        let hunks = diff_text(old, new);
        // Should produce 2 separate hunks since changes are far apart
        assert!(hunks.len() >= 2);
    }

    #[test]
    fn test_tokenize_words() {
        let tokens = tokenize_words("hello world_foo + bar");
        assert_eq!(
            tokens,
            vec!["hello", " ", "world_foo", " ", "+", " ", "bar"]
        );
    }

    #[test]
    fn test_word_diff_simple() {
        let segs = word_diff_line("the quick brown fox", "the slow brown fox");
        // "the " = equal, "quick" = remove, "slow" = insert, " brown fox" = equal
        let kinds: Vec<_> = segs.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&DiffLineKind::Remove));
        assert!(kinds.contains(&DiffLineKind::Add));
        assert!(kinds.contains(&DiffLineKind::Context));
    }

    #[test]
    fn test_word_diff_identical() {
        let segs = word_diff_line("no change", "no change");
        assert!(segs.iter().all(|s| s.kind == DiffLineKind::Context));
    }
}
