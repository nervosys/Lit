//! Intent-based workflow for agentic development
//!
//! Intents replace the branch+PR model with declared units of work that have
//! explicit scope, agent attribution, priority, and hierarchical decomposition.
//! Multiple intents can be active simultaneously on the same working tree
//! because they declare non-overlapping scopes.

use crate::core::find_repo_root;
use crate::errors::LitError;
use crate::response::IntentResponse;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ── Data types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntentStatus {
    Active,
    Converged,
    Abandoned,
}

impl std::fmt::Display for IntentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentStatus::Active => write!(f, "active"),
            IntentStatus::Converged => write!(f, "converged"),
            IntentStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntentPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for IntentPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentPriority::Low => write!(f, "low"),
            IntentPriority::Medium => write!(f, "medium"),
            IntentPriority::High => write!(f, "high"),
            IntentPriority::Critical => write!(f, "critical"),
        }
    }
}

/// A scope-checked conflict between two active intents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeConflict {
    pub intent_id: String,
    pub agent: String,
    pub overlapping_paths: Vec<String>,
}

/// A declared unit of work with scope, agent, and hierarchy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub title: String,
    pub agent: String,
    pub scope: Vec<String>,
    pub priority: IntentPriority,
    pub status: IntentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub commits: Vec<String>,
    #[serde(default)]
    pub children: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ucan_proof: Option<String>,
    pub created: String,
    pub updated: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn intents_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".lit").join("intents")
}

fn save_intent(repo_root: &Path, intent: &Intent) -> Result<(), LitError> {
    let dir = intents_dir(repo_root);
    fs::create_dir_all(&dir).map_err(|e| LitError::io(format!("Create intents dir: {}", e)))?;
    let path = dir.join(format!("{}.json", intent.id));
    let json = serde_json::to_string_pretty(intent)
        .map_err(|e| LitError::general(format!("Serialize intent: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write intent: {}", e)))?;
    Ok(())
}

pub fn load_intent(repo_root: &Path, id: &str) -> Result<Intent, LitError> {
    let path = intents_dir(repo_root).join(format!("{}.json", id));
    if !path.exists() {
        return Err(LitError::general(format!("Intent not found: {}", id)));
    }
    let json = fs::read_to_string(&path).map_err(|e| LitError::io(format!("Read: {}", e)))?;
    serde_json::from_str(&json).map_err(|e| LitError::general(format!("Parse intent: {}", e)))
}

fn load_all_intents(repo_root: &Path) -> Result<Vec<Intent>, LitError> {
    let dir = intents_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut intents = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(e.to_string()))? {
        let entry = entry.map_err(|e| LitError::io(e.to_string()))?;
        if entry
            .path()
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            let json = fs::read_to_string(entry.path()).map_err(|e| LitError::io(e.to_string()))?;
            if let Ok(intent) = serde_json::from_str::<Intent>(&json) {
                intents.push(intent);
            }
        }
    }
    Ok(intents)
}

/// Check whether two scope patterns overlap.
/// Patterns use simple prefix/glob matching: `src/auth/**` overlaps with `src/auth/jwt.rs`.
fn scopes_overlap(a: &[String], b: &[String]) -> Vec<String> {
    let mut overlaps = Vec::new();
    for pa in a {
        let pa_base = pa.trim_end_matches("/**").trim_end_matches("/*");
        for pb in b {
            let pb_base = pb.trim_end_matches("/**").trim_end_matches("/*");
            // Overlap if one is a prefix of the other, or they are the same
            if pa_base.starts_with(pb_base) || pb_base.starts_with(pa_base) || pa == pb {
                overlaps.push(format!("{} <-> {}", pa, pb));
            }
        }
    }
    overlaps
}

/// Auto-acquire swarm leases for intent scope paths
fn auto_acquire_leases(repo_root: &Path, agent: &str, scope: &[String]) {
    let leases_dir = repo_root.join(".lit").join("swarm").join("leases");
    let _ = fs::create_dir_all(&leases_dir);
    let now = chrono::Utc::now().timestamp();
    let duration = 3600i64; // 1 hour default
    for path in scope {
        let sanitized: String = path
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let lease_file = leases_dir.join(format!("{}.json", sanitized));
        // Only acquire if not already leased by another agent
        if lease_file.exists() {
            if let Ok(data) = fs::read_to_string(&lease_file) {
                if let Ok(existing) = serde_json::from_str::<serde_json::Value>(&data) {
                    if existing.get("agent_id").and_then(|v| v.as_str()) != Some(agent)
                        && existing
                            .get("expires_at")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                            > now
                    {
                        continue; // Already leased by someone else
                    }
                }
            }
        }
        let lease = serde_json::json!({
            "agent_id": agent,
            "path": path,
            "acquired_at": now,
            "expires_at": now + duration,
        });
        let _ = fs::write(
            &lease_file,
            serde_json::to_string_pretty(&lease).unwrap_or_default(),
        );
    }
}

/// Release swarm leases held by an agent for given scope paths
fn auto_release_leases(repo_root: &Path, agent: &str, scope: &[String]) {
    let leases_dir = repo_root.join(".lit").join("swarm").join("leases");
    if !leases_dir.exists() {
        return;
    }
    for path in scope {
        let sanitized: String = path
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let lease_file = leases_dir.join(format!("{}.json", sanitized));
        if lease_file.exists() {
            if let Ok(data) = fs::read_to_string(&lease_file) {
                if let Ok(existing) = serde_json::from_str::<serde_json::Value>(&data) {
                    if existing.get("agent_id").and_then(|v| v.as_str()) == Some(agent) {
                        let _ = fs::remove_file(&lease_file);
                    }
                }
            }
        }
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Create a new intent — checks for scope conflicts with other active intents
pub fn execute_create(
    title: String,
    agent: String,
    scope: Vec<String>,
    priority: IntentPriority,
    parent: Option<String>,
    ucan_proof: Option<String>,
) -> Result<IntentResponse, LitError> {
    let repo_root = find_repo_root()?;
    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("intent-{}", chrono::Utc::now().timestamp_millis());

    // Check scope conflicts with other active intents
    let all = load_all_intents(&repo_root)?;
    let mut conflicts = Vec::new();
    for existing in &all {
        if existing.status != IntentStatus::Active {
            continue;
        }
        let overlaps = scopes_overlap(&scope, &existing.scope);
        if !overlaps.is_empty() {
            conflicts.push(ScopeConflict {
                intent_id: existing.id.clone(),
                agent: existing.agent.clone(),
                overlapping_paths: overlaps,
            });
        }
    }

    // If there's a parent intent, register this as a child
    if let Some(ref parent_id) = parent {
        let mut parent_intent = load_intent(&repo_root, parent_id)?;
        parent_intent.children.push(id.clone());
        parent_intent.updated = now.clone();
        save_intent(&repo_root, &parent_intent)?;
    }

    let intent = Intent {
        id: id.clone(),
        title: title.clone(),
        agent: agent.clone(),
        scope: scope.clone(),
        priority,
        status: IntentStatus::Active,
        parent,
        commits: Vec::new(),
        children: Vec::new(),
        ucan_proof,
        created: now.clone(),
        updated: now,
    };

    save_intent(&repo_root, &intent)?;

    // Auto-acquire leases for the scoped paths
    auto_acquire_leases(&repo_root, &agent, &scope);

    let has_conflicts = !conflicts.is_empty();
    let details = serde_json::json!({
        "intent": intent,
        "conflicts": conflicts,
    });

    Ok(IntentResponse {
        action: "create".into(),
        intent_id: Some(id),
        message: if has_conflicts {
            format!(
                "Intent '{}' created with {} scope conflict(s) — lease negotiation may be required",
                title,
                conflicts.len()
            )
        } else {
            format!("Intent '{}' created — scope clear, leases acquired", title)
        },
        details: Some(details),
    })
}

/// List intents, optionally filtered by status or agent
pub fn execute_list(
    status_filter: Option<String>,
    agent_filter: Option<String>,
) -> Result<IntentResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut intents = load_all_intents(&repo_root)?;

    if let Some(ref status) = status_filter {
        intents.retain(|i| i.status.to_string() == *status);
    }
    if let Some(ref agent) = agent_filter {
        intents.retain(|i| i.agent == *agent);
    }

    let count = intents.len();
    Ok(IntentResponse {
        action: "list".into(),
        intent_id: None,
        message: format!("{} intent(s)", count),
        details: Some(serde_json::to_value(&intents).unwrap_or_default()),
    })
}

/// Show details of a specific intent
pub fn execute_show(intent_id: String) -> Result<IntentResponse, LitError> {
    let repo_root = find_repo_root()?;
    let intent = load_intent(&repo_root, &intent_id)?;

    Ok(IntentResponse {
        action: "show".into(),
        intent_id: Some(intent.id.clone()),
        message: format!(
            "{} [{}] — {} commit(s), {} child(ren)",
            intent.title,
            intent.status,
            intent.commits.len(),
            intent.children.len()
        ),
        details: Some(serde_json::to_value(&intent).unwrap_or_default()),
    })
}

/// Close (abandon) an intent — releases leases
pub fn execute_close(intent_id: String) -> Result<IntentResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut intent = load_intent(&repo_root, &intent_id)?;

    if intent.status != IntentStatus::Active {
        return Err(LitError::general(format!(
            "Intent {} is already {}",
            intent_id, intent.status
        )));
    }

    intent.status = IntentStatus::Abandoned;
    intent.updated = chrono::Utc::now().to_rfc3339();
    save_intent(&repo_root, &intent)?;

    // Release leases
    auto_release_leases(&repo_root, &intent.agent, &intent.scope);

    Ok(IntentResponse {
        action: "close".into(),
        intent_id: Some(intent_id),
        message: format!("Intent '{}' abandoned, leases released", intent.title),
        details: None,
    })
}

/// Attach a commit hash to an intent (called by `lit commit --intent`)
pub fn attach_commit(repo_root: &Path, intent_id: &str, commit_hash: &str) -> Result<(), LitError> {
    let mut intent = load_intent(repo_root, intent_id)?;
    if intent.status != IntentStatus::Active {
        return Err(LitError::general(format!(
            "Cannot commit to intent {} — status is {}",
            intent_id, intent.status
        )));
    }
    intent.commits.push(commit_hash.to_string());
    intent.updated = chrono::Utc::now().to_rfc3339();
    save_intent(repo_root, &intent)?;
    Ok(())
}

/// Mark an intent as converged (called by `lit converge`)
pub fn mark_converged(repo_root: &Path, intent_id: &str) -> Result<Intent, LitError> {
    let mut intent = load_intent(repo_root, intent_id)?;
    intent.status = IntentStatus::Converged;
    intent.updated = chrono::Utc::now().to_rfc3339();
    save_intent(repo_root, &intent)?;

    // Release leases
    auto_release_leases(repo_root, &intent.agent, &intent.scope);

    Ok(intent)
}
