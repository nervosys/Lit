use crate::core::{find_repo_root, get_current_branch, list_refs};
use crate::errors::LitError;
use crate::response::CommandResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CleanResponse {
    pub removed: Vec<String>,
    pub message: String,
}

impl CommandResponse for CleanResponse {
    fn command_name(&self) -> &'static str {
        "clean"
    }
    fn human_readable(&self) -> String {
        if self.removed.is_empty() {
            "No empty branches to clean\n".to_string()
        } else {
            let mut out = format!("{}\n", self.message);
            for b in &self.removed {
                out.push_str(&format!("  Removed: {}\n", b));
            }
            out
        }
    }
}

/// Remove empty branches from the workspace.
/// A branch is considered "empty" if it points to the same commit as its
/// base/parent branch or is identical to the default branch.
pub fn execute(dry_run: bool) -> Result<CleanResponse, LitError> {
    let repo_root = find_repo_root()?;
    let current = get_current_branch(&repo_root)?;
    let refs = list_refs(&repo_root, "heads").unwrap_or_default();

    // Load stack metadata to understand branch relationships
    let stack_meta_path = repo_root.join(".lit").join("stack.json");
    let stack_bases: std::collections::HashMap<String, String> = if stack_meta_path.exists() {
        #[derive(Deserialize)]
        struct SM {
            bases: std::collections::HashMap<String, String>,
        }
        match std::fs::read_to_string(&stack_meta_path) {
            Ok(data) => serde_json::from_str::<SM>(&data)
                .map(|s| s.bases)
                .unwrap_or_default(),
            Err(_) => std::collections::HashMap::new(),
        }
    } else {
        std::collections::HashMap::new()
    };

    // Find branches that point to the same commit as their base
    let mut to_remove = Vec::new();
    let default_branch_hash = refs
        .iter()
        .find(|r| r.name == "main" || r.name == "master")
        .map(|r| r.hash.clone());

    for r in &refs {
        if r.name == current {
            continue; // Never remove current branch
        }
        if r.name == "main" || r.name == "master" {
            continue; // Never remove default branch
        }

        let is_empty = if let Some(base_name) = stack_bases.get(&r.name) {
            // Check if branch points to same commit as its stack base
            refs.iter()
                .find(|br| br.name == *base_name)
                .map(|br| br.hash == r.hash)
                .unwrap_or(false)
        } else if let Some(ref dh) = default_branch_hash {
            // Check if branch points to same commit as default branch
            &r.hash == dh
        } else {
            false
        };

        if is_empty {
            to_remove.push(r.name.clone());
        }
    }

    if !dry_run {
        for name in &to_remove {
            let _ = crate::core::delete_ref(&repo_root, &format!("heads/{}", name));
        }
    }

    let msg = if dry_run {
        format!("Would remove {} empty branch(es)", to_remove.len())
    } else {
        format!("Removed {} empty branch(es)", to_remove.len())
    };

    Ok(CleanResponse {
        removed: to_remove,
        message: msg,
    })
}
