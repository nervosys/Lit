use crate::core::{find_repo_root, read_head, Object, ObjectHash};
use crate::response::{BlameLineInfo, BlameResponse};
use crate::storage::ObjectStore;

pub fn execute(file: String) -> Result<BlameResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Get current HEAD
    let head_hash = read_head(&repo_root)?;

    // Read current file content from working tree
    let file_path = repo_root.join(&file);
    if !file_path.exists() {
        return Err(format!("File '{}' not found", file));
    }

    let content =
        std::fs::read_to_string(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    let lines: Vec<&str> = content.lines().collect();

    // Walk commit history and attribute each line
    let mut blame: Vec<BlameLineInfo> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| BlameLineInfo {
            line_number: i + 1,
            content: line.to_string(),
            commit_hash: String::new(),
            author: String::new(),
            timestamp: 0,
        })
        .collect();

    // Simple blame: walk history, for each commit check if file exists and has same content
    let mut unblamed: Vec<usize> = (0..lines.len()).collect();
    let mut current_hash = head_hash;

    loop {
        if unblamed.is_empty() {
            break;
        }

        let hash = ObjectHash::from_hex(current_hash.clone());
        let commit = match store.read(&hash) {
            Ok(Object::Commit(c)) => c,
            _ => break,
        };

        // Get file content at this commit
        if let Some(file_content) = get_file_at_commit(&store, &commit.tree, &file) {
            let commit_lines: Vec<&str> = file_content.lines().collect();

            // Check parent to see which lines were introduced by this commit
            let parent_content = commit.parents.first().and_then(|parent_hash| {
                if let Ok(Object::Commit(parent)) = store.read(parent_hash) {
                    get_file_at_commit(&store, &parent.tree, &file)
                } else {
                    None
                }
            });

            let parent_lines: Vec<&str> = parent_content
                .as_deref()
                .map(|c| c.lines().collect())
                .unwrap_or_default();

            // Lines in this commit but not in parent → introduced here
            let mut newly_unblamed = Vec::new();
            for &idx in &unblamed {
                if idx < commit_lines.len() {
                    let line = commit_lines[idx];
                    let in_parent = idx < parent_lines.len() && parent_lines[idx] == line;

                    if !in_parent {
                        blame[idx].commit_hash =
                            current_hash[..16.min(current_hash.len())].to_string();
                        blame[idx].author = commit.author.clone();
                        blame[idx].timestamp = commit.timestamp;
                    } else {
                        newly_unblamed.push(idx);
                    }
                } else {
                    newly_unblamed.push(idx);
                }
            }
            unblamed = newly_unblamed;
        }

        // Move to parent
        match commit.parents.first() {
            Some(parent) => current_hash = parent.to_string(),
            None => {
                // Root commit — attribute remaining lines
                if let Ok(Object::Commit(c)) = store.read(&hash) {
                    for &idx in &unblamed {
                        blame[idx].commit_hash =
                            hash.as_str()[..16.min(hash.as_str().len())].to_string();
                        blame[idx].author = c.author.clone();
                        blame[idx].timestamp = c.timestamp;
                    }
                }
                break;
            }
        }
    }

    Ok(BlameResponse { file, lines: blame })
}

fn get_file_at_commit(
    store: &ObjectStore,
    tree_hash: &ObjectHash,
    file_path: &str,
) -> Option<String> {
    let tree = match store.read(tree_hash) {
        Ok(Object::Tree(t)) => t,
        _ => return None,
    };

    for entry in &tree.entries {
        if entry.name == file_path && entry.object_type == "blob" {
            if let Ok(Object::Blob(blob)) = store.read(&entry.hash) {
                return std::str::from_utf8(&blob.content)
                    .ok()
                    .map(|s| s.to_string());
            }
        }
    }

    None
}
