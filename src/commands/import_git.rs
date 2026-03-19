use crate::core::{find_repo_root, Blob, Commit, Object, ObjectHash, Tree};
use crate::response::ImportGitResponse;
use crate::storage::ObjectStore;
use sha1::Digest as Sha1Digest;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Import a Git repository into Lit format.
/// Reads Git objects (SHA-1), converts them to Lit objects (SHA3-512 + BLAKE3),
/// and imports all refs.
pub fn execute(source: String) -> Result<ImportGitResponse, crate::errors::LitError> {
    let source_path = PathBuf::from(&source);
    let git_dir = find_git_dir(&source_path)?;

    // Initialize lit repo in current directory if not already
    let repo_root = match find_repo_root() {
        Ok(r) => r,
        Err(_) => {
            crate::commands::init::execute(false, None)?;
            find_repo_root()?
        }
    };

    let store = ObjectStore::new(&repo_root);
    let mut hash_map: HashMap<String, ObjectHash> = HashMap::new();
    let mut objects_imported = 0u64;
    let mut refs_imported = 0u64;

    // Phase 1: Import all loose objects
    let objects_dir = git_dir.join("objects");
    if objects_dir.exists() {
        for entry in walkdir::WalkDir::new(&objects_dir)
            .min_depth(2)
            .max_depth(2)
        {
            let entry = entry.map_err(|e| format!("Failed to walk objects: {}", e))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            // Reconstruct the SHA-1 hex from dir/file
            if let (Some(dir_name), Some(file_name)) =
                (path.parent().and_then(|p| p.file_name()), path.file_name())
            {
                let dir_str = dir_name.to_string_lossy();
                let file_str = file_name.to_string_lossy();
                // Skip pack/info directories
                if dir_str == "pack" || dir_str == "info" {
                    continue;
                }
                let git_hash = format!("{}{}", dir_str, file_str);

                match import_loose_object(path, &git_hash, &store, &mut hash_map) {
                    Ok(_) => objects_imported += 1,
                    Err(e) => {
                        eprintln!("Warning: skipping object {}: {}", &git_hash[..8], e);
                    }
                }
            }
        }
    }

    // Phase 2: Import pack files
    let pack_dir = objects_dir.join("pack");
    if pack_dir.exists() {
        for entry in
            fs::read_dir(&pack_dir).map_err(|e| format!("Failed to read pack dir: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Pack dir entry error: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("pack") {
                match import_pack_file(&path, &store, &mut hash_map) {
                    Ok(count) => objects_imported += count,
                    Err(e) => {
                        eprintln!("Warning: skipping pack {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    // Phase 3: Import refs
    // branches
    let refs_heads = git_dir.join("refs").join("heads");
    if refs_heads.exists() {
        for entry in walkdir::WalkDir::new(&refs_heads).min_depth(1) {
            let entry = entry.map_err(|e| format!("Failed to walk refs: {}", e))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let branch_name = entry
                .path()
                .strip_prefix(&refs_heads)
                .map_err(|e| format!("Path error: {}", e))?
                .to_string_lossy()
                .replace('\\', "/");
            let git_hash = fs::read_to_string(entry.path())
                .map_err(|e| format!("Failed to read ref: {}", e))?
                .trim()
                .to_string();
            if let Some(lit_hash) = hash_map.get(&git_hash) {
                crate::core::write_ref(
                    &repo_root,
                    &format!("heads/{}", branch_name),
                    lit_hash.as_str(),
                )?;
                refs_imported += 1;
            }
        }
    }

    // tags
    let refs_tags = git_dir.join("refs").join("tags");
    if refs_tags.exists() {
        for entry in walkdir::WalkDir::new(&refs_tags).min_depth(1) {
            let entry = entry.map_err(|e| format!("Failed to walk tags: {}", e))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let tag_name = entry
                .path()
                .strip_prefix(&refs_tags)
                .map_err(|e| format!("Path error: {}", e))?
                .to_string_lossy()
                .replace('\\', "/");
            let git_hash = fs::read_to_string(entry.path())
                .map_err(|e| format!("Failed to read ref: {}", e))?
                .trim()
                .to_string();
            if let Some(lit_hash) = hash_map.get(&git_hash) {
                crate::core::write_ref(
                    &repo_root,
                    &format!("tags/{}", tag_name),
                    lit_hash.as_str(),
                )?;
                refs_imported += 1;
            }
        }
    }

    // HEAD
    let head_path = git_dir.join("HEAD");
    if head_path.exists() {
        let head_content =
            fs::read_to_string(&head_path).map_err(|e| format!("Failed to read HEAD: {}", e))?;
        let head_content = head_content.trim();
        if let Some(ref_target) = head_content.strip_prefix("ref: refs/heads/") {
            crate::core::update_head(&repo_root, ref_target)?;
        }
    }

    // Copy .gitignore as .litignore if present
    let gitignore = source_path.join(".gitignore");
    let litignore = repo_root.join(".litignore");
    if gitignore.exists() && !litignore.exists() {
        let _ = fs::copy(&gitignore, &litignore);
    }

    Ok(ImportGitResponse {
        source: source.clone(),
        objects_imported,
        refs_imported,
        hash_mapping_count: hash_map.len(),
        message: format!(
            "Imported {} objects and {} refs from Git repository",
            objects_imported, refs_imported
        ),
    })
}

/// Find the .git directory for a given path
fn find_git_dir(path: &Path) -> Result<PathBuf, crate::errors::LitError> {
    // Could be a bare repo or have .git directory
    let dot_git = path.join(".git");
    if dot_git.is_dir() {
        return Ok(dot_git);
    }
    // Bare repository — objects dir directly present
    if path.join("objects").is_dir() && path.join("refs").is_dir() {
        return Ok(path.to_path_buf());
    }
    Err(format!("Not a Git repository: {}", path.display()).into())
}

/// Import a single loose Git object
fn import_loose_object(
    path: &Path,
    _git_hash: &str,
    store: &ObjectStore,
    hash_map: &mut HashMap<String, ObjectHash>,
) -> Result<(), crate::errors::LitError> {
    let compressed = fs::read(path).map_err(|e| format!("Read error: {}", e))?;

    // Decompress zlib
    let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
    let mut raw = Vec::new();
    decoder
        .read_to_end(&mut raw)
        .map_err(|e| format!("Decompress error: {}", e))?;

    // Parse Git object format: "<type> <size>\0<content>"
    let null_pos = raw
        .iter()
        .position(|&b| b == 0)
        .ok_or("Invalid Git object: no null byte")?;
    let header = std::str::from_utf8(&raw[..null_pos]).map_err(|_| "Invalid Git object header")?;
    let content = &raw[null_pos + 1..];

    let (obj_type, _size_str) = header
        .split_once(' ')
        .ok_or("Invalid Git object header format")?;

    // Compute the original git hash for mapping
    let mut sha1 = sha1::Sha1::new();
    sha1.update(&raw);
    let git_hash_computed = hex::encode(sha1.finalize());

    let lit_obj = match obj_type {
        "blob" => Object::Blob(Blob::new(content.to_vec())),
        "tree" => {
            let tree = parse_git_tree(content, hash_map)?;
            Object::Tree(tree)
        }
        "commit" => {
            let commit = parse_git_commit(content, hash_map)?;
            Object::Commit(commit)
        }
        "tag" => {
            // Treat as blob for now — full tag object parsing is complex
            Object::Blob(Blob::new(content.to_vec()))
        }
        other => return Err(format!("Unknown object type: {}", other).into()),
    };

    let lit_hash = store.write(&lit_obj)?;
    hash_map.insert(git_hash_computed, lit_hash);
    Ok(())
}

/// Parse a Git tree object's binary content
fn parse_git_tree(content: &[u8], hash_map: &HashMap<String, ObjectHash>) -> Result<Tree, crate::errors::LitError> {
    let mut tree = Tree::new();
    let mut pos = 0;

    while pos < content.len() {
        // Format: "<mode> <name>\0<20-byte-sha1>"
        let space_pos = content[pos..]
            .iter()
            .position(|&b| b == b' ')
            .ok_or("Invalid tree entry: no space")?
            + pos;
        let null_pos = content[space_pos..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("Invalid tree entry: no null")?
            + space_pos;

        let mode = std::str::from_utf8(&content[pos..space_pos])
            .map_err(|_| "Invalid mode in tree")?
            .to_string();
        let name = std::str::from_utf8(&content[space_pos + 1..null_pos])
            .map_err(|_| "Invalid name in tree")?
            .to_string();

        if null_pos + 21 > content.len() {
            break;
        }
        let sha1_bytes = &content[null_pos + 1..null_pos + 21];
        let git_hash = hex::encode(sha1_bytes);

        // Map to lit hash or use placeholder
        let lit_hash = hash_map
            .get(&git_hash)
            .cloned()
            .unwrap_or_else(|| ObjectHash::from_hex(format!("{:0>192}", git_hash)));

        let obj_type = if mode.starts_with("40") {
            "tree"
        } else {
            "blob"
        }
        .to_string();

        tree.add_entry(mode, name, lit_hash, obj_type);
        pos = null_pos + 21;
    }

    Ok(tree)
}

/// Parse a Git commit object's text content
fn parse_git_commit(
    content: &[u8],
    hash_map: &HashMap<String, ObjectHash>,
) -> Result<Commit, crate::errors::LitError> {
    let text = std::str::from_utf8(content).map_err(|_| "Invalid commit: not UTF-8")?;

    let mut tree_hash = String::new();
    let mut parents = Vec::new();
    let mut author = String::new();
    let mut committer = String::new();
    let mut timestamp: i64 = 0;
    let mut in_body = false;
    let mut message_lines = Vec::new();

    for line in text.lines() {
        if in_body {
            message_lines.push(line);
            continue;
        }
        if line.is_empty() {
            in_body = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix("tree ") {
            tree_hash = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("parent ") {
            parents.push(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("author ") {
            let (name, ts) = parse_git_ident(rest);
            author = name;
            timestamp = ts;
        } else if let Some(rest) = line.strip_prefix("committer ") {
            let (name, _) = parse_git_ident(rest);
            committer = name;
        }
    }

    // Map git hashes to lit hashes
    let lit_tree = hash_map
        .get(&tree_hash)
        .cloned()
        .unwrap_or_else(|| ObjectHash::from_hex(format!("{:0>192}", tree_hash)));

    let lit_parents: Vec<ObjectHash> = parents
        .iter()
        .map(|p| {
            hash_map
                .get(p)
                .cloned()
                .unwrap_or_else(|| ObjectHash::from_hex(format!("{:0>192}", p)))
        })
        .collect();

    Ok(Commit {
        tree: lit_tree,
        parents: lit_parents,
        author,
        committer,
        timestamp,
        message: message_lines.join("\n"),
        pq_signature: None,
        metadata: None,
    })
}

/// Parse a Git identity line: "Name <email> timestamp timezone"
fn parse_git_ident(ident: &str) -> (String, i64) {
    // "John Doe <john@example.com> 1234567890 +0000"
    if let Some(bracket_pos) = ident.rfind('>') {
        let name_email = &ident[..=bracket_pos];
        let rest = ident[bracket_pos + 1..].trim();
        let timestamp = rest
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        (name_email.trim().to_string(), timestamp)
    } else {
        (ident.to_string(), 0)
    }
}

/// Import objects from a Git pack file
fn import_pack_file(
    pack_path: &Path,
    store: &ObjectStore,
    hash_map: &mut HashMap<String, ObjectHash>,
) -> Result<u64, crate::errors::LitError> {
    let data = fs::read(pack_path).map_err(|e| format!("Failed to read pack: {}", e))?;

    // Validate pack header: "PACK" magic, version 2/3, object count
    if data.len() < 12 {
        return Err("Pack file too small".into());
    }
    if &data[0..4] != b"PACK" {
        return Err("Invalid pack file magic".into());
    }
    let version = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if version != 2 && version != 3 {
        return Err(format!("Unsupported pack version: {}", version).into());
    }
    let num_objects = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let mut imported = 0u64;

    // Parse pack entries
    let mut pos = 12;
    for _ in 0..num_objects {
        if pos >= data.len() - 20 {
            break;
        }
        match parse_pack_entry(&data, &mut pos, store, hash_map) {
            Ok(_) => imported += 1,
            Err(e) => {
                eprintln!("Warning: skipping pack entry: {}", e);
                break;
            }
        }
    }

    Ok(imported)
}

/// Parse a single pack entry
fn parse_pack_entry(
    data: &[u8],
    pos: &mut usize,
    store: &ObjectStore,
    hash_map: &mut HashMap<String, ObjectHash>,
) -> Result<(), crate::errors::LitError> {
    if *pos >= data.len() {
        return Err("Unexpected end of pack".into());
    }

    // Read type and size from variable-length header
    let mut byte = data[*pos];
    let obj_type = (byte >> 4) & 0x07;
    let mut _size: u64 = (byte & 0x0f) as u64;
    let mut shift = 4;
    *pos += 1;

    while byte & 0x80 != 0 {
        if *pos >= data.len() {
            return Err("Truncated pack header".into());
        }
        byte = data[*pos];
        _size |= ((byte & 0x7f) as u64) << shift;
        shift += 7;
        *pos += 1;
    }

    match obj_type {
        1..=4 => {
            // Regular object types: commit, tree, blob, tag
            let mut decoder = flate2::read::ZlibDecoder::new(&data[*pos..]);
            let mut content = Vec::new();
            decoder
                .read_to_end(&mut content)
                .map_err(|e| format!("Decompress error: {}", e))?;
            *pos += decoder.total_in() as usize;

            // Compute git hash
            let type_name = match obj_type {
                1 => "commit",
                2 => "tree",
                3 => "blob",
                4 => "tag",
                _ => unreachable!(),
            };
            let header = format!("{} {}\0", type_name, content.len());
            let mut sha1 = sha1::Sha1::new();
            sha1.update(header.as_bytes());
            sha1.update(&content);
            let git_hash = hex::encode(sha1.finalize());

            let lit_obj = match obj_type {
                3 => Object::Blob(Blob::new(content)),
                2 => {
                    let tree = parse_git_tree(&content, hash_map)?;
                    Object::Tree(tree)
                }
                1 => {
                    let commit = parse_git_commit(&content, hash_map)?;
                    Object::Commit(commit)
                }
                _ => Object::Blob(Blob::new(content)),
            };

            let lit_hash = store.write(&lit_obj)?;
            hash_map.insert(git_hash, lit_hash);
        }
        6 => {
            // OFS_DELTA — skip for now
            // Read negative offset
            let mut byte = data[*pos];
            let mut _offset: u64 = (byte & 0x7f) as u64;
            *pos += 1;
            while byte & 0x80 != 0 {
                byte = data[*pos];
                _offset = ((_offset + 1) << 7) | (byte & 0x7f) as u64;
                *pos += 1;
            }
            // Skip compressed delta data
            let mut decoder = flate2::read::ZlibDecoder::new(&data[*pos..]);
            let mut delta = Vec::new();
            let _ = decoder.read_to_end(&mut delta);
            *pos += decoder.total_in() as usize;
        }
        7 => {
            // REF_DELTA
            if *pos + 20 > data.len() {
                return Err("Truncated ref delta".into());
            }
            *pos += 20; // Skip base hash
            let mut decoder = flate2::read::ZlibDecoder::new(&data[*pos..]);
            let mut delta = Vec::new();
            let _ = decoder.read_to_end(&mut delta);
            *pos += decoder.total_in() as usize;
        }
        _ => {
            return Err(format!("Unknown pack object type: {}", obj_type).into());
        }
    }

    Ok(())
}
