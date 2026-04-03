//! Pull Requests stored as git refs inside the repository
//!
//! PRs are stored at .lit/refs/prs/<id>.json and tracked inside the
//! repository, not in an external service.

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

impl std::fmt::Display for PrState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrState::Open => write!(f, "open"),
            PrState::Merged => write!(f, "merged"),
            PrState::Closed => write!(f, "closed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrComment {
    pub author: String,
    pub body: String,
    pub created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub author: String,
    /// Source branch
    pub head: String,
    /// Target branch
    pub base: String,
    pub state: PrState,
    pub labels: Vec<String>,
    pub reviewers: Vec<String>,
    pub comments: Vec<PrComment>,
    /// Commit hash at head when PR was created
    pub head_commit: Option<String>,
    pub created: String,
    pub updated: String,
}

fn prs_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("refs").join("prs")
}

fn next_id(repo_root: &Path) -> Result<u64, LitError> {
    let dir = prs_dir(repo_root);
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

/// Create a new pull request
pub fn create_pr(
    repo_root: &Path,
    title: &str,
    body: &str,
    author: &str,
    head: &str,
    base: &str,
    labels: Vec<String>,
) -> Result<PullRequest, LitError> {
    let dir = prs_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create PRs dir: {}", e)))?;

    let id = next_id(repo_root)?;
    let now = chrono::Utc::now().to_rfc3339();
    let pr = PullRequest {
        id,
        title: title.to_string(),
        body: body.to_string(),
        author: author.to_string(),
        head: head.to_string(),
        base: base.to_string(),
        state: PrState::Open,
        labels,
        reviewers: Vec::new(),
        comments: Vec::new(),
        head_commit: None,
        created: now.clone(),
        updated: now,
    };

    let path = dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&pr)
        .map_err(|e| LitError::general(format!("Serialize: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write: {}", e)))?;
    Ok(pr)
}

/// Get a PR by ID
pub fn get_pr(repo_root: &Path, id: u64) -> Result<PullRequest, LitError> {
    let path = prs_dir(repo_root).join(format!("{}.json", id));
    if !path.exists() {
        return Err(LitError::general(format!("PR #{} not found", id)));
    }
    let json = fs::read_to_string(&path).map_err(|e| LitError::io(format!("IO: {}", e)))?;
    serde_json::from_str(&json).map_err(|e| LitError::general(format!("Parse: {}", e)))
}

/// List all PRs, optionally filtered by state
pub fn list_prs(repo_root: &Path, state: Option<PrState>) -> Result<Vec<PullRequest>, LitError> {
    let dir = prs_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut prs = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if entry.path().extension().map_or(false, |e| e == "json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(pr) = serde_json::from_str::<PullRequest>(&json) {
                    if state.as_ref().map_or(true, |s| pr.state == *s) {
                        prs.push(pr);
                    }
                }
            }
        }
    }

    prs.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(prs)
}

/// Merge a PR (mark it as merged)
pub fn merge_pr(repo_root: &Path, id: u64) -> Result<PullRequest, LitError> {
    let mut pr = get_pr(repo_root, id)?;
    if pr.state != PrState::Open {
        return Err(LitError::general(format!(
            "PR #{} is not open ({})",
            id, pr.state
        )));
    }
    pr.state = PrState::Merged;
    pr.updated = chrono::Utc::now().to_rfc3339();
    save_pr(repo_root, &pr)?;
    Ok(pr)
}

/// Close a PR without merging
pub fn close_pr(repo_root: &Path, id: u64) -> Result<PullRequest, LitError> {
    let mut pr = get_pr(repo_root, id)?;
    pr.state = PrState::Closed;
    pr.updated = chrono::Utc::now().to_rfc3339();
    save_pr(repo_root, &pr)?;
    Ok(pr)
}

/// Add a comment to a PR
pub fn comment_pr(
    repo_root: &Path,
    id: u64,
    author: &str,
    body: &str,
) -> Result<PullRequest, LitError> {
    let mut pr = get_pr(repo_root, id)?;
    pr.comments.push(PrComment {
        author: author.to_string(),
        body: body.to_string(),
        created: chrono::Utc::now().to_rfc3339(),
    });
    pr.updated = chrono::Utc::now().to_rfc3339();
    save_pr(repo_root, &pr)?;
    Ok(pr)
}

/// Add a reviewer to a PR
pub fn add_reviewer(repo_root: &Path, id: u64, reviewer: &str) -> Result<PullRequest, LitError> {
    let mut pr = get_pr(repo_root, id)?;
    if !pr.reviewers.contains(&reviewer.to_string()) {
        pr.reviewers.push(reviewer.to_string());
        pr.updated = chrono::Utc::now().to_rfc3339();
        save_pr(repo_root, &pr)?;
    }
    Ok(pr)
}

fn save_pr(repo_root: &Path, pr: &PullRequest) -> Result<(), LitError> {
    let path = prs_dir(repo_root).join(format!("{}.json", pr.id));
    let json = serde_json::to_string_pretty(pr)
        .map_err(|e| LitError::general(format!("Serialize: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lit_pr_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_create_and_list() {
        let dir = tmp_dir();
        let pr = create_pr(
            &dir,
            "Add DID support",
            "Implements DIDs",
            "did:lit:user1",
            "feature/did",
            "main",
            vec!["feature".into()],
        )
        .unwrap();
        assert_eq!(pr.id, 1);
        assert_eq!(pr.state, PrState::Open);

        let prs = list_prs(&dir, None).unwrap();
        assert_eq!(prs.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_merge_pr() {
        let dir = tmp_dir();
        create_pr(&dir, "Test", "Body", "user1", "feature", "main", vec![]).unwrap();
        let merged = merge_pr(&dir, 1).unwrap();
        assert_eq!(merged.state, PrState::Merged);

        // Can't merge twice
        assert!(merge_pr(&dir, 1).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
