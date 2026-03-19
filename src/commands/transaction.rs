use crate::core::find_repo_root;
use crate::response::TransactionResponse;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransactionState {
    tx_id: String,
    started_at: i64,
    /// Write-ahead log entries: (operation_type, path, original_content_base64)
    wal: Vec<WalEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalEntry {
    op: String,
    path: String,
    /// Base64-encoded original content for rollback (None for creates)
    original: Option<String>,
}

fn tx_state_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("transaction.json")
}

fn lock_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("transaction.lock")
}

pub fn execute_begin() -> Result<TransactionResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let lock = lock_path(&repo_root);

    if lock.exists() {
        return Err(
            "Another transaction is in progress. Use 'lit tx rollback' to abort it.".into(),
        );
    }

    let tx_id = uuid::Uuid::new_v4().to_string();

    let state = TransactionState {
        tx_id: tx_id.clone(),
        started_at: chrono::Utc::now().timestamp(),
        wal: Vec::new(),
    };

    let data = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize transaction state: {}", e))?;
    fs::write(tx_state_path(&repo_root), &data)
        .map_err(|e| format!("Failed to write transaction state: {}", e))?;
    fs::write(&lock, &tx_id).map_err(|e| format!("Failed to create transaction lock: {}", e))?;

    Ok(TransactionResponse {
        action: "begin".to_string(),
        tx_id: Some(tx_id),
        message: "Transaction started".to_string(),
    })
}

pub fn execute_commit_tx() -> Result<TransactionResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let lock = lock_path(&repo_root);
    let state_path = tx_state_path(&repo_root);

    if !lock.exists() {
        return Err("No transaction in progress".into());
    }

    let data = fs::read_to_string(&state_path)
        .map_err(|e| format!("Failed to read transaction state: {}", e))?;
    let state: TransactionState =
        serde_json::from_str(&data).map_err(|e| format!("Corrupt transaction state: {}", e))?;

    // Commit = just remove the WAL and lock (changes are already applied)
    let _ = fs::remove_file(&state_path);
    let _ = fs::remove_file(&lock);

    Ok(TransactionResponse {
        action: "commit".to_string(),
        tx_id: Some(state.tx_id),
        message: "Transaction committed".to_string(),
    })
}

pub fn execute_rollback() -> Result<TransactionResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let lock = lock_path(&repo_root);
    let state_path = tx_state_path(&repo_root);

    if !lock.exists() {
        return Err("No transaction in progress".into());
    }

    let data = fs::read_to_string(&state_path)
        .map_err(|e| format!("Failed to read transaction state: {}", e))?;
    let state: TransactionState =
        serde_json::from_str(&data).map_err(|e| format!("Corrupt transaction state: {}", e))?;

    // Replay WAL in reverse to undo changes
    let mut rollback_errors = Vec::new();
    for entry in state.wal.iter().rev() {
        match entry.op.as_str() {
            "write" => {
                // Restore original content
                if let Some(original_b64) = &entry.original {
                    match base64_decode(original_b64) {
                        Ok(bytes) => {
                            if let Err(e) = fs::write(&entry.path, &bytes) {
                                rollback_errors
                                    .push(format!("Failed to restore {}: {}", entry.path, e));
                            }
                        }
                        Err(e) => {
                            rollback_errors
                                .push(format!("Failed to decode WAL for {}: {}", entry.path, e));
                        }
                    }
                }
            }
            "create" => {
                // Remove created file
                let _ = fs::remove_file(&entry.path);
            }
            "delete" => {
                // Restore deleted file
                if let Some(original_b64) = &entry.original {
                    if let Ok(bytes) = base64_decode(original_b64) {
                        let _ = fs::write(&entry.path, &bytes);
                    }
                }
            }
            _ => {}
        }
    }

    let _ = fs::remove_file(&state_path);
    let _ = fs::remove_file(&lock);

    let message = if rollback_errors.is_empty() {
        "Transaction rolled back".to_string()
    } else {
        format!(
            "Transaction rolled back with {} error(s)",
            rollback_errors.len()
        )
    };

    Ok(TransactionResponse {
        action: "rollback".to_string(),
        tx_id: Some(state.tx_id),
        message,
    })
}

/// Record a WAL entry for the current transaction (called from other commands)
pub fn record_wal(repo_root: &std::path::Path, op: &str, path: &str) -> Result<(), crate::errors::LitError> {
    let state_path = tx_state_path(repo_root);
    if !state_path.exists() {
        return Ok(()); // No active transaction
    }

    let data = fs::read_to_string(&state_path)
        .map_err(|e| format!("Failed to read transaction state: {}", e))?;
    let mut state: TransactionState =
        serde_json::from_str(&data).map_err(|e| format!("Corrupt transaction state: {}", e))?;

    let original = if op == "write" || op == "delete" {
        fs::read(path).ok().map(|bytes| base64_encode(&bytes))
    } else {
        None
    };

    state.wal.push(WalEntry {
        op: op.to_string(),
        path: path.to_string(),
        original,
    });

    let data = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize transaction state: {}", e))?;
    fs::write(&state_path, &data)
        .map_err(|e| format!("Failed to write transaction state: {}", e))?;

    Ok(())
}

fn base64_encode(data: &[u8]) -> String {
    // Simple base64 without pulling in another crate
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(n & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(s: &str) -> Result<Vec<u8>, crate::errors::LitError> {
    let mut result = Vec::new();
    let chars: Vec<u8> = s.bytes().filter(|b| *b != b'\n' && *b != b'\r').collect();
    for chunk in chars.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let vals: Vec<u32> = chunk
            .iter()
            .map(|&c| {
                match c {
                    b'A'..=b'Z' => (c - b'A') as u32,
                    b'a'..=b'z' => (c - b'a' + 26) as u32,
                    b'0'..=b'9' => (c - b'0' + 52) as u32,
                    b'+' => 62,
                    b'/' => 63,
                    _ => 0, // padding
                }
            })
            .collect();
        let n = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        result.push(((n >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            result.push(((n >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            result.push((n & 0xFF) as u8);
        }
    }
    Ok(result)
}
