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
    /// Where `lit gc` puts packs. Objects live loose until they are packed, and
    /// packing removes the loose copy, so every lookup that misses on disk has
    /// to consult the packs before concluding the object is absent.
    packs_dir: PathBuf,
    encryption: Arc<Mutex<EncryptionManager>>,
}

impl ObjectStore {
    /// Create a new object store
    pub fn new(repo_path: &Path) -> Self {
        let objects_dir = repo_path.join(".lit").join("objects");

        // Load encryption configuration
        let encryption_config = EncryptionConfig::load(repo_path).unwrap_or_default();
        let encryption = Arc::new(Mutex::new(EncryptionManager::new_auto(
            encryption_config,
            repo_path,
        )));

        ObjectStore {
            objects_dir,
            packs_dir: crate::storage::pack::packs_dir(repo_path),
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
            packs_dir: crate::storage::pack::packs_dir(repo_path),
            encryption,
        })
    }

    /// Create an object store that uses a manager the caller already built.
    ///
    /// `rotate-key` needs to write objects under a key that is not the one the
    /// key file describes yet, which no passphrase-based constructor can
    /// express.
    pub fn with_encryption_manager(repo_path: &Path, manager: EncryptionManager) -> Self {
        ObjectStore {
            objects_dir: repo_path.join(".lit").join("objects"),
            packs_dir: crate::storage::pack::packs_dir(repo_path),
            encryption: Arc::new(Mutex::new(manager)),
        }
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
            // Not loose: it may have been packed by `lit gc`, which removes the
            // loose copy once the pack is written.
            return self.read_packed(hash);
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
            || crate::storage::pack::load_all(&self.packs_dir).contains_key(hash.as_str())
    }

    /// Read an object that lives in a pack rather than loose on disk.
    fn read_packed(&self, hash: &ObjectHash) -> Result<Object, String> {
        let packed = crate::storage::pack::load_all(&self.packs_dir);
        let Some((pack_path, offset)) = packed.get(hash.as_str()) else {
            return Err(format!("Object {} not found", hash.short()));
        };

        let encryption = self
            .encryption
            .lock()
            .map_err(|e| format!("Failed to acquire encryption lock: {}", e))?;

        crate::storage::pack::read_pack_object(pack_path, *offset, &encryption)
            .map_err(|e| format!("Failed to read {} from pack: {}", hash.short(), e))
    }

    /// The encryption manager this store reads and writes through.
    ///
    /// `gc` needs it so that packed objects get exactly the treatment the loose
    /// ones had, rather than landing on disk in the clear.
    pub fn encryption(&self) -> Arc<Mutex<EncryptionManager>> {
        Arc::clone(&self.encryption)
    }

    /// List all objects
    pub fn list(&self) -> Result<Vec<ObjectHash>, String> {
        let mut objects = Vec::new();

        // Packed objects have no loose file to walk, so take them from the pack
        // indexes. A hash can appear in both if a pack was written but the loose
        // copy not yet removed, so the two sets are de-duplicated at the end.
        for hash in crate::storage::pack::load_all(&self.packs_dir).into_keys() {
            objects.push(ObjectHash::from_hex(hash));
        }

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

        objects.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        objects.dedup_by(|a, b| a.as_str() == b.as_str());
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
