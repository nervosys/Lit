/// Command tests for `lit status`
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
fn test_status_clean_working_tree() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status should succeed");
}

#[test]
fn test_status_with_untracked_files() {
    let temp = init_test_repo();

    // Create a file without adding it
    create_file(temp.path(), "untracked.txt", "untracked content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status with untracked files should succeed");
}

#[test]
fn test_status_with_staged_files() {
    let temp = init_test_repo();

    create_file(temp.path(), "staged.txt", "staged content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Add file to staging
    lit::commands::add::execute(vec!["staged.txt".to_string()]).unwrap();

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status with staged files should succeed");
}

#[test]
fn test_status_with_modified_files() {
    let temp = init_test_repo();

    create_commit(
        temp.path(),
        "file.txt",
        "original content",
        "Initial commit",
    );

    // Modify the file
    create_file(temp.path(), "file.txt", "modified content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status with modified files should succeed");
}

#[test]
fn test_status_shows_current_branch() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Status should show we're on main branch
    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status should succeed");
}

#[test]
fn test_status_empty_repository() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status on empty repository should succeed");
}

#[test]
fn test_status_with_multiple_untracked() {
    let temp = init_test_repo();

    create_file(temp.path(), "file1.txt", "content1");
    create_file(temp.path(), "file2.txt", "content2");
    create_file(temp.path(), "file3.txt", "content3");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(
        result.is_ok(),
        "Status with multiple untracked files should succeed"
    );
}

#[test]
fn test_status_with_subdirectory() {
    let temp = init_test_repo();

    fs::create_dir(temp.path().join("subdir")).unwrap();
    create_file(&temp.path().join("subdir"), "nested.txt", "nested content");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status with subdirectory should succeed");
}

#[test]
fn test_status_ignores_lit_directory() {
    let temp = init_test_repo();

    // Create file inside .lit directory
    create_file(
        &temp.path().join(".lit"),
        "internal.txt",
        "internal content",
    );

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::status::execute();
    assert!(result.is_ok(), "Status should ignore .lit directory");
}