/// Command tests for `lit commit`
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

#[test]
fn test_commit_creates_commit_object() {
    let temp = init_test_repo();

    create_file(temp.path(), "test.txt", "Hello, World!");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Add file
    lit::commands::add::execute(vec!["test.txt".to_string()]).unwrap();

    // Commit
    let result = lit::commands::commit::execute(
        "Initial commit".to_string(),
        Some("Test Author".to_string()),
    );
    assert!(result.is_ok(), "Commit should succeed");

    // Verify HEAD was updated
    let head_content = fs::read_to_string(temp.path().join(".lit/HEAD")).unwrap();
    assert!(
        head_content.contains("refs/heads/main"),
        "HEAD should point to main branch"
    );

    // Verify branch ref exists
    let branch_ref = temp.path().join(".lit/refs/heads/main");
    assert!(branch_ref.exists(), "main branch reference should exist");
}

#[test]
fn test_commit_fails_with_empty_staging() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::commit::execute("Empty commit".to_string(), None);
    assert!(result.is_err(), "Commit with empty staging should fail");
    assert!(
        result.unwrap_err().contains("empty"),
        "Error should mention empty staging"
    );
}

#[test]
fn test_commit_with_message() {
    let temp = init_test_repo();

    create_file(temp.path(), "file.txt", "content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();

    let message = "Test commit message";
    let result = lit::commands::commit::execute(message.to_string(), None);
    assert!(result.is_ok(), "Commit should succeed");
}

#[test]
fn test_commit_creates_parent_chain() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // First commit
    create_file(temp.path(), "file1.txt", "content1");
    lit::commands::add::execute(vec!["file1.txt".to_string()]).unwrap();
    lit::commands::commit::execute("First commit".to_string(), None).unwrap();

    let first_commit_hash = fs::read_to_string(temp.path().join(".lit/refs/heads/main")).unwrap();

    // Second commit
    create_file(temp.path(), "file2.txt", "content2");
    lit::commands::add::execute(vec!["file2.txt".to_string()]).unwrap();
    lit::commands::commit::execute("Second commit".to_string(), None).unwrap();

    let second_commit_hash = fs::read_to_string(temp.path().join(".lit/refs/heads/main")).unwrap();

    // Hashes should be different
    assert_ne!(
        first_commit_hash.trim(),
        second_commit_hash.trim(),
        "Commits should have different hashes"
    );
}

#[test]
fn test_commit_with_custom_author() {
    let temp = init_test_repo();

    create_file(temp.path(), "test.txt", "content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec!["test.txt".to_string()]).unwrap();

    let result = lit::commands::commit::execute(
        "Test commit".to_string(),
        Some("Custom Author <author@example.com>".to_string()),
    );
    assert!(result.is_ok(), "Commit with custom author should succeed");
}

#[test]
fn test_commit_updates_branch_reference() {
    let temp = init_test_repo();

    create_file(temp.path(), "file.txt", "content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();

    // Branch ref shouldn't exist yet
    let branch_ref_path = temp.path().join(".lit/refs/heads/main");
    assert!(
        !branch_ref_path.exists(),
        "Branch ref shouldn't exist before first commit"
    );

    lit::commands::commit::execute("First commit".to_string(), None).unwrap();

    // Now it should exist
    assert!(
        branch_ref_path.exists(),
        "Branch ref should exist after commit"
    );

    let commit_hash = fs::read_to_string(branch_ref_path).unwrap();
    assert!(
        !commit_hash.trim().is_empty(),
        "Commit hash should not be empty"
    );
}

#[test]
fn test_commit_with_multiple_files() {
    let temp = init_test_repo();

    create_file(temp.path(), "file1.txt", "content1");
    create_file(temp.path(), "file2.txt", "content2");
    create_file(temp.path(), "file3.txt", "content3");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec![
        "file1.txt".to_string(),
        "file2.txt".to_string(),
        "file3.txt".to_string(),
    ])
    .unwrap();

    let result = lit::commands::commit::execute("Multi-file commit".to_string(), None);
    assert!(result.is_ok(), "Commit with multiple files should succeed");
}

#[test]
fn test_commit_with_subdirectory() {
    let temp = init_test_repo();

    fs::create_dir(temp.path().join("subdir")).unwrap();
    create_file(&temp.path().join("subdir"), "nested.txt", "nested content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec!["subdir".to_string()]).unwrap();

    let result = lit::commands::commit::execute("Commit with subdirectory".to_string(), None);
    assert!(result.is_ok(), "Commit with subdirectory should succeed");
}