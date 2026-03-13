/// Command tests for ``lit cherry-pick``
///
/// NOTE: These tests modify the current working directory and must be run with
/// ``cargo test --test command_tests -- --test-threads=1`` to avoid test interference.
use std::fs;
use tempfile::TempDir;

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

fn create_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

fn add_and_commit(temp: &TempDir, filename: &str, content: &str, msg: &str) -> String {
    create_file(temp.path(), filename, content);
    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    let resp = lit::commands::commit::execute(msg.to_string(), None).unwrap();
    resp.hash
}

#[test]
fn test_cherry_pick_commit() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create base commit on main
    add_and_commit(&temp, "base.txt", "base", "base commit");

    // Create a feature branch with a unique commit
    lit::commands::branch::execute(Some("feature".to_string()), false, false).unwrap();
    lit::commands::checkout::execute("feature".to_string(), false).unwrap();
    let pick_hash = add_and_commit(&temp, "feature.txt", "feature content", "feature commit");

    // Switch back to main
    lit::commands::checkout::execute("main".to_string(), false).unwrap();

    // Cherry-pick the feature commit onto main
    let result = lit::commands::cherry_pick::execute(pick_hash);
    assert!(
        result.is_ok(),
        "Cherry-pick should succeed: {:?}",
        result.err()
    );

    // Verify the file was brought in
    assert!(temp.path().join("feature.txt").exists());
    let content = fs::read_to_string(temp.path().join("feature.txt")).unwrap();
    assert_eq!(content, "feature content");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_cherry_pick_invalid_target() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    add_and_commit(&temp, "f.txt", "x", "first");

    let result = lit::commands::cherry_pick::execute("nonexistent_hash".to_string());
    assert!(result.is_err(), "Cherry-pick of invalid target should fail");

    std::env::set_current_dir(original_dir).unwrap();
}
