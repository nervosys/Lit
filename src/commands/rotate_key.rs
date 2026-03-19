/// Passphrase Rotation Command
/// Re-encrypts all repository data with a new passphrase
use crate::core::refs;
use crate::crypto::encryption::{
    clear_passphrase_cache, prompt_for_passphrase, prompt_for_passphrase_confirmation,
    EncryptionConfig, EncryptionKey, EncryptionManager,
};
use crate::response::RotateKeyResponse;
use std::fs;

/// Rotate encryption passphrase
///
/// This command:
/// 1. Prompts for current passphrase
/// 2. Decrypts all repository data
/// 3. Prompts for new passphrase
/// 4. Generates new salt
/// 5. Re-encrypts all data with new key
/// 6. Updates encryption.key file
/// 7. Clears passphrase cache
pub fn rotate_key() -> Result<RotateKeyResponse, crate::errors::LitError> {
    let repo_path = refs::find_repo_root()?;

    // Load encryption config
    let config = EncryptionConfig::load(&repo_path)?;

    if !config.enabled {
        return Err("Encryption is not enabled for this repository".into());
    }

    // Step 1: Get current passphrase
    let old_passphrase = prompt_for_passphrase(
        repo_path.to_str().ok_or("Non-UTF-8 repository path")?,
        &config,
        "Enter current passphrase: ",
    )?;

    // Initialize encryption manager with old passphrase
    let mut old_manager = EncryptionManager::new(config.clone());
    old_manager.initialize(&old_passphrase)?;

    // Step 2: Decrypt all repository data
    let objects_dir = repo_path.join(".lit").join("objects");
    let mut encrypted_objects = Vec::new();

    if objects_dir.exists() {
        for entry in walkdir::WalkDir::new(&objects_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let path = entry.path();
                let encrypted_data = fs::read(path)
                    .map_err(|e| format!("Failed to read object {}: {}", path.display(), e))?;

                let decrypted_data = old_manager.decrypt(&encrypted_data)?;
                encrypted_objects.push((path.to_path_buf(), decrypted_data));
            }
        }
    }

    // Read index if exists
    let index_path = repo_path.join(".lit").join("index");
    let index_data = if index_path.exists() {
        let encrypted =
            fs::read(&index_path).map_err(|e| format!("Failed to read index: {}", e))?;
        Some(old_manager.decrypt(&encrypted)?)
    } else {
        None
    };

    // Read all refs
    let refs_dir = repo_path.join(".lit").join("refs");
    let mut ref_data = Vec::new();

    if refs_dir.exists() {
        for entry in walkdir::WalkDir::new(&refs_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let path = entry.path();
                let encrypted = fs::read(path)
                    .map_err(|e| format!("Failed to read ref {}: {}", path.display(), e))?;

                let decrypted = old_manager.decrypt(&encrypted)?;
                ref_data.push((path.to_path_buf(), decrypted));
            }
        }
    }

    // Read HEAD
    let head_path = repo_path.join(".lit").join("HEAD");
    let head_data = if head_path.exists() {
        let encrypted = fs::read(&head_path).map_err(|e| format!("Failed to read HEAD: {}", e))?;
        Some(old_manager.decrypt(&encrypted)?)
    } else {
        None
    };

    let objects_count = encrypted_objects.len();
    let refs_count = ref_data.len();

    // Step 3: Get new passphrase
    let new_passphrase = prompt_for_passphrase_confirmation("Enter new passphrase: ")?;

    // Step 4: Generate new salt and derive new key
    let new_salt = EncryptionKey::generate_salt();
    let new_key = EncryptionKey::from_passphrase(&new_passphrase, &new_salt)?;

    // Backup old key file before rotation
    let expanded = shellexpand::tilde(&config.key_file);
    let key_file_path = std::path::Path::new(expanded.as_ref());
    let backup_key_path = key_file_path.with_extension("bak");

    if key_file_path.exists() {
        fs::copy(key_file_path, &backup_key_path)
            .map_err(|e| format!("Failed to backup key file: {}", e))?;
    }

    // Step 5: Re-encrypt all data with new key (TWO-PHASE COMMIT)
    let mut new_manager = EncryptionManager::new(config.clone());
    new_manager.initialize(&new_passphrase)?;

    // PHASE 1: Write all encrypted data to temporary files
    for (path, data) in &encrypted_objects {
        let encrypted = new_manager.encrypt(data)?;
        let temp_path = path.with_extension("new");
        fs::write(&temp_path, encrypted)
            .map_err(|e| format!("Failed to write temp object {}: {}", temp_path.display(), e))?;
    }

    let temp_index_path = index_path.with_extension("new");
    if let Some(data) = &index_data {
        let encrypted = new_manager.encrypt(data)?;
        fs::write(&temp_index_path, encrypted)
            .map_err(|e| format!("Failed to write temp index: {}", e))?;
    }

    for (path, data) in &ref_data {
        let encrypted = new_manager.encrypt(data)?;
        let temp_path = path.with_extension("new");
        fs::write(&temp_path, encrypted)
            .map_err(|e| format!("Failed to write temp ref {}: {}", temp_path.display(), e))?;
    }

    let temp_head_path = head_path.with_extension("new");
    if let Some(data) = &head_data {
        let encrypted = new_manager.encrypt(data)?;
        fs::write(&temp_head_path, encrypted)
            .map_err(|e| format!("Failed to write temp HEAD: {}", e))?;
    }

    // PHASE 2: Atomic rename (all-or-nothing)
    new_key.save(expanded.as_ref(), &new_passphrase)?;

    for (path, _) in &encrypted_objects {
        let temp_path = path.with_extension("new");
        fs::rename(&temp_path, path)
            .map_err(|e| format!("Failed to rename object {}: {}", path.display(), e))?;
    }

    if index_data.is_some() {
        fs::rename(&temp_index_path, &index_path)
            .map_err(|e| format!("Failed to rename index: {}", e))?;
    }

    for (path, _) in &ref_data {
        let temp_path = path.with_extension("new");
        fs::rename(&temp_path, path)
            .map_err(|e| format!("Failed to rename ref {}: {}", path.display(), e))?;
    }

    if head_data.is_some() {
        fs::rename(&temp_head_path, &head_path)
            .map_err(|e| format!("Failed to rename HEAD: {}", e))?;
    }

    // Remove backup key file on success
    if backup_key_path.exists() {
        fs::remove_file(&backup_key_path).ok();
    }

    // Clear passphrase cache
    clear_passphrase_cache();

    Ok(RotateKeyResponse {
        objects_rotated: objects_count,
        refs_rotated: refs_count,
    })
}
