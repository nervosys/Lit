use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

/// Magic header for binary index files
const BINARY_INDEX_MAGIC: &[u8; 4] = b"LITX";
/// Binary index version
const BINARY_INDEX_VERSION: u32 = 1;

/// Binary index entry — fixed-format, fast load/save replacement for JSON index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryIndexEntry {
    pub path: String,
    pub hash: String,
    pub mode: String,
    /// File size in bytes
    pub size: u64,
    /// Modification timestamp (unix epoch)
    pub mtime: i64,
}

/// Binary index — high-performance staging area
#[derive(Debug, Clone)]
pub struct BinaryIndex {
    pub entries: HashMap<String, BinaryIndexEntry>,
}

impl Default for BinaryIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl BinaryIndex {
    pub fn new() -> Self {
        BinaryIndex {
            entries: HashMap::new(),
        }
    }

    /// Add or update an entry
    pub fn add(
        &mut self,
        path: String,
        hash: String,
        mode: String,
        size: u64,
        mtime: i64,
    ) {
        self.entries.insert(
            path.clone(),
            BinaryIndexEntry {
                path,
                hash,
                mode,
                size,
                mtime,
            },
        );
    }

    /// Remove an entry
    pub fn remove(&mut self, path: &str) -> Option<BinaryIndexEntry> {
        self.entries.remove(path)
    }

    /// Get sorted entries
    pub fn sorted_entries(&self) -> Vec<&BinaryIndexEntry> {
        let mut entries: Vec<&BinaryIndexEntry> = self.entries.values().collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        entries
    }

    /// Save to binary format
    ///
    /// Format:
    ///   Header: LITX(4) + version(4) + entry_count(4)
    ///   Each entry:
    ///     path_len(4) + path_bytes + hash_len(4) + hash_bytes +
    ///     mode_len(2) + mode_bytes + size(8) + mtime(8)
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut buf: Vec<u8> = Vec::new();

        // Header
        buf.extend_from_slice(BINARY_INDEX_MAGIC);
        buf.write_u32::<BigEndian>(BINARY_INDEX_VERSION)
            .map_err(|e| format!("Write error: {}", e))?;
        buf.write_u32::<BigEndian>(self.entries.len() as u32)
            .map_err(|e| format!("Write error: {}", e))?;

        // Write entries sorted by path for deterministic output
        let sorted = self.sorted_entries();
        for entry in &sorted {
            // path
            let path_bytes = entry.path.as_bytes();
            buf.write_u32::<BigEndian>(path_bytes.len() as u32)
                .map_err(|e| format!("Write error: {}", e))?;
            buf.extend_from_slice(path_bytes);

            // hash
            let hash_bytes = entry.hash.as_bytes();
            buf.write_u32::<BigEndian>(hash_bytes.len() as u32)
                .map_err(|e| format!("Write error: {}", e))?;
            buf.extend_from_slice(hash_bytes);

            // mode
            let mode_bytes = entry.mode.as_bytes();
            buf.write_u16::<BigEndian>(mode_bytes.len() as u16)
                .map_err(|e| format!("Write error: {}", e))?;
            buf.extend_from_slice(mode_bytes);

            // size + mtime
            buf.write_u64::<BigEndian>(entry.size)
                .map_err(|e| format!("Write error: {}", e))?;
            buf.write_i64::<BigEndian>(entry.mtime)
                .map_err(|e| format!("Write error: {}", e))?;
        }

        fs::write(path, &buf).map_err(|e| format!("Failed to write binary index: {}", e))
    }

    /// Load from binary format
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(BinaryIndex::new());
        }

        let data = fs::read(path).map_err(|e| format!("Failed to read binary index: {}", e))?;
        let mut cursor = Cursor::new(&data);

        // Verify header
        let mut magic = [0u8; 4];
        cursor
            .read_exact(&mut magic)
            .map_err(|e| format!("Read error: {}", e))?;
        if &magic != BINARY_INDEX_MAGIC {
            return Err("Invalid binary index magic".to_string());
        }

        let version = cursor
            .read_u32::<BigEndian>()
            .map_err(|e| format!("Read error: {}", e))?;
        if version != BINARY_INDEX_VERSION {
            return Err(format!("Unsupported binary index version: {}", version));
        }

        let count = cursor
            .read_u32::<BigEndian>()
            .map_err(|e| format!("Read error: {}", e))? as usize;

        let mut entries = HashMap::with_capacity(count);

        for _ in 0..count {
            // path
            let path_len = cursor
                .read_u32::<BigEndian>()
                .map_err(|e| format!("Read error: {}", e))? as usize;
            let mut path_buf = vec![0u8; path_len];
            cursor
                .read_exact(&mut path_buf)
                .map_err(|e| format!("Read error: {}", e))?;
            let path_str = String::from_utf8(path_buf)
                .map_err(|e| format!("Invalid path UTF-8: {}", e))?;

            // hash
            let hash_len = cursor
                .read_u32::<BigEndian>()
                .map_err(|e| format!("Read error: {}", e))? as usize;
            let mut hash_buf = vec![0u8; hash_len];
            cursor
                .read_exact(&mut hash_buf)
                .map_err(|e| format!("Read error: {}", e))?;
            let hash_str = String::from_utf8(hash_buf)
                .map_err(|e| format!("Invalid hash UTF-8: {}", e))?;

            // mode
            let mode_len = cursor
                .read_u16::<BigEndian>()
                .map_err(|e| format!("Read error: {}", e))? as usize;
            let mut mode_buf = vec![0u8; mode_len];
            cursor
                .read_exact(&mut mode_buf)
                .map_err(|e| format!("Read error: {}", e))?;
            let mode_str = String::from_utf8(mode_buf)
                .map_err(|e| format!("Invalid mode UTF-8: {}", e))?;

            // size + mtime
            let size = cursor
                .read_u64::<BigEndian>()
                .map_err(|e| format!("Read error: {}", e))?;
            let mtime = cursor
                .read_i64::<BigEndian>()
                .map_err(|e| format!("Read error: {}", e))?;

            entries.insert(
                path_str.clone(),
                BinaryIndexEntry {
                    path: path_str,
                    hash: hash_str,
                    mode: mode_str,
                    size,
                    mtime,
                },
            );
        }

        Ok(BinaryIndex { entries })
    }

    /// Convert from the JSON-based Index
    pub fn from_json_index(index: &crate::storage::Index) -> Self {
        let mut binary = BinaryIndex::new();
        for entry in index.entries.values() {
            binary.add(
                entry.path.clone(),
                entry.hash.clone(),
                entry.mode.clone(),
                0, // size not tracked in old format
                0, // mtime not tracked in old format
            );
        }
        binary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_binary_index_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let index_path = temp_dir.path().join("index.bin");

        let mut index = BinaryIndex::new();
        index.add(
            "src/main.rs".to_string(),
            "abc123def456".to_string(),
            "100644".to_string(),
            1024,
            1700000000,
        );
        index.add(
            "README.md".to_string(),
            "789xyz".to_string(),
            "100644".to_string(),
            256,
            1700000100,
        );

        index.save(&index_path).unwrap();
        let loaded = BinaryIndex::load(&index_path).unwrap();

        assert_eq!(loaded.entries.len(), 2);
        assert!(loaded.entries.contains_key("src/main.rs"));
        assert!(loaded.entries.contains_key("README.md"));
        assert_eq!(loaded.entries["src/main.rs"].size, 1024);
        assert_eq!(loaded.entries["README.md"].mtime, 1700000100);
    }
}
