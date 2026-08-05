use crate::crypto::encryption::{EncryptionConfig, EncryptionManager};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Index entry - represents a staged file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub path: String,
    pub hash: String,
    pub mode: String,
}

/// The staging area (index)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub entries: HashMap<String, IndexEntry>,
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}

impl Index {
    /// Create a new empty index
    pub fn new() -> Self {
        Index {
            entries: HashMap::new(),
        }
    }

    /// Load index from disk
    pub fn load(repo_path: &Path) -> Result<Self, String> {
        Self::load_with_encryption(repo_path, None)
    }

    /// Load index from disk with encryption support
    pub fn load_with_encryption(
        repo_path: &Path,
        passphrase: Option<&str>,
    ) -> Result<Self, String> {
        let index_path = repo_path.join(".lit").join("index");

        if !index_path.exists() {
            return Ok(Index::new());
        }

        let encrypted_data =
            fs::read(&index_path).map_err(|e| format!("Failed to read index: {}", e))?;

        // Decrypt if encryption is enabled
        let data = {
            let encryption_config = EncryptionConfig::load(repo_path)?;
            // An explicit passphrase wins; otherwise fall back to the
            // non-interactive sources, the same way the object store does.
            // Without this the index stayed locked even when the caller had
            // LIT_PASSPHRASE set, so every command that loads it failed.
            let encryption_manager = match passphrase {
                Some(pass) => {
                    let mut manager = EncryptionManager::new(encryption_config);
                    manager.initialize(pass)?;
                    manager
                }
                None => EncryptionManager::new_auto(encryption_config, repo_path),
            };

            encryption_manager.decrypt(&encrypted_data)?
        };

        serde_json::from_slice(&data).map_err(|e| format!("Failed to parse index: {}", e))
    }

    /// Save index to disk
    pub fn save(&self, repo_path: &Path) -> Result<(), String> {
        self.save_with_encryption(repo_path, None)
    }

    /// Save index to disk with encryption support
    pub fn save_with_encryption(
        &self,
        repo_path: &Path,
        passphrase: Option<&str>,
    ) -> Result<(), String> {
        let index_path = repo_path.join(".lit").join("index");

        let data = serde_json::to_vec_pretty(self)
            .map_err(|e| format!("Failed to serialize index: {}", e))?;

        // Encrypt if encryption is enabled
        let final_data = {
            let encryption_config = EncryptionConfig::load(repo_path)?;
            let encryption_manager = match passphrase {
                Some(pass) => {
                    let mut manager = EncryptionManager::new(encryption_config);
                    manager.initialize(pass)?;
                    manager
                }
                None => EncryptionManager::new_auto(encryption_config, repo_path),
            };

            encryption_manager.encrypt(&data)?
        };

        fs::write(&index_path, final_data).map_err(|e| format!("Failed to write index: {}", e))
    }

    /// Add or update an entry in the index
    pub fn add(&mut self, path: String, hash: String, mode: String) {
        self.entries
            .insert(path.clone(), IndexEntry { path, hash, mode });
    }

    /// Remove an entry from the index
    pub fn remove(&mut self, path: &str) -> Option<IndexEntry> {
        self.entries.remove(path)
    }

    /// Get all entries sorted by path
    pub fn sorted_entries(&self) -> Vec<&IndexEntry> {
        let mut entries: Vec<&IndexEntry> = self.entries.values().collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
    }

    /// Clear the index
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        fs::create_dir_all(repo_path.join(".lit")).unwrap();

        let mut index = Index::new();
        index.add(
            "file.txt".to_string(),
            "abc123".to_string(),
            "100644".to_string(),
        );

        index.save(repo_path).unwrap();

        let loaded = Index::load(repo_path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert!(loaded.entries.contains_key("file.txt"));
    }
}
