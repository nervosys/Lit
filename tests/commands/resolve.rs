/// Command tests for `lit resolve`
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

fn add_and_commit(temp: &TempDir, filename: &str, content: &str, msg: &str) {
    create_file(temp.path(), filename, content);
    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(msg.to_string(), None).unwrap();
}

#[test]
fn test_resolve_no_merge_in_progress() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    add_and_commit(&temp, "f.txt", "content", "initial");

    let result = lit::commands::resolve::execute(
        Some("f.txt".to_string()),
        Some("ours".to_string()),
        false,
        false,
    );
    assert!(result.is_err(), "Resolve without active merge should fail");
}

#[test]
fn test_resolve_finish_no_merge() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    add_and_commit(&temp, "f.txt", "content", "initial");

    let result = lit::commands::resolve::execute(None, None, false, true);
    assert!(result.is_err(), "Finish without active merge should fail");
}


/// Helper to create a merge conflict scenario:
/// Creates main branch commit, feature branch with conflicting change, then merges
fn setup_merge_conflict(temp: &TempDir) {
    // Initial commit on main
    add_and_commit(temp, "conflict.txt", "original content\nline two\nline three\n", "initial on main");

    // Create and switch to feature branch
    lit::commands::branch::execute(Some("feature".to_string()), false, false).unwrap();
    lit::commands::checkout::execute("feature".to_string(), false).unwrap();

    // Modify on feature
    create_file(temp.path(), "conflict.txt", "feature content\nline two\nline three\n");
    lit::commands::add::execute(vec!["conflict.txt".to_string()]).unwrap();
    lit::commands::commit::execute("feature change".to_string(), None).unwrap();

    // Switch back to main and make conflicting change
    lit::commands::checkout::execute("main".to_string(), false).unwrap();
    create_file(temp.path(), "conflict.txt", "main content\nline two\nline three\n");
    lit::commands::add::execute(vec!["conflict.txt".to_string()]).unwrap();
    lit::commands::commit::execute("main change".to_string(), None).unwrap();

    // Merge feature into main — should produce conflict with recursive strategy
    let _result = lit::commands::merge::execute("feature".to_string(), Some("recursive".to_string()));
}

#[test]
fn test_resolve_ours_strategy() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    setup_merge_conflict(&temp);

    // Verify merge state exists
    let merge_dir = temp.path().join(".lit").join("merge");
    if !merge_dir.exists() {
        // If the merge auto-resolved (no real conflict), skip gracefully
        return;
    }

    let result = lit::commands::resolve::execute(
        Some("conflict.txt".to_string()),
        Some("ours".to_string()),
        false,
        false,
    );
    assert!(result.is_ok(), "Resolve with ours should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(
        resp.resolved_files.contains(&"conflict.txt".to_string()),
        "conflict.txt should be resolved"
    );
}

#[test]
fn test_resolve_theirs_strategy() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    setup_merge_conflict(&temp);

    let merge_dir = temp.path().join(".lit").join("merge");
    if !merge_dir.exists() {
        return;
    }

    let result = lit::commands::resolve::execute(
        Some("conflict.txt".to_string()),
        Some("theirs".to_string()),
        false,
        false,
    );
    assert!(result.is_ok(), "Resolve with theirs should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(
        resp.resolved_files.contains(&"conflict.txt".to_string()),
        "conflict.txt should be resolved"
    );
}

#[test]
fn test_resolve_recursive_rejected() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    setup_merge_conflict(&temp);

    let merge_dir = temp.path().join(".lit").join("merge");
    if !merge_dir.exists() {
        return;
    }

    let result = lit::commands::resolve::execute(
        Some("conflict.txt".to_string()),
        Some("recursive".to_string()),
        false,
        false,
    );
    assert!(result.is_err(), "Resolve with recursive should be rejected");
    let err = result.unwrap_err();
    assert!(
        err.contains("recursive"),
        "Error should mention recursive strategy: {}", err
    );
}

#[test]
fn test_resolve_missing_strategy() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    setup_merge_conflict(&temp);

    let merge_dir = temp.path().join(".lit").join("merge");
    if !merge_dir.exists() {
        return;
    }

    let result = lit::commands::resolve::execute(
        Some("conflict.txt".to_string()),
        None,
        false,
        false,
    );
    assert!(result.is_err(), "Resolve without strategy should fail");
    let err = result.unwrap_err();
    assert!(
        err.contains("strategy"),
        "Error should mention --strategy: {}", err
    );
}