use crate::core::find_repo_root;
use crate::response::{BatchOperationResult, BatchResponse};
use serde::Deserialize;
use std::io::{self, BufRead};

/// A single operation in a batch JSONL stream
#[derive(Debug, Deserialize)]
struct BatchOperation {
    command: String,
    #[serde(default)]
    args: serde_json::Value,
}

pub fn execute(atomic: bool, dry_run: bool) -> Result<BatchResponse, crate::errors::LitError> {
    let _repo_root = find_repo_root()?;

    let stdin = io::stdin();
    let operations: Vec<BatchOperation> = stdin
        .lock()
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str(trimmed).ok()
        })
        .collect();

    if operations.is_empty() {
        return Err("No operations provided on stdin (expected JSONL)".into());
    }

    let total = operations.len();
    let mut results = Vec::with_capacity(total);
    let mut succeeded = 0usize;
    let mut failed = 0usize;

    for (i, op) in operations.iter().enumerate() {
        if dry_run {
            results.push(BatchOperationResult {
                index: i,
                command: op.command.clone(),
                status: "ok".to_string(),
                result: Some(serde_json::json!({"dry_run": true})),
                error: None,
            });
            succeeded += 1;
            continue;
        }

        match execute_single_operation(op) {
            Ok(value) => {
                results.push(BatchOperationResult {
                    index: i,
                    command: op.command.clone(),
                    status: "ok".to_string(),
                    result: Some(value),
                    error: None,
                });
                succeeded += 1;
            }
            Err(e) => {
                results.push(BatchOperationResult {
                    index: i,
                    command: op.command.clone(),
                    status: "error".to_string(),
                    result: None,
                    error: Some(e.internal_message().to_string()),
                });
                failed += 1;

                if atomic {
                    // In atomic mode, stop on first failure
                    // Mark remaining as skipped
                    for (j, op) in operations.iter().enumerate().skip(i + 1) {
                        results.push(BatchOperationResult {
                            index: j,
                            command: op.command.clone(),
                            status: "skipped".to_string(),
                            result: None,
                            error: Some("Skipped due to atomic rollback".to_string()),
                        });
                    }
                    break;
                }
            }
        }
    }

    Ok(BatchResponse {
        total,
        succeeded,
        failed,
        atomic,
        dry_run,
        results,
    })
}

fn execute_single_operation(
    op: &BatchOperation,
) -> Result<serde_json::Value, crate::errors::LitError> {
    match op.command.as_str() {
        "add" => {
            let files: Vec<String> = op
                .args
                .get("files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if files.is_empty() {
                return Err("'files' argument required for add".into());
            }
            let resp = crate::commands::add::execute(files)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "commit" => {
            let message = op
                .args
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("'message' argument required for commit")?
                .to_string();
            let author = op
                .args
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = crate::commands::commit::execute(message, author)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "status" => {
            let resp = crate::commands::status::execute()?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "branch" => {
            let name = op
                .args
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let delete = op
                .args
                .get("delete")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let all = op
                .args
                .get("all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = crate::commands::branch::execute(name, delete, all)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "checkout" => {
            let target = op
                .args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("'target' argument required for checkout")?
                .to_string();
            let b = op
                .args
                .get("create")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = crate::commands::checkout::execute(target, b)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "log" => {
            let count = op.args.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let oneline = op
                .args
                .get("oneline")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = crate::commands::log::execute(count, oneline)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "snapshot" => {
            let message = op
                .args
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("'message' argument required for snapshot")?
                .to_string();
            let author = op
                .args
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let metadata = op.args.get("metadata").cloned();
            let resp = crate::commands::snapshot::execute(message, author, metadata)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        _ => Err(format!("Unknown batch command: '{}'", op.command).into()),
    }
}
