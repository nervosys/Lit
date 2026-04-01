/// Command tests for `lit revert`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
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
fn test_revert_commit() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    add_and_commit(&temp, "base.txt", "base content", "initial");
    let target_hash = add_and_commit(&temp, "added.txt", "added content", "add file");

    assert!(temp.path().join("added.txt").exists());

    let result = lit::commands::revert::execute(target_hash);
    assert!(result.is_ok(), "Revert should succeed: {:?}", result.err());
}

#[test]
fn test_revert_invalid_target() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    add_and_commit(&temp, "f.txt", "x", "first");

    let result = lit::commands::revert::execute("nonexistent".to_string());
    assert!(result.is_err(), "Revert of invalid target should fail");
}
