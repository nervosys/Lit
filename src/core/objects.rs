use crate::crypto::signatures::PQSignature;
use blake3;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use std::fmt;

/// Object hash type (SHA3-512 + BLAKE3 composite)
/// Uses NIST-approved SHA-3 as primary hash with BLAKE3 for quantum resistance
/// Format: sha3_512(data) || blake3(data) = 128 hex chars (64 + 64)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectHash(pub String);

impl ObjectHash {
    /// Create a quantum-resistant hash from bytes
    /// Combines SHA3-512 (NIST FIPS 202) and BLAKE3 for defense-in-depth
    pub fn from_bytes(data: &[u8]) -> Self {
        // SHA3-512 (NIST standard, quantum-resistant hash function)
        let mut sha3_hasher = Sha3_512::new();
        sha3_hasher.update(data);
        let sha3_result = sha3_hasher.finalize();

        // BLAKE3 (additional quantum-resistant security)
        let blake3_result = blake3::hash(data);

        // Combine both hashes for maximum security
        let combined = format!(
            "{}{}",
            hex::encode(sha3_result),
            hex::encode(blake3_result.as_bytes())
        );
        ObjectHash(combined)
    }

    /// Get the hash as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create from a hex string
    pub fn from_hex(hex: String) -> Self {
        ObjectHash(hex)
    }

    /// Get short hash (first 16 characters for quantum-resistant display)
    pub fn short(&self) -> String {
        self.0.chars().take(16).collect()
    }

    /// Get hash length in characters
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ObjectHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lit object types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Object {
    /// File content
    Blob(Blob),
    /// Directory structure
    Tree(Tree),
    /// Commit snapshot
    Commit(Commit),
    /// Annotated tag
    Tag(Tag),
}

impl Object {
    /// Get the object type name
    pub fn type_name(&self) -> &str {
        match self {
            Object::Blob(_) => "blob",
            Object::Tree(_) => "tree",
            Object::Commit(_) => "commit",
            Object::Tag(_) => "tag",
        }
    }

    /// Serialize the object to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize object")
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|e| format!("Failed to deserialize object: {}", e))
    }

    /// Calculate the hash of this object
    pub fn hash(&self) -> ObjectHash {
        ObjectHash::from_bytes(&self.to_bytes())
    }
}

/// Blob - stores file content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Self {
        Blob { content }
    }
}

/// Tree entry - represents a file or subdirectory in a tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: ObjectHash,
    pub object_type: String,
}

/// Tree - represents a directory structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    pub fn new() -> Self {
        Tree {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, mode: String, name: String, hash: ObjectHash, object_type: String) {
        self.entries.push(TreeEntry {
            mode,
            name,
            hash,
            object_type,
        });
    }
}

/// Commit - represents a snapshot in history with quantum-resistant signatures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub tree: ObjectHash,
    pub parents: Vec<ObjectHash>,
    pub author: String,
    pub committer: String,
    pub timestamp: i64,
    pub message: String,
    /// Optional post-quantum signature (ML-DSA/Dilithium)
    /// Provides quantum-resistant verification of commit authenticity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pq_signature: Option<PQSignature>,
    /// Optional metadata (agent annotations, tool context, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Commit {
    pub fn new(
        tree: ObjectHash,
        parents: Vec<ObjectHash>,
        author: String,
        message: String,
    ) -> Self {
        let timestamp = chrono::Utc::now().timestamp();
        Commit {
            tree,
            parents,
            author: author.clone(),
            committer: author,
            timestamp,
            message,
            pq_signature: None, // Can be added after creation
            metadata: None,
        }
    }

    /// Sign this commit with post-quantum signature
    pub fn sign(&mut self, keypair: &crate::crypto::signatures::PQKeyPair) {
        // Serialize commit data (excluding signature)
        let mut commit_data = self.clone();
        commit_data.pq_signature = None;
        let data = serde_json::to_vec(&commit_data).expect("Failed to serialize commit");

        // Generate quantum-resistant signature
        self.pq_signature = Some(keypair.sign(&data));
    }

    /// Verify post-quantum signature
    pub fn verify_signature(&self) -> Result<(), String> {
        match &self.pq_signature {
            Some(sig) => {
                // Reconstruct commit without signature
                let mut commit_data = self.clone();
                commit_data.pq_signature = None;
                let data = serde_json::to_vec(&commit_data)
                    .map_err(|e| format!("Failed to serialize: {}", e))?;

                sig.verify(&data)
            }
            None => Err("Commit is not signed".to_string()),
        }
    }
}

/// Tag - annotated tag object with metadata and optional signature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// Hash of the tagged object (usually a commit)
    pub target: ObjectHash,
    /// Type of the tagged object
    pub target_type: String,
    /// Tag name
    pub tag_name: String,
    /// Tagger identity
    pub tagger: String,
    /// Creation timestamp
    pub timestamp: i64,
    /// Tag message
    pub message: String,
    /// Optional post-quantum signature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pq_signature: Option<PQSignature>,
    /// Optional metadata (agent annotations, tool context, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Tag {
    pub fn new(
        target: ObjectHash,
        target_type: String,
        tag_name: String,
        tagger: String,
        message: String,
    ) -> Self {
        Tag {
            target,
            target_type,
            tag_name,
            tagger,
            timestamp: chrono::Utc::now().timestamp(),
            message,
            pq_signature: None,
            metadata: None,
        }
    }

    /// Sign this tag with post-quantum signature
    pub fn sign(&mut self, keypair: &crate::crypto::signatures::PQKeyPair) {
        let mut tag_data = self.clone();
        tag_data.pq_signature = None;
        let data = serde_json::to_vec(&tag_data).expect("Failed to serialize tag");
        self.pq_signature = Some(keypair.sign(&data));
    }

    /// Verify post-quantum signature
    pub fn verify_signature(&self) -> Result<(), String> {
        match &self.pq_signature {
            Some(sig) => {
                let mut tag_data = self.clone();
                tag_data.pq_signature = None;
                let data = serde_json::to_vec(&tag_data)
                    .map_err(|e| format!("Failed to serialize: {}", e))?;
                sig.verify(&data)
            }
            None => Err("Tag is not signed".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_hash() {
        let data = b"test content";
        let hash = ObjectHash::from_bytes(data);
        // SHA3-512 (128 hex) + BLAKE3 (64 hex) = 192 hex chars total
        // Note: BLAKE3 produces 32 bytes = 64 hex chars
        assert_eq!(hash.as_str().len(), 192); // Composite quantum-resistant hash
    }

    #[test]
    fn test_blob() {
        let content = b"Hello, world!".to_vec();
        let blob = Blob::new(content.clone());
        assert_eq!(blob.content, content);
    }

    #[test]
    fn test_tree() {
        let mut tree = Tree::new();
        let hash = ObjectHash::from_hex("abc123".to_string());
        tree.add_entry(
            "100644".to_string(),
            "file.txt".to_string(),
            hash,
            "blob".to_string(),
        );
        assert_eq!(tree.entries.len(), 1);
    }
}
