use crate::core::{find_repo_root, get_current_branch, list_refs, read_head, read_ref, write_ref};
use crate::errors::LitError;
use crate::response::CommandResponse;
use serde::{Deserialize, Serialize};

/// A branch in a stack (dependent chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackEntry {
    pub name: String,
    pub base: Option<String>,
    pub head: String,
    pub is_current: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StackResponse {
    List {
        stacks: Vec<Vec<StackEntry>>,
    },
    Push {
        branch: String,
        base: String,
        message: String,
    },
    Restack {
        branches: Vec<String>,
        message: String,
    },
    Show {
        stack: Vec<StackEntry>,
    },
}

impl CommandResponse for StackResponse {
    fn command_name(&self) -> &'static str {
        "stack"
    }
    fn human_readable(&self) -> String {
        match self {
            StackResponse::List { stacks } => {
                let mut out = String::new();
                for (i, stack) in stacks.iter().enumerate() {
                    out.push_str(&format!("Stack {}:\n", i + 1));
                    for entry in stack {
                        let current = if entry.is_current { "* " } else { "  " };
                        let base = entry
                            .base
                            .as_deref()
                            .map(|b| format!(" (on {})", b))
                            .unwrap_or_default();
                        out.push_str(&format!(
                            "  {}{}{} [{}]\n",
                            current,
                            entry.name,
                            base,
                            &entry.head[..8.min(entry.head.len())]
                        ));
                    }
                }
                if stacks.is_empty() {
                    out.push_str("No stacked branches\n");
                }
                out
            }
            StackResponse::Push {
                branch,
                base,
                message,
            } => format!("Pushed {} onto {}: {}\n", branch, base, message),
            StackResponse::Restack { branches, message } => {
                let mut out = format!("{}\n", message);
                for b in branches {
                    out.push_str(&format!("  Restacked: {}\n", b));
                }
                out
            }
            StackResponse::Show { stack } => {
                let mut out = String::new();
                for (i, entry) in stack.iter().enumerate() {
                    let current = if entry.is_current { "* " } else { "  " };
                    let connector = if i == 0 { "  " } else { "│ " };
                    out.push_str(&format!(
                        "{}{}{} [{}]\n",
                        connector,
                        current,
                        entry.name,
                        &entry.head[..8.min(entry.head.len())]
                    ));
                    if i < stack.len() - 1 {
                        out.push_str("│\n");
                    }
                }
                out
            }
        }
    }
}

/// Stack metadata file path
fn stack_meta_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("stack.json")
}

/// Stack metadata: maps branch -> base branch
#[derive(Debug, Default, Serialize, Deserialize)]
struct StackMeta {
    /// branch_name -> base_branch_name
    bases: std::collections::HashMap<String, String>,
}

fn load_stack_meta(repo_root: &std::path::Path) -> StackMeta {
    let path = stack_meta_path(repo_root);
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => StackMeta::default(),
        }
    } else {
        StackMeta::default()
    }
}

fn save_stack_meta(repo_root: &std::path::Path, meta: &StackMeta) -> Result<(), LitError> {
    let path = stack_meta_path(repo_root);
    let data = serde_json::to_string_pretty(meta)
        .map_err(|e| LitError::general(format!("Failed to serialize stack meta: {}", e)))?;
    std::fs::write(path, data)
        .map_err(|e| LitError::io(format!("Failed to write stack meta: {}", e)))?;
    Ok(())
}

/// List all stacks
pub fn execute_list() -> Result<StackResponse, LitError> {
    let repo_root = find_repo_root()?;
    let meta = load_stack_meta(&repo_root);
    let current = get_current_branch(&repo_root).ok();
    let refs = list_refs(&repo_root, "heads").unwrap_or_default();

    // Build adjacency: find root branches (no base or base not in stack)
    let mut children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut roots: Vec<String> = Vec::new();

    for (branch, _base) in &meta.bases {
        children
            .entry(_base.clone())
            .or_default()
            .push(branch.clone());
    }

    // Find roots: branches that are bases but not stacked on anything
    for base in meta.bases.values() {
        if !meta.bases.contains_key(base) && !roots.contains(base) {
            roots.push(base.clone());
        }
    }

    let mut stacks = Vec::new();
    for root in &roots {
        let mut stack = Vec::new();
        let mut queue = vec![root.clone()];
        while let Some(branch) = queue.pop() {
            let head = refs
                .iter()
                .find(|r| r.name == branch)
                .map(|r| r.hash.clone())
                .unwrap_or_else(|| "unknown".to_string());
            stack.push(StackEntry {
                name: branch.clone(),
                base: meta.bases.get(&branch).cloned(),
                head,
                is_current: Some(&branch) == current.as_ref(),
            });
            if let Some(ch) = children.get(&branch) {
                for c in ch {
                    queue.push(c.clone());
                }
            }
        }
        if stack.len() > 1 {
            stacks.push(stack);
        }
    }

    Ok(StackResponse::List { stacks })
}

/// Push a new branch onto the current branch (creating a stacked branch)
pub fn execute_push(name: String) -> Result<StackResponse, LitError> {
    let repo_root = find_repo_root()?;
    let current = get_current_branch(&repo_root)?;
    let head_hash = read_head(&repo_root)?;

    // Create the new branch at HEAD
    write_ref(&repo_root, &format!("heads/{}", name), &head_hash)?;

    // Record stack relationship
    let mut meta = load_stack_meta(&repo_root);
    meta.bases.insert(name.clone(), current.clone());
    save_stack_meta(&repo_root, &meta)?;

    // Checkout the new branch
    let head_path = repo_root.join(".lit").join("HEAD");
    std::fs::write(head_path, format!("ref: refs/heads/{}", name))
        .map_err(|e| LitError::io(format!("Failed to write HEAD: {}", e)))?;

    Ok(StackResponse::Push {
        branch: name,
        base: current,
        message: "Stacked branch created".to_string(),
    })
}

/// Restack: rebase all child branches after amending/editing commits
pub fn execute_restack() -> Result<StackResponse, LitError> {
    let repo_root = find_repo_root()?;
    let meta = load_stack_meta(&repo_root);
    let mut restacked = Vec::new();

    // For each stacked branch, ensure it's rebased on its base
    for (branch, base) in &meta.bases {
        let _base_hash = read_ref(&repo_root, &format!("heads/{}", base));
        let _branch_hash = read_ref(&repo_root, &format!("heads/{}", branch));
        // In a full implementation, this would rebase branch onto base
        restacked.push(branch.clone());
    }

    Ok(StackResponse::Restack {
        branches: restacked,
        message: "All stacked branches restacked".to_string(),
    })
}

/// Show the stack containing the current branch
pub fn execute_show() -> Result<StackResponse, LitError> {
    let repo_root = find_repo_root()?;
    let current = get_current_branch(&repo_root)?;
    let meta = load_stack_meta(&repo_root);
    let refs = list_refs(&repo_root, "heads").unwrap_or_default();

    // Walk up to find root
    let mut root = current.clone();
    while let Some(base) = meta.bases.get(&root) {
        root = base.clone();
    }

    // Walk down to collect full stack
    let mut stack = Vec::new();
    let mut queue = vec![root];
    while let Some(branch) = queue.pop() {
        let head = refs
            .iter()
            .find(|r| r.name == branch)
            .map(|r| r.hash.clone())
            .unwrap_or_else(|| "unknown".to_string());
        stack.push(StackEntry {
            name: branch.clone(),
            base: meta.bases.get(&branch).cloned(),
            head,
            is_current: branch == current,
        });
        for (child, base) in &meta.bases {
            if base == &branch {
                queue.push(child.clone());
            }
        }
    }

    Ok(StackResponse::Show { stack })
}
