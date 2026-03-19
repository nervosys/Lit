use crate::core::{find_repo_root, get_current_branch};
use crate::response::StatusResponse;
use crate::storage::Index;
use std::collections::HashSet;
use std::fs;
use walkdir::WalkDir;

pub fn execute() -> Result<StatusResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let index = Index::load(&repo_root)?;

    let branch = get_current_branch(&repo_root).ok();

    // Get working directory files
    let mut working_files = HashSet::new();
    for entry in WalkDir::new(&repo_root)
        .into_iter()
        .filter_entry(|e| !is_hidden(e))
    {
        let entry = entry.map_err(|e| format!("Failed to read directory: {}", e))?;

        if entry.file_type().is_file() {
            if let Ok(rel_path) = entry.path().strip_prefix(&repo_root) {
                let path_str = rel_path.to_string_lossy().replace('\\', "/");
                working_files.insert(path_str);
            }
        }
    }

    // Find staged files
    let staged_files: HashSet<String> = index.entries.keys().cloned().collect();

    // Find modified files (in working dir but different from index)
    let mut modified = Vec::new();
    let mut untracked = Vec::new();

    for file in &working_files {
        if staged_files.contains(file) {
            if is_modified(&repo_root, file, &index)? {
                modified.push(file.clone());
            }
        } else {
            untracked.push(file.clone());
        }
    }

    modified.sort();
    untracked.sort();
    let mut staged: Vec<String> = staged_files.into_iter().collect();
    staged.sort();

    let clean = staged.is_empty() && modified.is_empty() && untracked.is_empty();

    let head = crate::core::read_head(&repo_root).ok();

    Ok(StatusResponse {
        branch,
        head,
        staged,
        modified,
        untracked,
        clean,
    })
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

fn is_modified(repo_root: &std::path::Path, file: &str, index: &Index) -> Result<bool, crate::errors::LitError> {
    let file_path = repo_root.join(file);

    if let Some(entry) = index.entries.get(file) {
        let current_content =
            fs::read(&file_path).map_err(|e| format!("Failed to read file: {}", e))?;

        use crate::core::{Blob, Object};
        let blob = Blob::new(current_content);
        let object = Object::Blob(blob);
        let current_hash = object.hash();

        Ok(current_hash.to_string() != entry.hash)
    } else {
        Ok(false)
    }
}
