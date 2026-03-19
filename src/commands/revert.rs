use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash,
    Tree,
};
use crate::response::RevertResponse;
use crate::storage::ObjectStore;

pub fn execute(target: String) -> Result<RevertResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Resolve target to commit
    let commit_hash = crate::commands::reset::execute_resolve(&repo_root, &target)?;
    let hash_obj = ObjectHash::from_hex(commit_hash.clone());

    let commit = match store.read(&hash_obj)? {
        Object::Commit(c) => c,
        _ => return Err(format!("'{}' is not a commit", target).into()),
    };

    // Get the parent of the commit to revert
    let parent_hash = commit
        .parents
        .first()
        .ok_or("Cannot revert a root commit")?;

    let parent_commit = match store.read(parent_hash)? {
        Object::Commit(c) => c,
        _ => return Err("Parent is not a commit".into()),
    };

    // Get current HEAD tree
    let head_hash = read_head(&repo_root)?;
    let head_obj = ObjectHash::from_hex(head_hash.clone());
    let head_commit = match store.read(&head_obj)? {
        Object::Commit(c) => c,
        _ => return Err("HEAD is not a commit".into()),
    };

    // Build the inverse: for each file changed in commit, restore to parent state
    let commit_tree = load_tree_files(&store, &commit.tree)?;
    let parent_tree = load_tree_files(&store, &parent_commit.tree)?;
    let head_tree = load_tree_files(&store, &head_commit.tree)?;

    let mut new_tree = Tree::new();
    let mut reverted_files = Vec::new();

    // Start with HEAD tree, apply inverse changes
    for (path, (hash, mode)) in &head_tree {
        let commit_version = commit_tree.get(path);
        let parent_version = parent_tree.get(path);

        match (commit_version, parent_version) {
            // File was added in the commit â†’ remove it in revert
            (Some(_), None) => {
                reverted_files.push(path.clone());
                continue; // Don't add to new tree
            }
            // File was modified in the commit â†’ restore parent version
            (Some(cv), Some(pv)) if cv.0 != pv.0 => {
                reverted_files.push(path.clone());
                new_tree.add_entry(
                    pv.1.clone(),
                    path.clone(),
                    ObjectHash::from_hex(pv.0.clone()),
                    "blob".to_string(),
                );
            }
            // File not changed by the commit â†’ keep current
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

    // Files deleted in the commit â†’ restore them
    for (path, (hash, mode)) in &parent_tree {
        if !commit_tree.contains_key(path) && !head_tree.contains_key(path) {
            reverted_files.push(path.clone());
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

    let revert_commit = Commit::new(
        tree_hash,
        vec![ObjectHash::from_hex(head_hash)],
        author,
        format!("Revert \"{}\"", commit.message),
    );

    let revert_hash = store.write(&Object::Commit(revert_commit))?;

    // Update branch ref
    let branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    write_ref(
        &repo_root,
        &format!("heads/{}", branch),
        revert_hash.as_str(),
    )?;

    Ok(RevertResponse {
        reverted_commit: commit_hash[..16.min(commit_hash.len())].to_string(),
        new_commit: revert_hash.short(),
        files_changed: reverted_files.len(),
        message: format!(
            "Reverted commit {} in {}",
            &commit_hash[..16.min(commit_hash.len())],
            revert_hash.short()
        ),
    })
}

fn load_tree_files(
    store: &ObjectStore,
    tree_hash: &ObjectHash,
) -> Result<std::collections::HashMap<String, (String, String)>, String> {
    let tree = match store.read(tree_hash)? {
        Object::Tree(t) => t,
        _ => return Err("Not a tree".into()),
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
