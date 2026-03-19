/// Command tests for `lit verify`
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
fn test_verify_empty_repo() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::verify::execute();
    assert!(
        result.is_ok(),
        "Verify on empty repo should succeed: {:?}",
        result.err()
    );
    let resp = result.unwrap();
    assert!(resp.valid, "Empty repo should be valid");
}

#[test]
fn test_verify_repo_with_commits() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "hello");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first".to_string(), None).unwrap();

    create_file(temp.path(), "b.txt", "world");
    lit::commands::add::execute(vec!["b.txt".to_string()]).unwrap();
    lit::commands::commit::execute("second".to_string(), None).unwrap();

    let result = lit::commands::verify::execute();
    assert!(result.is_ok(), "Verify should succeed");
    let resp = result.unwrap();
    assert!(resp.valid, "Repo with valid commits should be valid");
    assert!(!resp.checks.is_empty(), "Should have verification checks");
}

#[test]
fn test_verify_repo_with_branches() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "f.txt", "content");
    lit::commands::add::execute(vec!["f.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    lit::commands::branch::execute(Some("feature".to_string()), false, false).unwrap();

    let result = lit::commands::verify::execute();
    assert!(result.is_ok(), "Verify with branches should succeed");
    let resp = result.unwrap();
    assert!(resp.valid, "Repo with branches should be valid");
}

#[test]
fn test_verify_corrupt_object() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "test data for corruption");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    // Find an object file and corrupt it
    let objects_dir = temp.path().join(".lit").join("objects");
    let mut corrupted = false;
    for dir_entry in fs::read_dir(&objects_dir).unwrap() {
        let dir_entry = dir_entry.unwrap();
        if dir_entry.file_type().unwrap().is_dir() {
            for file_entry in fs::read_dir(dir_entry.path()).unwrap() {
                let file_entry = file_entry.unwrap();
                if !corrupted {
                    fs::write(file_entry.path(), b"CORRUPTED DATA").unwrap();
                    corrupted = true;
                }
            }
        }
    }
    assert!(corrupted, "Should have found an object to corrupt");

    let result = lit::commands::verify::execute();
    assert!(
        result.is_ok(),
        "Verify should return Ok even with corruption"
    );
    let resp = result.unwrap();
    assert!(!resp.valid, "Repo with corrupt object should be invalid");
    assert!(
        resp.checks.iter().any(|c| c.status == "error"),
        "Should have error check results"
    );
}

#[test]
fn test_verify_dangling_ref() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "content");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    // Create a branch ref pointing to a nonexistent hash
    let fake_hash = "f".repeat(192);
    let refs_dir = temp.path().join(".lit").join("refs").join("heads");
    fs::write(refs_dir.join("dangling-branch"), &fake_hash).unwrap();

    let result = lit::commands::verify::execute();
    assert!(result.is_ok(), "Verify should return Ok");
    let resp = result.unwrap();
    assert!(!resp.valid, "Repo with dangling ref should be invalid");
    assert!(
        resp.checks
            .iter()
            .any(|c| c.check.contains("dangling-branch")
                && c.status == "error"
                && c.details.as_ref().is_some_and(|d| d.contains("Dangling"))),
        "Should report dangling ref for dangling-branch"
    );
}

#[test]
fn test_verify_checks_present() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "content");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    let result = lit::commands::verify::execute().unwrap();
    assert!(result.valid);

    // Verify the expected check categories are present
    let check_names: Vec<&str> = result.checks.iter().map(|c| c.check.as_str()).collect();
    assert!(
        check_names.iter().any(|n| n.contains("object")),
        "Should have object check: {:?}",
        check_names
    );
    assert!(
        check_names.iter().any(|n| n.contains("ref")),
        "Should have refs check: {:?}",
        check_names
    );
    assert!(
        check_names.iter().any(|n| n.contains("dag")),
        "Should have DAG check: {:?}",
        check_names
    );
}
