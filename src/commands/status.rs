use crate::core::{find_repo_root, get_current_branch};
use crate::response::StatusResponse;
use crate::storage::Index;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Maximum files to visit during the untracked-file walk before stopping.
const MAX_WALK_FILES: usize = 50_000;

pub fn execute() -> Result<StatusResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let index = Index::load(&repo_root)?;

    let branch = get_current_branch(&repo_root).ok();
    let ignore_dirs = load_ignore_dirs(&repo_root);

    // Phase 1: Check index entries for modifications (no filesystem walk needed)
    let staged_set: HashSet<String> = index.entries.keys().cloned().collect();
    let mut modified = Vec::new();
    for file in &staged_set {
        if is_modified(&repo_root, file, &index)? {
            modified.push(file.clone());
        }
    }

    // Phase 2: Walk for untracked files.
    // Walk from CWD if it's inside repo_root (scopes the search to what the
    // user is actually working on), otherwise fall back to repo_root.
    let cwd = std::env::current_dir().unwrap_or_else(|_| repo_root.clone());
    let walk_root = if cwd.starts_with(&repo_root) {
        &cwd
    } else {
        &repo_root
    };

    let mut untracked = Vec::new();
    let mut visited: usize = 0;
    for entry in WalkDir::new(walk_root)
        .into_iter()
        .filter_entry(|e| !should_skip_entry(e, &ignore_dirs))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.file_type().is_file() {
            visited += 1;
            if visited > MAX_WALK_FILES {
                break;
            }
            if let Ok(rel_path) = entry.path().strip_prefix(&repo_root) {
                let path_str = rel_path.to_string_lossy().replace('\\', "/");
                if !staged_set.contains(&path_str) {
                    untracked.push(path_str);
                }
            }
        }
    }

    modified.sort();
    untracked.sort();
    let mut staged: Vec<String> = staged_set.into_iter().collect();
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

/// Load directory names to ignore from built-in defaults + .litignore file.
fn load_ignore_dirs(repo_root: &Path) -> HashSet<String> {
    let mut dirs: HashSet<String> = [
        // VCS / build
        "target",
        "node_modules",
        "venv",
        "__pycache__",
        "dist",
        "out",
        "bin",
        "obj",
        // Toolchain caches
        ".cargo",
        ".rustup",
        ".npm",
        ".nvm",
        ".pyenv",
        ".conda",
        ".local",
        ".cache",
        ".thumbnails",
        // OS user-profile directories (home-dir-as-repo safety)
        "AppData",
        "Application Data",
        "Library",
        "Caches",
        // Cloud sync
        "OneDrive",
        "Dropbox",
        "Google Drive",
        // Windows user folders
        "Documents",
        "Downloads",
        "Desktop",
        "Music",
        "Pictures",
        "Videos",
        "Contacts",
        "Favorites",
        "Links",
        "Saved Games",
        "Searches",
        "3D Objects",
        "scoop",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    // Read .litignore (one directory name per line, # for comments)
    let ignore_path = repo_root.join(".litignore");
    if let Ok(content) = fs::read_to_string(&ignore_path) {
        for line in content.lines() {
            let line = line.trim().trim_end_matches('/');
            if !line.is_empty() && !line.starts_with('#') {
                dirs.insert(line.to_string());
            }
        }
    }

    dirs
}

/// Decide whether a walkdir entry should be pruned.
fn should_skip_entry(entry: &walkdir::DirEntry, ignore_dirs: &HashSet<String>) -> bool {
    let name = entry.file_name().to_string_lossy();

    // Always skip hidden (dot-prefixed)
    if name.starts_with('.') {
        return true;
    }

    // Skip directories by name
    entry.file_type().is_dir() && ignore_dirs.contains(name.as_ref())
}

fn is_modified(
    repo_root: &Path,
    file: &str,
    index: &Index,
) -> Result<bool, crate::errors::LitError> {
    let file_path = repo_root.join(file);

    if let Some(entry) = index.entries.get(file) {
        // Fast path: if file was deleted, it's modified
        if !file_path.exists() {
            return Ok(true);
        }

        let current_content =
            fs::read(&file_path).map_err(|e| format!("Failed to read file {}: {}", file, e))?;

        use crate::core::{Blob, Object};
        let blob = Blob::new(current_content);
        let object = Object::Blob(blob);
        let current_hash = object.hash();

        Ok(current_hash.to_string() != entry.hash)
    } else {
        Ok(false)
    }
}
