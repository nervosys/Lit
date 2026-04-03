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
        if entry.path().extension().map_or(false, |e| e == "json") {
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
        if let Ok(hash) = fs::read_to_string(entry.path()) {
            let hash = hash.trim().to_string();
            // Check if we have this object
            let obj_path = repo_root
                .join(".lit")
                .join("objects")
                .join(&hash[..2])
                .join(&hash[2..]);
            if !obj_path.exists() && !hash.is_empty() {
                wants.push(hash);
            }
        }
    }
    Ok(wants)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lit_fed_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
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
        add_peer(&dir, &peer).unwrap();

        let peers = list_peers(&dir).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].did, "did:lit:peer1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_content_id() {
        let cid = ContentId::from_bytes(b"hello world");
        assert_eq!(cid.algorithm, "sha3-256");
        assert!(!cid.hash.is_empty());
        assert!(cid.short().contains(".."));
    }
}
