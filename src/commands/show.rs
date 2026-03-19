use crate::core::{find_repo_root, Object, ObjectHash};
use crate::response::{ShowResponse, TreeEntryInfo};
use crate::storage::ObjectStore;

pub fn execute(object: String) -> Result<ShowResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Parse object reference (could be hash or ref)
    let hash = if object.len() == 64 {
        ObjectHash::from_hex(object)
    } else {
        use crate::core::read_ref;
        let resolved = read_ref(&repo_root, &format!("heads/{}", object))
            .or_else(|_| read_ref(&repo_root, &format!("tags/{}", object)))
            .unwrap_or(object);
        ObjectHash::from_hex(resolved)
    };

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
