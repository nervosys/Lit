use crate::core::{find_repo_root, list_refs, read_head, Object, ObjectHash};
use crate::response::ExportGitResponse;
use crate::storage::ObjectStore;
use sha1::Digest as Sha1Digest;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Export a Lit repository to Git format.
/// Converts Lit objects (SHA3-512 + BLAKE3) back to Git objects (SHA-1).
pub fn execute(destination: String) -> Result<ExportGitResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let dest_path = PathBuf::from(&destination);

    // Create bare Git repository structure
    fs::create_dir_all(&dest_path).map_err(|e| format!("Failed to create destination: {}", e))?;
    for dir in &["objects", "refs/heads", "refs/tags"] {
        fs::create_dir_all(dest_path.join(dir))
            .map_err(|e| format!("Failed to create {}: {}", dir, e))?;
    }

    // Write HEAD
    fs::write(dest_path.join("HEAD"), "ref: refs/heads/main\n")
        .map_err(|e| format!("Failed to write HEAD: {}", e))?;

    let store = ObjectStore::new(&repo_root);
    let mut hash_map: HashMap<String, String> = HashMap::new(); // lit_hash -> git_hash
    let mut objects_exported = 0u64;
    let mut refs_exported = 0u64;

    // Export all objects.
    //
    // Git trees, commits and tags embed the SHA-1 of the objects they point at,
    // so an object can only be serialized once everything it references has
    // been written and its Lit -> Git mapping recorded. `list()` returns
    // objects in filesystem order, which puts no such guarantee on the caller,
    // so walk the object graph in dependency order instead.
    let all_objects = store
        .list()
        .map_err(|e| format!("Failed to list objects: {}", e))?;

    let mut scheduled: HashSet<String> = HashSet::new();
    for lit_hash in &all_objects {
        objects_exported +=
            export_subgraph(&store, lit_hash, &dest_path, &mut hash_map, &mut scheduled)?;
    }

    // Export refs
    // Branches
    let branches =
        list_refs(&repo_root, "heads").map_err(|e| format!("Failed to list branches: {}", e))?;
    for branch_ref in &branches {
        if let Some(git_hash) = hash_map.get(&branch_ref.hash) {
            let ref_path = dest_path.join("refs").join("heads").join(&branch_ref.name);
            if let Some(parent) = ref_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&ref_path, format!("{}\n", git_hash))
                .map_err(|e| format!("Failed to write ref: {}", e))?;
            refs_exported += 1;
        }
    }

    // Tags
    let tags = list_refs(&repo_root, "tags").map_err(|e| format!("Failed to list tags: {}", e))?;
    for tag_ref in &tags {
        if let Some(git_hash) = hash_map.get(&tag_ref.hash) {
            let ref_path = dest_path.join("refs").join("tags").join(&tag_ref.name);
            if let Some(parent) = ref_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&ref_path, format!("{}\n", git_hash))
                .map_err(|e| format!("Failed to write ref: {}", e))?;
            refs_exported += 1;
        }
    }

    // Remote-tracking refs
    let remotes = list_refs(&repo_root, "remotes").unwrap_or_default();
    for remote_ref in &remotes {
        if let Some(git_hash) = hash_map.get(&remote_ref.hash) {
            let ref_path = dest_path
                .join("refs")
                .join("remotes")
                .join(&remote_ref.name);
            if let Some(parent) = ref_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&ref_path, format!("{}\n", git_hash))
                .map_err(|e| format!("Failed to write remote ref: {}", e))?;
            refs_exported += 1;
        }
    }

    // Write packed-refs for efficiency (Git optimization)
    let mut packed_refs = String::from("# pack-refs with: peeled fully-peeled sorted \n");
    let mut has_packed = false;
    for branch_ref in &branches {
        if let Some(git_hash) = hash_map.get(&branch_ref.hash) {
            packed_refs.push_str(&format!("{} refs/heads/{}\n", git_hash, branch_ref.name));
            has_packed = true;
        }
    }
    for tag_ref in &tags {
        if let Some(git_hash) = hash_map.get(&tag_ref.hash) {
            packed_refs.push_str(&format!("{} refs/tags/{}\n", git_hash, tag_ref.name));
            has_packed = true;
        }
    }
    if has_packed {
        fs::write(dest_path.join("packed-refs"), &packed_refs)
            .map_err(|e| format!("Failed to write packed-refs: {}", e))?;
    }

    // Update HEAD to point to current branch
    let head = read_head(&repo_root).unwrap_or_else(|_| "main".to_string());
    if head.contains('/') || head.len() > 50 {
        // Detached HEAD — try to map hash
        if let Some(git_hash) = hash_map.get(&head) {
            fs::write(dest_path.join("HEAD"), format!("{}\n", git_hash))
                .map_err(|e| format!("Failed to write HEAD: {}", e))?;
        }
    } else {
        fs::write(
            dest_path.join("HEAD"),
            format!("ref: refs/heads/{}\n", head),
        )
        .map_err(|e| format!("Failed to write HEAD: {}", e))?;
    }

    // Copy .litignore as .gitignore
    let litignore = repo_root.join(".litignore");
    let gitignore = dest_path.parent().unwrap_or(&dest_path).join(".gitignore");
    if litignore.exists() && !gitignore.exists() {
        let _ = fs::copy(&litignore, &gitignore);
    }

    Ok(ExportGitResponse {
        destination: destination.clone(),
        objects_exported,
        refs_exported,
        message: format!(
            "Exported {} objects and {} refs to Git repository",
            objects_exported, refs_exported
        ),
    })
}

/// A step in the iterative post-order walk of the Lit object graph.
enum Step {
    /// Expand this object's dependencies before writing it.
    Visit(ObjectHash),
    /// Every dependency has been written; serialize and write this object.
    Emit(ObjectHash),
}

/// Export `root` and everything it references, dependencies first.
///
/// Returns the number of objects written. Objects already exported are
/// skipped, so this can be driven over every hash in the store without
/// writing anything twice. The walk is iterative rather than recursive
/// because commit chains are as deep as the repository is long.
fn export_subgraph(
    store: &ObjectStore,
    root: &ObjectHash,
    dest: &Path,
    hash_map: &mut HashMap<String, String>,
    scheduled: &mut HashSet<String>,
) -> Result<u64, crate::errors::LitError> {
    let mut exported = 0u64;
    let mut stack = vec![Step::Visit(root.clone())];

    while let Some(step) = stack.pop() {
        match step {
            Step::Visit(hash) => {
                if hash_map.contains_key(hash.as_str())
                    || !scheduled.insert(hash.as_str().to_string())
                {
                    continue;
                }
                let deps = match dependencies_of(store, &hash) {
                    Ok(deps) => deps,
                    Err(e) => {
                        // An unreadable object stays unmapped. Anything
                        // referencing it now fails loudly in the serializers
                        // below instead of being written against a made-up
                        // hash, which would corrupt the exported repository.
                        eprintln!("Warning: skipping object {}: {}", hash.short(), e);
                        continue;
                    }
                };
                stack.push(Step::Emit(hash));
                for dep in deps {
                    stack.push(Step::Visit(dep));
                }
            }
            Step::Emit(hash) => {
                export_object(store, &hash, dest, hash_map)?;
                exported += 1;
            }
        }
    }

    Ok(exported)
}

/// The objects a given object references, all of which must be exported first.
fn dependencies_of(
    store: &ObjectStore,
    hash: &ObjectHash,
) -> Result<Vec<ObjectHash>, crate::errors::LitError> {
    Ok(match store.read(hash)? {
        Object::Blob(_) => Vec::new(),
        Object::Tree(tree) => tree.entries.iter().map(|e| e.hash.clone()).collect(),
        Object::Commit(commit) => std::iter::once(commit.tree.clone())
            .chain(commit.parents.iter().cloned())
            .collect(),
        Object::Tag(tag) => vec![tag.target.clone()],
    })
}

/// Look up the Git hash that a referenced Lit object was exported as.
///
/// A miss means the object was never written. Substituting a placeholder here
/// would produce a Git repository whose trees and commits point at objects
/// that do not exist, so an incomplete graph is reported rather than encoded.
fn lookup_git_hash(
    hash_map: &HashMap<String, String>,
    lit_hash: &ObjectHash,
    context: &str,
) -> Result<String, crate::errors::LitError> {
    hash_map.get(lit_hash.as_str()).cloned().ok_or_else(|| {
        crate::errors::LitError::general(format!(
            "Cannot export {}: referenced object {} is missing from this repository",
            context,
            lit_hash.short()
        ))
    })
}

/// Decode a 40-character Git SHA-1 hex string into its 20 raw bytes.
fn decode_sha1(git_hex: &str) -> Result<Vec<u8>, crate::errors::LitError> {
    let bytes = hex::decode(git_hex).map_err(|e| format!("Invalid Git hash hex: {}", e))?;
    if bytes.len() != 20 {
        return Err(format!(
            "Expected a 20-byte Git SHA-1, got {} bytes from '{}'",
            bytes.len(),
            git_hex
        )
        .into());
    }
    Ok(bytes)
}

/// Export a single Lit object to Git format
fn export_object(
    store: &ObjectStore,
    lit_hash: &ObjectHash,
    dest: &Path,
    hash_map: &mut HashMap<String, String>,
) -> Result<(), crate::errors::LitError> {
    let obj = store.read(lit_hash)?;

    let (type_name, content) = match &obj {
        Object::Blob(blob) => ("blob", blob.content.clone()),
        Object::Tree(tree) => {
            let content = serialize_git_tree(tree, hash_map)?;
            ("tree", content)
        }
        Object::Commit(commit) => {
            let content = serialize_git_commit(commit, hash_map)?;
            ("commit", content)
        }
        Object::Tag(tag) => {
            let content = serialize_git_tag(tag, hash_map)?;
            ("tag", content)
        }
    };

    // Compute Git SHA-1
    let header = format!("{} {}\0", type_name, content.len());
    let mut sha1 = sha1::Sha1::new();
    sha1.update(header.as_bytes());
    sha1.update(&content);
    let git_hash = hex::encode(sha1.finalize());

    // Write as loose Git object
    let obj_dir = dest.join("objects").join(&git_hash[..2]);
    let obj_path = obj_dir.join(&git_hash[2..]);
    fs::create_dir_all(&obj_dir).map_err(|e| format!("Failed to create object dir: {}", e))?;

    let mut raw = Vec::new();
    raw.extend_from_slice(header.as_bytes());
    raw.extend_from_slice(&content);

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(&raw)
        .map_err(|e| format!("Compress error: {}", e))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("Compress finish error: {}", e))?;

    fs::write(&obj_path, &compressed).map_err(|e| format!("Failed to write object: {}", e))?;

    hash_map.insert(lit_hash.as_str().to_string(), git_hash);
    Ok(())
}

/// Serialize a Lit tree into Git tree binary format
fn serialize_git_tree(
    tree: &crate::core::Tree,
    hash_map: &HashMap<String, String>,
) -> Result<Vec<u8>, crate::errors::LitError> {
    let mut buf = Vec::new();
    for entry in &tree.entries {
        // mode SP name NUL sha1-bytes
        buf.extend_from_slice(entry.mode.as_bytes());
        buf.push(b' ');
        buf.extend_from_slice(entry.name.as_bytes());
        buf.push(0);

        // Git stores the referenced object's SHA-1 as 20 raw bytes.
        let git_hex = lookup_git_hash(
            hash_map,
            &entry.hash,
            &format!("tree entry '{}'", entry.name),
        )?;
        buf.extend_from_slice(&decode_sha1(&git_hex)?);
    }
    Ok(buf)
}

/// Serialize a Lit commit into Git commit text format
fn serialize_git_commit(
    commit: &crate::core::Commit,
    hash_map: &HashMap<String, String>,
) -> Result<Vec<u8>, crate::errors::LitError> {
    let mut lines = Vec::new();

    // tree
    let tree_hash = lookup_git_hash(hash_map, &commit.tree, "commit tree")?;
    lines.push(format!("tree {}", tree_hash));

    // parents
    for parent in &commit.parents {
        let parent_hash = lookup_git_hash(hash_map, parent, "commit parent")?;
        lines.push(format!("parent {}", parent_hash));
    }

    // author and committer. A commit imported from Git carries the offset it
    // was written with; Lit's own commits are UTC.
    let timezone = commit.timezone.as_deref().unwrap_or("+0000");
    lines.push(format!(
        "author {} {} {}",
        commit.author, commit.timestamp, timezone
    ));
    lines.push(format!(
        "committer {} {} {}",
        commit.committer, commit.timestamp, timezone
    ));

    // Lit metadata as Git notes (appended to message)
    let mut message = commit.message.clone();
    if let Some(ref meta) = commit.metadata {
        message.push_str(&format!("\n\nLit-Metadata: {}", meta));
    }

    lines.push(String::new()); // empty line before message
    lines.push(message);

    Ok(lines.join("\n").into_bytes())
}

/// Serialize a Lit tag into Git tag text format
fn serialize_git_tag(
    tag: &crate::core::Tag,
    hash_map: &HashMap<String, String>,
) -> Result<Vec<u8>, crate::errors::LitError> {
    let target_hash = lookup_git_hash(hash_map, &tag.target, "tag target")?;

    // As for commits, a tag imported from Git carries the offset it was
    // written with; Lit's own tags are UTC.
    let timezone = tag.timezone.as_deref().unwrap_or("+0000");

    let mut lines = Vec::new();
    lines.push(format!("object {}", target_hash));
    lines.push(format!("type {}", tag.target_type));
    lines.push(format!("tag {}", tag.tag_name));
    lines.push(format!(
        "tagger {} {} {}",
        tag.tagger, tag.timestamp, timezone
    ));
    lines.push(String::new());
    lines.push(tag.message.clone());

    Ok(lines.join("\n").into_bytes())
}
