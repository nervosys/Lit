/// File-based transport for local and file:// repositories
///
/// Handles object and ref transfer between local repositories.
/// Supports direct paths and file:// URLs. For HTTPS, SSH, and lit://
/// URLs, delegates to the corresponding transport module.
use crate::core::{Object, ObjectHash};
use crate::storage::ObjectStore;
use std::cell::RefCell;
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
/// transports. For network transports (HTTPS, SSH, lit://), use
/// `RemoteRepo::open()` instead — this function only resolves local paths.
pub fn resolve_url(url: &str) -> Result<PathBuf, String> {
    match detect_transport(url) {
        TransportKind::Local => resolve_path(url),
        TransportKind::Https => Err(format!(
            "HTTPS URLs cannot be resolved to a local path. \
             Use RemoteRepo::open() for '{}' instead.",
            url
        )),
        TransportKind::Ssh => Err(format!(
            "SSH URLs cannot be resolved to a local path. \
             Use RemoteRepo::open() for '{}' instead.",
            url
        )),
        TransportKind::Lit => Err(format!(
            "lit:// URLs cannot be resolved to a local path. \
             Use RemoteRepo::open() for '{}' instead.",
            url
        )),
    }
}

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

// ── RemoteRepo — unified abstraction for file and HTTP remotes ──

/// A reference on a remote (name + hash)
#[derive(Debug, Clone)]
pub struct RemoteRef {
    pub kind: String,
    pub name: String,
    pub hash: String,
}

/// A remote repository that can be accessed via filesystem, HTTP, SSH, or lit://
pub enum RemoteRepo {
    File {
        path: PathBuf,
    },
    Http {
        base_url: String,
        token: Option<String>,
    },
    Ssh {
        pipe: RefCell<crate::network::ssh::SshPipe>,
    },
    Lit {
        conn: RefCell<crate::network::lit_protocol::LitConnection>,
    },
}

impl RemoteRepo {
    /// Create a RemoteRepo from a URL, auto-detecting transport type
    pub fn open(url: &str) -> Result<Self, String> {
        match detect_transport(url) {
            TransportKind::Local => {
                let path = resolve_path(url)?;
                Ok(RemoteRepo::File { path })
            }
            TransportKind::Https => {
                // Strip trailing slash
                let base = url.trim_end_matches('/').to_string();
                // Check for token in LIT_TOKEN env var
                let token = std::env::var("LIT_TOKEN").ok();
                Ok(RemoteRepo::Http {
                    base_url: base,
                    token,
                })
            }
            TransportKind::Ssh => {
                let parsed = crate::network::ssh::parse_ssh_url(url)?;
                let pipe = crate::network::ssh::SshPipe::open(&parsed)?;
                Ok(RemoteRepo::Ssh {
                    pipe: RefCell::new(pipe),
                })
            }
            TransportKind::Lit => {
                let parsed = crate::network::lit_protocol::parse_lit_url(url)?;
                let conn = crate::network::lit_protocol::LitConnection::open(&parsed)?;
                Ok(RemoteRepo::Lit {
                    conn: RefCell::new(conn),
                })
            }
        }
    }

    /// List refs (branches and/or tags) on the remote
    pub fn list_refs(&self, kind: &str) -> Result<Vec<RemoteRef>, String> {
        match self {
            RemoteRepo::File { path } => {
                let mut result = Vec::new();
                if kind == "all" || kind == "heads" {
                    if let Ok(refs) = crate::core::refs::list_refs(path, "heads") {
                        for r in refs {
                            result.push(RemoteRef {
                                kind: "heads".into(),
                                name: r.name,
                                hash: r.hash,
                            });
                        }
                    }
                }
                if kind == "all" || kind == "tags" {
                    if let Ok(refs) = crate::core::refs::list_refs(path, "tags") {
                        for r in refs {
                            result.push(RemoteRef {
                                kind: "tags".into(),
                                name: r.name,
                                hash: r.hash,
                            });
                        }
                    }
                }
                Ok(result)
            }
            RemoteRepo::Http { base_url, token } => {
                crate::network::https::list_refs_http(base_url, kind, token.as_deref())
            }
            RemoteRepo::Ssh { pipe } => {
                crate::network::ssh::list_refs_ssh(&mut pipe.borrow_mut(), kind)
            }
            RemoteRepo::Lit { conn } => {
                crate::network::lit_protocol::list_refs_lit(&mut conn.borrow_mut(), kind)
            }
        }
    }

    /// List branches on the remote (convenience method)
    pub fn list_branches(&self) -> Result<Vec<(String, String)>, String> {
        let refs = self.list_refs("heads")?;
        Ok(refs.into_iter().map(|r| (r.name, r.hash)).collect())
    }

    /// Read a branch ref on the remote
    pub fn read_branch_ref(&self, branch: &str) -> Result<String, String> {
        match self {
            RemoteRepo::File { path } => read_remote_ref(path, branch),
            RemoteRepo::Http { base_url, token } => {
                crate::network::https::read_ref_http(base_url, branch, token.as_deref())
            }
            RemoteRepo::Ssh { pipe } => {
                crate::network::ssh::read_ref_ssh(&mut pipe.borrow_mut(), branch)
            }
            RemoteRepo::Lit { conn } => {
                crate::network::lit_protocol::read_ref_lit(&mut conn.borrow_mut(), branch)
            }
        }
    }

    /// Read HEAD from the remote
    pub fn read_head(&self) -> Result<String, String> {
        match self {
            RemoteRepo::File { path } => read_remote_head(path),
            RemoteRepo::Http { base_url, token } => {
                crate::network::https::read_head_http(base_url, token.as_deref())
            }
            RemoteRepo::Ssh { pipe } => crate::network::ssh::read_head_ssh(&mut pipe.borrow_mut()),
            RemoteRepo::Lit { conn } => {
                crate::network::lit_protocol::read_head_lit(&mut conn.borrow_mut())
            }
        }
    }

    /// Update a branch ref on the remote
    pub fn update_branch_ref(&self, branch: &str, hash: &str, force: bool) -> Result<(), String> {
        match self {
            RemoteRepo::File { path } => update_remote_branch_ref(path, branch, hash),
            RemoteRepo::Http { base_url, token } => crate::network::https::update_ref_http(
                base_url,
                branch,
                hash,
                force,
                token.as_deref(),
            ),
            RemoteRepo::Ssh { pipe } => {
                crate::network::ssh::update_ref_ssh(&mut pipe.borrow_mut(), branch, hash, force)
            }
            RemoteRepo::Lit { conn } => crate::network::lit_protocol::update_ref_lit(
                &mut conn.borrow_mut(),
                branch,
                hash,
                force,
            ),
        }
    }

    /// Negotiate which objects need to be transferred (returns hashes of needed objects)
    pub fn negotiate_download(
        &self,
        local_store: &ObjectStore,
        wants: &[String],
    ) -> Result<Vec<ObjectHash>, String> {
        match self {
            RemoteRepo::File { path } => {
                let remote_store = ObjectStore::new(path);
                let known = collect_known_hashes_from_store(local_store);
                let mut all = Vec::new();
                for want in wants {
                    let hash = ObjectHash::from_hex(want.clone());
                    let needed = walk_commit_graph(&remote_store, &hash, &known)?;
                    for h in needed {
                        if !all.iter().any(|x: &ObjectHash| x.as_str() == h.as_str()) {
                            all.push(h);
                        }
                    }
                }
                Ok(all)
            }
            RemoteRepo::Http { base_url, token } => {
                let known = collect_known_hashes_from_store(local_store);
                let haves: Vec<String> = known.into_iter().collect();
                crate::network::https::negotiate_http(base_url, wants, &haves, token.as_deref())
            }
            RemoteRepo::Ssh { pipe } => {
                let known = collect_known_hashes_from_store(local_store);
                let haves: Vec<String> = known.into_iter().collect();
                crate::network::ssh::negotiate_ssh(&mut pipe.borrow_mut(), wants, &haves)
            }
            RemoteRepo::Lit { conn } => {
                let known = collect_known_hashes_from_store(local_store);
                let haves: Vec<String> = known.into_iter().collect();
                crate::network::lit_protocol::negotiate_lit(&mut conn.borrow_mut(), wants, &haves)
            }
        }
    }

    /// Download objects from remote into local store
    pub fn download_objects(
        &self,
        local_store: &ObjectStore,
        hashes: &[ObjectHash],
    ) -> Result<usize, String> {
        match self {
            RemoteRepo::File { path } => {
                let remote_store = ObjectStore::new(path);
                transfer_objects(&remote_store, local_store, hashes)
            }
            RemoteRepo::Http { base_url, token } => crate::network::https::download_objects_http(
                base_url,
                local_store,
                hashes,
                token.as_deref(),
            ),
            RemoteRepo::Ssh { pipe } => crate::network::ssh::download_objects_ssh(
                &mut pipe.borrow_mut(),
                local_store,
                hashes,
            ),
            RemoteRepo::Lit { conn } => crate::network::lit_protocol::download_objects_lit(
                &mut conn.borrow_mut(),
                local_store,
                hashes,
            ),
        }
    }

    /// Upload objects from local store to remote
    pub fn upload_objects(
        &self,
        local_store: &ObjectStore,
        hashes: &[ObjectHash],
    ) -> Result<usize, String> {
        match self {
            RemoteRepo::File { path } => {
                let remote_store = ObjectStore::new(path);
                transfer_objects(local_store, &remote_store, hashes)
            }
            RemoteRepo::Http { base_url, token } => crate::network::https::upload_objects_http(
                base_url,
                local_store,
                hashes,
                token.as_deref(),
            ),
            RemoteRepo::Ssh { pipe } => {
                crate::network::ssh::upload_objects_ssh(&mut pipe.borrow_mut(), local_store, hashes)
            }
            RemoteRepo::Lit { conn } => crate::network::lit_protocol::upload_objects_lit(
                &mut conn.borrow_mut(),
                local_store,
                hashes,
            ),
        }
    }

    /// Negotiate which objects need to be uploaded (walk local graph, exclude known remote objects)
    pub fn negotiate_upload(
        &self,
        local_store: &ObjectStore,
        wants: &[String],
        remote_has: &HashSet<String>,
    ) -> Result<Vec<ObjectHash>, String> {
        let mut all = Vec::new();
        for want in wants {
            let hash = ObjectHash::from_hex(want.clone());
            let needed = walk_commit_graph(local_store, &hash, remote_has)?;
            for h in needed {
                if !all.iter().any(|x: &ObjectHash| x.as_str() == h.as_str()) {
                    all.push(h);
                }
            }
        }
        Ok(all)
    }

    /// Check if a push would be fast-forward
    pub fn check_fast_forward(
        &self,
        local_store: &ObjectStore,
        new_hash: &ObjectHash,
        old_hash: &ObjectHash,
    ) -> Result<bool, String> {
        // For both file and HTTP, we check locally since we should have
        // downloaded the remote's objects first (or they share an ancestor)
        is_fast_forward_push(local_store, new_hash, old_hash)
    }
}

/// Collect known hashes from a local ObjectStore (for negotiation)
fn collect_known_hashes_from_store(store: &ObjectStore) -> HashSet<String> {
    store
        .list()
        .unwrap_or_default()
        .into_iter()
        .map(|h| h.as_str().to_string())
        .collect()
}
