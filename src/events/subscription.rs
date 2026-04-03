//! Event subscription and notification engine
//!
//! Stores subscriptions as JSON in .lit/events/subscriptions/ and emits
//! events to a log that can be tailed or queried.

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Types of events that can be subscribed to
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EventType {
    CommitPushed,
    BranchCreated,
    BranchDeleted,
    BranchUpdated,
    MergeCompleted,
    TagCreated,
    IssueOpened,
    IssueClosed,
    PrOpened,
    PrMerged,
    PrClosed,
    AgentJoined,
    AgentLeft,
    TaskDelegated,
    TaskCompleted,
    TrustChanged,
    UcanIssued,
    UcanRevoked,
    All,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for EventType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "commitpushed" | "commit" => Ok(EventType::CommitPushed),
            "branchcreated" | "branch-created" => Ok(EventType::BranchCreated),
            "branchdeleted" | "branch-deleted" => Ok(EventType::BranchDeleted),
            "branchupdated" | "branch-updated" => Ok(EventType::BranchUpdated),
            "mergecompleted" | "merge" => Ok(EventType::MergeCompleted),
            "tagcreated" | "tag" => Ok(EventType::TagCreated),
            "issueopened" | "issue-opened" => Ok(EventType::IssueOpened),
            "issueclosed" | "issue-closed" => Ok(EventType::IssueClosed),
            "propened" | "pr-opened" => Ok(EventType::PrOpened),
            "prmerged" | "pr-merged" => Ok(EventType::PrMerged),
            "prclosed" | "pr-closed" => Ok(EventType::PrClosed),
            "agentjoined" | "agent-joined" => Ok(EventType::AgentJoined),
            "agentleft" | "agent-left" => Ok(EventType::AgentLeft),
            "taskdelegated" | "task-delegated" => Ok(EventType::TaskDelegated),
            "taskcompleted" | "task-completed" => Ok(EventType::TaskCompleted),
            "trustchanged" | "trust-changed" => Ok(EventType::TrustChanged),
            "ucanissued" | "ucan-issued" => Ok(EventType::UcanIssued),
            "ucanrevoked" | "ucan-revoked" => Ok(EventType::UcanRevoked),
            "all" | "*" => Ok(EventType::All),
            _ => Err(format!("Unknown event type: {}", s)),
        }
    }
}

/// A subscription entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSubscription {
    /// Unique subscription ID
    pub id: String,
    /// Subscriber DID (or local user if no DID)
    pub subscriber: String,
    /// Event types to subscribe to
    pub event_types: Vec<EventType>,
    /// Optional branch filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_filter: Option<String>,
    /// Created timestamp
    pub created: String,
    /// Whether the subscription is active
    pub active: bool,
}

/// A recorded event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event type
    pub event_type: EventType,
    /// Timestamp
    pub timestamp: String,
    /// Actor DID or identifier
    pub actor: String,
    /// Event-specific payload
    pub payload: serde_json::Value,
    /// Optional branch context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

fn subscriptions_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("events").join("subscriptions")
}

fn events_log_path(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("events").join("log.jsonl")
}

/// Subscribe to event types
pub fn subscribe(
    repo_root: &Path,
    subscriber: &str,
    event_types: Vec<EventType>,
    branch_filter: Option<String>,
) -> Result<EventSubscription, LitError> {
    let dir = subscriptions_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create subscriptions dir: {}", e)))?;

    let id = format!(
        "{:016x}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let sub = EventSubscription {
        id: id.clone(),
        subscriber: subscriber.to_string(),
        event_types,
        branch_filter,
        created: chrono::Utc::now().to_rfc3339(),
        active: true,
    };

    let path = dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&sub)
        .map_err(|e| LitError::general(format!("Serialize error: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write error: {}", e)))?;

    Ok(sub)
}

/// List all subscriptions
pub fn list_subscriptions(repo_root: &Path) -> Result<Vec<EventSubscription>, LitError> {
    let dir = subscriptions_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut subs = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if entry.path().extension().map_or(false, |e| e == "json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(sub) = serde_json::from_str::<EventSubscription>(&json) {
                    subs.push(sub);
                }
            }
        }
    }
    Ok(subs)
}

/// Remove a subscription
pub fn unsubscribe(repo_root: &Path, sub_id: &str) -> Result<(), LitError> {
    let path = subscriptions_dir(repo_root).join(format!("{}.json", sub_id));
    if path.exists() {
        fs::remove_file(&path).map_err(|e| LitError::io(format!("Remove error: {}", e)))?;
        Ok(())
    } else {
        Err(LitError::general(format!(
            "Subscription not found: {}",
            sub_id
        )))
    }
}

/// Emit an event to the event log
pub fn emit_event(repo_root: &Path, event: &Event) -> Result<(), LitError> {
    let log_path = events_log_path(repo_root);
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent).map_err(|e| LitError::io(format!("IO: {}", e)))?;
    }

    let line = serde_json::to_string(event)
        .map_err(|e| LitError::general(format!("Serialize error: {}", e)))?;

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| LitError::io(format!("IO: {}", e)))?;
    writeln!(file, "{}", line).map_err(|e| LitError::io(format!("Write error: {}", e)))?;

    Ok(())
}

/// Read recent events from the log, optionally filtered
pub fn read_events(
    repo_root: &Path,
    event_type: Option<&EventType>,
    limit: usize,
) -> Result<Vec<Event>, LitError> {
    let log_path = events_log_path(repo_root);
    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&log_path).map_err(|e| LitError::io(format!("IO: {}", e)))?;

    let mut events: Vec<Event> = content
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
        .filter(|e| event_type.map_or(true, |et| *et == EventType::All || e.event_type == *et))
        .collect();

    // Return most recent first
    events.reverse();
    events.truncate(limit);
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lit_events_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_subscribe_and_list() {
        let dir = tmp_dir();
        let sub = subscribe(
            &dir,
            "did:lit:user1",
            vec![EventType::CommitPushed, EventType::MergeCompleted],
            Some("main".to_string()),
        )
        .unwrap();

        assert!(sub.active);
        assert_eq!(sub.event_types.len(), 2);

        let subs = list_subscriptions(&dir).unwrap();
        assert_eq!(subs.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_emit_and_read() {
        let dir = tmp_dir();
        let event = Event {
            event_type: EventType::CommitPushed,
            timestamp: chrono::Utc::now().to_rfc3339(),
            actor: "did:lit:agent1".to_string(),
            payload: serde_json::json!({"hash": "abc123"}),
            branch: Some("main".to_string()),
        };
        emit_event(&dir, &event).unwrap();

        let events = read_events(&dir, None, 10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::CommitPushed);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_event_type_parse() {
        assert_eq!(
            "commit".parse::<EventType>().unwrap(),
            EventType::CommitPushed
        );
        assert_eq!("all".parse::<EventType>().unwrap(), EventType::All);
        assert!("invalid".parse::<EventType>().is_err());
    }
}
