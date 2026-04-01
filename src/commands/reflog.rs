use crate::core::{find_repo_root, get_current_branch};
use crate::response::{ReflogEntry, ReflogResponse};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReflogRecord {
    pub old_hash: String,
    pub new_hash: String,
    pub action: String,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReflogFile {
    entries: Vec<ReflogRecord>,
}

/// Append a reflog entry (called from other commands)
pub fn append_reflog(
    repo_root: &std::path::Path,
    ref_name: &str,
    old_hash: &str,
    new_hash: &str,
    action: &str,
    message: &str,
) -> Result<(), crate::errors::LitError> {
    let reflog_dir = repo_root.join(".lit").join("reflog");
    fs::create_dir_all(&reflog_dir)
        .map_err(|e| format!("Failed to create reflog directory: {}", e))?;

    let safe_name = ref_name.replace('/', "_");
    let path = reflog_dir.join(&safe_name);

    let mut reflog = if path.exists() {
        let data =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read reflog: {}", e))?;
        serde_json::from_str(&data).unwrap_or(ReflogFile {
            entries: Vec::new(),
        })
    } else {
        ReflogFile {
            entries: Vec::new(),
        }
    };

    reflog.entries.push(ReflogRecord {
        old_hash: old_hash.to_string(),
        new_hash: new_hash.to_string(),
        action: action.to_string(),
        message: message.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    });

    let data = serde_json::to_string_pretty(&reflog)
        .map_err(|e| format!("Failed to serialize reflog: {}", e))?;
    fs::write(&path, data).map_err(|e| format!("Failed to write reflog: {}", e).into())
}

pub fn execute(
    ref_name: Option<String>,
    count: usize,
) -> Result<ReflogResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    let target_ref = match ref_name {
        Some(r) => r,
        None => {
            // Default to HEAD / current branch
            get_current_branch(&repo_root).unwrap_or_else(|_| "HEAD".to_string())
        }
    };

    let reflog_dir = repo_root.join(".lit").join("reflog");
    let safe_name = target_ref.replace('/', "_");
    let path = reflog_dir.join(&safe_name);

    if !path.exists() {
        return Ok(ReflogResponse {
            ref_name: target_ref,
            entries: Vec::new(),
        });
    }

    let data = fs::read_to_string(&path).map_err(|e| format!("Failed to read reflog: {}", e))?;
    let reflog: ReflogFile =
        serde_json::from_str(&data).map_err(|e| format!("Failed to parse reflog: {}", e))?;

    let entries: Vec<ReflogEntry> = reflog
        .entries
        .iter()
        .rev()
        .take(count)
        .enumerate()
        .map(|(i, r)| ReflogEntry {
            index: i,
            old_hash: r.old_hash[..16.min(r.old_hash.len())].to_string(),
            new_hash: r.new_hash[..16.min(r.new_hash.len())].to_string(),
            action: r.action.clone(),
            message: r.message.clone(),
            timestamp: r.timestamp,
        })
        .collect();

    Ok(ReflogResponse {
        ref_name: target_ref,
        entries,
    })
}
