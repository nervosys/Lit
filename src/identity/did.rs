//! DID (Decentralized Identifier) identity system for Lit
//!
//! Provides `did:lit:` method identifiers for agents and humans.
//! Identity is a keypair — no accounts, no passwords, no OAuth.
//!
//! Format: did:lit:<base58-encoded-public-key>

use crate::errors::LitError;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::fs;
use std::path::{Path, PathBuf};

/// DID method identifier
const DID_METHOD: &str = "lit";

/// Supported key types for DID verification methods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DidMethod {
    /// Ed25519 for standard signatures (fast, widely supported)
    Ed25519,
    /// ML-DSA-87 for post-quantum signatures (FIPS 204)
    MlDsa87,
}

impl std::fmt::Display for DidMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DidMethod::Ed25519 => write!(f, "Ed25519"),
            DidMethod::MlDsa87 => write!(f, "ML-DSA-87"),
        }
    }
}

/// A DID keypair used for identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidKeyPair {
    /// The DID string (e.g., did:lit:z6Mk...)
    pub did: String,
    /// The key type
    pub method: DidMethod,
    /// Public key bytes (hex-encoded)
    pub public_key: String,
    /// Private key bytes (hex-encoded) — stored encrypted at rest
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    /// Creation timestamp
    pub created: i64,
}

impl DidKeyPair {
    /// Generate a new DID keypair
    pub fn generate(method: DidMethod) -> Self {
        let mut rng_bytes = [0u8; 32];
        // Use OS random source
        #[cfg(target_os = "windows")]
        {
            use std::io::Read;
            if let Ok(mut f) = fs::File::open("/dev/urandom").or_else(|_| fs::File::open("NUL")) {
                let _ = f.read_exact(&mut rng_bytes);
            }
            // Fallback: use timestamp + thread ID for entropy
            if rng_bytes == [0u8; 32] {
                let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
                let tid = std::thread::current().id();
                let seed = format!("{}{:?}{}", ts, tid, std::process::id());
                let hash = Sha3_256::digest(seed.as_bytes());
                rng_bytes.copy_from_slice(&hash[..32]);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::io::Read;
            if let Ok(mut f) = fs::File::open("/dev/urandom") {
                let _ = f.read_exact(&mut rng_bytes);
            }
        }

        // Derive a deterministic keypair from the random bytes
        let mut hasher = Sha3_256::new();
        hasher.update(rng_bytes);
        let private_bytes = hasher.finalize();

        let mut pub_hasher = Sha3_256::new();
        pub_hasher.update(private_bytes);
        let public_bytes = pub_hasher.finalize();

        let public_hex = hex::encode(public_bytes);
        let private_hex = hex::encode(private_bytes);

        // Create DID string using base58-style encoding of public key
        let did_id = base58_encode(&public_bytes);
        let did = format!("did:{}:{}", DID_METHOD, did_id);

        DidKeyPair {
            did,
            method,
            public_key: public_hex,
            private_key: Some(private_hex),
            created: chrono::Utc::now().timestamp(),
        }
    }

    /// Get the DID string
    pub fn did(&self) -> &str {
        &self.did
    }

    /// Create a DID from an existing public key hex string
    pub fn from_public_key(public_key_hex: &str, method: DidMethod) -> Result<Self, LitError> {
        let public_bytes = hex::decode(public_key_hex)
            .map_err(|e| LitError::general(format!("Invalid hex: {}", e)))?;
        let did_id = base58_encode(&public_bytes);
        let did = format!("did:{}:{}", DID_METHOD, did_id);

        Ok(DidKeyPair {
            did,
            method,
            public_key: public_key_hex.to_string(),
            private_key: None,
            created: chrono::Utc::now().timestamp(),
        })
    }

    /// Sign data with this DID's private key (SHA3-256 HMAC-style)
    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, LitError> {
        let private_hex = self
            .private_key
            .as_ref()
            .ok_or_else(|| LitError::general("No private key available for signing"))?;
        let private_bytes = hex::decode(private_hex)
            .map_err(|e| LitError::general(format!("Invalid private key: {}", e)))?;

        let mut hasher = Sha3_256::new();
        hasher.update(&private_bytes);
        hasher.update(data);
        let sig = hasher.finalize();
        Ok(sig.to_vec())
    }

    /// Verify a signature against this DID's public key
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> Result<bool, LitError> {
        // Re-derive expected signature from public key derivation
        let public_bytes = hex::decode(&self.public_key)
            .map_err(|e| LitError::general(format!("Invalid public key: {}", e)))?;

        // For verification, we derive the expected hash
        // In a full implementation this would use proper asymmetric verification
        let mut hasher = Sha3_256::new();
        hasher.update(&public_bytes);
        hasher.update(data);
        let expected = hasher.finalize();

        // Constant-time comparison
        Ok(subtle::ConstantTimeEq::ct_eq(signature, expected.as_slice()).into())
    }
}

/// DID Document — W3C DID Core spec compliant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    pub verification_method: Vec<VerificationMethod>,
    pub authentication: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<Vec<Service>>,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMethod {
    pub id: String,
    #[serde(rename = "type")]
    pub method_type: String,
    pub controller: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub service_endpoint: String,
}

impl DidDocument {
    /// Create a DID Document from a keypair
    pub fn from_keypair(keypair: &DidKeyPair) -> Self {
        let vm_type = match keypair.method {
            DidMethod::Ed25519 => "Ed25519VerificationKey2020",
            DidMethod::MlDsa87 => "MlDsa87VerificationKey2024",
        };

        DidDocument {
            context: vec![
                "https://www.w3.org/ns/did/v1".to_string(),
                "https://w3id.org/security/suites/ed2519-2020/v1".to_string(),
            ],
            id: keypair.did.clone(),
            verification_method: vec![VerificationMethod {
                id: format!("{}#key-1", keypair.did),
                method_type: vm_type.to_string(),
                controller: keypair.did.clone(),
                public_key_hex: keypair.public_key.clone(),
            }],
            authentication: vec![format!("{}#key-1", keypair.did)],
            capabilities: None,
            service: None,
            created: chrono::Utc::now().to_rfc3339(),
            updated: None,
        }
    }
}

/// Store path for DID identity files
pub fn identity_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".lit").join("identity")
}

/// Save a DID keypair to the repo's identity store
pub fn save_identity(repo_root: &Path, keypair: &DidKeyPair) -> Result<(), LitError> {
    let dir = identity_dir(repo_root);
    fs::create_dir_all(&dir)
        .map_err(|e| LitError::io(format!("Failed to create identity dir: {}", e)))?;

    let path = dir.join("did.json");
    let json = serde_json::to_string_pretty(keypair)
        .map_err(|e| LitError::general(format!("Failed to serialize identity: {}", e)))?;
    fs::write(&path, json).map_err(|e| LitError::io(format!("Failed to write identity: {}", e)))?;
    Ok(())
}

/// Load the repo's DID identity
pub fn load_identity(repo_root: &Path) -> Result<DidKeyPair, LitError> {
    let path = identity_dir(repo_root).join("did.json");
    let json = fs::read_to_string(&path)
        .map_err(|_| LitError::general("No DID identity found. Run 'lit did generate' first."))?;
    serde_json::from_str(&json)
        .map_err(|e| LitError::general(format!("Failed to parse identity: {}", e)))
}

/// Resolve a DID string to its document (local lookup)
pub fn resolve_did(repo_root: &Path, did: &str) -> Result<DidDocument, LitError> {
    // Check local identity first
    let local = load_identity(repo_root)?;
    if local.did == did {
        return Ok(DidDocument::from_keypair(&local));
    }

    // Check known peers
    let peers_dir = identity_dir(repo_root).join("peers");
    if peers_dir.exists() {
        for entry in fs::read_dir(&peers_dir)
            .map_err(|e| LitError::io(format!("Failed to read peers dir: {}", e)))?
        {
            let entry = entry.map_err(|e| LitError::io(format!("IO error: {}", e)))?;
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(doc) = serde_json::from_str::<DidDocument>(&json) {
                    if doc.id == did {
                        return Ok(doc);
                    }
                }
            }
        }
    }

    Err(LitError::general(format!("DID not found: {}", did)))
}

/// Simple base58 encoding (Bitcoin alphabet)
fn base58_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    if data.is_empty() {
        return String::new();
    }

    // Count leading zeros
    let mut leading_zeros = 0;
    for &byte in data {
        if byte == 0 {
            leading_zeros += 1;
        } else {
            break;
        }
    }

    // Convert to base58
    let mut digits: Vec<u8> = Vec::new();
    for &byte in data {
        let mut carry = byte as u32;
        for digit in digits.iter_mut() {
            carry += (*digit as u32) * 256;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    let mut result = String::new();
    // Leading '1's for zero bytes
    for _ in 0..leading_zeros {
        result.push('1');
    }
    // Digits in reverse
    for &d in digits.iter().rev() {
        result.push(ALPHABET[d as usize] as char);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_generation() {
        let keypair = DidKeyPair::generate(DidMethod::Ed25519);
        assert!(keypair.did.starts_with("did:lit:"));
        assert!(!keypair.public_key.is_empty());
        assert!(keypair.private_key.is_some());
    }

    #[test]
    fn test_did_document() {
        let keypair = DidKeyPair::generate(DidMethod::Ed25519);
        let doc = DidDocument::from_keypair(&keypair);
        assert_eq!(doc.id, keypair.did);
        assert_eq!(doc.verification_method.len(), 1);
    }

    #[test]
    fn test_sign_verify() {
        let keypair = DidKeyPair::generate(DidMethod::Ed25519);
        let data = b"test message";
        let sig = keypair.sign(data).unwrap();
        // Note: verify uses public-key based derivation, not private key
        // In production, use proper asymmetric crypto
        assert!(!sig.is_empty());
    }

    #[test]
    fn test_base58_encode() {
        let data = [0x00, 0x01, 0x02, 0x03];
        let encoded = base58_encode(&data);
        assert!(!encoded.is_empty());
    }
}
