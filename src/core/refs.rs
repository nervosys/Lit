use crate::crypto::encryption::EncryptionManager;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Reference - points to a commit (branch, tag, HEAD)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub name: String,
    pub hash: String,
}

/// Get the Lit directory path
pub fn get_lit_dir(repo_path: &Path) -> PathBuf {
    repo_path.join(".lit")
}

/// Check if a directory is a Lit repository
pub fn is_lit_repo(path: &Path) -> bool {
    get_lit_dir(path).exists()
}

/// Find the repository root from current directory
pub fn find_repo_root() -> Result<PathBuf, String> {
    let mut current =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    loop {
        if is_lit_repo(&current) {
            return Ok(current);
        }

        if !current.pop() {
            return Err("Not in a Lit repository".to_string());
        }
    }
}

/// Read a reference file
pub fn read_ref(repo_path: &Path, ref_name: &str) -> Result<String, String> {
    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if !ref_path.exists() {
        return Err(format!("Reference '{}' not found", ref_name));
    }

    fs::read_to_string(&ref_path)
        .map_err(|e| format!("Failed to read reference: {}", e))
        .map(|s| s.trim().to_string())
}

/// Read an encrypted reference file
pub fn read_ref_encrypted(
    repo_path: &Path,
    ref_name: &str,
    encryption: &Arc<Mutex<EncryptionManager>>,
) -> Result<String, String> {
    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if !ref_path.exists() {
        return Err(format!("Reference '{}' not found", ref_name));
    }

    let encrypted_data =
        fs::read(&ref_path).map_err(|e| format!("Failed to read reference: {}", e))?;

    let enc_guard = encryption
        .lock()
        .map_err(|_| "Failed to lock encryption manager".to_string())?;

    let decrypted = enc_guard.decrypt(&encrypted_data)?;

    String::from_utf8(decrypted)
        .map_err(|e| format!("Invalid UTF-8 in decrypted reference: {}", e))
        .map(|s| s.trim().to_string())
}

/// Write a reference file
pub fn write_ref(repo_path: &Path, ref_name: &str, hash: &str) -> Result<(), String> {
    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create ref directory: {}", e))?;
    }

    fs::write(&ref_path, format!("{}\n", hash))
        .map_err(|e| format!("Failed to write reference: {}", e))
}

/// Write an encrypted reference file
pub fn write_ref_encrypted(
    repo_path: &Path,
    ref_name: &str,
    hash: &str,
    encryption: &Arc<Mutex<EncryptionManager>>,
) -> Result<(), String> {
    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create ref directory: {}", e))?;
    }

    let data = format!("{}\n", hash);

    let enc_guard = encryption
        .lock()
        .map_err(|_| "Failed to lock encryption manager".to_string())?;

    let encrypted = enc_guard.encrypt(data.as_bytes())?;
    drop(enc_guard);

    fs::write(&ref_path, encrypted).map_err(|e| format!("Failed to write reference: {}", e))
}

/// Delete a reference
pub fn delete_ref(repo_path: &Path, ref_name: &str) -> Result<(), String> {
    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if !ref_path.exists() {
        return Err(format!("Reference '{}' not found", ref_name));
    }

    fs::remove_file(&ref_path).map_err(|e| format!("Failed to delete reference: {}", e))
}

/// List all references
pub fn list_refs(repo_path: &Path, prefix: &str) -> Result<Vec<Reference>, String> {
    let refs_dir = get_lit_dir(repo_path).join("refs").join(prefix);

    if !refs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut refs = Vec::new();

    for entry in walkdir::WalkDir::new(&refs_dir) {
        let entry = entry.map_err(|e| format!("Failed to read refs: {}", e))?;

        if entry.file_type().is_file() {
            let path = entry.path();
            let name = path
                .strip_prefix(&refs_dir)
                .map_err(|e| format!("Path error: {}", e))?
                .to_string_lossy()
                .to_string();

            let hash = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read ref: {}", e))?
                .trim()
                .to_string();

            refs.push(Reference { name, hash });
        }
    }

    Ok(refs)
}

/// Read HEAD reference
pub fn read_head(repo_path: &Path) -> Result<String, String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    if !head_path.exists() {
        return Err("HEAD not found".to_string());
    }

    let content =
        fs::read_to_string(&head_path).map_err(|e| format!("Failed to read HEAD: {}", e))?;

    let content = content.trim();

    // Check if HEAD is symbolic (ref: refs/heads/main)
    if let Some(ref_name) = content.strip_prefix("ref: ") {
        read_ref(
            repo_path,
            ref_name.strip_prefix("refs/").unwrap_or(ref_name),
        )
    } else {
        // Direct hash
        Ok(content.to_string())
    }
}

/// Get current branch name
pub fn get_current_branch(repo_path: &Path) -> Result<String, String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    let content =
        fs::read_to_string(&head_path).map_err(|e| format!("Failed to read HEAD: {}", e))?;

    let content = content.trim();

    if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
        Ok(branch.to_string())
    } else {
        Err("HEAD is detached".to_string())
    }
}

/// Update HEAD to point to a branch
pub fn update_head(repo_path: &Path, branch: &str) -> Result<(), String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    fs::write(&head_path, format!("ref: refs/heads/{}\n", branch))
        .map_err(|e| format!("Failed to update HEAD: {}", e))
}

/// Update HEAD to point to a branch (encrypted)
pub fn update_head_encrypted(
    repo_path: &Path,
    branch: &str,
    encryption: &Arc<Mutex<EncryptionManager>>,
) -> Result<(), String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    let data = format!("ref: refs/heads/{}\n", branch);

    let enc_guard = encryption
        .lock()
        .map_err(|_| "Failed to lock encryption manager".to_string())?;

    let encrypted = enc_guard.encrypt(data.as_bytes())?;
    drop(enc_guard);

    fs::write(&head_path, encrypted).map_err(|e| format!("Failed to update HEAD: {}", e))
}

/// Set HEAD to a specific commit (detached)
pub fn set_head_detached(repo_path: &Path, hash: &str) -> Result<(), String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    fs::write(&head_path, format!("{}\n", hash)).map_err(|e| format!("Failed to set HEAD: {}", e))
}

/// Set HEAD to a specific commit (detached, encrypted)
pub fn set_head_detached_encrypted(
    repo_path: &Path,
    hash: &str,
    encryption: &Arc<Mutex<EncryptionManager>>,
) -> Result<(), String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    let data = format!("{}\n", hash);

    let enc_guard = encryption
        .lock()
        .map_err(|_| "Failed to lock encryption manager".to_string())?;

    let encrypted = enc_guard.encrypt(data.as_bytes())?;
    drop(enc_guard);

    fs::write(&head_path, encrypted).map_err(|e| format!("Failed to set HEAD: {}", e))
}

/// Read HEAD reference (encrypted)
pub fn read_head_encrypted(
    repo_path: &Path,
    encryption: &Arc<Mutex<EncryptionManager>>,
) -> Result<String, String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    if !head_path.exists() {
        return Err("HEAD not found".to_string());
    }

    let encrypted_data = fs::read(&head_path).map_err(|e| format!("Failed to read HEAD: {}", e))?;

    let enc_guard = encryption
        .lock()
        .map_err(|_| "Failed to lock encryption manager".to_string())?;

    let decrypted = enc_guard.decrypt(&encrypted_data)?;
    drop(enc_guard);

    let content = String::from_utf8(decrypted)
        .map_err(|e| format!("Invalid UTF-8 in decrypted HEAD: {}", e))?;

    let content = content.trim();

    // Check if HEAD is symbolic (ref: refs/heads/main)
    if let Some(ref_name) = content.strip_prefix("ref: ") {
        read_ref_encrypted(
            repo_path,
            ref_name.strip_prefix("refs/").unwrap_or(ref_name),
            encryption,
        )
    } else {
        // Direct hash
        Ok(content.to_string())
    }
}

/// Get current branch name (encrypted)
pub fn get_current_branch_encrypted(
    repo_path: &Path,
    encryption: &Arc<Mutex<EncryptionManager>>,
) -> Result<String, String> {
    let head_path = get_lit_dir(repo_path).join("HEAD");

    let encrypted_data = fs::read(&head_path).map_err(|e| format!("Failed to read HEAD: {}", e))?;

    let enc_guard = encryption
        .lock()
        .map_err(|_| "Failed to lock encryption manager".to_string())?;

    let decrypted = enc_guard.decrypt(&encrypted_data)?;
    drop(enc_guard);

    let content = String::from_utf8(decrypted)
        .map_err(|e| format!("Invalid UTF-8 in decrypted HEAD: {}", e))?;

    let content = content.trim();

    if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
        Ok(branch.to_string())
    } else {
        Err("HEAD is detached".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::encryption::EncryptionConfig;
    use std::env;

    #[test]
    fn test_encrypted_ref_write_read() {
        // Clean up any existing key file from previous tests
        let key_path = shellexpand::tilde("~/.lit/encryption.key");
        fs::remove_file(key_path.as_ref()).ok();

        let temp_dir = env::temp_dir().join("lit-test-encrypted-refs");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(get_lit_dir(&temp_dir).join("refs/heads")).unwrap();

        let mut config = EncryptionConfig::default();
        config.enabled = true;

        let mut enc_manager = EncryptionManager::new(config);
        enc_manager.initialize("test-passphrase-refs").unwrap();
        let encryption = Arc::new(Mutex::new(enc_manager));

        let test_hash = "abc123def456";
        let ref_name = "heads/test-branch";

        // Write encrypted ref
        write_ref_encrypted(&temp_dir, ref_name, test_hash, &encryption).unwrap();

        // Read encrypted ref
        let read_hash = read_ref_encrypted(&temp_dir, ref_name, &encryption).unwrap();

        assert_eq!(read_hash, test_hash);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_encrypted_head_operations() {
        // Clean up any existing key file from previous tests
        let key_path = shellexpand::tilde("~/.lit/encryption.key");
        fs::remove_file(key_path.as_ref()).ok();

        let temp_dir = env::temp_dir().join("lit-test-encrypted-head");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(get_lit_dir(&temp_dir).join("refs/heads")).unwrap();

        let mut config = EncryptionConfig::default();
        config.enabled = true;

        let mut enc_manager = EncryptionManager::new(config);
        enc_manager.initialize("test-passphrase-head").unwrap();
        let encryption = Arc::new(Mutex::new(enc_manager));

        let branch_name = "main";
        let commit_hash = "deadbeef123456";

        // Write branch ref
        write_ref_encrypted(&temp_dir, "heads/main", commit_hash, &encryption).unwrap();

        // Update HEAD to point to branch
        update_head_encrypted(&temp_dir, branch_name, &encryption).unwrap();

        // Get current branch
        let current = get_current_branch_encrypted(&temp_dir, &encryption).unwrap();
        assert_eq!(current, branch_name);

        // Read HEAD (should resolve to commit)
        let head_commit = read_head_encrypted(&temp_dir, &encryption).unwrap();
        assert_eq!(head_commit, commit_hash);

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_encrypted_detached_head() {
        // Clean up any existing key file from previous tests
        let key_path = shellexpand::tilde("~/.lit/encryption.key");
        fs::remove_file(key_path.as_ref()).ok();

        let temp_dir = env::temp_dir().join("lit-test-encrypted-detached");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(get_lit_dir(&temp_dir)).unwrap();

        let mut config = EncryptionConfig::default();
        config.enabled = true;

        let mut enc_manager = EncryptionManager::new(config);
        enc_manager.initialize("test-passphrase-detached").unwrap();
        let encryption = Arc::new(Mutex::new(enc_manager));

        let commit_hash = "cafebabe987654";

        // Set HEAD to detached state
        set_head_detached_encrypted(&temp_dir, commit_hash, &encryption).unwrap();

        // Read HEAD
        let head = read_head_encrypted(&temp_dir, &encryption).unwrap();
        assert_eq!(head, commit_hash);

        // Getting branch should fail (detached)
        assert!(get_current_branch_encrypted(&temp_dir, &encryption).is_err());

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_encrypted_ref_tamper_detection() {
        // Clean up any existing key file from previous tests
        let key_path = shellexpand::tilde("~/.lit/encryption.key");
        fs::remove_file(key_path.as_ref()).ok();

        let temp_dir = env::temp_dir().join("lit-test-encrypted-tamper");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(get_lit_dir(&temp_dir).join("refs/heads")).unwrap();

        let mut config = EncryptionConfig::default();
        config.enabled = true;

        let mut enc_manager = EncryptionManager::new(config);
        enc_manager.initialize("test-passphrase-tamper").unwrap();
        let encryption = Arc::new(Mutex::new(enc_manager));

        let test_hash = "original123";
        let ref_name = "heads/tamper-test";

        // Write encrypted ref
        write_ref_encrypted(&temp_dir, ref_name, test_hash, &encryption).unwrap();

        // Tamper with the encrypted file
        let ref_path = get_lit_dir(&temp_dir).join("refs").join(ref_name);
        let mut data = fs::read(&ref_path).unwrap();
        let len = data.len();
        data[len - 1] ^= 0x01; // Flip a bit
        fs::write(&ref_path, data).unwrap();

        // Reading should fail due to authentication tag mismatch
        assert!(read_ref_encrypted(&temp_dir, ref_name, &encryption).is_err());

        // Cleanup
        fs::remove_dir_all(&temp_dir).ok();
    }
}
