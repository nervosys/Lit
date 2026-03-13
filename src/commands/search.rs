use crate::core::{find_repo_root, read_head, Object, ObjectHash};
use crate::response::{SearchMatch, SearchResponse};
use crate::storage::ObjectStore;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn execute(
    query: String,
    messages: bool,
    metadata_filter: Option<String>,
    max_results: usize,
) -> Result<SearchResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    if messages {
        return search_commit_messages(&repo_root, &store, &query, max_results);
    }

    if let Some(filter) = metadata_filter {
        return search_metadata(&repo_root, &store, &filter, max_results);
    }

    // Default: search file contents in working tree
    search_file_contents(&repo_root, &query, max_results)
}

fn search_file_contents(
    repo_root: &Path,
    query: &str,
    max_results: usize,
) -> Result<SearchResponse, String> {
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

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

        // Skip binary files by checking for null bytes in first 512 bytes
        let content = match fs::read(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.iter().take(512).any(|&b| b == 0) {
            continue;
        }
        let text = match String::from_utf8(content) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let rel_path = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        for (line_num, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                matches.push(SearchMatch {
                    file: rel_path.clone(),
                    line_number: line_num + 1,
                    content: line.to_string(),
                    commit: None,
                    match_type: "content".to_string(),
                });
                if matches.len() >= max_results {
                    break;
                }
            }
        }
        if matches.len() >= max_results {
            break;
        }
    }

    let total = matches.len();
    Ok(SearchResponse {
        query: query.to_string(),
        match_type: "content".to_string(),
        matches,
        total,
    })
}

fn search_commit_messages(
    repo_root: &Path,
    store: &ObjectStore,
    query: &str,
    max_results: usize,
) -> Result<SearchResponse, String> {
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    let head = match read_head(repo_root) {
        Ok(h) => h,
        Err(_) => {
            return Ok(SearchResponse {
                query: query.to_string(),
                match_type: "message".to_string(),
                matches: Vec::new(),
                total: 0,
            })
        }
    };

    let mut current = Some(head);
    while let Some(hash) = current {
        if matches.len() >= max_results {
            break;
        }
        let hash_obj = ObjectHash::from_hex(hash.clone());
        match store.read(&hash_obj) {
            Ok(Object::Commit(commit)) => {
                if commit.message.to_lowercase().contains(&query_lower) {
                    for (i, line) in commit.message.lines().enumerate() {
                        if line.to_lowercase().contains(&query_lower) {
                            matches.push(SearchMatch {
                                file: String::new(),
                                line_number: i + 1,
                                content: line.to_string(),
                                commit: Some(hash.clone()),
                                match_type: "message".to_string(),
                            });
                        }
                    }
                }
                current = commit.parents.first().map(|p| p.to_string());
            }
            _ => break,
        }
    }

    let total = matches.len();
    Ok(SearchResponse {
        query: query.to_string(),
        match_type: "message".to_string(),
        matches,
        total,
    })
}

fn search_metadata(
    repo_root: &Path,
    store: &ObjectStore,
    filter: &str,
    max_results: usize,
) -> Result<SearchResponse, String> {
    // Parse key=value filter
    let (key, value) = filter
        .split_once('=')
        .ok_or("Metadata filter must be in key=value format")?;

    let mut matches = Vec::new();

    let head = match read_head(repo_root) {
        Ok(h) => h,
        Err(_) => {
            return Ok(SearchResponse {
                query: filter.to_string(),
                match_type: "metadata".to_string(),
                matches: Vec::new(),
                total: 0,
            })
        }
    };

    let mut current = Some(head);
    while let Some(hash) = current {
        if matches.len() >= max_results {
            break;
        }
        let hash_obj = ObjectHash::from_hex(hash.clone());
        match store.read(&hash_obj) {
            Ok(Object::Commit(commit)) => {
                if let Some(meta) = &commit.metadata {
                    if let Some(meta_value) = meta.get(key) {
                        let meta_str = match meta_value {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        if meta_str.to_lowercase() == value.to_lowercase() {
                            matches.push(SearchMatch {
                                file: String::new(),
                                line_number: 0,
                                content: format!("{}: {}", commit.message, meta),
                                commit: Some(hash.clone()),
                                match_type: "metadata".to_string(),
                            });
                        }
                    }
                }
                current = commit.parents.first().map(|p| p.to_string());
            }
            _ => break,
        }
    }

    let total = matches.len();
    Ok(SearchResponse {
        query: filter.to_string(),
        match_type: "metadata".to_string(),
        matches,
        total,
    })
}
