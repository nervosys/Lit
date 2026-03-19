use crate::core::{find_repo_root, list_refs, Object, ObjectHash};
use crate::response::{VerifyResponse, VerifyResult};
use crate::storage::ObjectStore;

pub fn execute() -> Result<VerifyResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let mut checks = Vec::new();
    let mut objects_checked = 0usize;
    let mut refs_checked = 0usize;
    let mut all_valid = true;

    // 1. Verify all objects
    match verify_objects(&repo_root, &store) {
        Ok((count, results)) => {
            objects_checked = count;
            for r in results {
                if r.status != "ok" {
                    all_valid = false;
                }
                checks.push(r);
            }
        }
        Err(e) => {
            all_valid = false;
            checks.push(VerifyResult {
                check: "object_store".to_string(),
                status: "error".to_string(),
                details: Some(e),
            });
        }
    }

    // 2. Verify refs
    match verify_refs(&repo_root, &store) {
        Ok((count, results)) => {
            refs_checked = count;
            for r in results {
                if r.status != "ok" {
                    all_valid = false;
                }
                checks.push(r);
            }
        }
        Err(e) => {
            all_valid = false;
            checks.push(VerifyResult {
                check: "refs".to_string(),
                status: "error".to_string(),
                details: Some(e),
            });
        }
    }

    // 3. Verify DAG connectivity
    match verify_dag(&repo_root, &store) {
        Ok(result) => {
            if result.status != "ok" {
                all_valid = false;
            }
            checks.push(result);
        }
        Err(e) => {
            all_valid = false;
            checks.push(VerifyResult {
                check: "dag_connectivity".to_string(),
                status: "error".to_string(),
                details: Some(e.internal_message().to_string()),
            });
        }
    }

    // 4. Verify index consistency
    match verify_index(&repo_root, &store) {
        Ok(result) => {
            if result.status != "ok" {
                all_valid = false;
            }
            checks.push(result);
        }
        Err(e) => {
            all_valid = false;
            checks.push(VerifyResult {
                check: "index".to_string(),
                status: "error".to_string(),
                details: Some(e.internal_message().to_string()),
            });
        }
    }

    let message = if all_valid {
        "Repository is valid".to_string()
    } else {
        "Repository has errors".to_string()
    };

    Ok(VerifyResponse {
        valid: all_valid,
        checks,
        objects_checked,
        refs_checked,
        message,
    })
}

fn verify_objects(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<(usize, Vec<VerifyResult>), String> {
    let objects_dir = repo_root.join(".lit").join("objects");
    let mut results = Vec::new();
    let mut count = 0usize;
    let mut corrupt = 0usize;

    if !objects_dir.exists() {
        return Ok((0, vec![VerifyResult {
            check: "object_store".to_string(),
            status: "ok".to_string(),
            details: Some("No objects directory (empty repository)".to_string()),
        }]));
    }

    // Walk the objects directory
    for dir_entry in std::fs::read_dir(&objects_dir)
        .map_err(|e| format!("Failed to read objects dir: {}", e))?
    {
        let dir_entry = dir_entry.map_err(|e| format!("Dir entry error: {}", e))?;
        if !dir_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let prefix = dir_entry.file_name().to_string_lossy().to_string();
        let subdir = objects_dir.join(&prefix);

        for file_entry in std::fs::read_dir(&subdir)
            .map_err(|e| format!("Failed to read subdir {}: {}", prefix, e))?
        {
            let file_entry = file_entry.map_err(|e| format!("File entry error: {}", e))?;
            let filename = file_entry.file_name().to_string_lossy().to_string();
            let hash_str = format!("{}{}", prefix, filename);

            let hash = ObjectHash::from_hex(hash_str.clone());
            match store.read(&hash) {
                Ok(_) => count += 1,
                Err(e) => {
                    corrupt += 1;
                    count += 1;
                    results.push(VerifyResult {
                        check: format!("object:{}", &hash_str[..16.min(hash_str.len())]),
                        status: "error".to_string(),
                        details: Some(format!("Corrupt object: {}", e)),
                    });
                }
            }
        }
    }

    if corrupt == 0 {
        results.push(VerifyResult {
            check: "object_hashes".to_string(),
            status: "ok".to_string(),
            details: Some(format!("All {} objects verified", count)),
        });
    }

    Ok((count, results))
}

fn verify_refs(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<(usize, Vec<VerifyResult>), String> {
    let mut results = Vec::new();
    let mut count = 0usize;
    let mut dangling = 0usize;

    // Check heads
    let heads = list_refs(repo_root, "heads")
        .unwrap_or_default();
    for r in &heads { let (name, hash) = (&r.name, &r.hash);
        count += 1;
        let hash_obj = ObjectHash::from_hex(hash.clone());
        match store.read(&hash_obj) {
            Ok(Object::Commit(_)) => {}
            Ok(_) => {
                dangling += 1;
                results.push(VerifyResult {
                    check: format!("ref:heads/{}", name),
                    status: "error".to_string(),
                    details: Some("Points to non-commit object".to_string()),
                });
            }
            Err(_) => {
                dangling += 1;
                results.push(VerifyResult {
                    check: format!("ref:heads/{}", name),
                    status: "error".to_string(),
                    details: Some(format!("Dangling reference: {}", &hash[..16.min(hash.len())])),
                });
            }
        }
    }

    // Check tags
    let tags = list_refs(repo_root, "tags")
        .unwrap_or_default();
    for r in &tags { let (name, hash) = (&r.name, &r.hash);
        count += 1;
        let hash_obj = ObjectHash::from_hex(hash.clone());
        if store.read(&hash_obj).is_err() {
            dangling += 1;
            results.push(VerifyResult {
                check: format!("ref:tags/{}", name),
                status: "error".to_string(),
                details: Some(format!("Dangling reference: {}", &hash[..16.min(hash.len())])),
            });
        }
    }

    if dangling == 0 {
        results.push(VerifyResult {
            check: "refs".to_string(),
            status: "ok".to_string(),
            details: Some(format!("All {} refs valid", count)),
        });
    }

    Ok((count, results))
}

fn verify_dag(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<VerifyResult, crate::errors::LitError> {
    let heads = list_refs(repo_root, "heads")
        .unwrap_or_default();

    let mut visited = std::collections::HashSet::new();
    let mut missing_parents = Vec::new();

    for r in &heads { let hash = &r.hash;
        walk_commit_dag(store, hash, &mut visited, &mut missing_parents);
    }

    if missing_parents.is_empty() {
        Ok(VerifyResult {
            check: "dag_connectivity".to_string(),
            status: "ok".to_string(),
            details: Some(format!("DAG is connected ({} commits reachable)", visited.len())),
        })
    } else {
        Ok(VerifyResult {
            check: "dag_connectivity".to_string(),
            status: "error".to_string(),
            details: Some(format!("{} missing parent commit(s)", missing_parents.len())),
        })
    }
}

fn walk_commit_dag(
    store: &ObjectStore,
    hash: &str,
    visited: &mut std::collections::HashSet<String>,
    missing: &mut Vec<String>,
) {
    if visited.contains(hash) {
        return;
    }
    visited.insert(hash.to_string());

    let hash_obj = ObjectHash::from_hex(hash.to_string());
    if let Ok(Object::Commit(commit)) = store.read(&hash_obj) {
        for parent in &commit.parents {
            let parent_str = parent.to_string();
            if store.exists(parent) {
                walk_commit_dag(store, &parent_str, visited, missing);
            } else {
                missing.push(parent_str);
            }
        }
    }
}

fn verify_index(
    repo_root: &std::path::Path,
    store: &ObjectStore,
) -> Result<VerifyResult, crate::errors::LitError> {
    let index = crate::storage::Index::load(repo_root)?;
    let mut missing = 0usize;

    for entry in index.sorted_entries() {
        let hash = ObjectHash::from_hex(entry.hash.clone());
        if !store.exists(&hash) {
            missing += 1;
        }
    }

    if missing == 0 {
        Ok(VerifyResult {
            check: "index".to_string(),
            status: "ok".to_string(),
            details: Some(format!(
                "All {} index entries reference valid objects",
                index.entries.len()
            )),
        })
    } else {
        Ok(VerifyResult {
            check: "index".to_string(),
            status: "error".to_string(),
            details: Some(format!("{} index entries reference missing objects", missing)),
        })
    }
}
