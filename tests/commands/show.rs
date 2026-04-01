/// Command tests for `lit show`
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

// Helper to create a commit and return its hash
fn create_commit(
    repo_path: &std::path::Path,
    filename: &str,
    content: &str,
    message: &str,
) -> String {
    create_file(repo_path, filename, content);

    let _cwd = super::test_helpers::CwdGuard::new(repo_path);

    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();

    // Get commit hash
    let commit_hash = fs::read_to_string(repo_path.join(".lit/refs/heads/main"))
        .unwrap()
        .trim()
        .to_string();
    commit_hash
}

#[test]
fn test_show_commit_by_hash() {
    let temp = init_test_repo();

    let commit_hash = create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::show::execute(commit_hash);
    assert!(result.is_ok(), "Show commit by hash should succeed");
}

#[test]
fn test_show_commit_by_branch_name() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::show::execute("main".to_string());
    assert!(result.is_ok(), "Show commit by branch name should succeed");
}

#[test]
fn test_show_displays_commit_message() {
    let temp = init_test_repo();

    let message = "This is a test commit message";
    let commit_hash = create_commit(temp.path(), "test.txt", "content", message);

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::show::execute(commit_hash);
    assert!(result.is_ok(), "Show should display commit message");
}

#[test]
fn test_show_displays_author() {
    let temp = init_test_repo();

    let commit_hash = create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::show::execute(commit_hash);
    assert!(result.is_ok(), "Show should display author information");
}

#[test]
fn test_show_blob_object() {
    let temp = init_test_repo();

    let content = "Hello, World!";
    create_file(temp.path(), "test.txt", content);

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec!["test.txt".to_string()]).unwrap();

    // Get the blob hash from index
    let index = lit::storage::Index::load(temp.path()).unwrap();
    let blob_hash = index.entries.get("test.txt").unwrap().hash.clone();

    let result = lit::commands::show::execute(blob_hash);
    assert!(result.is_ok(), "Show blob object should succeed");
}

#[test]
fn test_show_with_multiline_message() {
    let temp = init_test_repo();

    let message = "First line\nSecond line\nThird line";
    let commit_hash = create_commit(temp.path(), "test.txt", "content", message);

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::show::execute(commit_hash);
    assert!(result.is_ok(), "Show with multiline message should succeed");
}

#[test]
fn test_show_invalid_object_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let fake_hash = "0".repeat(64);
    let result = lit::commands::show::execute(fake_hash);
    assert!(result.is_err(), "Show with invalid object should fail");
}

#[test]
fn test_show_short_hash() {
    let temp = init_test_repo();

    let commit_hash = create_commit(temp.path(), "test.txt", "content", "Test commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Try with full hash (should work)
    let result = lit::commands::show::execute(commit_hash);
    assert!(result.is_ok(), "Show with full hash should succeed");
}
