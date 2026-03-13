use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash, Tree,
};
use crate::response::CherryPickResponse;
use crate::storage::ObjectStore;

pub fn execute(target: String) -> Result<CherryPickResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Resolve target to commit
    let commit_hash = resolve_rev(&repo_root, &target)?;
    let hash_obj = ObjectHash::from_hex(commit_hash.clone());

    let commit = match store.read(&hash_obj)? {
        Object::Commit(c) => c,
        _ => return Err(format!("'{}' is not a commit", target)),
    };

    // Get parent tree of the commit being cherry-picked
    let parent_hash = commit
        .parents
        .first()
        .ok_or("Cannot cherry-pick a root commit")?;
    let parent_commit = match store.read(parent_hash)? {
        Object::Commit(c) => c,
        _ => return Err("Parent is not a commit".to_string()),
    };

    // Get current HEAD
    let head_hash_str = read_head(&repo_root)?;
    let head_hash = ObjectHash::from_hex(head_hash_str.clone());
    let head_commit = match store.read(&head_hash)? {
        Object::Commit(c) => c,
        _ => return Err("HEAD is not a commit".to_string()),
    };

    // Apply the diff (parent → commit) onto HEAD
    let parent_files = load_tree_files(&store, &parent_commit.tree)?;
    let commit_files = load_tree_files(&store, &commit.tree)?;
    let head_files = load_tree_files(&store, &head_commit.tree)?;

    let mut new_tree = Tree::new();
    let mut changed_files = Vec::new();

    // Start with HEAD tree
    for (path, (hash, mode)) in &head_files {
        let in_parent = parent_files.get(path);
        let in_commit = commit_files.get(path);

        match (in_parent, in_commit) {
            // File modified by the cherry-picked commit
            (Some(pv), Some(cv)) if pv.0 != cv.0 => {
                changed_files.push(path.clone());
                new_tree.add_entry(
                    cv.1.clone(),
                    path.clone(),
                    ObjectHash::from_hex(cv.0.clone()),
                    "blob".to_string(),
                );
            }
            // File deleted by the cherry-picked commit
            (Some(_), None) => {
                changed_files.push(path.clone());
                continue; // Don't include in new tree
            }
            // File not changed by cherry-picked commit → keep HEAD version
            _ => {
                new_tree.add_entry(
                    mode.clone(),
                    path.clone(),
                    ObjectHash::from_hex(hash.clone()),
                    "blob".to_string(),
                );
            }
        }
    }

    // Files added by the cherry-picked commit
    for (path, (hash, mode)) in &commit_files {
        if !parent_files.contains_key(path) && !head_files.contains_key(path) {
            changed_files.push(path.clone());
            new_tree.add_entry(
                mode.clone(),
                path.clone(),
                ObjectHash::from_hex(hash.clone()),
                "blob".to_string(),
            );
        }
    }

    let tree_hash = store.write(&Object::Tree(new_tree))?;

    let author = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "Unknown".to_string());

    let new_commit = Commit::new(
        tree_hash,
        vec![ObjectHash::from_hex(head_hash_str)],
        author,
        commit.message.clone(),
    );

    let new_hash = store.write(&Object::Commit(new_commit))?;

    let branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    write_ref(&repo_root, &format!("heads/{}", branch), new_hash.as_str())?;

    Ok(CherryPickResponse {
        source_commit: commit_hash[..16.min(commit_hash.len())].to_string(),
        new_commit: new_hash.short(),
        files_changed: changed_files.len(),
        message: format!(
            "Cherry-picked {} as {}",
            &commit_hash[..16.min(commit_hash.len())],
            new_hash.short()
        ),
    })
}

fn load_tree_files(
    store: &ObjectStore,
    tree_hash: &ObjectHash,
) -> Result<std::collections::HashMap<String, (String, String)>, String> {
    let tree = match store.read(tree_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".to_string()),
    };
    let mut files = std::collections::HashMap::new();
    for entry in &tree.entries {
        files.insert(
            entry.name.clone(),
            (entry.hash.to_string(), entry.mode.clone()),
        );
    }
    Ok(files)
}

fn resolve_rev(repo_root: &std::path::Path, target: &str) -> Result<String, String> {
    if target.starts_with("HEAD~") || target.starts_with("HEAD^") {
        let count: usize = target[5..].parse().unwrap_or(1);
        let mut current = read_head(repo_root)?;
        let store = ObjectStore::new(repo_root);
        for _ in 0..count {
            let hash = ObjectHash::from_hex(current);
            let commit = match store.read(&hash)? {
                Object::Commit(c) => c,
                _ => return Err("Not a commit in history".to_string()),
            };
            current = commit
                .parents
                .first()
                .ok_or("No parent commit")?
                .to_string();
        }
        return Ok(current);
    }
    if target == "HEAD" {
        return read_head(repo_root);
    }
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("heads/{}", target)) {
        return Ok(hash);
    }
    if let Ok(hash) = crate::core::read_ref(repo_root, &format!("tags/{}", target)) {
        return Ok(hash);
    }
    if target.len() >= 16 && target.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(target.to_string());
    }
    Err(format!("Cannot resolve '{}' to a commit", target))
}
