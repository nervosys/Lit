use crate::core::{find_repo_root, list_refs, read_head};
use crate::errors::LitError;
use crate::response::CommandResponse;
use serde::{Deserialize, Serialize};

/// Workspace represents a set of parallel (virtual) branches applied simultaneously.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualBranch {
    pub name: String,
    pub head: String,
    pub files: Vec<String>,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum WorkspaceResponse {
    List {
        branches: Vec<VirtualBranch>,
    },
    Create {
        name: String,
        message: String,
    },
    Apply {
        name: String,
        message: String,
    },
    Unapply {
        name: String,
        message: String,
    },
    MoveFile {
        file: String,
        from: String,
        to: String,
        message: String,
    },
}

impl CommandResponse for WorkspaceResponse {
    fn command_name(&self) -> &'static str {
        "workspace"
    }
    fn human_readable(&self) -> String {
        match self {
            WorkspaceResponse::List { branches } => {
                let mut out = String::from("Virtual branches:\n");
                for br in branches {
                    let status = if br.active { "active" } else { "unapplied" };
                    out.push_str(&format!(
                        "  {} [{}] ({} files) - {}\n",
                        br.name,
                        &br.head[..8.min(br.head.len())],
                        br.files.len(),
                        status
                    ));
                }
                if branches.is_empty() {
                    out.push_str("  No virtual branches\n");
                }
                out
            }
            WorkspaceResponse::Create { name, message } => {
                format!("Created virtual branch '{}': {}\n", name, message)
            }
            WorkspaceResponse::Apply { name, message } => {
                format!("Applied '{}': {}\n", name, message)
            }
            WorkspaceResponse::Unapply { name, message } => {
                format!("Unapplied '{}': {}\n", name, message)
            }
            WorkspaceResponse::MoveFile {
                file,
                from,
                to,
                message,
            } => format!(
                "Moved '{}' from '{}' to '{}': {}\n",
                file, from, to, message
            ),
        }
    }
}

/// Workspace metadata path
fn workspace_path(repo_root: &std::path::Path) -> std::path::PathBuf {
    repo_root.join(".lit").join("workspace.json")
}

/// Workspace metadata
#[derive(Debug, Default, Serialize, Deserialize)]
struct WorkspaceMeta {
    branches: Vec<VirtualBranchMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VirtualBranchMeta {
    name: String,
    branch_ref: String,
    files: Vec<String>,
    active: bool,
}

fn load_workspace(repo_root: &std::path::Path) -> WorkspaceMeta {
    let path = workspace_path(repo_root);
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => WorkspaceMeta::default(),
        }
    } else {
        WorkspaceMeta::default()
    }
}

fn save_workspace(repo_root: &std::path::Path, meta: &WorkspaceMeta) -> Result<(), LitError> {
    let path = workspace_path(repo_root);
    let data = serde_json::to_string_pretty(meta)
        .map_err(|e| LitError::general(format!("Failed to serialize workspace: {}", e)))?;
    std::fs::write(path, data)
        .map_err(|e| LitError::io(format!("Failed to write workspace: {}", e)))?;
    Ok(())
}

/// List all virtual branches in the workspace
pub fn execute_list() -> Result<WorkspaceResponse, LitError> {
    let repo_root = find_repo_root()?;
    let meta = load_workspace(&repo_root);
    let refs = list_refs(&repo_root, "heads").unwrap_or_default();

    let branches = meta
        .branches
        .iter()
        .map(|vb| {
            let head = refs
                .iter()
                .find(|r| format!("refs/heads/{}", r.name) == vb.branch_ref || r.name == vb.name)
                .map(|r| r.hash.clone())
                .unwrap_or_else(|| "unknown".to_string());
            VirtualBranch {
                name: vb.name.clone(),
                head,
                files: vb.files.clone(),
                active: vb.active,
            }
        })
        .collect();

    Ok(WorkspaceResponse::List { branches })
}

/// Create a new virtual branch
pub fn execute_create(name: String) -> Result<WorkspaceResponse, LitError> {
    let repo_root = find_repo_root()?;
    let head_hash = read_head(&repo_root)?;
    let mut meta = load_workspace(&repo_root);

    // Check for duplicate
    if meta.branches.iter().any(|b| b.name == name) {
        return Err(LitError::general(format!(
            "Virtual branch '{}' already exists",
            name
        )));
    }

    // Create the underlying branch ref
    crate::core::write_ref(&repo_root, &format!("heads/{}", name), &head_hash)?;

    meta.branches.push(VirtualBranchMeta {
        name: name.clone(),
        branch_ref: format!("refs/heads/{}", name),
        files: Vec::new(),
        active: true,
    });
    save_workspace(&repo_root, &meta)?;

    Ok(WorkspaceResponse::Create {
        name,
        message: "Virtual branch created and applied to workspace".to_string(),
    })
}

/// Apply a virtual branch to the workspace
pub fn execute_apply(name: String) -> Result<WorkspaceResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut meta = load_workspace(&repo_root);

    let branch = meta
        .branches
        .iter_mut()
        .find(|b| b.name == name)
        .ok_or_else(|| LitError::general(format!("Virtual branch '{}' not found", name)))?;

    branch.active = true;
    save_workspace(&repo_root, &meta)?;

    Ok(WorkspaceResponse::Apply {
        name,
        message: "Virtual branch applied to workspace".to_string(),
    })
}

/// Unapply a virtual branch from the workspace
pub fn execute_unapply(name: String) -> Result<WorkspaceResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut meta = load_workspace(&repo_root);

    let branch = meta
        .branches
        .iter_mut()
        .find(|b| b.name == name)
        .ok_or_else(|| LitError::general(format!("Virtual branch '{}' not found", name)))?;

    branch.active = false;
    save_workspace(&repo_root, &meta)?;

    Ok(WorkspaceResponse::Unapply {
        name,
        message: "Virtual branch unapplied from workspace".to_string(),
    })
}

/// Move a file from one virtual branch to another
pub fn execute_move_file(
    file: String,
    from: String,
    to: String,
) -> Result<WorkspaceResponse, LitError> {
    let repo_root = find_repo_root()?;
    let mut meta = load_workspace(&repo_root);

    // Remove file from source branch
    if let Some(src) = meta.branches.iter_mut().find(|b| b.name == from) {
        src.files.retain(|f| f != &file);
    } else {
        return Err(LitError::general(format!(
            "Source branch '{}' not found",
            from
        )));
    }

    // Add file to destination branch
    if let Some(dst) = meta.branches.iter_mut().find(|b| b.name == to) {
        if !dst.files.contains(&file) {
            dst.files.push(file.clone());
        }
    } else {
        return Err(LitError::general(format!(
            "Destination branch '{}' not found",
            to
        )));
    }

    save_workspace(&repo_root, &meta)?;

    Ok(WorkspaceResponse::MoveFile {
        file,
        from,
        to,
        message: "File moved between virtual branches".to_string(),
    })
}
