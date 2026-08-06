//! Re-encrypt a repository under a new passphrase.
//!
//! Rotation replaces the key file's salt, so every byte written under the old
//! key has to be rewritten under the new one before that file is saved. The
//! order below is what makes an interrupted run survivable: everything is
//! decrypted first, the replacements are written beside the originals, and only
//! then does the key file change and the renames happen.

use crate::core::refs;
use crate::crypto::encryption::{
    clear_derived_key_cache, clear_passphrase_cache, prompt_for_passphrase,
    prompt_for_passphrase_confirmation, EncryptionConfig, EncryptionKey, EncryptionManager,
};
use crate::response::RotateKeyResponse;
use crate::storage::{pack, ObjectStore};
use std::fs;
use std::path::{Path, PathBuf};

/// Rotate the repository passphrase, prompting for both.
pub fn rotate_key() -> Result<RotateKeyResponse, crate::errors::LitError> {
    let repo_path = refs::find_repo_root()?;
    let config = EncryptionConfig::load(&repo_path)?;

    if !config.enabled {
        return Err("Encryption is not enabled for this repository".into());
    }

    let old_passphrase = prompt_for_passphrase(
        repo_path.to_str().ok_or("Non-UTF-8 repository path")?,
        &config,
        "Enter current passphrase: ",
    )?;
    let new_passphrase = prompt_for_passphrase_confirmation("Enter new passphrase: ")?;

    rotate_with_passphrases(&old_passphrase, &new_passphrase)
}

/// Where the re-encrypted copy of `path` waits until the key file has changed.
///
/// The suffix is appended rather than replacing an extension: doing the latter
/// turns `refs.enc` into `refs.new`, which is a different file from the one
/// being replaced and renames into the wrong place.
fn rotating_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".rotating");
    path.with_file_name(name)
}

/// The contents of `path` in the clear, whichever key era it belongs to.
///
/// Not everything under `.lit` is ciphertext even in an encrypted repository:
/// `lit init` writes `HEAD` before `encryption.toml` can say to encrypt it, and
/// it stays that way until something moves HEAD. Failing the rotation over that
/// would make the command unusable on any repository that had not switched
/// branches. Such a file is passed through and re-written encrypted, which is
/// also how it stops being plaintext.
fn plaintext_of(path: &Path, old_manager: &EncryptionManager) -> Result<Vec<u8>, String> {
    let raw = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    if !EncryptionManager::is_encrypted_payload(&raw) {
        return Ok(raw);
    }

    old_manager
        .decrypt(&raw)
        .map_err(|e| format!("Failed to decrypt {}: {}", path.display(), e))
}

/// Every whole-file blob an encrypted repository writes through the cipher.
///
/// The index and HEAD were always here. `refs.enc` — the encrypted ref index
/// that holds every branch and tag, and which is the only copy once refs stop
/// being loose files — was not, so rotation left the whole ref namespace
/// encrypted under the old key.
fn encrypted_blobs(repo_path: &Path) -> Vec<PathBuf> {
    let lit = repo_path.join(".lit");
    let mut paths = vec![lit.join("index"), lit.join("HEAD"), lit.join("refs.enc")];

    // Loose refs, for a repository that predates the encrypted ref index.
    let refs_dir = lit.join("refs");
    if refs_dir.exists() {
        paths.extend(
            walkdir::WalkDir::new(&refs_dir)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .map(|e| e.path().to_path_buf()),
        );
    }

    paths.retain(|p| p.exists());
    paths
}

/// Rotate to `new_passphrase`, taking both passphrases as given.
///
/// Separate from [`rotate_key`] because the prompts made the actual rotation
/// untestable — the one test this command had covered the case where
/// encryption is switched off, and the path that does the work had never run.
pub fn rotate_with_passphrases(
    old_passphrase: &str,
    new_passphrase: &str,
) -> Result<RotateKeyResponse, crate::errors::LitError> {
    let repo_path = refs::find_repo_root()?;
    let config = EncryptionConfig::load(&repo_path)?;

    if !config.enabled {
        return Err("Encryption is not enabled for this repository".into());
    }

    let mut old_manager = EncryptionManager::new(config.clone());
    old_manager.initialize(old_passphrase)?;

    // The new key is derived here rather than by initializing a manager from
    // the key file, which still holds the old salt.
    let new_salt = EncryptionKey::generate_salt();
    let new_key = EncryptionKey::from_passphrase(new_passphrase, &new_salt)?;
    let new_manager = EncryptionManager::from_key(config.clone(), &new_key)?;

    // --- Read everything under the old key ---

    let objects_dir = repo_path.join(".lit").join("objects");
    let mut objects = Vec::new();
    if objects_dir.exists() {
        for entry in walkdir::WalkDir::new(&objects_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            // Leftovers from a run that died before the renames. They are not
            // objects, and decrypting them would fail the whole rotation.
            .filter(|e| e.path().extension().is_none_or(|ext| ext != "rotating"))
        {
            let path = entry.path().to_path_buf();
            let plaintext = plaintext_of(&path, &old_manager)?;
            objects.push((path, plaintext));
        }
    }

    let mut blobs = Vec::new();
    for path in encrypted_blobs(&repo_path) {
        let plaintext = plaintext_of(&path, &old_manager)?;
        blobs.push((path, plaintext));
    }

    // Packed objects are encrypted individually inside the pack, and a pack
    // cannot be rewritten in place without recomputing every offset and its
    // index. Rotation therefore explodes packs back to loose objects under the
    // new key, exactly as `migrate-encryption` does; `lit gc` packs them again.
    // Leaving them alone — which is what happened before — left every packed
    // object readable only by a key that no longer existed anywhere.
    let packs_dir = pack::packs_dir(&repo_path);
    let packed = pack::load_all(&packs_dir);
    let mut unpacked = Vec::new();
    for (hash, (pack_path, offset)) in &packed {
        let object: crate::core::Object = pack::read_pack_object(pack_path, *offset, &old_manager)
            .map_err(|e| format!("Failed to read {} from its pack: {}", &hash[..8], e))?;
        unpacked.push(object);
    }

    let objects_count = objects.len() + unpacked.len();
    // HEAD and the ref index are references; the staging index is not one, and
    // counting it as one overstates what was rotated.
    let refs_count = blobs
        .iter()
        .filter(|(p, _)| p.file_name().is_some_and(|n| n != "index"))
        .count();

    // --- Write everything under the new key, beside the originals ---

    for (path, plaintext) in &objects {
        let encrypted = new_manager.encrypt(plaintext)?;
        let temp = rotating_path(path);
        fs::write(&temp, encrypted)
            .map_err(|e| format!("Failed to write {}: {}", temp.display(), e))?;
    }

    for (path, plaintext) in &blobs {
        let encrypted = new_manager.encrypt(plaintext)?;
        let temp = rotating_path(path);
        fs::write(&temp, encrypted)
            .map_err(|e| format!("Failed to write {}: {}", temp.display(), e))?;
    }

    // Objects out of a pack have no loose file to replace, so they are written
    // straight to their final path. Nothing reads them until the pack is gone.
    if !unpacked.is_empty() {
        let store = ObjectStore::with_encryption_manager(&repo_path, new_manager);
        for object in &unpacked {
            store.write(object)?;
        }
    }

    // --- Commit ---

    let expanded = shellexpand::tilde(&config.key_file);
    let key_file_path = Path::new(expanded.as_ref());
    let backup_key_path = key_file_path.with_extension("bak");
    if key_file_path.exists() {
        fs::copy(key_file_path, &backup_key_path)
            .map_err(|e| format!("Failed to backup key file: {}", e))?;
    }

    new_key.save(expanded.as_ref(), new_passphrase)?;

    // Past this point the key file already describes the new passphrase, so a
    // failure here leaves some files rotated and some not. There is no way back
    // to a consistent repository by unwinding — what recovers it is finishing.
    // Say where the pieces are rather than leaving that to be worked out.
    for (path, _) in objects.iter().chain(blobs.iter()) {
        let temp = rotating_path(path);
        fs::rename(&temp, path).map_err(|e| {
            format!(
                "Failed to replace {} while finishing rotation: {}. The key file now \
                 holds the new passphrase and some files have been replaced. Every \
                 remaining `.rotating` file under .lit is that file's new-passphrase \
                 content and needs to be renamed over the original; the previous key \
                 file is at {}.",
                path.display(),
                e,
                backup_key_path.display()
            )
        })?;
    }

    for entry in walkdir::WalkDir::new(&packs_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        fs::remove_file(entry.path())
            .map_err(|e| format!("Failed to remove {}: {}", entry.path().display(), e))?;
    }

    if backup_key_path.exists() {
        fs::remove_file(&backup_key_path).ok();
    }

    // Both caches key off the old passphrase, and the derived-key cache is
    // consulted before the key file is even opened. Left in place, the rest of
    // this process would keep encrypting with the key that was just retired.
    clear_passphrase_cache();
    clear_derived_key_cache();

    Ok(RotateKeyResponse {
        objects_rotated: objects_count,
        refs_rotated: refs_count,
    })
}
