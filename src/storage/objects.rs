use crate::core::{Object, ObjectHash};
use crate::crypto::encryption::{EncryptionConfig, EncryptionManager};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Object storage - handles reading/writing objects to disk with optional encryption
pub struct ObjectStore {
    objects_dir: PathBuf,
    encryption: Arc<Mutex<EncryptionManager>>,
}

impl ObjectStore {
    /// Create a new object store
    pub fn new(repo_path: &Path) -> Self {
        let objects_dir = repo_path.join(".lit").join("objects");

        // Load encryption configuration
        let encryption_config = EncryptionConfig::load(repo_path).unwrap_or_default();
        let encryption = Arc::new(Mutex::new(EncryptionManager::new(encryption_config)));

        ObjectStore {
            objects_dir,
            encryption,
        }
    }

    /// Create object store with encryption passphrase
    pub fn new_with_encryption(repo_path: &Path, passphrase: Option<&str>) -> Result<Self, String> {
        let objects_dir = repo_path.join(".lit").join("objects");

        // Load encryption configuration
        let encryption_config = EncryptionConfig::load(repo_path)?;
        let mut encryption_manager = EncryptionManager::new(encryption_config);

        // Initialize encryption if passphrase provided
        if let Some(pass) = passphrase {
            encryption_manager.initialize(pass)?;
        }

        let encryption = Arc::new(Mutex::new(encryption_manager));

        Ok(ObjectStore {
            objects_dir,
            encryption,
        })
    }

    /// Get the path for an object by its hash
    /// Uses first 4 chars for directory sharding (65,536 shards for better distribution)
    fn object_path(&self, hash: &ObjectHash) -> PathBuf {
        let hash_str = hash.as_str();
        let (dir, file) = hash_str.split_at(4);
        self.objects_dir.join(dir).join(file)
    }

    /// Write an object to storage
    pub fn write(&self, object: &Object) -> Result<ObjectHash, String> {
        let hash = object.hash();
        let path = self.object_path(&hash);

        // Create parent directory
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create object directory: {}", e))?;
        }

        // Serialize and compress
        let data = object.to_bytes();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&data)
            .map_err(|e| format!("Failed to compress object: {}", e))?;
        let compressed = encoder
            .finish()
            .map_err(|e| format!("Failed to finish compression: {}", e))?;

        // Encrypt if enabled
        let final_data = {
            let encryption = self
                .encryption
                .lock()
                .map_err(|e| format!("Failed to acquire encryption lock: {}", e))?;
            encryption.encrypt(&compressed)?
        };

        fs::write(&path, final_data).map_err(|e| format!("Failed to write object: {}", e))?;

        Ok(hash)
    }

    /// Read an object from storage
    pub fn read(&self, hash: &ObjectHash) -> Result<Object, String> {
        let path = self.object_path(hash);

        if !path.exists() {
            return Err(format!("Object {} not found", hash.short()));
        }

        // Read encrypted/compressed data
        let encrypted_data =
            fs::read(&path).map_err(|e| format!("Failed to read object: {}", e))?;

        // Decrypt if enabled
        let compressed = {
            let encryption = self
                .encryption
                .lock()
                .map_err(|e| format!("Failed to acquire encryption lock: {}", e))?;
            encryption.decrypt(&encrypted_data)?
        };

        // Decompress
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut data = Vec::new();
        decoder
            .read_to_end(&mut data)
            .map_err(|e| format!("Failed to decompress object: {}", e))?;

        Object::from_bytes(&data)
    }

    /// Check if an object exists
    pub fn exists(&self, hash: &ObjectHash) -> bool {
        self.object_path(hash).exists()
    }

    /// List all objects
    pub fn list(&self) -> Result<Vec<ObjectHash>, String> {
        let mut objects = Vec::new();

        if !self.objects_dir.exists() {
            return Ok(objects);
        }

        for entry in walkdir::WalkDir::new(&self.objects_dir)
            .min_depth(2)
            .max_depth(2)
        {
            let entry = entry.map_err(|e| format!("Failed to read objects: {}", e))?;

            if entry.file_type().is_file() {
                let path = entry.path();

                // Reconstruct hash from path
                if let Some(file_name) = path.file_name() {
                    if let Some(parent) = path.parent() {
                        if let Some(dir_name) = parent.file_name() {
                            let hash = format!(
                                "{}{}",
                                dir_name.to_string_lossy(),
                                file_name.to_string_lossy()
                            );
                            objects.push(ObjectHash::from_hex(hash));
                        }
                    }
                }
            }
        }

        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Blob;
    use tempfile::TempDir;

    #[test]
    fn test_object_store() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create .lit/objects directory
        fs::create_dir_all(repo_path.join(".lit").join("objects")).unwrap();

        let store = ObjectStore::new(repo_path);

        // Create and write a blob
        let content = b"Hello, world!".to_vec();
        let blob = Blob::new(content.clone());
        let object = Object::Blob(blob);

        let hash = store.write(&object).unwrap();

        // Read it back
        let read_object = store.read(&hash).unwrap();

        match read_object {
            Object::Blob(blob) => assert_eq!(blob.content, content),
            _ => panic!("Expected blob"),
        }
    }
}
