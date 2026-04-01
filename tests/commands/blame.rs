/// Command tests for `lit blame`
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

#[test]
fn test_blame_single_commit() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "hello.txt", "line1\nline2\nline3\n");
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    let result = lit::commands::blame::execute("hello.txt".to_string());
    assert!(result.is_ok(), "Blame should succeed: {:?}", result.err());
}

#[test]
fn test_blame_nonexistent_file() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "dummy.txt", "x");
    lit::commands::add::execute(vec!["dummy.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first".to_string(), None).unwrap();

    let result = lit::commands::blame::execute("nonexistent.txt".to_string());
    assert!(result.is_err(), "Blame on missing file should fail");
}

#[test]
fn test_blame_multiple_commits() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "file.txt", "line1\n");
    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first commit".to_string(), None).unwrap();

    create_file(temp.path(), "file.txt", "line1\nline2\n");
    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();
    lit::commands::commit::execute("second commit".to_string(), None).unwrap();

    let result = lit::commands::blame::execute("file.txt".to_string());
    assert!(result.is_ok(), "Blame should succeed with history");
}
