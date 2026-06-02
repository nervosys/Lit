use crate::core::find_repo_root;
use crate::response::{BatchOperationResult, BatchResponse};
use serde::Deserialize;
use std::io::{self, BufRead};

/// Maximum size of a single JSONL request line (1 MiB). A larger line is
/// rejected without being fully buffered, so a malformed or hostile stream
/// (e.g. a very long line with no newline) cannot exhaust memory.
const MAX_LINE_BYTES: usize = 1_048_576;

/// Read a single newline-terminated record from `reader`, capping buffering at
/// `max_bytes`. Returns `Ok(None)` at end of input, otherwise the line bytes
/// (without the trailing newline) and a flag indicating the line exceeded
/// `max_bytes` and was truncated (the remainder is drained from the stream).
fn read_capped_line<R: BufRead>(
    reader: &mut R,
    max_bytes: usize,
) -> io::Result<Option<(Vec<u8>, bool)>> {
    let mut buf = Vec::new();
    let mut oversized = false;
    let mut saw_any = false;
    loop {
        let available = match reader.fill_buf() {
            Ok(b) => b,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            if !saw_any {
                return Ok(None);
            }
            return Ok(Some((buf, oversized)));
        }
        saw_any = true;
        let newline = available.iter().position(|&b| b == b'\n');
        let chunk_len = newline.map_or(available.len(), |idx| idx);
        let room = max_bytes.saturating_sub(buf.len());
        let take = room.min(chunk_len);
        buf.extend_from_slice(&available[..take]);
        if chunk_len > room {
            oversized = true;
        }
        match newline {
            Some(idx) => {
                reader.consume(idx + 1);
                return Ok(Some((buf, oversized)));
            }
            None => {
                let consumed = available.len();
                reader.consume(consumed);
            }
        }
    }
}

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
    let mut reader = stdin.lock();
    let mut operations: Vec<BatchOperation> = Vec::new();
    while let Some((raw, oversized)) = read_capped_line(&mut reader, MAX_LINE_BYTES)
        .map_err(|e| crate::errors::LitError::io(e.to_string()))?
    {
        // Skip oversized lines: the reader never buffered more than
        // MAX_LINE_BYTES, so an unbounded line cannot exhaust memory.
        if oversized {
            continue;
        }
        let line = String::from_utf8_lossy(&raw);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(op) = serde_json::from_str(trimmed) {
            operations.push(op);
        }
    }

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
