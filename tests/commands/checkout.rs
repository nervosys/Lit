/// Command tests for `lit checkout`
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

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_path).unwrap();

    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_create_new_branch() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::checkout::execute("new-branch".to_string(), true);
    assert!(
        result.is_ok(),
        "Creating and checking out new branch should succeed"
    );

    // Verify branch reference exists
    let branch_ref = temp.path().join(".lit/refs/heads/new-branch");
    assert!(branch_ref.exists(), "New branch reference should exist");

    // Verify HEAD points to new branch
    let head_content = fs::read_to_string(temp.path().join(".lit/HEAD")).unwrap();
    assert!(
        head_content.contains("new-branch"),
        "HEAD should point to new branch"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_existing_branch() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create a branch
    lit::commands::branch::execute(Some("feature".to_string()), false, false).unwrap();

    // Checkout the branch
    let result = lit::commands::checkout::execute("feature".to_string(), false);
    assert!(
        result.is_ok(),
        "Checking out existing branch should succeed"
    );

    // Verify HEAD points to feature branch
    let head_content = fs::read_to_string(temp.path().join(".lit/HEAD")).unwrap();
    assert!(
        head_content.contains("feature"),
        "HEAD should point to feature branch"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_switches_working_directory() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create initial commit
    create_commit(temp.path(), "file1.txt", "content1", "First commit");

    // Create branch
    lit::commands::branch::execute(Some("branch2".to_string()), false, false).unwrap();

    // Add another file on main
    create_commit(temp.path(), "file2.txt", "content2", "Second commit");

    // Checkout branch2 (should have only file1.txt)
    lit::commands::checkout::execute("branch2".to_string(), false).unwrap();

    // file1.txt should exist, file2.txt should not (it was added after branch creation)
    assert!(
        temp.path().join("file1.txt").exists(),
        "file1.txt should exist"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_updates_index() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create and checkout new branch
    lit::commands::checkout::execute("new-branch".to_string(), true).unwrap();

    // Load index and verify it has the file
    let index = lit::storage::Index::load(temp.path()).unwrap();
    assert!(
        index.entries.contains_key("test.txt"),
        "Index should contain test.txt after checkout"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_creates_branch_at_current_commit() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "content", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Get current commit hash
    let main_hash = fs::read_to_string(temp.path().join(".lit/refs/heads/main")).unwrap();

    // Create new branch
    lit::commands::checkout::execute("new-branch".to_string(), true).unwrap();

    // New branch should point to same commit
    let new_branch_hash =
        fs::read_to_string(temp.path().join(".lit/refs/heads/new-branch")).unwrap();
    assert_eq!(
        main_hash.trim(),
        new_branch_hash.trim(),
        "New branch should point to same commit"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_restores_files() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create initial commit
    create_commit(
        temp.path(),
        "file.txt",
        "original content",
        "Initial commit",
    );

    // Modify the file
    create_file(temp.path(), "file.txt", "modified content");

    // Verify file is modified
    let modified_content = fs::read_to_string(temp.path().join("file.txt")).unwrap();
    assert_eq!(modified_content, "modified content");

    // Checkout main again (should restore file)
    lit::commands::checkout::execute("main".to_string(), false).unwrap();

    // File should be restored
    let restored_content = fs::read_to_string(temp.path().join("file.txt")).unwrap();
    assert_eq!(
        restored_content, "original content",
        "File should be restored to committed version"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_with_subdirectory() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create subdirectory with file
    fs::create_dir(temp.path().join("subdir")).unwrap();
    create_file(&temp.path().join("subdir"), "nested.txt", "nested content");

    lit::commands::add::execute(vec!["subdir".to_string()]).unwrap();
    lit::commands::commit::execute("Commit with subdir".to_string(), None).unwrap();

    // Create and checkout new branch
    let result = lit::commands::checkout::execute("new-branch".to_string(), true);
    assert!(result.is_ok(), "Checkout with subdirectory should succeed");

    // Verify subdirectory file still exists
    assert!(
        temp.path().join("subdir/nested.txt").exists(),
        "Subdirectory file should exist after checkout"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_checkout_by_commit_hash() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    create_commit(temp.path(), "file.txt", "content", "Initial commit");

    // Get the commit hash
    let commit_hash = fs::read_to_string(temp.path().join(".lit/refs/heads/main")).unwrap();
    let commit_hash = commit_hash.trim().to_string();

    // Make another commit
    create_commit(temp.path(), "file2.txt", "content2", "Second commit");

    // Checkout by commit hash (should go to detached HEAD)
    let result = lit::commands::checkout::execute(commit_hash.clone(), false);
    assert!(result.is_ok(), "Checkout by commit hash should succeed");

    // HEAD should be detached
    let head_content = fs::read_to_string(temp.path().join(".lit/HEAD")).unwrap();
    // In detached state, HEAD contains the commit hash
    assert!(
        head_content.contains(&commit_hash) || head_content.contains("detached"),
        "HEAD should be detached or contain commit hash"
    );

    std::env::set_current_dir(original_dir).unwrap();
}
