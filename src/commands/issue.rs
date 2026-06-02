//! Issues stored as git refs inside the repository
//!
//! Issues are stored at .lit/refs/issues/<id>.json and tracked inside
//! the repository itself, not in an external database.

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueState {
    Open,
    Closed,
}

impl std::fmt::Display for IssueState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueState::Open => write!(f, "open"),
            IssueState::Closed => write!(f, "closed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueComment {
    pub author: String,
    pub body: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    pub state: IssueState,
    pub labels: Vec<String>,
    pub comments: Vec<IssueComment>,
    pub created: String,
    pub updated: String,
}

fn issues_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("refs").join("issues")
}

fn next_id(repo_root: &Path) -> Result<u64, LitError> {
    let dir = issues_dir(repo_root);
    if !dir.exists() {
        return Ok(1);
    }
    let mut max_id: u64 = 0;
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if let Some(stem) = entry.path().file_stem() {
            if let Ok(id) = stem.to_string_lossy().parse::<u64>() {
                if id > max_id {
                    max_id = id;
                }
            }
        }
    }
    Ok(max_id + 1)
}

/// Create a new issue
pub fn create_issue(
    repo_root: &Path,
    title: &str,
    body: &str,
    author: &str,
    labels: Vec<String>,
) -> Result<Issue, LitError> {
    let dir = issues_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create issues dir: {}", e)))?;

    let id = next_id(repo_root)?;
    let now = chrono::Utc::now().to_rfc3339();
    let issue = Issue {
        id,
        title: title.to_string(),
        body: body.to_string(),
        author: author.to_string(),
        state: IssueState::Open,
        labels,
        comments: Vec::new(),
        created: now.clone(),
        updated: now,
    };

    let path = dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&issue)
        .map_err(|e| LitError::general(format!("Serialize: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write: {}", e)))?;
    Ok(issue)
}

/// Get an issue by ID
pub fn get_issue(repo_root: &Path, id: u64) -> Result<Issue, LitError> {
    let path = issues_dir(repo_root).join(format!("{}.json", id));
    if !path.exists() {
        return Err(LitError::general(format!("Issue #{} not found", id)));
    }
    let json = fs::read_to_string(&path).map_err(|e| LitError::io(format!("IO: {}", e)))?;
    serde_json::from_str(&json).map_err(|e| LitError::general(format!("Parse: {}", e)))
}

/// List all issues, optionally filtered by state
pub fn list_issues(repo_root: &Path, state: Option<IssueState>) -> Result<Vec<Issue>, LitError> {
    let dir = issues_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut issues = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(issue) = serde_json::from_str::<Issue>(&json) {
                    if state.as_ref().is_none_or(|s| issue.state == *s) {
                        issues.push(issue);
                    }
                }
            }
        }
    }

    issues.sort_by_key(|b| std::cmp::Reverse(b.id));
    Ok(issues)
}

/// Close an issue
pub fn close_issue(repo_root: &Path, id: u64) -> Result<Issue, LitError> {
    let mut issue = get_issue(repo_root, id)?;
    issue.state = IssueState::Closed;
    issue.updated = chrono::Utc::now().to_rfc3339();
    save_issue(repo_root, &issue)?;
    Ok(issue)
}

/// Add a comment to an issue
pub fn comment_issue(
    repo_root: &Path,
    id: u64,
    author: &str,
    body: &str,
) -> Result<Issue, LitError> {
    let mut issue = get_issue(repo_root, id)?;
    issue.comments.push(IssueComment {
        author: author.to_string(),
        body: body.to_string(),
        created: chrono::Utc::now().to_rfc3339(),
    });
    issue.updated = chrono::Utc::now().to_rfc3339();
    save_issue(repo_root, &issue)?;
    Ok(issue)
}

fn save_issue(repo_root: &Path, issue: &Issue) -> Result<(), LitError> {
    let path = issues_dir(repo_root).join(format!("{}.json", issue.id));
    let json = serde_json::to_string_pretty(issue)
        .map_err(|e| LitError::general(format!("Serialize: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tmp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("lit_issue_test_{}_{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_create_and_list_issues() {
        let dir = tmp_dir();
        let issue = create_issue(
            &dir,
            "Bug: crash on merge",
            "Merging fails with panic",
            "did:lit:user1",
            vec!["bug".into()],
        )
        .unwrap();
        assert_eq!(issue.id, 1);
        assert_eq!(issue.state, IssueState::Open);

        let issues = list_issues(&dir, None).unwrap();
        assert_eq!(issues.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_close_issue() {
        let dir = tmp_dir();
        create_issue(&dir, "Test", "Body", "user1", vec![]).unwrap();
        let closed = close_issue(&dir, 1).unwrap();
        assert_eq!(closed.state, IssueState::Closed);

        let open = list_issues(&dir, Some(IssueState::Open)).unwrap();
        assert_eq!(open.len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_comment_issue() {
        let dir = tmp_dir();
        create_issue(&dir, "Test", "Body", "user1", vec![]).unwrap();
        let issue = comment_issue(&dir, 1, "user2", "This needs fixing").unwrap();
        assert_eq!(issue.comments.len(), 1);
        assert_eq!(issue.comments[0].author, "user2");

        let _ = fs::remove_dir_all(&dir);
    }
}
