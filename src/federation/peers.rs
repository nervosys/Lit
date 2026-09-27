//! Peer discovery and federation management
//!
//! Manages known peers, their DIDs, endpoints, and synchronization state.

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::fs;
use std::path::Path;

/// Information about a federated peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Peer's DID
    pub did: String,
    /// Human-readable alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Network endpoint (e.g., "https://peer.example.com:8443")
    pub endpoint: String,
    /// Peer's public key hex for verification
    pub public_key_hex: String,
    /// Content ID of the peer's latest known head
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_known_head: Option<String>,
    /// Last successful sync timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync: Option<String>,
    /// Whether the peer is currently reachable
    pub reachable: bool,
    /// When this peer was first added
    pub added: String,
}

/// Content identifier for a lit object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ContentId {
    /// Hash algorithm used
    pub algorithm: String,
    /// Hex-encoded hash
    pub hash: String,
}

impl ContentId {
    /// Create a CID from raw bytes
    pub fn from_bytes(data: &[u8]) -> Self {
        let hash = Sha3_256::digest(data);
        ContentId {
            algorithm: "sha3-256".to_string(),
            hash: hex::encode(hash),
        }
    }

    /// Short display form
    pub fn short(&self) -> String {
        if self.hash.len() > 12 {
            format!("{}..{}", &self.hash[..6], &self.hash[self.hash.len() - 6..])
        } else {
            self.hash.clone()
        }
    }
}

impl std::fmt::Display for ContentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.hash)
    }
}

fn peers_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("federation").join("peers")
}

/// Add a new peer
pub fn add_peer(repo_root: &Path, peer: &PeerInfo) -> Result<(), LitError> {
    let dir = peers_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create peers dir: {}", e)))?;

    let safe_name: String = peer
        .did
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let path = dir.join(format!("{}.json", safe_name));

    let json = serde_json::to_string_pretty(peer)
        .map_err(|e| LitError::general(format!("Serialize error: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Write error: {}", e)))?;
    Ok(())
}

/// Remove a peer
pub fn remove_peer(repo_root: &Path, did: &str) -> Result<(), LitError> {
    let safe_name: String = did
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let path = peers_dir(repo_root).join(format!("{}.json", safe_name));
    if path.exists() {
        fs::remove_file(&path).map_err(|e| LitError::io(format!("Remove error: {}", e)))?;
        Ok(())
    } else {
        Err(LitError::general(format!("Peer not found: {}", did)))
    }
}

/// List all known peers
pub fn list_peers(repo_root: &Path) -> Result<Vec<PeerInfo>, LitError> {
    let dir = peers_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut peers = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(peer) = serde_json::from_str::<PeerInfo>(&json) {
                    peers.push(peer);
                }
            }
        }
    }
    Ok(peers)
}

/// Get a specific peer by DID
pub fn get_peer(repo_root: &Path, did: &str) -> Result<PeerInfo, LitError> {
    let safe_name: String = did
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let path = peers_dir(repo_root).join(format!("{}.json", safe_name));
    if !path.exists() {
        return Err(LitError::general(format!("Peer not found: {}", did)));
    }
    let json = fs::read_to_string(&path).map_err(|e| LitError::io(format!("IO: {}", e)))?;
    serde_json::from_str(&json).map_err(|e| LitError::general(format!("Parse error: {}", e)))
}

/// Update a peer's last sync info
pub fn update_peer_sync(repo_root: &Path, did: &str, head: &str) -> Result<(), LitError> {
    let mut peer = get_peer(repo_root, did)?;
    peer.last_known_head = Some(head.to_string());
    peer.last_sync = Some(chrono::Utc::now().to_rfc3339());
    peer.reachable = true;
    add_peer(repo_root, &peer)
}

/// Generate a want list — CIDs this repo wants from peers
pub fn generate_want_list(repo_root: &Path) -> Result<Vec<String>, LitError> {
    // Check for any refs that reference objects we don't have locally
    let refs_dir = repo_root.join(".lit").join("refs").join("remotes");
    if !refs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut wants = Vec::new();
    for entry in fs::read_dir(&refs_dir).map_err(|e| LitError::io(format!("IO: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO: {}", e)))?;
        if let Ok(contents) = fs::read_to_string(entry.path()) {
            let hash = contents.trim();

            // Validate before slicing. The emptiness check used to sit *after*
            // `&hash[..2]`, so it guarded nothing: an empty or one-character
            // ref file panicked on the slice, and a multi-byte first character
            // panicked on a char boundary. This function is reached from the
            // federation CLI, where a truncated or half-written remote ref is
            // an ordinary state rather than an exotic one, and a panic there
            // takes the whole process down — `unwrap_or_default` at the call
            // site catches an `Err`, not an unwind.
            //
            // A hash needs two characters for the fan-out directory and at
            // least one for the file name, and anything that is not hex cannot
            // name an object we store.
            if hash.len() < 3 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                continue;
            }

            let obj_path = repo_root
                .join(".lit")
                .join("objects")
                .join(&hash[..2])
                .join(&hash[2..]);
            if !obj_path.exists() {
                wants.push(hash.to_string());
            }
        }
    }
    Ok(wants)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A scratch repository root that cleans itself up.
    ///
    /// This used to build a path from the process id and delete it at the end of
    /// each test. Process ids are reused, so a directory left by an earlier run
    /// could be adopted by a later one; and because the cleanup was the last
    /// statement, a test that failed never reached it and left the directory
    /// behind for exactly that to happen. `TempDir` is randomly named and drops
    /// during unwind, so neither does.
    fn tmp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    /// A remote ref file that is empty, truncated, or not a hash at all must not
    /// take the process down. `generate_want_list` builds the object path by
    /// slicing the file's contents, and it is reached from the federation CLI
    /// where a half-written ref file is not an exotic state.
    #[test]
    fn a_malformed_remote_ref_does_not_panic_the_want_list() {
        let dir = tmp_dir();
        let remotes = dir.path().join(".lit").join("refs").join("remotes");
        fs::create_dir_all(&remotes).unwrap();

        // Each of these would have panicked on `&hash[..2]`, which ran before
        // the emptiness check that was supposed to guard it.
        fs::write(remotes.join("empty"), "").unwrap();
        fs::write(remotes.join("one-char"), "a").unwrap();
        fs::write(
            remotes.join("whitespace"),
            "   
",
        )
        .unwrap();
        fs::write(remotes.join("two-char"), "ab").unwrap();
        // A multi-byte character would panic on a char boundary rather than a
        // length check.
        fs::write(remotes.join("multibyte"), "é1234").unwrap();
        // And one genuine-looking hash, so the function is still doing its job.
        fs::write(remotes.join("real"), "abc123def4567890").unwrap();

        let wants = generate_want_list(dir.path()).expect("must not error");

        assert_eq!(
            wants,
            vec!["abc123def4567890".to_string()],
            "only the well-formed hash should be wanted"
        );
    }

    #[test]
    fn test_add_and_list_peer() {
        let dir = tmp_dir();
        let peer = PeerInfo {
            did: "did:lit:peer1".to_string(),
            alias: Some("Alice".to_string()),
            endpoint: "https://alice.example.com:8443".to_string(),
            public_key_hex: "abcdef1234567890".to_string(),
            last_known_head: None,
            last_sync: None,
            reachable: false,
            added: chrono::Utc::now().to_rfc3339(),
        };
        add_peer(dir.path(), &peer).unwrap();

        let peers = list_peers(dir.path()).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].did, "did:lit:peer1");
    }

    #[test]
    fn test_content_id() {
        let cid = ContentId::from_bytes(b"hello world");
        assert_eq!(cid.algorithm, "sha3-256");
        assert!(!cid.hash.is_empty());
        assert!(cid.short().contains(".."));
    }
}
