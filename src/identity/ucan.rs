//! UCAN (User Controlled Authorization Networks) capability delegation
//!
//! Enables agents to delegate a subset of their permissions to other agents.
//! Example: "You can push to branch 'feature' for 1 hour."
//!
//! Based on UCAN spec: https://ucan.xyz

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::fs;
use std::path::Path;

/// A capability that can be delegated via UCAN
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capability {
    /// Resource the capability applies to (e.g., "repo:*", "branch:main", "file:src/*")
    pub resource: String,
    /// Allowed action (e.g., "push", "commit", "merge", "read", "admin")
    pub action: String,
    /// Optional constraints (e.g., max commits, read-only paths)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caveats: Option<serde_json::Value>,
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.resource, self.action)
    }
}

/// A UCAN token for capability delegation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UcanToken {
    /// Token version
    pub version: String,
    /// Issuer DID (the delegator)
    pub issuer: String,
    /// Audience DID (the delegatee)
    pub audience: String,
    /// Capabilities being delegated
    pub capabilities: Vec<Capability>,
    /// Expiration timestamp (Unix epoch seconds)
    pub expiration: i64,
    /// Not-before timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_before: Option<i64>,
    /// Nonce for uniqueness
    pub nonce: String,
    /// Proof chain — parent UCAN CIDs that authorize this delegation
    #[serde(default)]
    pub proof: Vec<String>,
    /// Signature of the token payload by the issuer
    pub signature: String,
}

impl UcanToken {
    /// Create a new UCAN token (unsigned)
    pub fn new(
        issuer: String,
        audience: String,
        capabilities: Vec<Capability>,
        duration_secs: i64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        let nonce = format!("{:016x}", now ^ std::process::id() as i64);

        UcanToken {
            version: "0.10.0".to_string(),
            issuer,
            audience,
            capabilities,
            expiration: now + duration_secs,
            not_before: Some(now),
            nonce,
            proof: Vec::new(),
            signature: String::new(),
        }
    }

    /// Sign the token with the issuer's private key material
    pub fn sign(&mut self, private_key_hex: &str) -> Result<(), LitError> {
        let payload = self.payload_bytes()?;
        let private_bytes = hex::decode(private_key_hex)
            .map_err(|e| LitError::general(format!("Invalid key: {}", e)))?;

        let mut hasher = Sha3_256::new();
        hasher.update(&private_bytes);
        hasher.update(&payload);
        let sig = hasher.finalize();
        self.signature = hex::encode(sig);
        Ok(())
    }

    /// Verify the token's signature
    pub fn verify(&self, issuer_public_key_hex: &str) -> Result<bool, LitError> {
        let payload = self.payload_bytes()?;
        let public_bytes = hex::decode(issuer_public_key_hex)
            .map_err(|e| LitError::general(format!("Invalid key: {}", e)))?;

        let mut hasher = Sha3_256::new();
        hasher.update(&public_bytes);
        hasher.update(&payload);
        let expected = hasher.finalize();

        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| LitError::general(format!("Invalid signature: {}", e)))?;

        Ok(subtle::ConstantTimeEq::ct_eq(sig_bytes.as_slice(), expected.as_slice()).into())
    }

    /// Check if the token is currently valid (not expired, not before)
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        if now > self.expiration {
            return false;
        }
        if let Some(nb) = self.not_before {
            if now < nb {
                return false;
            }
        }
        true
    }

    /// Check if this token grants a specific capability
    pub fn has_capability(&self, resource: &str, action: &str) -> bool {
        self.capabilities.iter().any(|cap| {
            let resource_match = cap.resource == "*"
                || cap.resource == resource
                || cap
                    .resource
                    .strip_suffix('*')
                    .map(|prefix| resource.starts_with(prefix))
                    .unwrap_or(false)
                || resource.starts_with(&cap.resource);
            resource_match && (cap.action == "*" || cap.action == action)
        })
    }

    /// Get the content-addressable ID (hash) of this token
    pub fn cid(&self) -> Result<String, LitError> {
        let bytes = self.payload_bytes()?;
        let hash = Sha3_256::digest(&bytes);
        Ok(hex::encode(hash))
    }

    /// Delegate a subset of capabilities to another agent (create a child UCAN)
    pub fn delegate(
        &self,
        new_audience: String,
        capabilities: Vec<Capability>,
        duration_secs: i64,
    ) -> Result<UcanToken, LitError> {
        // Verify all delegated capabilities are a subset of parent
        for cap in &capabilities {
            if !self.has_capability(&cap.resource, &cap.action) {
                return Err(LitError::general(format!(
                    "Cannot delegate capability '{}:{}' — not held by parent token",
                    cap.resource, cap.action
                )));
            }
        }

        let parent_cid = self.cid()?;
        let mut child = UcanToken::new(
            self.audience.clone(), // Delegatee becomes the new issuer
            new_audience,
            capabilities,
            duration_secs,
        );
        child.proof.push(parent_cid);

        // Inherit parent's proof chain
        for p in &self.proof {
            child.proof.push(p.clone());
        }

        Ok(child)
    }

    fn payload_bytes(&self) -> Result<Vec<u8>, LitError> {
        // Serialize everything except signature for hashing
        let payload = serde_json::json!({
            "version": self.version,
            "issuer": self.issuer,
            "audience": self.audience,
            "capabilities": self.capabilities,
            "expiration": self.expiration,
            "not_before": self.not_before,
            "nonce": self.nonce,
            "proof": self.proof,
        });
        serde_json::to_vec(&payload)
            .map_err(|e| LitError::general(format!("Failed to serialize UCAN: {}", e)))
    }
}

/// Store for managing UCAN tokens
pub fn ucan_dir(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("ucan")
}

/// Save a UCAN token
pub fn save_token(repo_root: &Path, token: &UcanToken) -> Result<String, LitError> {
    let dir = ucan_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create UCAN dir: {}", e)))?;

    let cid = token.cid()?;
    let path = dir.join(format!("{}.json", &cid[..16]));
    let json = serde_json::to_string_pretty(token)
        .map_err(|e| LitError::general(format!("Failed to serialize token: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Failed to write token: {}", e)))?;
    Ok(cid)
}

/// Load all UCAN tokens for a given audience
pub fn load_tokens_for(repo_root: &Path, audience_did: &str) -> Result<Vec<UcanToken>, LitError> {
    let dir = ucan_dir(repo_root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut tokens = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| LitError::io(format!("IO error: {}", e)))? {
        let entry = entry.map_err(|e| LitError::io(format!("IO error: {}", e)))?;
        if entry.path().extension().is_some_and(|e| e == "json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(token) = serde_json::from_str::<UcanToken>(&json) {
                    if token.audience == audience_did && token.is_valid() {
                        tokens.push(token);
                    }
                }
            }
        }
    }
    Ok(tokens)
}

/// Revoke a UCAN token by CID
pub fn revoke_token(repo_root: &Path, cid_prefix: &str) -> Result<(), LitError> {
    let dir = ucan_dir(repo_root);
    let path = dir.join(format!("{}.json", cid_prefix));
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| LitError::io(format!("Failed to revoke token: {}", e)))?;
        Ok(())
    } else {
        Err(LitError::general(format!(
            "Token not found: {}",
            cid_prefix
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ucan_creation() {
        let token = UcanToken::new(
            "did:lit:issuer123".to_string(),
            "did:lit:audience456".to_string(),
            vec![Capability {
                resource: "branch:main".to_string(),
                action: "push".to_string(),
                caveats: None,
            }],
            3600,
        );
        assert!(token.is_valid());
        assert!(token.has_capability("branch:main", "push"));
        assert!(!token.has_capability("branch:main", "delete"));
    }

    #[test]
    fn test_ucan_expiration() {
        let mut token = UcanToken::new(
            "did:lit:a".to_string(),
            "did:lit:b".to_string(),
            vec![],
            -1, // Already expired
        );
        token.not_before = None;
        assert!(!token.is_valid());
    }

    #[test]
    fn test_ucan_delegation() {
        let parent = UcanToken::new(
            "did:lit:root".to_string(),
            "did:lit:agent1".to_string(),
            vec![Capability {
                resource: "branch:*".to_string(),
                action: "*".to_string(),
                caveats: None,
            }],
            3600,
        );

        let child = parent
            .delegate(
                "did:lit:agent2".to_string(),
                vec![Capability {
                    resource: "branch:feature".to_string(),
                    action: "push".to_string(),
                    caveats: None,
                }],
                1800,
            )
            .unwrap();

        assert_eq!(child.issuer, "did:lit:agent1");
        assert_eq!(child.audience, "did:lit:agent2");
        assert!(!child.proof.is_empty());
    }
}
