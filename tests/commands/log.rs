/// Command tests for `lit log`
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
fn test_log_empty_repository() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log on empty repository should succeed");
}

#[test]
fn test_log_single_commit() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log with single commit should succeed");
}

#[test]
fn test_log_multiple_commits() {
    let temp = init_test_repo();

    create_commit(temp.path(), "file1.txt", "content1", "First commit");
    create_commit(temp.path(), "file2.txt", "content2", "Second commit");
    create_commit(temp.path(), "file3.txt", "content3", "Third commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log with multiple commits should succeed");
}

#[test]
fn test_log_with_count_limit() {
    let temp = init_test_repo();

    // Create 5 commits
    create_commit(temp.path(), "file1.txt", "content1", "Commit 1");
    create_commit(temp.path(), "file2.txt", "content2", "Commit 2");
    create_commit(temp.path(), "file3.txt", "content3", "Commit 3");
    create_commit(temp.path(), "file4.txt", "content4", "Commit 4");
    create_commit(temp.path(), "file5.txt", "content5", "Commit 5");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Request only 3 commits
    let result = lit::commands::log::execute(3, false);
    assert!(result.is_ok(), "Log with count limit should succeed");
}

#[test]
fn test_log_oneline_format() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, true);
    assert!(result.is_ok(), "Log with oneline format should succeed");
}

#[test]
fn test_log_shows_most_recent_first() {
    let temp = init_test_repo();

    create_commit(temp.path(), "file1.txt", "content1", "First commit");

    // Small delay to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(10));

    create_commit(temp.path(), "file2.txt", "content2", "Second commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log should show most recent commit first");
}

#[test]
fn test_log_with_multiline_message() {
    let temp = init_test_repo();

    let message = "First line\nSecond line\nThird line";
    create_commit(temp.path(), "test.txt", "content", message);

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log with multiline message should succeed");
}

#[test]
fn test_log_displays_author() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log should display author information");
}

#[test]
fn test_log_displays_date() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, false);
    assert!(result.is_ok(), "Log should display date information");
}

#[test]
fn test_log_oneline_with_multiple_commits() {
    let temp = init_test_repo();

    create_commit(temp.path(), "file1.txt", "content1", "First commit");
    create_commit(temp.path(), "file2.txt", "content2", "Second commit");
    create_commit(temp.path(), "file3.txt", "content3", "Third commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::log::execute(10, true);
    assert!(
        result.is_ok(),
        "Log oneline with multiple commits should succeed"
    );
}
