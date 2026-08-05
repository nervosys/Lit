use crate::crypto::encryption::EncryptionManager;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

/// The encryption manager guarding a repository's refs.
///
/// Built the same way the object store and index build theirs, so refs are
/// covered by the same passphrase and the same non-interactive sources. When
/// encryption is off the manager passes bytes through untouched.
fn ref_encryption(repo_path: &Path) -> EncryptionManager {
    let config = crate::crypto::encryption::EncryptionConfig::load(repo_path).unwrap_or_default();
    EncryptionManager::new_auto(config, repo_path)
}

/// Write ref-shaped text, encrypted when the repository is.
fn write_ref_file(path: &Path, repo_path: &Path, text: &str) -> Result<(), String> {
    let data = ref_encryption(repo_path).encrypt(text.as_bytes())?;
    fs::write(path, data).map_err(|e| format!("Failed to write reference: {}", e))
}

/// Read ref-shaped text, decrypting it when it was encrypted.
///
/// A ref written before encryption was switched on carries no header and is
/// returned as it stands, so a repository that has not been migrated still
/// reads. `migrate-encryption` converts them.
fn read_ref_file(path: &Path, repo_path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("Failed to read reference: {}", e))?;

    let plain = if EncryptionManager::is_encrypted_payload(&data) {
        ref_encryption(repo_path).decrypt(&data)?
    } else {
        data
    };

    String::from_utf8(plain)
        .map_err(|e| format!("Reference is not valid UTF-8: {}", e))
        .map(|s| s.trim().to_string())
}

/// Where an encrypted repository keeps all of its refs.
fn refs_index_path(repo_path: &Path) -> PathBuf {
    get_lit_dir(repo_path).join("refs.enc")
}

/// Whether this repository encrypts at rest.
fn encryption_enabled(repo_path: &Path) -> bool {
    crate::crypto::encryption::EncryptionConfig::load(repo_path)
        .map(|config| config.enabled)
        .unwrap_or(false)
}

/// Load the encrypted ref index, empty when there is none yet.
///
/// A ref name is a filename, so a directory holding one file per ref leaks
/// every branch and tag name however well the contents are encrypted.
/// Collapsing them into a single encrypted map hides the names as well.
///
/// The cost is granularity: refs become read-modify-write as a unit, so two
/// processes updating different branches at the same moment can race where
/// separate files could not. That is why this is used only when encryption is
/// on — an unencrypted repository keeps the directory and its concurrency.
fn load_refs_index(repo_path: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = refs_index_path(repo_path);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let data = fs::read(&path).map_err(|e| format!("Failed to read ref index: {}", e))?;
    let plain = if EncryptionManager::is_encrypted_payload(&data) {
        ref_encryption(repo_path).decrypt(&data)?
    } else {
        data
    };

    serde_json::from_slice(&plain).map_err(|e| format!("Failed to parse ref index: {}", e))
}

/// Write the ref index back, encrypted.
fn save_refs_index(repo_path: &Path, refs: &BTreeMap<String, String>) -> Result<(), String> {
    let json =
        serde_json::to_vec(refs).map_err(|e| format!("Failed to serialize ref index: {}", e))?;
    let data = ref_encryption(repo_path).encrypt(&json)?;
    fs::write(refs_index_path(repo_path), data)
        .map_err(|e| format!("Failed to write ref index: {}", e))
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
    // An encrypted repository keeps its refs in one file so the names are not
    // exposed as directory entries. A repository that has not been migrated
    // still has them loose, so fall through to that rather than failing.
    if encryption_enabled(repo_path) {
        if let Some(hash) = load_refs_index(repo_path)?.get(ref_name) {
            return Ok(hash.clone());
        }
    }

    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if !ref_path.exists() {
        return Err(format!("Reference '{}' not found", ref_name));
    }

    read_ref_file(&ref_path, repo_path)
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
    if encryption_enabled(repo_path) {
        let mut refs = load_refs_index(repo_path)?;
        refs.insert(ref_name.to_string(), hash.to_string());
        return save_refs_index(repo_path, &refs);
    }

    let ref_path = get_lit_dir(repo_path).join("refs").join(ref_name);

    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create ref directory: {}", e))?;
    }

    write_ref_file(&ref_path, repo_path, &format!("{}\n", hash))
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

    // Remove from both, since a part-migrated repository may hold it in either.
    let mut removed = false;

    if encryption_enabled(repo_path) {
        let mut refs = load_refs_index(repo_path)?;
        if refs.remove(ref_name).is_some() {
            save_refs_index(repo_path, &refs)?;
            removed = true;
        }
    }

    if ref_path.exists() {
        fs::remove_file(&ref_path).map_err(|e| format!("Failed to delete reference: {}", e))?;
        removed = true;
    }

    if removed {
        Ok(())
    } else {
        Err(format!("Reference '{}' not found", ref_name))
    }
}

/// List all references
pub fn list_refs(repo_path: &Path, prefix: &str) -> Result<Vec<Reference>, String> {
    let refs_dir = get_lit_dir(repo_path).join("refs").join(prefix);
    let mut refs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Encrypted repositories hold their refs in the index; a repository that
    // has not been migrated may still have loose files, so take both and let
    // the index win.
    if encryption_enabled(repo_path) {
        let with_slash = format!("{}/", prefix);
        for (name, hash) in load_refs_index(repo_path)? {
            if let Some(short) = name.strip_prefix(&with_slash) {
                seen.insert(short.to_string());
                refs.push(Reference {
                    name: short.to_string(),
                    hash,
                });
            }
        }
    }

    if !refs_dir.exists() {
        return Ok(refs);
    }

    for entry in walkdir::WalkDir::new(&refs_dir) {
        let entry = entry.map_err(|e| format!("Failed to read refs: {}", e))?;

        if entry.file_type().is_file() {
            let path = entry.path();
            let name = path
                .strip_prefix(&refs_dir)
                .map_err(|e| format!("Path error: {}", e))?
                .to_string_lossy()
                .to_string();

            if seen.contains(&name) {
                continue;
            }

            let hash = read_ref_file(path, repo_path)?;

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

    let content = read_ref_file(&head_path, repo_path)?;
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

    let content = read_ref_file(&head_path, repo_path)?;
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

    write_ref_file(
        &head_path,
        repo_path,
        &format!("ref: refs/heads/{}\n", branch),
    )
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

    write_ref_file(&head_path, repo_path, &format!("{}\n", hash))
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
    use tempfile::TempDir;

    #[test]
    fn test_encrypted_ref_write_read() {
        let temp = TempDir::new().unwrap();
        let temp_dir = temp.path().to_path_buf();
        fs::create_dir_all(get_lit_dir(&temp_dir).join("refs/heads")).unwrap();

        let config = EncryptionConfig {
            enabled: true,
            key_file: temp_dir
                .join("encryption.key")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        };

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
    }

    #[test]
    fn test_encrypted_head_operations() {
        let temp = TempDir::new().unwrap();
        let temp_dir = temp.path().to_path_buf();
        fs::create_dir_all(get_lit_dir(&temp_dir).join("refs/heads")).unwrap();

        let config = EncryptionConfig {
            enabled: true,
            key_file: temp_dir
                .join("encryption.key")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        };

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
    }

    #[test]
    fn test_encrypted_detached_head() {
        let temp = TempDir::new().unwrap();
        let temp_dir = temp.path().to_path_buf();
        fs::create_dir_all(get_lit_dir(&temp_dir)).unwrap();

        let config = EncryptionConfig {
            enabled: true,
            key_file: temp_dir
                .join("encryption.key")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        };

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
    }

    #[test]
    fn test_encrypted_ref_tamper_detection() {
        let temp = TempDir::new().unwrap();
        let temp_dir = temp.path().to_path_buf();
        fs::create_dir_all(get_lit_dir(&temp_dir).join("refs/heads")).unwrap();

        let config = EncryptionConfig {
            enabled: true,
            key_file: temp_dir
                .join("encryption.key")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        };

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
    }
}
