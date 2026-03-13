use crate::core::{find_repo_root, Object, ObjectHash};
use crate::response::GcResponse;
use crate::storage::ObjectStore;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use crc32fast::Hasher as Crc32Hasher;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

/// Magic header for Lit pack files
const PACK_MAGIC: &[u8; 4] = b"LITP";
/// Pack version
const PACK_VERSION: u32 = 1;

/// Index magic header
const INDEX_MAGIC: &[u8; 4] = b"LITI";
/// Index version
const INDEX_VERSION: u32 = 1;

/// Pack file entry - one object in the pack
#[derive(Debug, Clone)]
pub struct PackEntry {
    /// Object type: 1=blob, 2=tree, 3=commit, 4=tag
    pub obj_type: u8,
    /// Uncompressed size
    pub size: u64,
    /// Compressed data
    pub data: Vec<u8>,
    /// CRC32 of the compressed data
    pub crc32: u32,
}

/// Pack index entry - maps hash to offset in pack
#[derive(Debug, Clone)]
pub struct PackIndexEntry {
    pub hash: ObjectHash,
    pub offset: u64,
    pub crc32: u32,
}

/// Write a pack file from a set of objects
pub fn write_pack(
    objects: &[(ObjectHash, Object)],
    pack_path: &Path,
) -> Result<Vec<PackIndexEntry>, String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut index_entries = Vec::new();

    // Header: magic + version + count
    buf.extend_from_slice(PACK_MAGIC);
    buf.write_u32::<BigEndian>(PACK_VERSION)
        .map_err(|e| format!("Write error: {}", e))?;
    buf.write_u32::<BigEndian>(objects.len() as u32)
        .map_err(|e| format!("Write error: {}", e))?;

    for (hash, obj) in objects {
        let offset = buf.len() as u64;

        let type_byte = match obj {
            Object::Blob(_) => 1u8,
            Object::Tree(_) => 2u8,
            Object::Commit(_) => 3u8,
            Object::Tag(_) => 4u8,
        };

        let raw = obj.to_bytes();
        let uncompressed_size = raw.len() as u64;

        // Compress data
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(&raw)
            .map_err(|e| format!("Compress error: {}", e))?;
        let compressed = encoder
            .finish()
            .map_err(|e| format!("Compress finish error: {}", e))?;

        // CRC32 of compressed data
        let mut crc32 = Crc32Hasher::new();
        crc32.update(&compressed);
        let crc_val = crc32.finalize();

        // Write entry header: type(1) + uncompressed_size(8) + compressed_size(8)
        buf.push(type_byte);
        buf.write_u64::<BigEndian>(uncompressed_size)
            .map_err(|e| format!("Write error: {}", e))?;
        buf.write_u64::<BigEndian>(compressed.len() as u64)
            .map_err(|e| format!("Write error: {}", e))?;
        buf.extend_from_slice(&compressed);

        index_entries.push(PackIndexEntry {
            hash: hash.clone(),
            offset,
            crc32: crc_val,
        });
    }

    fs::write(pack_path, &buf).map_err(|e| format!("Failed to write pack: {}", e))?;

    Ok(index_entries)
}

/// Write a pack index file
pub fn write_pack_index(entries: &[PackIndexEntry], index_path: &Path) -> Result<(), String> {
    let mut buf: Vec<u8> = Vec::new();

    // Header
    buf.extend_from_slice(INDEX_MAGIC);
    buf.write_u32::<BigEndian>(INDEX_VERSION)
        .map_err(|e| format!("Write error: {}", e))?;
    buf.write_u32::<BigEndian>(entries.len() as u32)
        .map_err(|e| format!("Write error: {}", e))?;

    // Write sorted entries: hash_len(4) + hash + offset(8) + crc32(4)
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.hash.as_str().cmp(b.hash.as_str()));

    for entry in &sorted {
        let hash_bytes = entry.hash.as_str().as_bytes();
        buf.write_u32::<BigEndian>(hash_bytes.len() as u32)
            .map_err(|e| format!("Write error: {}", e))?;
        buf.extend_from_slice(hash_bytes);
        buf.write_u64::<BigEndian>(entry.offset)
            .map_err(|e| format!("Write error: {}", e))?;
        buf.write_u32::<BigEndian>(entry.crc32)
            .map_err(|e| format!("Write error: {}", e))?;
    }

    fs::write(index_path, &buf).map_err(|e| format!("Failed to write index: {}", e))?;
    Ok(())
}

/// Read a single object from a pack file by offset
pub fn read_pack_object(pack_path: &Path, offset: u64) -> Result<Object, String> {
    let pack_data = fs::read(pack_path).map_err(|e| format!("Failed to read pack: {}", e))?;
    let mut cursor = Cursor::new(&pack_data);
    cursor.set_position(offset);

    // Read type
    let _type_byte = cursor
        .read_u8()
        .map_err(|e| format!("Read error: {}", e))?;

    // Read sizes
    let _uncompressed_size = cursor
        .read_u64::<BigEndian>()
        .map_err(|e| format!("Read error: {}", e))?;
    let compressed_size = cursor
        .read_u64::<BigEndian>()
        .map_err(|e| format!("Read error: {}", e))?;

    // Read compressed data
    let pos = cursor.position() as usize;
    let end = pos + compressed_size as usize;
    if end > pack_data.len() {
        return Err("Pack data truncated".to_string());
    }
    let compressed = &pack_data[pos..end];

    // Decompress
    let mut decoder = ZlibDecoder::new(compressed);
    let mut raw = Vec::new();
    decoder
        .read_to_end(&mut raw)
        .map_err(|e| format!("Decompress error: {}", e))?;

    Object::from_bytes(&raw)
}

/// Load a pack index and build a hash→(pack_path, offset) map
pub fn load_pack_index(
    index_path: &Path,
) -> Result<HashMap<String, (PathBuf, u64)>, String> {
    let data = fs::read(index_path).map_err(|e| format!("Failed to read index: {}", e))?;
    let mut cursor = Cursor::new(&data);

    // Verify header
    let mut magic = [0u8; 4];
    cursor
        .read_exact(&mut magic)
        .map_err(|e| format!("Read error: {}", e))?;
    if &magic != INDEX_MAGIC {
        return Err("Invalid pack index magic".to_string());
    }

    let _version = cursor
        .read_u32::<BigEndian>()
        .map_err(|e| format!("Read error: {}", e))?;
    let count = cursor
        .read_u32::<BigEndian>()
        .map_err(|e| format!("Read error: {}", e))?;

    let pack_path = index_path.with_extension("pack");
    let mut map = HashMap::new();

    for _ in 0..count {
        let hash_len = cursor
            .read_u32::<BigEndian>()
            .map_err(|e| format!("Read error: {}", e))? as usize;

        let pos = cursor.position() as usize;
        if pos + hash_len > data.len() {
            return Err("Index data truncated".to_string());
        }
        let hash_str =
            String::from_utf8(data[pos..pos + hash_len].to_vec())
                .map_err(|e| format!("Invalid hash UTF-8: {}", e))?;
        cursor.set_position((pos + hash_len) as u64);

        let offset = cursor
            .read_u64::<BigEndian>()
            .map_err(|e| format!("Read error: {}", e))?;
        let _crc32 = cursor
            .read_u32::<BigEndian>()
            .map_err(|e| format!("Read error: {}", e))?;

        map.insert(hash_str, (pack_path.clone(), offset));
    }

    Ok(map)
}

/// Execute the `gc` (garbage collection) command.
/// Packs all loose objects into a single pack file and removes the loose files.
pub fn execute() -> Result<GcResponse, String> {
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
    let index_entries = write_pack(&objects, &pack_path)?;
    write_pack_index(&index_entries, &index_path)?;

    let pack_bytes = fs::metadata(&pack_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let index_bytes = fs::metadata(&index_path)
        .map(|m| m.len())
        .unwrap_or(0);

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
