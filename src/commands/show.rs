use crate::core::{find_repo_root, Object, ObjectHash};
use crate::response::{ShowResponse, TreeEntryInfo};
use crate::storage::ObjectStore;

/// Resolve what the user typed to an object hash.
///
/// Accepts `HEAD`, a branch name, a tag name, or a literal hash. An earlier
/// version gated the literal-hash case on `object.len() == 64`, but a Lit hash
/// is 192 characters (SHA3-512 followed by BLAKE3), so that arm never ran and
/// hashes only worked by falling through the ref lookups unchanged. `HEAD` had
/// no case at all and resolved to itself, so `lit show HEAD` always reported
/// the object as missing.
fn resolve_object(
    repo_root: &std::path::Path,
    object: String,
) -> Result<ObjectHash, crate::errors::LitError> {
    use crate::core::{read_head, read_ref};

    if object == "HEAD" {
        // read_head gives the current branch, or the hash itself when detached.
        let head = read_head(repo_root)?;
        return Ok(match read_ref(repo_root, &format!("heads/{}", head)) {
            Ok(hash) => ObjectHash::from_hex(hash),
            Err(_) => ObjectHash::from_hex(head),
        });
    }

    // A name that matches no ref is taken to be a hash already.
    let resolved = read_ref(repo_root, &format!("heads/{}", object))
        .or_else(|_| read_ref(repo_root, &format!("tags/{}", object)))
        .unwrap_or(object);
    Ok(ObjectHash::from_hex(resolved))
}

pub fn execute(object: String) -> Result<ShowResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let hash = resolve_object(&repo_root, object)?;

    let obj = store.read(&hash)?;

    match obj {
        Object::Commit(commit) => Ok(ShowResponse::Commit {
            hash: hash.to_string(),
            author: commit.author,
            timestamp: commit.timestamp,
            message: commit.message,
        }),
        Object::Tree(tree) => {
            let entries = tree
                .entries
                .into_iter()
                .map(|e| TreeEntryInfo {
                    mode: e.mode,
                    object_type: e.object_type,
                    hash: e.hash.to_string(),
                    name: e.name,
                })
                .collect();
            Ok(ShowResponse::Tree {
                hash: hash.to_string(),
                entries,
            })
        }
        Object::Blob(blob) => {
            let content = std::str::from_utf8(&blob.content)
                .map(|s| s.to_string())
                .ok();
            let is_binary = content.is_none();
            Ok(ShowResponse::Blob {
                hash: hash.to_string(),
                size: blob.content.len(),
                content,
                is_binary,
            })
        }
        Object::Tag(tag) => Ok(ShowResponse::Commit {
            hash: hash.to_string(),
            author: tag.tagger.clone(),
            timestamp: tag.timestamp,
            message: format!(
                "tag {}\nTarget: {}\n\n{}",
                tag.tag_name,
                tag.target.as_str(),
                tag.message
            ),
        }),
    }
}
