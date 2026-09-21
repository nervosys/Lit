use crate::core::{
    delete_ref, find_repo_root, get_current_branch, list_refs, read_head, write_ref,
};
use crate::response::{BranchEntry, BranchResponse};

pub fn execute(
    name: Option<String>,
    delete: bool,
    _all: bool,
) -> Result<BranchResponse, crate::errors::LitError> {
    execute_at(find_repo_root()?, name, delete, _all)
}

/// Like [`execute`], but against an explicit repository root instead of
/// searching from the process's working directory.
///
/// The server needs this: it is handed the repository to serve, and a
/// long-lived process must not depend on where it happened to be started.
pub fn execute_at(
    repo_root: std::path::PathBuf,
    name: Option<String>,
    delete: bool,
    _all: bool,
) -> Result<BranchResponse, crate::errors::LitError> {
    if delete {
        if let Some(branch_name) = name {
            // Check if trying to delete current branch
            if let Ok(current) = get_current_branch(&repo_root) {
                if current == branch_name {
                    return Err("Cannot delete the currently checked out branch".into());
                }
            }
            delete_ref(&repo_root, &format!("heads/{}", branch_name))?;
            Ok(BranchResponse::Delete { name: branch_name })
        } else {
            Err("Branch name required for deletion".into())
        }
    } else if let Some(branch_name) = name {
        let head_hash = read_head(&repo_root)?;
        write_ref(&repo_root, &format!("heads/{}", branch_name), &head_hash)?;
        Ok(BranchResponse::Create { name: branch_name })
    } else {
        let refs = list_refs(&repo_root, "heads")?;
        let current = get_current_branch(&repo_root).ok();
        let branches = refs
            .into_iter()
            .map(|r| BranchEntry {
                name: r.name.clone(),
                is_current: Some(&r.name) == current.as_ref(),
            })
            .collect();
        Ok(BranchResponse::List { branches })
    }
}
