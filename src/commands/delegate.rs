//! Agent Task Delegation Protocol
//!
//! Enables formal agent-to-agent work assignment with tracking, status
//! updates, and integration with the trust scoring system.

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Accepted,
    InProgress,
    Completed,
    Failed,
    Rejected,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Accepted => write!(f, "accepted"),
            TaskStatus::InProgress => write!(f, "in-progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskPriority::Low => write!(f, "low"),
            TaskPriority::Medium => write!(f, "medium"),
            TaskPriority::High => write!(f, "high"),
            TaskPriority::Critical => write!(f, "critical"),
        }
    }
}

/// A task delegation from one agent to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedTask {
    /// Unique task ID
    pub id: String,
    /// Delegator DID
    pub delegator: String,
    /// Delegatee DID
    pub delegatee: String,
    /// Task title
    pub title: String,
    /// Detailed description / specification
    pub description: String,
    /// Task priority
    pub priority: TaskPriority,
    /// Current status
    pub status: TaskStatus,
    /// UCAN token CID that authorizes this delegation (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ucan_proof: Option<String>,
    /// Specific files or paths this task applies to
    #[serde(default)]
    pub scope: Vec<String>,
    /// Optional deadline
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    /// Result or output when completed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// Status history
    pub history: Vec<TaskHistoryEntry>,
    pub created: String,
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHistoryEntry {
    pub status: TaskStatus,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn tasks_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("delegations")
}

/// Create a new delegated task
#[allow(clippy::too_many_arguments)]
pub fn create_task(
    repo_root: &Path,
    delegator: &str,
    delegatee: &str,
    title: &str,
    description: &str,
    priority: TaskPriority,
    scope: Vec<String>,
    deadline: Option<String>,
    ucan_proof: Option<String>,
) -> Result<DelegatedTask, LitError> {
    let dir = tasks_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create delegations dir: {}", e)))?;

    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("task-{}", chrono::Utc::now().timestamp_millis());
    let task = DelegatedTask {
        id: id.clone(),
        delegator: delegator.to_string(),
        delegatee: delegatee.to_string(),
        title: title.to_string(),
        description: description.to_string(),
        priority,
        status: TaskStatus::Pending,
        ucan_proof,
        scope,
        deadline,
        result: None,
        history: vec![TaskHistoryEntry {
            status: TaskStatus::Pending,
            timestamp: now.clone(),
            message: Some("Task created".to_string()),
        }],
        created: now.clone(),
        updated: now,
    };

    let path = dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&task)
        .map_err(|e| LitError::general(format!("Serialize: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write: {}", e)))?;
    Ok(task)
}

/// Update a task's status
pub fn update_task_status(
    repo_root: &Path,
    task_id: &str,
    new_status: TaskStatus,
    message: Option<String>,
) -> Result<DelegatedTask, LitError> {
    let mut task = get_task(repo_root, task_id)?;
    let now = chrono::Utc::now().to_rfc3339();

    task.history.push(TaskHistoryEntry {
        status: new_status.clone(),
        timestamp: now.clone(),
        message,
    });
    task.status = new_status;
    task.updated = now;

    save_task(repo_root, &task)?;
    Ok(task)
}

/// Complete a task with a result message
pub fn complete_task(
    repo_root: &Path,
    task_id: &str,
    result: &str,
) -> Result<DelegatedTask, LitError> {
    let mut task = get_task(repo_root, task_id)?;
    let now = chrono::Utc::now().to_rfc3339();

    task.result = Some(result.to_string());
    task.history.push(TaskHistoryEntry {
        status: TaskStatus::Completed,
        timestamp: now.clone(),
        message: Some(result.to_string()),
    });
    task.status = TaskStatus::Completed;
    task.updated = now;

    save_task(repo_root, &task)?;
    Ok(task)
}

/// Get a task by ID
pub fn get_task(repo_root: &Path, task_id: &str) -> Result<DelegatedTask, LitError> {
    let path = tasks_dir(repo_root).join(format!("{}.json", task_id));
    if !path.exists() {
        return Err(LitError::general(format!("Task not found: {}", task_id)));
    }
    let json = fs::read_to_string(&path).map_err(|e| LitError::io(format!("IO: {}", e)))?;
    serde_json::from_str(&json).map_err(|e| LitError::general(format!("Parse: {}", e)))
}

/// List tasks, optionally filtered by delegator or delegatee
pub fn list_tasks(
    repo_root: &Path,
    agent_did: Option<&str>,
    status: Option<TaskStatus>,
) -> Result<Vec<DelegatedTask>, LitError> {
    let dir = tasks_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(task) = serde_json::from_str::<DelegatedTask>(&json) {
                    let agent_match =
                        agent_did.is_none_or(|did| task.delegator == did || task.delegatee == did);
                    let status_match = status.as_ref().is_none_or(|s| task.status == *s);
                    if agent_match && status_match {
                        tasks.push(task);
                    }
                }
            }
        }
    }

    tasks.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(tasks)
}

fn save_task(repo_root: &Path, task: &DelegatedTask) -> Result<(), LitError> {
    let path = tasks_dir(repo_root).join(format!("{}.json", task.id));
    let json = serde_json::to_string_pretty(task)
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
        let dir =
            std::env::temp_dir().join(format!("lit_delegate_test_{}_{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_create_and_list_tasks() {
        let dir = tmp_dir();
        let task = create_task(
            &dir,
            "did:lit:manager",
            "did:lit:agent1",
            "Fix merge bug",
            "The merge function panics on empty branches",
            TaskPriority::High,
            vec!["src/commands/merge.rs".into()],
            None,
            None,
        )
        .unwrap();

        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.delegatee, "did:lit:agent1");

        let tasks = list_tasks(&dir, Some("did:lit:agent1"), None).unwrap();
        assert_eq!(tasks.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_task_lifecycle() {
        let dir = tmp_dir();
        let task = create_task(
            &dir,
            "did:lit:a",
            "did:lit:b",
            "Task",
            "Description",
            TaskPriority::Medium,
            vec![],
            None,
            None,
        )
        .unwrap();

        let task =
            update_task_status(&dir, &task.id, TaskStatus::Accepted, Some("On it".into())).unwrap();
        assert_eq!(task.status, TaskStatus::Accepted);

        let task = complete_task(&dir, &task.id, "Fixed in commit abc123").unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.history.len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }
}
