use crate::core::merge::{find_merge_base, is_ancestor, merge_trees, MergeStrategy};
use crate::core::{
    find_repo_root, get_current_branch, read_head, read_ref, write_ref, Commit, Object, ObjectHash,
};
use crate::response::MergeResponse;
use crate::storage::ObjectStore;
use std::str::FromStr;

pub fn execute(branch: String, strategy: Option<String>) -> Result<MergeResponse, String> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    let strategy = match &strategy {
        Some(s) => MergeStrategy::from_str(s)?,
        None => MergeStrategy::Recursive,
    };

    // Resolve HEAD (ours)
    let head_hash_str = read_head(&repo_root)?;
    let head_hash = ObjectHash::from_hex(head_hash_str);

    // Resolve target branch (try heads/, then remotes/)
    let target_hash_str = read_ref(&repo_root, &format!("heads/{}", branch))
        .or_else(|_| read_ref(&repo_root, &format!("remotes/{}", branch)))?;
    let target_hash = ObjectHash::from_hex(target_hash_str);

    // Check if already up to date
    if head_hash.to_string() == target_hash.to_string() {
        return Ok(MergeResponse {
            merged: true,
            fast_forward: false,
            commit_hash: None,
            message: "Already up to date.".to_string(),
            has_conflicts: false,
            file_results: vec![],
            strategy: format!("{:?}", strategy).to_lowercase(),
        });
    }

    // Check for fast-forward: if HEAD is an ancestor of target
    if is_ancestor(&store, &head_hash, &target_hash)? {
        // Fast-forward: just move HEAD to target
        let current_branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
        write_ref(
            &repo_root,
            &format!("heads/{}", current_branch),
            target_hash.as_str(),
        )?;

        return Ok(MergeResponse {
            merged: true,
            fast_forward: true,
            commit_hash: Some(target_hash.to_string()),
            message: format!("Fast-forward merge to {}", target_hash.short()),
            has_conflicts: false,
            file_results: vec![],
            strategy: format!("{:?}", strategy).to_lowercase(),
        });
    }

    // Check if target is ancestor of HEAD (already merged)
    if is_ancestor(&store, &target_hash, &head_hash)? {
        return Ok(MergeResponse {
            merged: true,
            fast_forward: false,
            commit_hash: None,
            message: "Already up to date.".to_string(),
            has_conflicts: false,
            file_results: vec![],
            strategy: format!("{:?}", strategy).to_lowercase(),
        });
    }

    // Find merge base
    let merge_base = find_merge_base(&store, &head_hash, &target_hash)?;

    // Load trees
    let ours_tree = get_commit_tree(&store, &head_hash)?;
    let theirs_tree = get_commit_tree(&store, &target_hash)?;
    let base_tree = match &merge_base {
        Some(base_hash) => Some(get_commit_tree(&store, base_hash)?),
        None => None,
    };

    // Perform 3-way merge
    let merge_result = merge_trees(
        &store,
        base_tree.as_ref(),
        &ours_tree,
        &theirs_tree,
        strategy,
    )?;

    let file_results: Vec<crate::response::FileMergeInfo> = merge_result
        .file_results
        .iter()
        .map(|r| crate::response::FileMergeInfo {
            path: r.path.clone(),
            status: format!("{:?}", r.status).to_lowercase(),
            conflict_count: r.conflicts.len(),
        })
        .collect();

    if merge_result.has_conflicts && strategy == MergeStrategy::Recursive {
        // Write conflict state for later resolution
        let conflict_dir = repo_root.join(".lit").join("merge");
        std::fs::create_dir_all(&conflict_dir)
            .map_err(|e| format!("Failed to create merge state dir: {}", e))?;

        // Save conflict metadata
        let conflict_data = serde_json::to_string_pretty(&merge_result.file_results)
            .map_err(|e| format!("Failed to serialize conflicts: {}", e))?;
        std::fs::write(conflict_dir.join("conflicts.json"), conflict_data)
            .map_err(|e| format!("Failed to write conflict state: {}", e))?;

        // Save merge head
        std::fs::write(conflict_dir.join("MERGE_HEAD"), target_hash.to_string())
            .map_err(|e| format!("Failed to write MERGE_HEAD: {}", e))?;

        let conflict_count: usize = merge_result
            .file_results
            .iter()
            .filter(|r| r.status == crate::core::merge::FileMergeStatus::Conflict)
            .count();

        return Ok(MergeResponse {
            merged: false,
            fast_forward: false,
            commit_hash: None,
            message: format!(
                "CONFLICT: Merge of '{}' has {} conflicting file(s). Use 'lit resolve' to resolve.",
                branch, conflict_count
            ),
            has_conflicts: true,
            file_results,
            strategy: format!("{:?}", strategy).to_lowercase(),
        });
    }

    // Create merge commit
    let tree_hash = merge_result.tree.ok_or("Merge failed to produce a tree")?;

    let current_branch = get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string());
    let author = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "Unknown".to_string());

    let commit = Commit::new(
        tree_hash,
        vec![head_hash, target_hash],
        author,
        format!("Merge branch '{}'", branch),
    );

    let commit_obj = Object::Commit(commit);
    let commit_hash = store.write(&commit_obj)?;

    write_ref(
        &repo_root,
        &format!("heads/{}", current_branch),
        commit_hash.as_str(),
    )?;

    // Clean up any merge state
    let merge_dir = repo_root.join(".lit").join("merge");
    if merge_dir.exists() {
        let _ = std::fs::remove_dir_all(&merge_dir);
    }

    Ok(MergeResponse {
        merged: true,
        fast_forward: false,
        commit_hash: Some(commit_hash.to_string()),
        message: format!("Merge branch '{}' ({})", branch, commit_hash.short()),
        has_conflicts: false,
        file_results,
        strategy: format!("{:?}", strategy).to_lowercase(),
    })
}

fn get_commit_tree(
    store: &ObjectStore,
    commit_hash: &ObjectHash,
) -> Result<crate::core::Tree, String> {
    let commit = match store.read(commit_hash)? {
        Object::Commit(c) => c,
        _ => return Err(format!("Expected commit object for {}", commit_hash)),
    };
    match store.read(&commit.tree)? {
        Object::Tree(t) => Ok(t),
        _ => Err(format!("Expected tree object for {}", commit.tree)),
    }
}
