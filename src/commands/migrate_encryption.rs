//! Encrypt a repository that was created before encryption was switched on.
//!
//! Turning `enabled = true` on an existing repository used to leave it
//! unreadable: the index and objects already on disk carry no encryption
//! header, so every command failed. This walks that content and encrypts it in
//! place, which is the step that was missing.
//!
//! The walk is per file and idempotent. Anything already encrypted is left
//! alone, so an interrupted run is finished by running it again rather than
//! leaving the repository half-converted.

use crate::core::{find_repo_root, Object};
use crate::crypto::encryption::{EncryptionConfig, EncryptionManager};
use crate::response::MigrateEncryptionResponse;
use crate::storage::{pack, ObjectStore};
use std::fs;
use std::path::{Path, PathBuf};

/// First byte of anything this repository encrypted.
const ENCRYPTION_VERSION: u8 = 1;

/// Whether `data` has already been through the cipher.
///
/// The version byte alone would be a guess — a zlib stream could in principle
/// begin with it — so the header is confirmed by actually decrypting.
fn already_encrypted(data: &[u8], encryption: &EncryptionManager) -> bool {
    data.first() == Some(&ENCRYPTION_VERSION) && encryption.decrypt(data).is_ok()
}

/// Encrypt one file in place unless it already is.
///
/// Returns whether anything was written. The temporary file and rename keep a
/// crash from leaving a half-written object behind; the original stays intact
/// until the replacement is complete.
fn encrypt_file(
    path: &Path,
    encryption: &EncryptionManager,
) -> Result<bool, crate::errors::LitError> {
    let data = fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    if already_encrypted(&data, encryption) {
        return Ok(false);
    }

    let encrypted = encryption.encrypt(&data)?;
    let temp = path.with_extension("migrating");
    fs::write(&temp, &encrypted)
        .map_err(|e| format!("Failed to write {}: {}", temp.display(), e))?;
    fs::rename(&temp, path).map_err(|e| format!("Failed to replace {}: {}", path.display(), e))?;

    Ok(true)
}

/// Every regular file under `dir`, if it exists.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Encrypt an existing repository in place.
pub fn execute() -> Result<MigrateEncryptionResponse, crate::errors::LitError> {
    let repo = find_repo_root()?;
    let config = EncryptionConfig::load(&repo)?;

    if !config.enabled {
        return Err(
            "Encryption is not enabled for this repository. Set enabled = true in \
                    .lit/encryption.toml first."
                .into(),
        );
    }

    // Needs a real key: the whole job is writing ciphertext.
    let encryption = EncryptionManager::new_auto(config.clone(), &repo);
    encryption.encrypt(b"probe")?;

    let lit = repo.join(".lit");
    let mut objects_encrypted = 0usize;
    let mut already = 0usize;

    // Everything the normal write path encrypts: loose objects, the index,
    // refs and HEAD.
    //
    // Refs were excluded while `write_ref` stored them in the clear —
    // encrypting them then produced files `read_ref` could not read, and
    // `branch` and `show` broke. Now that both sides of refs go through the
    // cipher, they migrate with the rest.
    for path in files_under(&lit.join("objects")) {
        if encrypt_file(&path, &encryption)? {
            objects_encrypted += 1;
        } else {
            already += 1;
        }
    }

    let mut refs_encrypted = 0usize;
    for path in files_under(&lit.join("refs")) {
        if encrypt_file(&path, &encryption)? {
            refs_encrypted += 1;
        } else {
            already += 1;
        }
    }

    let head = lit.join("HEAD");
    if head.exists() {
        if encrypt_file(&head, &encryption)? {
            refs_encrypted += 1;
        } else {
            already += 1;
        }
    }

    let index = lit.join("index");
    let index_encrypted = if index.exists() {
        encrypt_file(&index, &encryption)?
    } else {
        false
    };
    if index.exists() && !index_encrypted {
        already += 1;
    }

    // Packs last, so the objects written here are not walked again above.
    //
    // A pack written before encryption holds plain zlib payloads, and rewriting
    // one in place would mean recomputing every entry offset and its index.
    // Exploding it back to loose objects is simpler and self-correcting: they
    // go through the encrypted store, and `gc` can pack them again afterwards.
    let packs_dir = pack::packs_dir(&repo);
    let packed = pack::load_all(&packs_dir);
    let mut objects_unpacked = 0usize;
    let mut packs_expanded = 0usize;

    if !packed.is_empty() {
        let plaintext = EncryptionManager::new(EncryptionConfig {
            enabled: false,
            ..config.clone()
        });
        let store = ObjectStore::new(&repo);

        for (hash, (pack_path, offset)) in &packed {
            // A pack already encrypted reads through the real manager instead.
            let object: Object = pack::read_pack_object(pack_path, *offset, &plaintext)
                .or_else(|_| pack::read_pack_object(pack_path, *offset, &encryption))
                .map_err(|e| format!("Failed to read {} from its pack: {}", &hash[..8], e))?;

            store.write(&object)?;
            objects_unpacked += 1;
        }

        for entry in files_under(&packs_dir) {
            fs::remove_file(&entry)
                .map_err(|e| format!("Failed to remove {}: {}", entry.display(), e))?;
            packs_expanded += 1;
        }
    }

    Ok(MigrateEncryptionResponse {
        objects_encrypted,
        objects_unpacked,
        refs_encrypted,
        index_encrypted,
        packs_expanded,
        already_encrypted: already,
        message: format!(
            "Encrypted {} loose objects, {} unpacked from {} pack files and {} refs; \
             index {}; {} already encrypted",
            objects_encrypted,
            objects_unpacked,
            packs_expanded,
            refs_encrypted,
            if index_encrypted {
                "encrypted"
            } else {
                "left as it was"
            },
            already
        ),
    })
}
