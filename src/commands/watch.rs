use crate::core::find_repo_root;
use crate::response::{WatchEvent, WatchResponse};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use walkdir::WalkDir;

pub fn execute(debounce_ms: u64, filter: Option<String>) -> Result<WatchResponse, String> {
    let repo_root = find_repo_root()?;

    // Build initial file state snapshot
    let mut last_state = scan_files(&repo_root, filter.as_deref())?;

    let debounce = std::time::Duration::from_millis(debounce_ms);

    eprintln!(
        "Watching {} for changes (Ctrl+C to stop)...",
        repo_root.display()
    );

    // Emit a start event as JSON on stdout
    let start_event = WatchEvent {
        event_type: "start".to_string(),
        path: repo_root.to_string_lossy().to_string(),
        timestamp: chrono::Utc::now().timestamp(),
    };
    println!(
        "{}",
        serde_json::to_string(&start_event).unwrap_or_default()
    );

    loop {
        std::thread::sleep(debounce);

        let current_state = scan_files(&repo_root, filter.as_deref())?;
        let mut events = Vec::new();

        // Detect modifications and creations
        for (path, mtime) in &current_state {
            match last_state.get(path) {
                Some(old_mtime) if old_mtime != mtime => {
                    events.push(WatchEvent {
                        event_type: "modified".to_string(),
                        path: path.clone(),
                        timestamp: chrono::Utc::now().timestamp(),
                    });
                }
                None => {
                    events.push(WatchEvent {
                        event_type: "created".to_string(),
                        path: path.clone(),
                        timestamp: chrono::Utc::now().timestamp(),
                    });
                }
                _ => {}
            }
        }

        // Detect deletions
        for path in last_state.keys() {
            if !current_state.contains_key(path) {
                events.push(WatchEvent {
                    event_type: "deleted".to_string(),
                    path: path.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                });
            }
        }

        // Emit events as JSONL
        for event in &events {
            println!(
                "{}",
                serde_json::to_string(event).unwrap_or_default()
            );
        }

        last_state = current_state;
    }
}

/// Scan files and return map of relative_path -> last_modified_timestamp
fn scan_files(
    repo_root: &Path,
    filter: Option<&str>,
) -> Result<HashMap<String, u64>, String> {
    let mut files = HashMap::new();

    for entry in WalkDir::new(repo_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.') && name != "target" && name != "node_modules"
        })
    {
        let entry = entry.map_err(|e| format!("Walk error: {}", e))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.starts_with(repo_root.join(".lit")) {
            continue;
        }

        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        // Apply filter if present
        if let Some(f) = filter {
            if !rel_path.contains(f) {
                continue;
            }
        }

        let mtime = fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        files.insert(rel_path, mtime);
    }

    Ok(files)
}
