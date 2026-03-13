/// Command tests for ``lit reset``
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
fn test_reset_soft() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let hash1 = add_and_commit(&temp, "f1.txt", "v1", "commit 1");
    add_and_commit(&temp, "f2.txt", "v2", "commit 2");

    let result = lit::commands::reset::execute(hash1.clone(), true, false);
    assert!(
        result.is_ok(),
        "Soft reset should succeed: {:?}",
        result.err()
    );
    let resp = result.unwrap();
    assert!(
        resp.message.contains(&hash1[..16]),
        "HEAD should point to target commit"
    );

    // Files should still exist in working tree after soft reset
    assert!(temp.path().join("f1.txt").exists());
    assert!(temp.path().join("f2.txt").exists());

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_reset_hard() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let hash1 = add_and_commit(&temp, "first.txt", "original", "commit 1");
    add_and_commit(&temp, "first.txt", "modified", "commit 2");

    // After hard reset to commit 1, first.txt should have original content
    let result = lit::commands::reset::execute(hash1.clone(), false, true);
    assert!(
        result.is_ok(),
        "Hard reset should succeed: {:?}",
        result.err()
    );

    let content = fs::read_to_string(temp.path().join("first.txt")).unwrap();
    assert_eq!(
        content, "original",
        "File should be restored to commit 1 version"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_reset_head_tilde() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    add_and_commit(&temp, "a.txt", "a", "commit 1");
    add_and_commit(&temp, "b.txt", "b", "commit 2");
    add_and_commit(&temp, "c.txt", "c", "commit 3");

    let result = lit::commands::reset::execute("HEAD~2".to_string(), true, false);
    assert!(
        result.is_ok(),
        "Reset HEAD~2 should succeed: {:?}",
        result.err()
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_reset_invalid_target() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    add_and_commit(&temp, "f.txt", "x", "first");

    let result = lit::commands::reset::execute("nonexistent".to_string(), false, false);
    assert!(result.is_err(), "Reset to invalid target should fail");

    std::env::set_current_dir(original_dir).unwrap();
}
