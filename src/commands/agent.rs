//! `lit agent` — start, inspect, and stop the passphrase agent.
//!
//! See `crate::crypto::agent` for what the agent does and, more importantly,
//! what it does not protect against.

use crate::crypto::agent;
use crate::crypto::encryption::{prompt_for_passphrase, EncryptionConfig};
use crate::errors::LitError;
use crate::response::AgentResponse;
use std::time::Duration;

/// Which repository an entry belongs to.
///
/// This must be the exact string `EncryptionManager::new_auto` looks up under,
/// which is the repository root. Keying on anything else — the key file, say —
/// would let `lit agent unlock` report success while every later command went
/// on prompting, because it would be searching under a different name.
fn repo_key() -> Result<String, LitError> {
    let root = crate::core::find_repo_root()
        .map_err(|e| LitError::Repository(format!("Not inside a repository: {e}")))?;
    Ok(root.to_string_lossy().to_string())
}

pub fn start(timeout_secs: Option<u64>) -> Result<AgentResponse, LitError> {
    let timeout = timeout_secs.unwrap_or(agent::DEFAULT_IDLE_TIMEOUT_SECS);

    // `serve` blocks until the agent is told to stop, so anything printed here
    // would not reach the user until then. The caller is expected to background
    // this process; `lit agent status` is how you check on it.
    agent::serve(Duration::from_secs(timeout)).map_err(LitError::Encryption)?;

    Ok(AgentResponse {
        action: "stop".to_string(),
        entries: Some(0),
        idle_timeout_secs: Some(timeout),
        message: "Agent stopped".to_string(),
    })
}

pub fn unlock(timeout_secs: Option<u64>) -> Result<AgentResponse, LitError> {
    let key = repo_key()?;
    let root = crate::core::find_repo_root()
        .map_err(|e| LitError::Repository(format!("Not inside a repository: {e}")))?;
    let config = EncryptionConfig::load(&root).map_err(LitError::Config)?;

    let passphrase = prompt_for_passphrase(&key, &config, "Passphrase: ")
        .map_err(|e| LitError::Encryption(format!("Could not read passphrase: {e}")))?;

    agent::put(&key, &passphrase).map_err(LitError::Encryption)?;

    let (entries, idle) = agent::status().map_err(LitError::Encryption)?;
    Ok(AgentResponse {
        action: "unlock".to_string(),
        entries: Some(entries),
        idle_timeout_secs: Some(timeout_secs.unwrap_or(idle)),
        message: format!("Passphrase held for {key}"),
    })
}

pub fn lock(all: bool) -> Result<AgentResponse, LitError> {
    let target = if all { None } else { Some(repo_key()?) };
    agent::drop_entry(target.as_deref()).map_err(LitError::Encryption)?;

    let (entries, idle) = agent::status().map_err(LitError::Encryption)?;
    Ok(AgentResponse {
        action: "lock".to_string(),
        entries: Some(entries),
        idle_timeout_secs: Some(idle),
        message: if all {
            "Forgot every held passphrase".to_string()
        } else {
            "Forgot this repository's passphrase".to_string()
        },
    })
}

pub fn status() -> Result<AgentResponse, LitError> {
    match agent::status() {
        Ok((entries, idle_timeout_secs)) => Ok(AgentResponse {
            action: "status".to_string(),
            entries: Some(entries),
            idle_timeout_secs: Some(idle_timeout_secs),
            message: format!(
                "Agent running, holding {entries} passphrase(s), idle timeout {idle_timeout_secs}s"
            ),
        }),
        // Not running is an ordinary answer to "is it running", not a failure.
        Err(_) => Ok(AgentResponse {
            action: "status".to_string(),
            entries: None,
            idle_timeout_secs: None,
            message: "No agent is running".to_string(),
        }),
    }
}

pub fn stop() -> Result<AgentResponse, LitError> {
    agent::shutdown().map_err(LitError::Encryption)?;
    Ok(AgentResponse {
        action: "stop".to_string(),
        entries: Some(0),
        idle_timeout_secs: None,
        message: "Agent stopped".to_string(),
    })
}
