/// Command tests for `lit stash`
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
fn test_stash_push() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");

    // Create a modification to stash
    create_file(temp.path(), "test.txt", "modified content");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::stash::execute(Some(lit::StashCommands::Push {
        message: Some("WIP: test stash".to_string()),
    }));
    assert!(
        result.is_ok(),
        "Stash push should succeed: {:?}",
        result.err()
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_stash_push_default() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");
    create_file(temp.path(), "test.txt", "modified");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // None command defaults to push
    let result = lit::commands::stash::execute(None);
    assert!(result.is_ok(), "Stash with no subcommand should push");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_stash_list_empty() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::stash::execute(Some(lit::StashCommands::List));
    assert!(result.is_ok(), "Stash list should succeed on empty stash");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_stash_push_and_list() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");
    create_file(temp.path(), "test.txt", "modified");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Push a stash
    lit::commands::stash::execute(Some(lit::StashCommands::Push {
        message: Some("test stash entry".to_string()),
    }))
    .unwrap();

    // List should show the entry
    let result = lit::commands::stash::execute(Some(lit::StashCommands::List));
    assert!(result.is_ok(), "Stash list should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_stash_push_and_pop() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");
    create_file(temp.path(), "test.txt", "modified for pop");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Push
    lit::commands::stash::execute(Some(lit::StashCommands::Push {
        message: Some("pop test".to_string()),
    }))
    .unwrap();

    // Pop
    let result = lit::commands::stash::execute(Some(lit::StashCommands::Pop));
    assert!(
        result.is_ok(),
        "Stash pop should succeed: {:?}",
        result.err()
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_stash_pop_empty_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::stash::execute(Some(lit::StashCommands::Pop));
    assert!(result.is_err(), "Pop on empty stash should fail");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_stash_drop() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "initial", "Initial commit");
    create_file(temp.path(), "test.txt", "modified for drop");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Push
    lit::commands::stash::execute(Some(lit::StashCommands::Push {
        message: Some("drop test".to_string()),
    }))
    .unwrap();

    // Drop
    let result = lit::commands::stash::execute(Some(lit::StashCommands::Drop { index: None }));
    assert!(
        result.is_ok(),
        "Stash drop should succeed: {:?}",
        result.err()
    );

    std::env::set_current_dir(original_dir).unwrap();
}
