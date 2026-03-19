/// Command tests for `lit rebase`
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
fn test_rebase_non_interactive() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create base commit, then branch
    let _base_hash = add_and_commit(&temp, "base.txt", "base", "base commit");
    lit::commands::branch::execute(Some("feature".to_string()), false, false).unwrap();

    // Add commit on main
    add_and_commit(&temp, "main-update.txt", "main work", "main commit");

    // Switch to feature and add commit
    lit::commands::checkout::execute("feature".to_string(), false).unwrap();
    add_and_commit(&temp, "feat.txt", "feature work", "feature commit");

    // Rebase feature onto main (non-interactive)
    let result = lit::commands::rebase::execute(
        "main".to_string(),
        false, // not interactive
        None,  // no --onto
        false, // not abort
        false, // not continue
    );
    assert!(result.is_ok(), "Rebase should succeed: {:?}", result.err());
}

#[test]
fn test_rebase_abort_no_session() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    add_and_commit(&temp, "f.txt", "x", "first");

    let result = lit::commands::rebase::execute(
        "main".to_string(),
        false,
        None,
        true,  // abort
        false,
    );
    assert!(result.is_err(), "Abort without active rebase should fail");
}

#[test]
fn test_rebase_continue_no_session() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    add_and_commit(&temp, "f.txt", "x", "first");

    let result = lit::commands::rebase::execute(
        "main".to_string(),
        false,
        None,
        false,
        true, // continue
    );
    assert!(result.is_err(), "Continue without active rebase should fail");
}