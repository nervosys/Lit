use crate::core::{find_repo_root, Blob, Object};
use crate::response::AddResponse;
use crate::storage::{Index, ObjectStore};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn execute(files: Vec<String>) -> Result<AddResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);
    let mut index = Index::load(&repo_root)?;

    let mut total_added = 0usize;

    for file_pattern in &files {
        if file_pattern == "." {
            total_added += add_directory(&repo_root, &repo_root, &store, &mut index)?;
        } else {
            let file_path = PathBuf::from(file_pattern);
            let full_path = if file_path.is_absolute() {
                file_path
            } else {
                std::env::current_dir()
                    .map_err(|e| format!("Failed to get current directory: {}", e))?
                    .join(&file_path)
            };

            if !full_path.exists() {
                return Err(format!("File not found: {}", file_pattern));
            }

            if full_path.is_dir() {
                total_added += add_directory(&repo_root, &full_path, &store, &mut index)?;
            } else {
                add_file(&repo_root, &full_path, &store, &mut index)?;
                total_added += 1;
            }
        }
    }

    index.save(&repo_root)?;

    Ok(AddResponse {
        files_added: total_added,
    })
}

fn add_file(
    repo_root: &Path,
    file_path: &Path,
    store: &ObjectStore,
    index: &mut Index,
) -> Result<(), String> {
    // Skip .lit directory
    if file_path.starts_with(repo_root.join(".lit")) {
        return Ok(());
    }

    // Read file content
    let content = fs::read(file_path)
        .map_err(|e| format!("Failed to read file {}: {}", file_path.display(), e))?;

    // Create blob object
    let blob = Blob::new(content);
    let object = Object::Blob(blob);

    // Write to object store
    let hash = store.write(&object)?;

    // Get relative path from repo root
    let rel_path = file_path
        .strip_prefix(repo_root)
        .map_err(|e| format!("Path error: {}", e))?
        .to_string_lossy()
        .replace('\\', "/");

    // Add to index
    index.add(rel_path, hash.to_string(), "100644".to_string());

    Ok(())
}

fn add_directory(
    repo_root: &Path,
    dir_path: &Path,
    store: &ObjectStore,
    index: &mut Index,
) -> Result<usize, String> {
    let mut count = 0usize;
    for entry in WalkDir::new(dir_path)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_hidden(e))
    {
        let entry = entry.map_err(|e| format!("Failed to read directory: {}", e))?;

        if entry.file_type().is_file() {
            add_file(repo_root, entry.path(), store, index)?;
            count += 1;
        }
    }

    Ok(count)
}

fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}
