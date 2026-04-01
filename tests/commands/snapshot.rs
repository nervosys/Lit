/// Command tests for `lit snapshot`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use std::fs;
use tempfile::TempDir;

// Helper to initialize a test repository
fn init_test_repo() -> TempDir {
    let temp = tempfile::Builder::new()
        .prefix("lit_test_")
        .tempdir()
        .unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

// Helper to create a test file
fn create_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

#[test]
fn test_snapshot_creates_commit() {
    let temp = init_test_repo();

    create_file(temp.path(), "file1.txt", "hello world");
    create_file(temp.path(), "file2.txt", "goodbye world");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::snapshot::execute("Test snapshot".to_string(), None, None);
    assert!(
        result.is_ok(),
        "Snapshot should succeed: {:?}",
        result.err()
    );

    let response = result.unwrap();
    assert!(!response.hash.is_empty(), "Should have a commit hash");
    assert!(!response.short_hash.is_empty(), "Should have a short hash");
    assert_eq!(response.message, "Test snapshot");
    assert!(
        response.files_added >= 2,
        "Should have added at least 2 files"
    );
}

#[test]
fn test_snapshot_with_author() {
    let temp = init_test_repo();

    create_file(temp.path(), "test.txt", "content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::snapshot::execute(
        "Authored snapshot".to_string(),
        Some("Test Author".to_string()),
        None,
    );
    assert!(result.is_ok(), "Snapshot with author should succeed");

    let response = result.unwrap();
    assert_eq!(response.author, "Test Author");
}

#[test]
fn test_snapshot_with_metadata() {
    let temp = init_test_repo();

    create_file(temp.path(), "test.txt", "content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let metadata = serde_json::json!({
        "ticket": "PROJ-123",
        "priority": "high"
    });

    let result =
        lit::commands::snapshot::execute("Metadata snapshot".to_string(), None, Some(metadata));
    assert!(result.is_ok(), "Snapshot with metadata should succeed");
}

#[test]
fn test_snapshot_empty_directory_fails() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::snapshot::execute("Empty snapshot".to_string(), None, None);
    assert!(result.is_err(), "Snapshot on empty directory should fail");
}

#[test]
fn test_snapshot_creates_valid_commit_ref() {
    let temp = init_test_repo();

    create_file(temp.path(), "test.txt", "content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::snapshot::execute("Ref test snapshot".to_string(), None, None).unwrap();

    // HEAD should now point to the branch which has the commit
    let head_ref = fs::read_to_string(temp.path().join(".lit/refs/heads/main"));
    assert!(
        head_ref.is_ok(),
        "Should have a main branch ref after snapshot"
    );
    let hash = head_ref.unwrap().trim().to_string();
    assert!(!hash.is_empty(), "Branch ref should contain a hash");
}
