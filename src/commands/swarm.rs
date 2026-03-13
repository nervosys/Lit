use crate::core::find_repo_root;
use crate::response::SwarmResponse;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Lease state for exclusive file access
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileLease {
    agent_id: String,
    path: String,
    acquired_at: i64,
    expires_at: i64,
}

/// Agent registration record
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentRecord {
    agent_id: String,
    registered_at: i64,
    branches: Vec<String>,
    leases: Vec<String>,
}

fn swarm_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".lit").join("swarm")
}

fn leases_dir(repo_root: &Path) -> PathBuf {
    swarm_dir(repo_root).join("leases")
}

fn agents_dir(repo_root: &Path) -> PathBuf {
    swarm_dir(repo_root).join("agents")
}

/// Register an agent in the swarm — creates namespaced branch prefix
pub fn execute_register(agent_id: String) -> Result<SwarmResponse, String> {
    let repo_root = find_repo_root()?;
    let agents = agents_dir(&repo_root);
    fs::create_dir_all(&agents).map_err(|e| format!("Failed to create swarm dir: {}", e))?;
    fs::create_dir_all(leases_dir(&repo_root))
        .map_err(|e| format!("Failed to create leases dir: {}", e))?;

    // Create agent namespace directory for refs
    let agent_refs = repo_root
        .join(".lit")
        .join("refs")
        .join("agents")
        .join(&agent_id);
    fs::create_dir_all(&agent_refs).map_err(|e| format!("Failed to create agent refs: {}", e))?;

    let record = AgentRecord {
        agent_id: agent_id.clone(),
        registered_at: chrono::Utc::now().timestamp(),
        branches: Vec::new(),
        leases: Vec::new(),
    };

    let path = agents.join(format!("{}.json", sanitize_id(&agent_id)));
    let data = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Failed to serialize agent record: {}", e))?;
    fs::write(&path, &data).map_err(|e| format!("Failed to write agent record: {}", e))?;

    Ok(SwarmResponse {
        action: "register".to_string(),
        agent_id: Some(agent_id.clone()),
        message: format!(
            "Agent '{}' registered. Branch namespace: agents/{}/",
            agent_id, agent_id
        ),
        details: Some(serde_json::json!({
            "namespace": format!("agents/{}", agent_id),
            "refs_path": format!(".lit/refs/agents/{}", agent_id),
        })),
    })
}

/// List registered agents
pub fn execute_list() -> Result<SwarmResponse, String> {
    let repo_root = find_repo_root()?;
    let agents = agents_dir(&repo_root);

    if !agents.exists() {
        return Ok(SwarmResponse {
            action: "list".to_string(),
            agent_id: None,
            message: "No agents registered".to_string(),
            details: Some(serde_json::json!({ "agents": [] })),
        });
    }

    let mut agent_list = Vec::new();
    for entry in fs::read_dir(&agents).map_err(|e| format!("Failed to read agents dir: {}", e))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .path()
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            let data = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            if let Ok(record) = serde_json::from_str::<AgentRecord>(&data) {
                agent_list.push(serde_json::json!({
                    "agent_id": record.agent_id,
                    "registered_at": record.registered_at,
                    "branches": record.branches.len(),
                    "leases": record.leases.len(),
                }));
            }
        }
    }

    Ok(SwarmResponse {
        action: "list".to_string(),
        agent_id: None,
        message: format!("{} agent(s) registered", agent_list.len()),
        details: Some(serde_json::json!({ "agents": agent_list })),
    })
}

/// Acquire an exclusive file lease
pub fn execute_lease_acquire(
    agent_id: String,
    file_path: String,
    duration_secs: u64,
) -> Result<SwarmResponse, String> {
    let repo_root = find_repo_root()?;
    let leases = leases_dir(&repo_root);
    fs::create_dir_all(&leases).map_err(|e| format!("Failed to create leases dir: {}", e))?;

    let lease_file = leases.join(format!("{}.json", sanitize_path(&file_path)));
    let now = chrono::Utc::now().timestamp();

    // Check for existing lease
    if lease_file.exists() {
        let data = fs::read_to_string(&lease_file).map_err(|e| e.to_string())?;
        if let Ok(existing) = serde_json::from_str::<FileLease>(&data) {
            if existing.expires_at > now && existing.agent_id != agent_id {
                return Err(format!(
                    "File '{}' is leased by agent '{}' until timestamp {}",
                    file_path, existing.agent_id, existing.expires_at
                ));
            }
        }
    }

    let lease = FileLease {
        agent_id: agent_id.clone(),
        path: file_path.clone(),
        acquired_at: now,
        expires_at: now + duration_secs as i64,
    };

    let data = serde_json::to_string_pretty(&lease)
        .map_err(|e| format!("Failed to serialize lease: {}", e))?;
    fs::write(&lease_file, &data).map_err(|e| format!("Failed to write lease: {}", e))?;

    Ok(SwarmResponse {
        action: "lease-acquire".to_string(),
        agent_id: Some(agent_id),
        message: format!(
            "Lease acquired on '{}' for {} seconds",
            file_path, duration_secs
        ),
        details: Some(serde_json::json!({
            "path": file_path,
            "expires_at": lease.expires_at,
        })),
    })
}

/// Release a file lease
pub fn execute_lease_release(agent_id: String, file_path: String) -> Result<SwarmResponse, String> {
    let repo_root = find_repo_root()?;
    let lease_file = leases_dir(&repo_root).join(format!("{}.json", sanitize_path(&file_path)));

    if !lease_file.exists() {
        return Err(format!("No lease exists for '{}'", file_path));
    }

    let data = fs::read_to_string(&lease_file).map_err(|e| e.to_string())?;
    let existing: FileLease =
        serde_json::from_str(&data).map_err(|e| format!("Invalid lease file: {}", e))?;

    if existing.agent_id != agent_id {
        return Err(format!(
            "Lease on '{}' is held by agent '{}', not '{}'",
            file_path, existing.agent_id, agent_id
        ));
    }

    fs::remove_file(&lease_file).map_err(|e| format!("Failed to remove lease: {}", e))?;

    Ok(SwarmResponse {
        action: "lease-release".to_string(),
        agent_id: Some(agent_id),
        message: format!("Lease released on '{}'", file_path),
        details: None,
    })
}

/// List all active leases
pub fn execute_lease_list() -> Result<SwarmResponse, String> {
    let repo_root = find_repo_root()?;
    let leases = leases_dir(&repo_root);

    if !leases.exists() {
        return Ok(SwarmResponse {
            action: "lease-list".to_string(),
            agent_id: None,
            message: "No active leases".to_string(),
            details: Some(serde_json::json!({ "leases": [] })),
        });
    }

    let now = chrono::Utc::now().timestamp();
    let mut active_leases = Vec::new();

    for entry in fs::read_dir(&leases).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry
            .path()
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            let data = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
            if let Ok(lease) = serde_json::from_str::<FileLease>(&data) {
                let expired = lease.expires_at <= now;
                active_leases.push(serde_json::json!({
                    "agent_id": lease.agent_id,
                    "path": lease.path,
                    "acquired_at": lease.acquired_at,
                    "expires_at": lease.expires_at,
                    "expired": expired,
                }));
            }
        }
    }

    Ok(SwarmResponse {
        action: "lease-list".to_string(),
        agent_id: None,
        message: format!("{} lease(s)", active_leases.len()),
        details: Some(serde_json::json!({ "leases": active_leases })),
    })
}

/// Sanitize an agent ID for use as a filename
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Sanitize a file path for use as a lease filename
fn sanitize_path(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
