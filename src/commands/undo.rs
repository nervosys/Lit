use crate::core::find_repo_root;
use crate::errors::LitError;
use crate::response::CommandResponse;
use serde::{Deserialize, Serialize};

/// Operation log entry for undo timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpLogEntry {
    pub id: u64,
    pub timestamp: i64,
    pub operation: String,
    pub description: String,
    /// Snapshot hash before the operation (for reverting)
    pub before_snapshot: String,
    /// Snapshot hash after the operation
    pub after_snapshot: String,
    /// Whether this entry has been undone
    pub undone: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpLogResponse {
    pub action: String,
    pub message: String,
    pub entries: Option<Vec<OpLogEntry>>,
    pub undone_entry: Option<OpLogEntry>,
}

impl CommandResponse for OpLogResponse {
    fn command_name(&self) -> &'static str {
        "undo"
    }
    fn human_readable(&self) -> String {
        let mut out = format!("{}\n", self.message);
        if let Some(ref entries) = self.entries {
            for entry in entries {
                let status = if entry.undone { " (undone)" } else { "" };
                let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(entry.timestamp, 0)
                    .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                out.push_str(&format!(
                    "  {:>4}  {}  {}{}\n",
                    entry.id, dt, entry.description, status
                ));
            }
        }
        if let Some(ref entry) = self.undone_entry {
            out.push_str(&format!("  Reverted: {}\n", entry.description));
        }
        out
    }
}

/// Path to the operation log file
fn oplog_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("oplog.json")
}

/// Load the operation log
fn load_oplog(repo_root: &std::path::Path) -> Vec<OpLogEntry> {
    let path = oplog_path(repo_root);
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    }
}

/// Save the operation log
fn save_oplog(repo_root: &std::path::Path, entries: &[OpLogEntry]) -> Result<(), LitError> {
    let path = oplog_path(repo_root);
    let data = serde_json::to_string_pretty(entries)
        .map_err(|e| LitError::general(format!("Failed to serialize oplog: {}", e)))?;
    std::fs::write(path, data)
        .map_err(|e| LitError::io(format!("Failed to write oplog: {}", e)))?;
    Ok(())
}

/// Record an operation in the log (called by other commands)
pub fn record_operation(
    repo_root: &std::path::Path,
    operation: &str,
    description: &str,
    before_snapshot: &str,
    after_snapshot: &str,
) -> Result<(), LitError> {
    let mut entries = load_oplog(repo_root);
    let id = entries.last().map(|e| e.id + 1).unwrap_or(1);
    entries.push(OpLogEntry {
        id,
        timestamp: chrono::Utc::now().timestamp(),
        operation: operation.to_string(),
        description: description.to_string(),
        before_snapshot: before_snapshot.to_string(),
        after_snapshot: after_snapshot.to_string(),
        undone: false,
    });
    save_oplog(repo_root, &entries)?;
    Ok(())
}

/// List the operation log
pub fn execute_list(count: usize) -> Result<OpLogResponse, LitError> {
    let repo_root = find_repo_root()?;
    let entries = load_oplog(&repo_root);
    let shown: Vec<OpLogEntry> = entries.into_iter().rev().take(count).collect();

    Ok(OpLogResponse {
        action: "list".into(),
        message: format!("Showing {} operation(s)", shown.len()),
        entries: Some(shown),
        undone_entry: None,
    })
}

/// Undo the last operation (or a specific operation by ID)
pub fn execute_undo(target_id: Option<u64>) -> Result<OpLogResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut entries = load_oplog(&repo_root);

    if entries.is_empty() {
        return Err(LitError::general("No operations to undo"));
    }

    let target = if let Some(id) = target_id {
        entries
            .iter_mut()
            .find(|e| e.id == id && !e.undone)
            .ok_or_else(|| {
                LitError::general(format!("Operation {} not found or already undone", id))
            })?
    } else {
        entries
            .iter_mut()
            .rev()
            .find(|e| !e.undone)
            .ok_or_else(|| LitError::general("No operations to undo"))?
    };

    // Restore the before-snapshot state
    let before = target.before_snapshot.clone();
    let description = target.description.clone();
    target.undone = true;
    let undone_entry = target.clone();

    // Restore HEAD to the before-snapshot state
    let branch = crate::core::get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    if !before.is_empty() {
        crate::core::write_ref(&repo_root, &format!("heads/{}", branch), &before)?;
    }

    save_oplog(&repo_root, &entries)?;

    Ok(OpLogResponse {
        action: "undo".into(),
        message: format!("Undone: {}", description),
        entries: None,
        undone_entry: Some(undone_entry),
    })
}

/// Redo a previously undone operation
pub fn execute_redo(target_id: Option<u64>) -> Result<OpLogResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut entries = load_oplog(&repo_root);

    let target = if let Some(id) = target_id {
        entries
            .iter_mut()
            .find(|e| e.id == id && e.undone)
            .ok_or_else(|| LitError::general(format!("Operation {} not found or not undone", id)))?
    } else {
        entries
            .iter_mut()
            .rev()
            .find(|e| e.undone)
            .ok_or_else(|| LitError::general("No undone operations to redo"))?
    };

    let after = target.after_snapshot.clone();
    let description = target.description.clone();
    target.undone = false;
    let redone_entry = target.clone();

    let branch = crate::core::get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    if !after.is_empty() {
        crate::core::write_ref(&repo_root, &format!("heads/{}", branch), &after)?;
    }

    save_oplog(&repo_root, &entries)?;

    Ok(OpLogResponse {
        action: "redo".into(),
        message: format!("Redone: {}", description),
        entries: None,
        undone_entry: Some(redone_entry),
    })
}
