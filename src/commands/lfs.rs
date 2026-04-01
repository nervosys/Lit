use crate::core::{find_repo_root, ObjectHash};
use crate::response::{LfsMigrateResponse, LfsTrackResponse};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// LFS pointer file content format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfsPointer {
    /// LFS spec version
    pub version: String,
    /// SHA3+BLAKE3 hash of the original content
    pub oid: String,
    /// Original file size in bytes
    pub size: u64,
}

impl LfsPointer {
    /// Create a new LFS pointer from content
    pub fn from_content(content: &[u8]) -> Self {
        let hash = ObjectHash::from_bytes(content);
        LfsPointer {
            version: "https://lit-lfs.spec/v1".to_string(),
            oid: hash.as_str().to_string(),
            size: content.len() as u64,
        }
    }

    /// Serialize to pointer file text
    pub fn to_pointer_text(&self) -> String {
        format!(
            "version {}\noid sha3-blake3:{}\nsize {}\n",
            self.version, self.oid, self.size
        )
    }

    /// Parse from pointer file text
    pub fn from_pointer_text(text: &str) -> Option<Self> {
        let mut version = None;
        let mut oid = None;
        let mut size = None;

        for line in text.lines() {
            if let Some(v) = line.strip_prefix("version ") {
                version = Some(v.to_string());
            } else if let Some(o) = line.strip_prefix("oid sha3-blake3:") {
                oid = Some(o.to_string());
            } else if let Some(s) = line.strip_prefix("size ") {
                size = s.parse::<u64>().ok();
            }
        }

        Some(LfsPointer {
            version: version?,
            oid: oid?,
            size: size?,
        })
    }

    /// Check if a byte slice looks like an LFS pointer
    pub fn is_pointer(data: &[u8]) -> bool {
        if let Ok(text) = std::str::from_utf8(data) {
            text.starts_with("version ") && text.contains("oid sha3-blake3:")
        } else {
            false
        }
    }
}

/// Get LFS tracking patterns from .litattributes
fn load_track_patterns(repo_root: &Path) -> Vec<String> {
    let attrs_path = repo_root.join(".litattributes");
    if let Ok(content) = fs::read_to_string(&attrs_path) {
        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.contains("filter=lfs") {
                    // Format: "pattern filter=lfs diff=lfs merge=lfs -text"
                    line.split_whitespace().next().map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

/// Save LFS tracking patterns to .litattributes
fn save_track_patterns(
    repo_root: &Path,
    patterns: &[String],
) -> Result<(), crate::errors::LitError> {
    let attrs_path = repo_root.join(".litattributes");

    // Read existing non-LFS lines
    let existing = if attrs_path.exists() {
        fs::read_to_string(&attrs_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut lines: Vec<String> = existing
        .lines()
        .filter(|l| !l.contains("filter=lfs"))
        .map(|s| s.to_string())
        .collect();

    // Add LFS patterns
    for pat in patterns {
        lines.push(format!("{} filter=lfs diff=lfs merge=lfs -text", pat));
    }

    fs::write(&attrs_path, lines.join("\n") + "\n")
        .map_err(|e| format!("Failed to write .litattributes: {}", e).into())
}

/// Check if a file matches any LFS pattern
fn matches_lfs_pattern(file_path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if pattern.starts_with("*.") {
            let ext = &pattern[1..]; // ".ext"
            if file_path.ends_with(ext) {
                return true;
            }
        } else if file_path == pattern {
            return true;
        }
    }
    false
}

/// Store a large file in LFS storage
fn store_lfs_object(
    repo_root: &Path,
    content: &[u8],
) -> Result<LfsPointer, crate::errors::LitError> {
    let pointer = LfsPointer::from_content(content);

    // Store in .lit/lfs/objects/{oid[..4]}/{oid[4..]}
    let lfs_dir = repo_root
        .join(".lit")
        .join("lfs")
        .join("objects")
        .join(&pointer.oid[..4.min(pointer.oid.len())]);
    fs::create_dir_all(&lfs_dir).map_err(|e| format!("Failed to create LFS dir: {}", e))?;

    let oid_rest = if pointer.oid.len() > 4 {
        &pointer.oid[4..]
    } else {
        "data"
    };
    let obj_path = lfs_dir.join(oid_rest);

    // Write raw (uncompressed) for direct streaming access
    fs::write(&obj_path, content).map_err(|e| format!("Failed to write LFS object: {}", e))?;

    Ok(pointer)
}

/// Retrieve a large file from LFS storage
pub fn read_lfs_object(
    repo_root: &Path,
    pointer: &LfsPointer,
) -> Result<Vec<u8>, crate::errors::LitError> {
    let oid_prefix = &pointer.oid[..4.min(pointer.oid.len())];
    let oid_rest = if pointer.oid.len() > 4 {
        &pointer.oid[4..]
    } else {
        "data"
    };
    let obj_path = repo_root
        .join(".lit")
        .join("lfs")
        .join("objects")
        .join(oid_prefix)
        .join(oid_rest);

    fs::read(&obj_path).map_err(|e| format!("Failed to read LFS object: {}", e).into())
}

/// Execute `lfs track` — add tracking patterns for large files
pub fn execute_track(patterns: Vec<String>) -> Result<LfsTrackResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    let mut current = load_track_patterns(&repo_root);
    let mut added = Vec::new();

    for pat in &patterns {
        if !current.contains(pat) {
            current.push(pat.clone());
            added.push(pat.clone());
        }
    }

    save_track_patterns(&repo_root, &current)?;

    Ok(LfsTrackResponse {
        patterns: current,
        message: if added.is_empty() {
            "No new patterns added".to_string()
        } else {
            format!("Tracking {} new pattern(s)", added.len())
        },
    })
}

/// Execute `lfs migrate` — convert existing large files to LFS pointers
pub fn execute_migrate(
    threshold: Option<u64>,
) -> Result<LfsMigrateResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let threshold = threshold.unwrap_or(10 * 1024 * 1024); // 10MB default

    let patterns = load_track_patterns(&repo_root);
    let mut files_migrated = 0u64;
    let mut bytes_saved = 0u64;

    // Walk the working tree
    for entry in walkdir::WalkDir::new(&repo_root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".lit" && name != ".git" && name != "target"
        })
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let rel_path = path
            .strip_prefix(&repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");

        // Check size threshold or pattern match
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let should_migrate = meta.len() >= threshold || matches_lfs_pattern(&rel_path, &patterns);

        if !should_migrate {
            continue;
        }

        // Read file content
        let content = match fs::read(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Skip if already an LFS pointer
        if LfsPointer::is_pointer(&content) {
            continue;
        }

        let original_size = content.len() as u64;

        // Store in LFS
        let pointer = store_lfs_object(&repo_root, &content)?;

        // Replace file with pointer
        let pointer_text = pointer.to_pointer_text();
        fs::write(path, &pointer_text)
            .map_err(|e| format!("Failed to write pointer for {}: {}", rel_path, e))?;

        bytes_saved += original_size - pointer_text.len() as u64;
        files_migrated += 1;
    }

    Ok(LfsMigrateResponse {
        files_migrated,
        bytes_saved,
        message: format!(
            "Migrated {} file(s) to LFS, saved {} bytes",
            files_migrated, bytes_saved
        ),
    })
}
