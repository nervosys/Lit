/// File-based transport for local and file:// repositories
///
/// Handles object and ref transfer between local repositories.
/// Supports direct paths and file:// URLs. For HTTPS, SSH, and lit://
/// URLs, delegates to the corresponding transport module.
use crate::core::{Object, ObjectHash};
use crate::storage::ObjectStore;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Transport protocol type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Local,
    Https,
    Ssh,
    Lit,
}

/// Detect the transport protocol from a URL
pub fn detect_transport(url: &str) -> TransportKind {
    if crate::network::https::is_https_url(url) {
        TransportKind::Https
    } else if crate::network::ssh::is_ssh_url(url) {
        TransportKind::Ssh
    } else if crate::network::lit_protocol::is_lit_url(url) {
        TransportKind::Lit
    } else {
        TransportKind::Local
    }
}

/// Validate and resolve a remote URL, returning a local path for file-based
/// transports or an error with guidance for unimplemented transports.
pub fn resolve_url(url: &str) -> Result<PathBuf, String> {
    match detect_transport(url) {
        TransportKind::Local => resolve_path(url),
        TransportKind::Https => Err(format!(
            "HTTPS transport is not yet implemented. \
             Cannot connect to '{}'. Use a local path or file:// URL instead.",
            url
        )),
        TransportKind::Ssh => Err(format!(
            "SSH transport is not yet implemented. \
             Cannot connect to '{}'. Use a local path or file:// URL instead.",
            url
        )),
        TransportKind::Lit => Err(format!(
            "lit:// transport is not yet implemented. \
             Cannot connect to '{}'. Use a local path or file:// URL instead.",
            url
        )),
    }
}

/// Resolve a remote URL to a local path
/// Resolve a remote URL to a local path
pub fn resolve_path(url: &str) -> Result<PathBuf, String> {
    let path = if let Some(stripped) = url.strip_prefix("file://") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(url)
    };

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve remote path '{}': {}", url, e))?;

    if !canonical.join(".lit").exists() {
        return Err(format!(
            "'{}' does not appear to be a Lit repository",
            canonical.display()
        ));
    }

    Ok(canonical)
}

/// Find all objects reachable from a commit (commits, trees, blobs)
pub fn walk_commit_graph(
    store: &ObjectStore,
    start: &ObjectHash,
    known: &HashSet<String>,
) -> Result<Vec<ObjectHash>, String> {
    let mut to_visit = vec![start.clone()];
    let mut visited: HashSet<String> = known.clone();
    let mut result = Vec::new();

    while let Some(hash) = to_visit.pop() {
        if visited.contains(hash.as_str()) {
            continue;
        }
        visited.insert(hash.as_str().to_string());

        let obj = store.read(&hash)?;
        result.push(hash.clone());

        match &obj {
            Object::Commit(commit) => {
                to_visit.push(commit.tree.clone());
                for parent in &commit.parents {
                    to_visit.push(parent.clone());
                }
            }
            Object::Tree(tree) => {
                for entry in &tree.entries {
                    to_visit.push(entry.hash.clone());
                }
            }
            Object::Tag(tag) => {
                to_visit.push(tag.target.clone());
            }
            Object::Blob(_) => {}
        }
    }

    Ok(result)
}

/// Copy objects from source store to destination store, skipping those already present
pub fn transfer_objects(
    src_store: &ObjectStore,
    dst_store: &ObjectStore,
    objects: &[ObjectHash],
) -> Result<usize, String> {
    let mut count = 0;
    for hash in objects {
        if !dst_store.exists(hash) {
            let obj = src_store.read(hash)?;
            dst_store.write(&obj)?;
            count += 1;
        }
    }
    Ok(count)
}

/// List all ref hashes that a destination already has (for negotiation)
pub fn collect_known_hashes(repo_path: &Path) -> HashSet<String> {
    let mut known = HashSet::new();

    // Collect from heads
    if let Ok(refs) = crate::core::refs::list_refs(repo_path, "heads") {
        for r in refs {
            known.insert(r.hash);
        }
    }

    // Collect from remotes
    let remotes_dir = repo_path.join(".lit").join("refs").join("remotes");
    if remotes_dir.exists() {
        if let Ok(entries) = fs::read_dir(&remotes_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let remote_name = entry.file_name().to_string_lossy().to_string();
                    if let Ok(refs) =
                        crate::core::refs::list_refs(repo_path, &format!("remotes/{}", remote_name))
                    {
                        for r in refs {
                            known.insert(r.hash);
                        }
                    }
                }
            }
        }
    }

    known
}

/// Read a remote's ref (branch tip)
pub fn read_remote_ref(remote_path: &Path, branch: &str) -> Result<String, String> {
    crate::core::refs::read_ref(remote_path, &format!("heads/{}", branch))
}

/// List all branches on a remote
pub fn list_remote_branches(remote_path: &Path) -> Result<Vec<(String, String)>, String> {
    let refs = crate::core::refs::list_refs(remote_path, "heads")?;
    Ok(refs.into_iter().map(|r| (r.name, r.hash)).collect())
}

/// Update a remote-tracking ref
pub fn update_remote_tracking_ref(
    repo_path: &Path,
    remote_name: &str,
    branch: &str,
    hash: &str,
) -> Result<(), String> {
    crate::core::refs::write_ref(
        repo_path,
        &format!("remotes/{}/{}", remote_name, branch),
        hash,
    )
}

/// Update a ref on the remote repository
pub fn update_remote_branch_ref(
    remote_path: &Path,
    branch: &str,
    hash: &str,
) -> Result<(), String> {
    crate::core::refs::write_ref(remote_path, &format!("heads/{}", branch), hash)
}

/// Check if a push would be a fast-forward
pub fn is_fast_forward_push(
    store: &ObjectStore,
    new_hash: &ObjectHash,
    old_hash: &ObjectHash,
) -> Result<bool, String> {
    crate::core::merge::is_ancestor(store, old_hash, new_hash)
}

/// Read HEAD from a remote repo (for clone)
pub fn read_remote_head(remote_path: &Path) -> Result<String, String> {
    let head_path = remote_path.join(".lit").join("HEAD");
    if !head_path.exists() {
        return Err("Remote HEAD not found".to_string());
    }
    let content =
        fs::read_to_string(&head_path).map_err(|e| format!("Failed to read remote HEAD: {}", e))?;
    Ok(content.trim().to_string())
}
