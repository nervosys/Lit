use crate::core::{find_repo_root, read_head, read_ref, write_ref, Object, ObjectHash, Tag};
use crate::response::TagResponse;
use crate::storage::ObjectStore;

#[allow(clippy::too_many_arguments)]
pub fn execute(
    name: Option<String>,
    message: Option<String>,
    annotate: bool,
    delete: bool,
    sign: bool,
    verify: bool,
    list: bool,
    commit: Option<String>,
) -> Result<TagResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;

    // List tags: `lit tag` with no args, or `lit tag --list`
    if list || (name.is_none() && !delete && !verify) {
        return list_tags(&repo_root);
    }

    let tag_name = name.ok_or("Tag name is required")?;

    if delete {
        return delete_tag(&repo_root, &tag_name);
    }

    if verify {
        return verify_tag(&repo_root, &tag_name);
    }

    // Create tag
    let target_hash = resolve_target(&repo_root, commit)?;

    if annotate || sign || message.is_some() {
        create_annotated_tag(&repo_root, &tag_name, &target_hash, message, sign)
    } else {
        create_lightweight_tag(&repo_root, &tag_name, &target_hash)
    }
}

fn resolve_target(repo_root: &std::path::Path, commit: Option<String>) -> Result<String, crate::errors::LitError> {
    match commit {
        Some(rev) => {
            // Try as branch ref first, then as raw hash
            read_ref(repo_root, &format!("heads/{}", rev))
                .or_else(|_| read_ref(repo_root, &format!("tags/{}", rev)))
                .or_else(|_| {
                    // Verify it looks like a hash
                    if rev.len() >= 16 && rev.chars().all(|c| c.is_ascii_hexdigit()) {
                        Ok(rev)
                    } else {
                        Err(format!("Cannot resolve '{}' to a commit", rev).into())
                    }
                })
        }
        None => Ok(read_head(repo_root)?),
    }
}

fn create_lightweight_tag(
    repo_root: &std::path::Path,
    tag_name: &str,
    target_hash: &str,
) -> Result<TagResponse, crate::errors::LitError> {
    // Check tag doesn't already exist
    if crate::core::refs::read_ref(repo_root, &format!("tags/{}", tag_name)).is_ok() {
        return Err(format!("tag '{}' already exists", tag_name).into());
    }

    write_ref(repo_root, &format!("tags/{}", tag_name), target_hash)?;

    Ok(TagResponse::Create {
        name: tag_name.to_string(),
        hash: target_hash.to_string(),
        annotated: false,
        signed: false,
        message: format!("Created lightweight tag '{}'", tag_name),
    })
}

fn create_annotated_tag(
    repo_root: &std::path::Path,
    tag_name: &str,
    target_hash: &str,
    message: Option<String>,
    sign: bool,
) -> Result<TagResponse, crate::errors::LitError> {
    // Check tag doesn't already exist
    if crate::core::refs::read_ref(repo_root, &format!("tags/{}", tag_name)).is_ok() {
        return Err(format!("tag '{}' already exists", tag_name).into());
    }

    let tagger = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "Unknown".to_string());

    let msg = message.unwrap_or_default();
    let store = ObjectStore::new(repo_root);

    let mut tag = Tag::new(
        ObjectHash::from_hex(target_hash.to_string()),
        "commit".to_string(),
        tag_name.to_string(),
        tagger,
        msg,
    );

    if sign {
        let keypair = crate::crypto::signatures::PQKeyPair::generate();
        tag.sign(&keypair);
    }

    let tag_obj = Object::Tag(tag);
    let tag_hash = store.write(&tag_obj)?;

    write_ref(repo_root, &format!("tags/{}", tag_name), tag_hash.as_str())?;

    Ok(TagResponse::Create {
        name: tag_name.to_string(),
        hash: tag_hash.as_str().to_string(),
        annotated: true,
        signed: sign,
        message: if sign {
            format!("Created signed tag '{}' (PQ: ML-DSA-87)", tag_name)
        } else {
            format!("Created annotated tag '{}'", tag_name)
        },
    })
}

fn list_tags(repo_root: &std::path::Path) -> Result<TagResponse, crate::errors::LitError> {
    let refs = crate::core::refs::list_refs(repo_root, "tags")?;
    let tags: Vec<String> = refs.into_iter().map(|r| r.name).collect();
    Ok(TagResponse::List { tags })
}

fn delete_tag(repo_root: &std::path::Path, tag_name: &str) -> Result<TagResponse, crate::errors::LitError> {
    crate::core::refs::delete_ref(repo_root, &format!("tags/{}", tag_name))?;
    Ok(TagResponse::Delete {
        name: tag_name.to_string(),
        message: format!("Deleted tag '{}'", tag_name),
    })
}

fn verify_tag(repo_root: &std::path::Path, tag_name: &str) -> Result<TagResponse, crate::errors::LitError> {
    let hash_str =
        crate::core::refs::read_ref(repo_root, &format!("tags/{}", tag_name))?;
    let hash = ObjectHash::from_hex(hash_str);
    let store = ObjectStore::new(repo_root);
    let obj = store.read(&hash)?;

    match obj {
        Object::Tag(tag) => {
            let result = tag.verify_signature();
            match result {
                Ok(()) => Ok(TagResponse::Verify {
                    name: tag_name.to_string(),
                    valid: true,
                    algorithm: tag
                        .pq_signature
                        .as_ref()
                        .map(|s| s.algorithm.clone())
                        .unwrap_or_default(),
                    message: format!("Good signature on tag '{}' (PQ)", tag_name),
                }),
                Err(e) => Ok(TagResponse::Verify {
                    name: tag_name.to_string(),
                    valid: false,
                    algorithm: String::new(),
                    message: format!("Bad signature on tag '{}': {}", tag_name, e),
                }),
            }
        }
        _ => Err(format!(
            "Tag '{}' is a lightweight tag (not signed)",
            tag_name
        ).into()),
    }
}
