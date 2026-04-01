/// Command tests for `lit branch`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use std::fs;
use tempfile::TempDir;

// Helper to initialize a test repository
fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

// Helper to create a test file
fn create_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

// Helper to create a commit
fn create_commit(repo_path: &std::path::Path, filename: &str, message: &str) {
    create_file(repo_path, filename, "test content");

    let _cwd = super::test_helpers::CwdGuard::new(repo_path);

    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();
}

#[test]
fn test_branch_create() {
    let temp = init_test_repo();

    // Need at least one commit to create a branch
    create_commit(temp.path(), "initial.txt", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::branch::execute(Some("feature".to_string()), false, false);
    assert!(result.is_ok(), "Branch creation should succeed");

    // Verify branch reference exists
    let branch_ref = temp.path().join(".lit/refs/heads/feature");
    assert!(branch_ref.exists(), "Branch reference should exist");
}

#[test]
fn test_branch_list_empty() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // List branches (should show none or just main without commits)
    let result = lit::commands::branch::execute(None, false, false);
    assert!(result.is_ok(), "Listing branches should succeed");
}

#[test]
fn test_branch_list_with_branches() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create some branches
    lit::commands::branch::execute(Some("feature1".to_string()), false, false).unwrap();
    lit::commands::branch::execute(Some("feature2".to_string()), false, false).unwrap();

    // List branches
    let result = lit::commands::branch::execute(None, false, false);
    assert!(result.is_ok(), "Listing branches should succeed");

    // Verify branch files exist
    assert!(temp.path().join(".lit/refs/heads/feature1").exists());
    assert!(temp.path().join(".lit/refs/heads/feature2").exists());
}

#[test]
fn test_branch_delete() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create a branch
    lit::commands::branch::execute(Some("to-delete".to_string()), false, false).unwrap();

    let branch_ref = temp.path().join(".lit/refs/heads/to-delete");
    assert!(branch_ref.exists(), "Branch should exist before deletion");

    // Delete the branch
    let result = lit::commands::branch::execute(Some("to-delete".to_string()), true, false);
    assert!(result.is_ok(), "Branch deletion should succeed");

    assert!(
        !branch_ref.exists(),
        "Branch should not exist after deletion"
    );
}

#[test]
fn test_branch_delete_current_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Try to delete the current branch (main)
    let result = lit::commands::branch::execute(Some("main".to_string()), true, false);
    assert!(result.is_err(), "Deleting current branch should fail");
    assert!(
        result
            .unwrap_err()
            .internal_message()
            .contains("currently checked out"),
        "Error should mention current branch"
    );
}

#[test]
fn test_branch_delete_requires_name() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Try to delete without specifying a branch name
    let result = lit::commands::branch::execute(None, true, false);
    assert!(result.is_err(), "Delete without name should fail");
    assert!(
        result.unwrap_err().internal_message().contains("required"),
        "Error should mention name required"
    );
}

#[test]
fn test_branch_points_to_same_commit() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Get current commit hash
    let main_hash = fs::read_to_string(temp.path().join(".lit/refs/heads/main")).unwrap();

    // Create new branch
    lit::commands::branch::execute(Some("new-branch".to_string()), false, false).unwrap();

    // New branch should point to same commit
    let new_branch_hash =
        fs::read_to_string(temp.path().join(".lit/refs/heads/new-branch")).unwrap();
    assert_eq!(
        main_hash.trim(),
        new_branch_hash.trim(),
        "New branch should point to same commit as main"
    );
}

#[test]
fn test_branch_create_multiple() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create multiple branches
    lit::commands::branch::execute(Some("branch1".to_string()), false, false).unwrap();
    lit::commands::branch::execute(Some("branch2".to_string()), false, false).unwrap();
    lit::commands::branch::execute(Some("branch3".to_string()), false, false).unwrap();

    // All should exist
    assert!(temp.path().join(".lit/refs/heads/branch1").exists());
    assert!(temp.path().join(".lit/refs/heads/branch2").exists());
    assert!(temp.path().join(".lit/refs/heads/branch3").exists());
}
