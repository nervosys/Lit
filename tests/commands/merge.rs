/// Command tests for `lit merge`
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
fn create_commit(repo_path: &std::path::Path, filename: &str, content: &str, message: &str) {
    create_file(repo_path, filename, content);

    let _cwd = super::test_helpers::CwdGuard::new(repo_path);

    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();
}

#[test]
fn test_merge_fast_forward() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create a branch and add a commit on it
    lit::commands::branch::execute(Some("feature".to_string()), false, false).unwrap();
    lit::commands::checkout::execute("feature".to_string(), false).unwrap();
    create_commit(temp.path(), "feature.txt", "feature content", "Feature commit");

    // Switch back to main and merge feature (should fast-forward)
    lit::commands::checkout::execute("main".to_string(), false).unwrap();
    let result = lit::commands::merge::execute("feature".to_string(), None);
    assert!(result.is_ok(), "Merge should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(resp.merged, "Merge should be marked as merged");
    assert!(resp.fast_forward, "Should be a fast-forward merge");
    assert!(!resp.has_conflicts, "Should have no conflicts");
}

#[test]
fn test_merge_already_up_to_date() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Merge current branch into itself
    let result = lit::commands::merge::execute("main".to_string(), None);
    assert!(result.is_ok(), "Self-merge should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(resp.message.contains("up to date"));
}

#[test]
fn test_merge_with_nonexistent_branch() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::merge::execute("nonexistent".to_string(), None);
    assert!(result.is_err(), "Merge with nonexistent branch should fail");
}