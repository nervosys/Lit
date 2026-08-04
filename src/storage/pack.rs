//! Pack file format — many objects in one file, with a hash→offset index.
//!
//! A pack is written by `lit gc` and read back through [`ObjectStore`], which
//! consults packs whenever a hash has no loose object on disk. Both sides live
//! here so the format is defined in one place: a reader and a writer that drift
//! apart is how packed objects become unreadable.
//!
//! Layout, all integers big-endian:
//!
//! ```text
//! pack:  "LITP" version:u32 count:u32
//!        then per object: type:u8 uncompressed_len:u64 compressed_len:u64 zlib-data
//! index: "LITI" version:u32 count:u32
//!        then per object, sorted by hash: hash_len:u32 hash offset:u64 crc32:u32
//! ```
//!
//! [`ObjectStore`]: crate::storage::ObjectStore

use crate::core::{Object, ObjectHash};
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
pub const PACK_MAGIC: &[u8; 4] = b"LITP";
/// Pack version
pub const PACK_VERSION: u32 = 1;

/// Index magic header
pub const INDEX_MAGIC: &[u8; 4] = b"LITI";
/// Index version
pub const INDEX_VERSION: u32 = 1;

/// Pack index entry - maps hash to offset in pack
#[derive(Debug, Clone)]
pub struct PackIndexEntry {
    pub hash: ObjectHash,
    pub offset: u64,
    pub crc32: u32,
}

/// The directory holding a repository's packs.
pub fn packs_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".lit").join("packs")
}

/// Write a pack file from a set of objects
pub fn write_pack(
    objects: &[(ObjectHash, Object)],
    pack_path: &Path,
) -> Result<Vec<PackIndexEntry>, crate::errors::LitError> {
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
pub fn write_pack_index(
    entries: &[PackIndexEntry],
    index_path: &Path,
) -> Result<(), crate::errors::LitError> {
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
pub fn read_pack_object(pack_path: &Path, offset: u64) -> Result<Object, crate::errors::LitError> {
    let pack_data = fs::read(pack_path).map_err(|e| format!("Failed to read pack: {}", e))?;
    let mut cursor = Cursor::new(&pack_data);
    cursor.set_position(offset);

    // Read type
    let _type_byte = cursor.read_u8().map_err(|e| format!("Read error: {}", e))?;

    // Read sizes
    let _uncompressed_size = cursor
        .read_u64::<BigEndian>()
        .map_err(|e| format!("Read error: {}", e))?;
    let compressed_size = cursor
        .read_u64::<BigEndian>()
        .map_err(|e| format!("Read error: {}", e))?;

    // Read compressed data
    let pos = cursor.position() as usize;
    let end = pos
        .checked_add(compressed_size as usize)
        .ok_or("Pack entry length overflows")?;
    if end > pack_data.len() {
        return Err("Pack data truncated".into());
    }
    let compressed = &pack_data[pos..end];

    // Decompress
    let mut decoder = ZlibDecoder::new(compressed);
    let mut raw = Vec::new();
    decoder
        .read_to_end(&mut raw)
        .map_err(|e| format!("Decompress error: {}", e))?;

    Object::from_bytes(&raw).map_err(Into::into)
}

/// Load a pack index and build a hash→(pack_path, offset) map
pub fn load_pack_index(index_path: &Path) -> Result<HashMap<String, (PathBuf, u64)>, String> {
    let data = fs::read(index_path).map_err(|e| format!("Failed to read index: {}", e))?;
    let mut cursor = Cursor::new(&data);

    // Verify header
    let mut magic = [0u8; 4];
    cursor
        .read_exact(&mut magic)
        .map_err(|e| format!("Read error: {}", e))?;
    if &magic != INDEX_MAGIC {
        return Err("Invalid pack index magic".into());
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
            return Err("Index data truncated".into());
        }
        let hash_str = String::from_utf8(data[pos..pos + hash_len].to_vec())
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

/// Merge every pack index in `packs_dir` into one hash→(pack, offset) map.
///
/// A pack whose index will not parse is skipped rather than failing the whole
/// lookup: one damaged pack should not make the objects in the others
/// unreachable. Missing directory means no packs, which is not an error.
pub fn load_all(packs_dir: &Path) -> HashMap<String, (PathBuf, u64)> {
    let mut map = HashMap::new();

    let entries = match fs::read_dir(packs_dir) {
        Ok(entries) => entries,
        Err(_) => return map,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("idx") {
            continue;
        }
        match load_pack_index(&path) {
            Ok(entries) => map.extend(entries),
            Err(e) => eprintln!("Warning: ignoring unreadable pack index {:?}: {}", path, e),
        }
    }

    map
}
