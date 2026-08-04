use crate::core::{find_repo_root, Object, ObjectHash};
use crate::response::GcResponse;
use crate::storage::ObjectStore;
use std::fs;

// The pack format lives in `storage::pack`, next to the ObjectStore that reads
// it back. These re-exports keep the previously public paths resolving.
pub use crate::storage::pack::{
    load_pack_index, read_pack_object, write_pack, write_pack_index, PackIndexEntry, INDEX_MAGIC,
    INDEX_VERSION, PACK_MAGIC, PACK_VERSION,
};

/// Execute the `gc` (garbage collection) command.
/// Packs all loose objects into a single pack file and removes the loose files.
pub fn execute() -> Result<GcResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let all_hashes = store
        .list()
        .map_err(|e| format!("Failed to list objects: {}", e))?;

    if all_hashes.is_empty() {
        return Ok(GcResponse {
            objects_packed: 0,
            packs_created: 0,
            loose_removed: 0,
            bytes_saved: 0,
            message: "No objects to pack".to_string(),
        });
    }

    // Read all loose objects
    let mut objects: Vec<(ObjectHash, Object)> = Vec::new();
    let mut total_loose_bytes: u64 = 0;
    for hash in &all_hashes {
        let obj = store.read(hash)?;
        // Track size of loose file
        let loose_path = repo_root
            .join(".lit")
            .join("objects")
            .join(&hash.as_str()[..4])
            .join(&hash.as_str()[4..]);
        if let Ok(meta) = fs::metadata(&loose_path) {
            total_loose_bytes += meta.len();
        }
        objects.push((hash.clone(), obj));
    }

    // Create packs directory
    let packs_dir = repo_root.join(".lit").join("packs");
    fs::create_dir_all(&packs_dir)
        .map_err(|e| format!("Failed to create packs directory: {}", e))?;

    // Generate pack name from timestamp
    let pack_name = format!("pack-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"));
    let pack_path = packs_dir.join(format!("{}.pack", pack_name));
    let index_path = packs_dir.join(format!("{}.idx", pack_name));

    // Write pack and index
    // Pack through the store's own encryption manager, so a packed object is
    // protected exactly as the loose one it replaces was.
    let encryption = store.encryption();
    let index_entries = {
        let encryption = encryption
            .lock()
            .map_err(|e| format!("Failed to acquire encryption lock: {}", e))?;
        write_pack(&objects, &pack_path, &encryption)?
    };
    write_pack_index(&index_entries, &index_path)?;

    let pack_bytes = fs::metadata(&pack_path).map(|m| m.len()).unwrap_or(0);
    let index_bytes = fs::metadata(&index_path).map(|m| m.len()).unwrap_or(0);

    // Remove loose objects
    let mut loose_removed = 0u64;
    for hash in &all_hashes {
        let dir = hash.as_str()[..4].to_string();
        let file = hash.as_str()[4..].to_string();
        let loose_path = repo_root
            .join(".lit")
            .join("objects")
            .join(&dir)
            .join(&file);
        if loose_path.exists() {
            if fs::remove_file(&loose_path).is_ok() {
                loose_removed += 1;
            }
            // Clean up empty shard dirs
            let shard_dir = repo_root.join(".lit").join("objects").join(&dir);
            if let Ok(mut entries) = fs::read_dir(&shard_dir) {
                if entries.next().is_none() {
                    let _ = fs::remove_dir(&shard_dir);
                }
            }
        }
    }

    let bytes_saved = if total_loose_bytes > (pack_bytes + index_bytes) {
        total_loose_bytes - pack_bytes - index_bytes
    } else {
        0
    };

    Ok(GcResponse {
        objects_packed: objects.len() as u64,
        packs_created: 1,
        loose_removed,
        bytes_saved,
        message: format!(
            "Packed {} objects into {} ({} bytes saved)",
            objects.len(),
            pack_path.display(),
            bytes_saved
        ),
    })
}
