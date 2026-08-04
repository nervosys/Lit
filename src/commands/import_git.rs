use crate::core::{find_repo_root, Blob, Commit, Object, ObjectHash, Tag, Tree};
use crate::response::ImportGitResponse;
use crate::storage::ObjectStore;
use sha1::Digest as Sha1Digest;
use std::collections::{HashMap, HashSet};
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

    // A Lit tree records its children by Lit hash, and a Lit commit records
    // its tree and parents the same way, so an object can only be converted
    // once everything it references already has a Lit hash. Neither the
    // filesystem order of `objects/XX/…` nor pack order guarantees that, so
    // discovery and conversion are separate phases: find every object first,
    // then convert the graph in dependency order.
    let mut discovered: HashMap<String, DiscoveredObject> = HashMap::new();
    let mut deltas_unresolved = 0u64;

    // Phase 1: Discover all loose objects
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

                match discover_loose_object(path) {
                    Ok((hash, object)) => {
                        discovered.insert(hash, object);
                    }
                    Err(e) => {
                        eprintln!("Warning: skipping object {}: {}", &git_hash[..8], e);
                    }
                }
            }
        }
    }

    // Phase 2: Discover objects held in pack files
    let pack_dir = objects_dir.join("pack");
    if pack_dir.exists() {
        for entry in
            fs::read_dir(&pack_dir).map_err(|e| format!("Failed to read pack dir: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Pack dir entry error: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("pack") {
                if let Err(e) = scan_pack_file(&path, &mut discovered, &mut deltas_unresolved) {
                    eprintln!("Warning: skipping pack {}: {}", path.display(), e);
                }
            }
        }
    }

    // Phase 3: Convert the discovered graph, dependencies first
    let roots: Vec<String> = discovered.keys().cloned().collect();
    let mut scheduled: HashSet<String> = HashSet::new();
    for git_hash in &roots {
        objects_imported += import_subgraph(
            git_hash,
            &discovered,
            &store,
            &mut hash_map,
            &mut scheduled,
            deltas_unresolved,
        )?;
    }

    // Phase 4: Import refs
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

/// Where a discovered Git object's bytes can be read back from.
enum ObjectSource {
    /// A loose object file, re-inflated on demand.
    Loose(PathBuf),
    /// Content already inflated out of a pack file. Pack entries cannot be
    /// located a second time without building an index, so they are retained.
    Packed(Vec<u8>),
}

/// A Git object found in the source repository, ahead of conversion to Lit.
struct DiscoveredObject {
    source: ObjectSource,
    obj_type: String,
    /// Git hashes this object points at, all of which convert before it.
    deps: Vec<String>,
}

impl DiscoveredObject {
    /// The object's body, without the `<type> <size>\0` header.
    fn content(&self) -> Result<Vec<u8>, crate::errors::LitError> {
        match &self.source {
            ObjectSource::Loose(path) => Ok(read_loose_object(path)?.2),
            ObjectSource::Packed(content) => Ok(content.clone()),
        }
    }
}

/// A step in the iterative post-order walk of the Git object graph.
enum Step {
    /// Expand this object's dependencies before converting it.
    Visit(String),
    /// Every dependency now has a Lit hash; convert and store this object.
    Emit(String),
}

/// Read and inflate a loose Git object, returning its hash, type and body.
fn read_loose_object(path: &Path) -> Result<(String, String, Vec<u8>), crate::errors::LitError> {
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
    let (obj_type, _size_str) = header
        .split_once(' ')
        .ok_or("Invalid Git object header format")?;

    // The Git hash covers the header as well as the body
    let mut sha1 = sha1::Sha1::new();
    sha1.update(&raw);
    let git_hash = hex::encode(sha1.finalize());

    Ok((git_hash, obj_type.to_string(), raw[null_pos + 1..].to_vec()))
}

/// Record a loose Git object and what it references, without converting it.
fn discover_loose_object(
    path: &Path,
) -> Result<(String, DiscoveredObject), crate::errors::LitError> {
    let (git_hash, obj_type, content) = read_loose_object(path)?;
    let deps = git_dependencies(&obj_type, &content)?;
    Ok((
        git_hash,
        DiscoveredObject {
            source: ObjectSource::Loose(path.to_path_buf()),
            obj_type,
            deps,
        },
    ))
}

/// The Git hashes an object references, which must be converted before it.
fn git_dependencies(
    obj_type: &str,
    content: &[u8],
) -> Result<Vec<String>, crate::errors::LitError> {
    Ok(match obj_type {
        "tree" => git_tree_entries(content)?
            .into_iter()
            .map(|(_, _, hash)| hash)
            .collect(),
        "commit" => git_commit_refs(content),
        "tag" => git_tag_target(content).into_iter().collect(),
        _ => Vec::new(),
    })
}

/// The object an annotated tag points at.
fn git_tag_target(content: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(content).ok()?;
    git_header(text)
        .lines()
        .find_map(|line| line.strip_prefix("object "))
        .map(|hash| hash.trim().to_string())
}

/// The header of a Git commit or tag: everything before the first blank line.
fn git_header(text: &str) -> &str {
    git_header_and_message(text).0
}

/// Split a Git commit or tag into its header and its message.
///
/// The message is returned byte for byte — including its trailing newline —
/// so that re-exporting the object reproduces the original Git hash.
fn git_header_and_message(text: &str) -> (&str, &str) {
    text.split_once("\n\n").unwrap_or((text, ""))
}

/// Convert `root` and everything it references, dependencies first.
///
/// Returns the number of objects written. Objects already converted are
/// skipped, so this can be driven over every discovered hash. The walk is
/// iterative because commit chains are as deep as the history is long.
fn import_subgraph(
    root: &str,
    discovered: &HashMap<String, DiscoveredObject>,
    store: &ObjectStore,
    hash_map: &mut HashMap<String, ObjectHash>,
    scheduled: &mut HashSet<String>,
    deltas_unresolved: u64,
) -> Result<u64, crate::errors::LitError> {
    let mut imported = 0u64;
    let mut stack = vec![Step::Visit(root.to_string())];

    while let Some(step) = stack.pop() {
        match step {
            Step::Visit(git_hash) => {
                if hash_map.contains_key(&git_hash) || !scheduled.insert(git_hash.clone()) {
                    continue;
                }
                let object = discovered
                    .get(&git_hash)
                    .ok_or_else(|| missing_object_error(&git_hash, deltas_unresolved))?;
                let deps = object.deps.clone();
                stack.push(Step::Emit(git_hash));
                for dep in deps {
                    stack.push(Step::Visit(dep));
                }
            }
            Step::Emit(git_hash) => {
                let object = &discovered[&git_hash];
                let content = object.content()?;

                let lit_obj = match object.obj_type.as_str() {
                    "blob" => Object::Blob(Blob::new(content)),
                    "tree" => Object::Tree(parse_git_tree(&content, hash_map)?),
                    "commit" => Object::Commit(parse_git_commit(&content, hash_map)?),
                    "tag" => Object::Tag(parse_git_tag(&content, hash_map)?),
                    other => return Err(format!("Unknown object type: {}", other).into()),
                };

                let lit_hash = store.write(&lit_obj)?;
                hash_map.insert(git_hash, lit_hash);
                imported += 1;
            }
        }
    }

    Ok(imported)
}

/// Report an object that is referenced but that the source never yielded.
///
/// Recording a synthesized hash instead would produce a Lit repository whose
/// trees and commits point at objects that were never written, so an
/// incomplete source is reported rather than silently encoded.
fn missing_object_error(git_hash: &str, deltas_unresolved: u64) -> crate::errors::LitError {
    let mut msg = format!(
        "Git object {} is referenced but was not found in the source repository",
        &git_hash[..8.min(git_hash.len())]
    );
    if deltas_unresolved > 0 {
        msg.push_str(&format!(
            ". {} pack {} could not be resolved because the base {} not present \
             — the source looks like a thin pack; fetch it with \
             `git -C <source> index-pack --fix-thin` or unpack it first",
            deltas_unresolved,
            if deltas_unresolved == 1 {
                "delta"
            } else {
                "deltas"
            },
            if deltas_unresolved == 1 {
                "was"
            } else {
                "were"
            }
        ));
    }
    crate::errors::LitError::general(msg)
}

/// Look up the Lit hash a referenced Git object was converted to.
fn lookup_lit_hash(
    hash_map: &HashMap<String, ObjectHash>,
    git_hash: &str,
    context: &str,
) -> Result<ObjectHash, crate::errors::LitError> {
    hash_map.get(git_hash).cloned().ok_or_else(|| {
        crate::errors::LitError::general(format!(
            "Cannot import {}: referenced Git object {} has not been converted",
            context,
            &git_hash[..8.min(git_hash.len())]
        ))
    })
}

/// Walk a Git tree object's binary entries as (mode, name, git hash).
fn git_tree_entries(
    content: &[u8],
) -> Result<Vec<(String, String, String)>, crate::errors::LitError> {
    let mut entries = Vec::new();
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
        let git_hash = hex::encode(&content[null_pos + 1..null_pos + 21]);

        entries.push((mode, name, git_hash));
        pos = null_pos + 21;
    }

    Ok(entries)
}

/// Parse a Git tree object's binary content
fn parse_git_tree(
    content: &[u8],
    hash_map: &HashMap<String, ObjectHash>,
) -> Result<Tree, crate::errors::LitError> {
    let mut tree = Tree::new();

    for (mode, name, git_hash) in git_tree_entries(content)? {
        let lit_hash = lookup_lit_hash(hash_map, &git_hash, &format!("tree entry '{}'", name))?;

        let obj_type = if mode.starts_with("40") {
            "tree"
        } else {
            "blob"
        }
        .to_string();

        tree.add_entry(mode, name, lit_hash, obj_type);
    }

    Ok(tree)
}

/// The tree and parent hashes a Git commit references.
fn git_commit_refs(content: &[u8]) -> Vec<String> {
    let text = match std::str::from_utf8(content) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut refs = Vec::new();
    for line in text.lines() {
        // The header ends at the first blank line; the message may contain
        // anything, including lines that look like headers.
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("tree ") {
            refs.push(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("parent ") {
            refs.push(rest.trim().to_string());
        }
    }
    refs
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
    let mut timezone = None;

    let (header, message) = git_header_and_message(text);

    for line in header.lines() {
        if let Some(rest) = line.strip_prefix("tree ") {
            tree_hash = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("parent ") {
            parents.push(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("author ") {
            let (name, ts, tz) = parse_git_ident(rest);
            author = name;
            timestamp = ts;
            timezone = tz;
        } else if let Some(rest) = line.strip_prefix("committer ") {
            let (name, _, _) = parse_git_ident(rest);
            committer = name;
        }
    }

    // Map git hashes to lit hashes
    let lit_tree = lookup_lit_hash(hash_map, &tree_hash, "commit tree")?;

    let lit_parents: Vec<ObjectHash> = parents
        .iter()
        .map(|p| lookup_lit_hash(hash_map, p, "commit parent"))
        .collect::<Result<_, _>>()?;

    Ok(Commit {
        tree: lit_tree,
        parents: lit_parents,
        author,
        committer,
        timestamp,
        message: message.to_string(),
        pq_signature: None,
        metadata: None,
        timezone,
    })
}

/// Parse a Git annotated tag object into a Lit tag.
fn parse_git_tag(
    content: &[u8],
    hash_map: &HashMap<String, ObjectHash>,
) -> Result<Tag, crate::errors::LitError> {
    let text = std::str::from_utf8(content).map_err(|_| "Invalid tag: not UTF-8")?;
    let (header, message) = git_header_and_message(text);

    let mut target_hash = String::new();
    let mut target_type = String::new();
    let mut tag_name = String::new();
    let mut tagger = String::new();
    let mut timestamp: i64 = 0;
    let mut timezone = None;

    for line in header.lines() {
        if let Some(rest) = line.strip_prefix("object ") {
            target_hash = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("type ") {
            target_type = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("tag ") {
            tag_name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("tagger ") {
            let (name, ts, tz) = parse_git_ident(rest);
            tagger = name;
            timestamp = ts;
            timezone = tz;
        }
    }

    Ok(Tag {
        target: lookup_lit_hash(hash_map, &target_hash, "tag target")?,
        target_type,
        tag_name,
        tagger,
        timestamp,
        message: message.to_string(),
        pq_signature: None,
        metadata: None,
        timezone,
    })
}

/// Parse a Git identity line into its name, timestamp and timezone offset.
fn parse_git_ident(ident: &str) -> (String, i64, Option<String>) {
    // "John Doe <john@example.com> 1234567890 +0000"
    if let Some(bracket_pos) = ident.rfind('>') {
        let name_email = &ident[..=bracket_pos];
        let rest = ident[bracket_pos + 1..].trim();
        let mut fields = rest.split_whitespace();
        let timestamp = fields
            .next()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);
        let timezone = fields.next().map(|tz| tz.to_string());
        (name_email.trim().to_string(), timestamp, timezone)
    } else {
        (ident.to_string(), 0, None)
    }
}

/// A pack entry as it appears on disk, before any delta is applied.
enum PackEntry {
    /// A complete object: its type name and body.
    Whole { obj_type: String, content: Vec<u8> },
    /// A delta against another entry in this same pack, named by byte offset.
    OfsDelta { base_offset: usize, delta: Vec<u8> },
    /// A delta against an object named by SHA-1, which a thin pack may leave
    /// outside this file.
    RefDelta { base: String, delta: Vec<u8> },
}

/// Upper bound on how many resolution rounds a pack may take.
///
/// Each round resolves one more level of delta chain, so this caps the chain
/// depth. Git's own default packing depth is 50; the margin covers packs
/// produced with a larger `--depth`.
const MAX_DELTA_ROUNDS: usize = 1024;

/// Discover the objects held in a Git pack file, without converting them
fn scan_pack_file(
    pack_path: &Path,
    discovered: &mut HashMap<String, DiscoveredObject>,
    deltas_unresolved: &mut u64,
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

    // Read every entry, deltas included, keyed by the byte offset it starts
    // at — that is how OFS_DELTA entries name their base.
    let mut entries: HashMap<usize, PackEntry> = HashMap::new();
    let mut pos = 12;
    for _ in 0..num_objects {
        // The last 20 bytes are the pack checksum, not an entry.
        if pos + 20 > data.len() {
            break;
        }
        let start = pos;
        match read_pack_entry(&data, &mut pos, start) {
            Ok(entry) => {
                entries.insert(start, entry);
            }
            Err(e) => {
                eprintln!("Warning: skipping pack entry: {}", e);
                break;
            }
        }
    }

    resolve_pack_entries(&entries, discovered, deltas_unresolved)
}

/// Read one pack entry starting at `*pos`, advancing past it.
///
/// `entry_start` is the offset the entry begins at, which an OFS_DELTA needs
/// in order to turn its backward distance into an absolute base offset.
fn read_pack_entry(
    data: &[u8],
    pos: &mut usize,
    entry_start: usize,
) -> Result<PackEntry, crate::errors::LitError> {
    // Read type and size from variable-length header
    let mut byte = *data.get(*pos).ok_or("Unexpected end of pack")?;
    let obj_type = (byte >> 4) & 0x07;
    let mut _size: u64 = (byte & 0x0f) as u64;
    let mut shift = 4;
    *pos += 1;

    while byte & 0x80 != 0 {
        byte = *data.get(*pos).ok_or("Truncated pack header")?;
        _size |= ((byte & 0x7f) as u64) << shift;
        shift += 7;
        *pos += 1;
    }

    match obj_type {
        1..=4 => {
            // Regular object types: commit, tree, blob, tag
            let obj_type = match obj_type {
                1 => "commit",
                2 => "tree",
                3 => "blob",
                4 => "tag",
                _ => unreachable!(),
            };
            Ok(PackEntry::Whole {
                obj_type: obj_type.to_string(),
                content: inflate_at(data, pos)?,
            })
        }
        6 => {
            // OFS_DELTA: a distance *backwards* from this entry's own start,
            // encoded with an increment per continuation byte.
            let mut byte = *data.get(*pos).ok_or("Truncated offset delta")?;
            let mut back: u64 = (byte & 0x7f) as u64;
            *pos += 1;
            while byte & 0x80 != 0 {
                byte = *data.get(*pos).ok_or("Truncated offset delta")?;
                back = ((back + 1) << 7) | (byte & 0x7f) as u64;
                *pos += 1;
            }
            let base_offset = entry_start
                .checked_sub(back as usize)
                .ok_or("Offset delta points before the start of the pack")?;
            Ok(PackEntry::OfsDelta {
                base_offset,
                delta: inflate_at(data, pos)?,
            })
        }
        7 => {
            // REF_DELTA: the base is named by SHA-1
            if *pos + 20 > data.len() {
                return Err("Truncated ref delta".into());
            }
            let base = hex::encode(&data[*pos..*pos + 20]);
            *pos += 20;
            Ok(PackEntry::RefDelta {
                base,
                delta: inflate_at(data, pos)?,
            })
        }
        _ => Err(format!("Unknown pack object type: {}", obj_type).into()),
    }
}

/// Inflate the zlib stream at `*pos`, advancing past the compressed bytes.
fn inflate_at(data: &[u8], pos: &mut usize) -> Result<Vec<u8>, crate::errors::LitError> {
    let mut decoder = flate2::read::ZlibDecoder::new(&data[*pos..]);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("Decompress error: {}", e))?;
    *pos += decoder.total_in() as usize;
    Ok(out)
}

/// The SHA-1 Git would store an object under.
fn git_object_hash(obj_type: &str, content: &[u8]) -> String {
    let mut sha1 = sha1::Sha1::new();
    sha1.update(format!("{} {}\0", obj_type, content.len()).as_bytes());
    sha1.update(content);
    hex::encode(sha1.finalize())
}

/// Resolve every delta in a pack and record the results in `discovered`.
///
/// Resolution runs in rounds: each round settles the entries whose base is
/// already known, so a round resolves one more level of delta chain. Bases
/// generally precede their deltas in a pack, but nothing requires that, and
/// rounds make the outcome independent of the order entries appear in.
///
/// Returns the number of objects recorded; entries that never resolve (a thin
/// pack whose bases live elsewhere) are counted in `deltas_unresolved`.
fn resolve_pack_entries(
    entries: &HashMap<usize, PackEntry>,
    discovered: &mut HashMap<String, DiscoveredObject>,
    deltas_unresolved: &mut u64,
) -> Result<u64, crate::errors::LitError> {
    let mut offsets: Vec<usize> = entries.keys().copied().collect();
    offsets.sort_unstable();

    // offset -> (git hash, type, content)
    let mut resolved: HashMap<usize, (String, String, Vec<u8>)> = HashMap::new();
    // git hash -> offset, so a REF_DELTA can find a base inside this pack
    let mut by_hash: HashMap<String, usize> = HashMap::new();

    for _ in 0..MAX_DELTA_ROUNDS {
        let mut progressed = false;
        for &offset in &offsets {
            if resolved.contains_key(&offset) {
                continue;
            }
            if let Some(object) =
                try_resolve_entry(offset, entries, &resolved, &by_hash, discovered)?
            {
                by_hash.insert(object.0.clone(), offset);
                resolved.insert(offset, object);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }

    let mut recorded = 0u64;
    for offset in &offsets {
        match resolved.remove(offset) {
            Some((git_hash, obj_type, content)) => {
                let deps = git_dependencies(&obj_type, &content)?;
                discovered.insert(
                    git_hash,
                    DiscoveredObject {
                        source: ObjectSource::Packed(content),
                        obj_type,
                        deps,
                    },
                );
                recorded += 1;
            }
            None => *deltas_unresolved += 1,
        }
    }

    Ok(recorded)
}

/// Resolve one entry if its base is available, else `None` to retry next round.
fn try_resolve_entry(
    offset: usize,
    entries: &HashMap<usize, PackEntry>,
    resolved: &HashMap<usize, (String, String, Vec<u8>)>,
    by_hash: &HashMap<String, usize>,
    discovered: &HashMap<String, DiscoveredObject>,
) -> Result<Option<(String, String, Vec<u8>)>, crate::errors::LitError> {
    let entry = match entries.get(&offset) {
        Some(entry) => entry,
        None => return Ok(None),
    };

    let (obj_type, content) = match entry {
        PackEntry::Whole { obj_type, content } => (obj_type.clone(), content.clone()),
        PackEntry::OfsDelta { base_offset, delta } => match resolved.get(base_offset) {
            Some((_, base_type, base)) => (base_type.clone(), apply_delta(base, delta)?),
            None => return Ok(None),
        },
        PackEntry::RefDelta { base, delta } => {
            // The base may sit in this pack, or have arrived with the loose
            // objects and earlier packs already folded into `discovered`.
            let resolved_base = by_hash
                .get(base)
                .and_then(|offset| resolved.get(offset))
                .map(|(_, base_type, base)| (base_type.clone(), base.clone()));

            match resolved_base {
                Some((base_type, base_content)) => (base_type, apply_delta(&base_content, delta)?),
                None => match discovered.get(base) {
                    Some(object) => (
                        object.obj_type.clone(),
                        apply_delta(&object.content()?, delta)?,
                    ),
                    None => return Ok(None),
                },
            }
        }
    };

    let git_hash = git_object_hash(&obj_type, &content);
    Ok(Some((git_hash, obj_type, content)))
}

/// Read a little-endian base-128 varint, as used for the delta header sizes.
fn read_delta_varint(data: &[u8], pos: &mut usize) -> Result<u64, crate::errors::LitError> {
    let mut value: u64 = 0;
    let mut shift = 0;
    loop {
        let byte = *data.get(*pos).ok_or("Truncated delta size")?;
        *pos += 1;
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift > 63 {
            return Err("Delta size overflows 64 bits".into());
        }
    }
}

/// Apply a Git delta to its base object, producing the target content.
///
/// A delta is a source size, a target size, then a run of instructions: a
/// high-bit-set byte copies a range out of the base, and any other non-zero
/// byte inserts that many literal bytes that follow it.
fn apply_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>, crate::errors::LitError> {
    let mut pos = 0;

    let base_size = read_delta_varint(delta, &mut pos)?;
    if base_size != base.len() as u64 {
        return Err(format!(
            "Delta expects a {}-byte base, but the base object is {} bytes",
            base_size,
            base.len()
        )
        .into());
    }
    let target_size = read_delta_varint(delta, &mut pos)?;

    let mut out: Vec<u8> = Vec::with_capacity(target_size as usize);
    while pos < delta.len() {
        let instruction = delta[pos];
        pos += 1;

        if instruction & 0x80 != 0 {
            // Copy: the low nibble flags which offset bytes are present, the
            // next three bits which size bytes are.
            let mut copy_offset: usize = 0;
            for shift in 0..4 {
                if instruction & (1 << shift) != 0 {
                    let byte = *delta.get(pos).ok_or("Truncated delta copy offset")?;
                    pos += 1;
                    copy_offset |= (byte as usize) << (8 * shift);
                }
            }
            let mut copy_size: usize = 0;
            for shift in 0..3 {
                if instruction & (0x10 << shift) != 0 {
                    let byte = *delta.get(pos).ok_or("Truncated delta copy size")?;
                    pos += 1;
                    copy_size |= (byte as usize) << (8 * shift);
                }
            }
            if copy_size == 0 {
                copy_size = 0x10000; // a zero size means 64K
            }

            let end = copy_offset
                .checked_add(copy_size)
                .ok_or("Delta copy range overflows")?;
            if end > base.len() {
                return Err(format!(
                    "Delta copies bytes {}..{} from a {}-byte base",
                    copy_offset,
                    end,
                    base.len()
                )
                .into());
            }
            out.extend_from_slice(&base[copy_offset..end]);
        } else if instruction != 0 {
            // Insert: the instruction byte is the length of the literal run.
            let len = (instruction & 0x7f) as usize;
            let end = pos.checked_add(len).ok_or("Delta insert range overflows")?;
            if end > delta.len() {
                return Err("Delta insert runs past the end of the delta".into());
            }
            out.extend_from_slice(&delta[pos..end]);
            pos = end;
        } else {
            return Err("Delta contains a reserved 0x00 instruction".into());
        }
    }

    if out.len() as u64 != target_size {
        return Err(format!(
            "Delta produced {} bytes, but its header declares {}",
            out.len(),
            target_size
        )
        .into());
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a little-endian base-128 varint, as the delta header uses.
    fn varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                return out;
            }
        }
    }

    /// Build a copy instruction, omitting zero bytes the way Git does.
    ///
    /// A size of 0x10000 therefore encodes as no size bytes at all, which is
    /// how the format spells "64K".
    fn copy(offset: u32, size: u32) -> Vec<u8> {
        let mut instruction = 0x80u8;
        let mut operands = Vec::new();
        for i in 0..4 {
            let byte = ((offset >> (8 * i)) & 0xff) as u8;
            if byte != 0 {
                instruction |= 1 << i;
                operands.push(byte);
            }
        }
        for i in 0..3 {
            let byte = ((size >> (8 * i)) & 0xff) as u8;
            if byte != 0 {
                instruction |= 0x10 << i;
                operands.push(byte);
            }
        }
        let mut out = vec![instruction];
        out.extend(operands);
        out
    }

    /// Build an insert instruction carrying literal bytes.
    fn insert(data: &[u8]) -> Vec<u8> {
        let mut out = vec![data.len() as u8];
        out.extend_from_slice(data);
        out
    }

    /// Assemble a delta from its declared sizes and instruction stream.
    fn delta(base_len: u64, target_len: u64, body: &[Vec<u8>]) -> Vec<u8> {
        let mut out = varint(base_len);
        out.extend(varint(target_len));
        for chunk in body {
            out.extend_from_slice(chunk);
        }
        out
    }

    #[test]
    fn apply_delta_inserts_literal_bytes() {
        let d = delta(0, 5, &[insert(b"hello")]);
        assert_eq!(apply_delta(b"", &d).unwrap(), b"hello");
    }

    #[test]
    fn apply_delta_copies_from_base() {
        let base = b"hello world";
        let d = delta(
            base.len() as u64,
            11,
            &[copy(6, 5), insert(b" "), copy(0, 5)],
        );
        assert_eq!(apply_delta(base, &d).unwrap(), b"world hello");
    }

    #[test]
    fn apply_delta_treats_zero_size_as_64k() {
        // All three size bytes zero is the format's encoding of 0x10000.
        let base = vec![b'x'; 0x10000];
        let d = delta(base.len() as u64, 0x10000, &[copy(0, 0x10000)]);
        assert_eq!(apply_delta(&base, &d).unwrap(), base);
    }

    #[test]
    fn apply_delta_rejects_a_base_of_the_wrong_size() {
        let d = delta(99, 5, &[insert(b"hello")]);
        let err = apply_delta(b"short", &d).unwrap_err();
        // `Display` is deliberately sanitized, so assert on the internal text.
        let detail = err.internal_message();
        assert!(
            detail.contains("99"),
            "error should name the expected size: {}",
            detail
        );
    }

    #[test]
    fn apply_delta_rejects_a_copy_past_the_end_of_the_base() {
        let base = b"tiny";
        let d = delta(base.len() as u64, 100, &[copy(0, 100)]);
        assert!(apply_delta(base, &d).is_err());
    }

    #[test]
    fn apply_delta_rejects_the_reserved_instruction() {
        let d = delta(0, 1, &[vec![0x00]]);
        assert!(apply_delta(b"", &d).is_err());
    }

    #[test]
    fn apply_delta_rejects_output_of_the_wrong_length() {
        // The header claims 10 bytes; the instructions produce 5.
        let d = delta(0, 10, &[insert(b"hello")]);
        assert!(apply_delta(b"", &d).is_err());
    }
}
