/// Command tests for `lit bisect`
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
fn test_bisect_start() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::bisect::execute(Some(lit::BisectCommands::Start));
    assert!(
        result.is_ok(),
        "Bisect start should succeed: {:?}",
        result.err()
    );

    assert!(temp.path().join(".lit").join("bisect.json").exists());
}

#[test]
fn test_bisect_status_no_session() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::bisect::execute(None);
    assert!(result.is_err(), "Status without active bisect should error");
}

#[test]
fn test_bisect_reset() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::bisect::execute(Some(lit::BisectCommands::Start)).unwrap();
    assert!(temp.path().join(".lit").join("bisect.json").exists());

    let result = lit::commands::bisect::execute(Some(lit::BisectCommands::Reset));
    assert!(result.is_ok(), "Reset should succeed");
    assert!(!temp.path().join(".lit").join("bisect.json").exists());
}

#[test]
fn test_bisect_good_bad_marks() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let hash1 = add_and_commit(&temp, "f1.txt", "v1", "commit 1");
    let _hash2 = add_and_commit(&temp, "f1.txt", "v2", "commit 2");
    let hash3 = add_and_commit(&temp, "f1.txt", "v3", "commit 3");

    lit::commands::bisect::execute(Some(lit::BisectCommands::Start)).unwrap();

    let result = lit::commands::bisect::execute(Some(lit::BisectCommands::Good {
        commit: hash1.clone(),
    }));
    assert!(result.is_ok(), "Mark good should succeed");

    let result = lit::commands::bisect::execute(Some(lit::BisectCommands::Bad {
        commit: hash3.clone(),
    }));
    assert!(result.is_ok(), "Mark bad should succeed");
}