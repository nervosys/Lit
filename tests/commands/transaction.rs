/// Command tests for `lit transaction`
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

// Helper to clean up any stale transaction state
fn cleanup_transaction(repo_path: &std::path::Path) {
    let lock = repo_path.join(".lit/transaction.lock");
    let state = repo_path.join(".lit/transaction.json");
    let _ = fs::remove_file(&lock);
    let _ = fs::remove_file(&state);
}

#[test]
fn test_transaction_begin() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::transaction::execute_begin();
    assert!(result.is_ok(), "Transaction begin should succeed");

    let response = result.unwrap();
    assert_eq!(response.action, "begin");
    assert!(response.tx_id.is_some(), "Should have a transaction ID");

    // Lock file should exist
    assert!(
        temp.path().join(".lit/transaction.lock").exists(),
        "Transaction lock should exist"
    );

    cleanup_transaction(temp.path());
}

#[test]
fn test_transaction_begin_commit_lifecycle() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Begin
    let begin_result = lit::commands::transaction::execute_begin();
    assert!(begin_result.is_ok(), "Transaction begin should succeed");

    // Commit
    let commit_result = lit::commands::transaction::execute_commit_tx();
    assert!(commit_result.is_ok(), "Transaction commit should succeed");

    let response = commit_result.unwrap();
    assert_eq!(response.action, "commit");

    // Lock file should be gone
    assert!(
        !temp.path().join(".lit/transaction.lock").exists(),
        "Transaction lock should be removed after commit"
    );
}

#[test]
fn test_transaction_begin_rollback_lifecycle() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Begin
    let begin_result = lit::commands::transaction::execute_begin();
    assert!(begin_result.is_ok(), "Transaction begin should succeed");

    // Rollback
    let rollback_result = lit::commands::transaction::execute_rollback();
    assert!(
        rollback_result.is_ok(),
        "Transaction rollback should succeed"
    );

    let response = rollback_result.unwrap();
    assert_eq!(response.action, "rollback");

    // Lock file should be gone
    assert!(
        !temp.path().join(".lit/transaction.lock").exists(),
        "Transaction lock should be removed after rollback"
    );
}

#[test]
fn test_transaction_double_begin_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // First begin succeeds
    let result1 = lit::commands::transaction::execute_begin();
    assert!(result1.is_ok(), "First begin should succeed");

    // Second begin should fail (lock already exists)
    let result2 = lit::commands::transaction::execute_begin();
    assert!(
        result2.is_err(),
        "Second begin should fail due to existing lock"
    );

    cleanup_transaction(temp.path());
}

#[test]
fn test_transaction_commit_without_begin_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Commit without begin should fail
    let result = lit::commands::transaction::execute_commit_tx();
    assert!(result.is_err(), "Commit without begin should fail");
}

#[test]
fn test_transaction_rollback_without_begin_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Rollback without begin should fail
    let result = lit::commands::transaction::execute_rollback();
    assert!(result.is_err(), "Rollback without begin should fail");
}