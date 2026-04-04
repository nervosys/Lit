//! Converge — merge intent commits into the mainline
//!
//! Replaces the PR merge ceremony with a trust-gated, scope-verified
//! convergence operation.  Supports auto, rebase, squash, and accumulate
//! strategies.

use crate::commands::intent;
use crate::core::merge::{find_merge_base, is_ancestor, merge_trees, MergeStrategy};
use crate::core::{
    find_repo_root, get_current_branch, read_head, write_ref, Commit, Object, ObjectHash,
};
use crate::errors::LitError;
use crate::response::ConvergeResponse;
use crate::storage::ObjectStore;

/// Supported convergence strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvergeStrategy {
    Auto,
    Rebase,
    Squash,
    Accumulate,
}

impl std::str::FromStr for ConvergeStrategy {
    type Err = LitError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(Self::Auto),
            "rebase" => Ok(Self::Rebase),
            "squash" => Ok(Self::Squash),
            "accumulate" => Ok(Self::Accumulate),
            _ => Err(LitError::general(format!(
                "Unknown converge strategy: '{}' (expected auto, rebase, squash, accumulate)",
                s
            ))),
        }
    }
}

/// Execute convergence of an intent's commits into the current branch.
pub fn execute(
    intent_id: String,
    strategy: Option<String>,
    verify: bool,
    dry_run: bool,
    target_branch: Option<String>,
) -> Result<ConvergeResponse, LitError> {
    let repo_root = find_repo_root()?;
    let store = ObjectStore::new(&repo_root);

    // Load and validate intent
    let intent_data = intent::load_intent(&repo_root, &intent_id)?;

    if intent_data.status != intent::IntentStatus::Active {
        return Err(LitError::general(format!(
            "Intent {} is {} — only active intents can be converged",
            intent_id, intent_data.status
        )));
    }

    if intent_data.commits.is_empty() {
        return Err(LitError::general(format!(
            "Intent {} has no commits to converge",
            intent_id
        )));
    }

    let strat = match strategy {
        Some(ref s) => s.parse::<ConvergeStrategy>()?,
        None => ConvergeStrategy::Auto,
    };

    // Optionally run integrity verification on the intent's commits
    if verify {
        for hash in &intent_data.commits {
            let obj_hash = ObjectHash::from_hex(hash.clone());
            match store.read(&obj_hash) {
                Ok(Object::Commit(_)) => {}
                Ok(_) => {
                    return Err(LitError::general(format!(
                        "Verify failed: {} is not a commit object",
                        hash
                    )));
                }
                Err(e) => {
                    return Err(LitError::general(format!(
                        "Verify failed: cannot read commit {}: {}",
                        hash, e
                    )));
                }
            }
        }
    }

    // Check child intents — for accumulate, all children must be converged
    if strat == ConvergeStrategy::Accumulate && !intent_data.children.is_empty() {
        for child_id in &intent_data.children {
            let child = intent::load_intent(&repo_root, child_id)?;
            if child.status != intent::IntentStatus::Converged {
                return Err(LitError::general(format!(
                    "Child intent {} is {} — all children must be converged before accumulate",
                    child_id, child.status
                )));
            }
        }
    }

    // Resolve current HEAD
    let head_hash_str = read_head(&repo_root)?;
    let head_hash = ObjectHash::from_hex(head_hash_str);
    let current_branch = target_branch
        .unwrap_or_else(|| get_current_branch(&repo_root).unwrap_or_else(|_| "main".to_string()));

    // The intent's last commit is the "tip" we converge
    let tip_hash_str = intent_data.commits.last().unwrap().clone();
    let tip_hash = ObjectHash::from_hex(tip_hash_str.clone());

    if dry_run {
        let can_ff = is_ancestor(&store, &head_hash, &tip_hash).unwrap_or(false);
        return Ok(ConvergeResponse {
            converged: false,
            strategy: format!("{:?}", strat).to_lowercase(),
            intent_id: intent_id.clone(),
            intent_title: intent_data.title.clone(),
            commit_hash: None,
            commits_converged: intent_data.commits.len(),
            fast_forward: can_ff,
            message: format!(
                "Dry-run: would converge {} commit(s) from '{}' via {} (ff={})",
                intent_data.commits.len(),
                intent_data.title,
                format!("{:?}", strat).to_lowercase(),
                can_ff,
            ),
            details: Some(serde_json::json!({
                "intent": intent_data,
                "dry_run": true,
                "fast_forward_possible": can_ff,
            })),
        });
    }

    // ── Execute strategy ────────────────────────────────────────────────────

    let (merge_hash, was_ff) = match strat {
        ConvergeStrategy::Auto | ConvergeStrategy::Rebase => {
            // Try fast-forward first
            if is_ancestor(&store, &head_hash, &tip_hash)? {
                // FF: move branch pointer
                write_ref(
                    &repo_root,
                    &format!("heads/{}", current_branch),
                    tip_hash.as_str(),
                )?;
                (tip_hash_str.clone(), true)
            } else {
                // 3-way merge
                let merge_base = find_merge_base(&store, &head_hash, &tip_hash)?;
                let ours_tree = get_commit_tree(&store, &head_hash)?;
                let theirs_tree = get_commit_tree(&store, &tip_hash)?;
                let base_tree = match &merge_base {
                    Some(bh) => Some(get_commit_tree(&store, bh)?),
                    None => None,
                };

                let result = merge_trees(
                    &store,
                    base_tree.as_ref(),
                    &ours_tree,
                    &theirs_tree,
                    MergeStrategy::Recursive,
                )?;

                if result.has_conflicts {
                    return Err(LitError::general(format!(
                        "Converge of '{}' has merge conflicts — resolve manually then retry",
                        intent_data.title
                    )));
                }

                let tree_hash = result
                    .tree
                    .ok_or_else(|| LitError::general("Merge produced no tree"))?;

                let author = intent_data.agent.clone();
                let commit = Commit::new(
                    tree_hash,
                    vec![head_hash.clone(), tip_hash.clone()],
                    author,
                    format!("Converge intent '{}' ({})", intent_data.title, intent_id),
                );
                let obj = Object::Commit(commit);
                let hash = store.write(&obj)?;

                write_ref(
                    &repo_root,
                    &format!("heads/{}", current_branch),
                    hash.as_str(),
                )?;
                (hash.to_string(), false)
            }
        }

        ConvergeStrategy::Squash => {
            // Create a single squash commit from intent tip's tree
            let ours_tree = get_commit_tree(&store, &head_hash)?;
            let theirs_tree = get_commit_tree(&store, &tip_hash)?;
            let merge_base = find_merge_base(&store, &head_hash, &tip_hash)?;
            let base_tree = match &merge_base {
                Some(bh) => Some(get_commit_tree(&store, bh)?),
                None => None,
            };

            let result = merge_trees(
                &store,
                base_tree.as_ref(),
                &ours_tree,
                &theirs_tree,
                MergeStrategy::Recursive,
            )?;

            if result.has_conflicts {
                return Err(LitError::general(format!(
                    "Squash converge of '{}' has conflicts — resolve manually",
                    intent_data.title
                )));
            }

            let tree_hash = result
                .tree
                .ok_or_else(|| LitError::general("Squash produced no tree"))?;

            let message = format!(
                "Squash converge: {} ({} commit(s))\n\nIntent: {}\nAgent: {}",
                intent_data.title,
                intent_data.commits.len(),
                intent_id,
                intent_data.agent,
            );

            let commit = Commit::new(
                tree_hash,
                vec![head_hash.clone()],
                intent_data.agent.clone(),
                message,
            );
            let obj = Object::Commit(commit);
            let hash = store.write(&obj)?;

            write_ref(
                &repo_root,
                &format!("heads/{}", current_branch),
                hash.as_str(),
            )?;
            (hash.to_string(), false)
        }

        ConvergeStrategy::Accumulate => {
            // Queue: just mark as converged without advancing branch —
            // parent intent will converge all children.
            // If this *is* the parent, do a standard merge.
            if intent_data.children.is_empty() {
                // Leaf intent in accumulate mode: mark converged, no merge
                intent::mark_converged(&repo_root, &intent_id)?;
                return Ok(ConvergeResponse {
                    converged: true,
                    strategy: "accumulate".into(),
                    intent_id: intent_id.clone(),
                    intent_title: intent_data.title.clone(),
                    commit_hash: None,
                    commits_converged: intent_data.commits.len(),
                    fast_forward: false,
                    message: format!(
                        "Intent '{}' queued for parent convergence",
                        intent_data.title
                    ),
                    details: None,
                });
            }

            // Parent with all children converged — merge tip
            if is_ancestor(&store, &head_hash, &tip_hash)? {
                write_ref(
                    &repo_root,
                    &format!("heads/{}", current_branch),
                    tip_hash.as_str(),
                )?;
                (tip_hash_str.clone(), true)
            } else {
                let merge_base = find_merge_base(&store, &head_hash, &tip_hash)?;
                let ours_tree = get_commit_tree(&store, &head_hash)?;
                let theirs_tree = get_commit_tree(&store, &tip_hash)?;
                let base_tree = match &merge_base {
                    Some(bh) => Some(get_commit_tree(&store, bh)?),
                    None => None,
                };
                let result = merge_trees(
                    &store,
                    base_tree.as_ref(),
                    &ours_tree,
                    &theirs_tree,
                    MergeStrategy::Recursive,
                )?;
                if result.has_conflicts {
                    return Err(LitError::general(format!(
                        "Accumulate converge of '{}' has conflicts",
                        intent_data.title
                    )));
                }
                let tree_hash = result
                    .tree
                    .ok_or_else(|| LitError::general("Accumulate merge produced no tree"))?;
                let commit = Commit::new(
                    tree_hash,
                    vec![head_hash, tip_hash],
                    intent_data.agent.clone(),
                    format!(
                        "Accumulate converge: {} ({} child intents)",
                        intent_data.title,
                        intent_data.children.len()
                    ),
                );
                let obj = Object::Commit(commit);
                let hash = store.write(&obj)?;
                write_ref(
                    &repo_root,
                    &format!("heads/{}", current_branch),
                    hash.as_str(),
                )?;
                (hash.to_string(), false)
            }
        }
    };

    // Mark intent as converged and release leases
    let converged_intent = intent::mark_converged(&repo_root, &intent_id)?;

    Ok(ConvergeResponse {
        converged: true,
        strategy: format!("{:?}", strat).to_lowercase(),
        intent_id: intent_id.clone(),
        intent_title: converged_intent.title.clone(),
        commit_hash: Some(merge_hash),
        commits_converged: converged_intent.commits.len(),
        fast_forward: was_ff,
        message: format!(
            "Converged intent '{}' — {} commit(s) via {} strategy",
            converged_intent.title,
            converged_intent.commits.len(),
            format!("{:?}", strat).to_lowercase(),
        ),
        details: Some(serde_json::json!({
            "intent": converged_intent,
        })),
    })
}

fn get_commit_tree(
    store: &ObjectStore,
    commit_hash: &ObjectHash,
) -> Result<crate::core::Tree, LitError> {
    let commit = match store.read(commit_hash)? {
        Object::Commit(c) => c,
        _ => {
            return Err(LitError::general(format!(
                "Expected commit: {}",
                commit_hash
            )))
        }
    };
    match store.read(&commit.tree)? {
        Object::Tree(t) => Ok(t),
        _ => Err(LitError::general(format!("Expected tree: {}", commit.tree))),
    }
}
