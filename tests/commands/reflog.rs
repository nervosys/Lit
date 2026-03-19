/// Command tests for `lit reflog`
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
fn test_reflog_after_commits() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "aaa");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first".to_string(), None).unwrap();

    create_file(temp.path(), "b.txt", "bbb");
    lit::commands::add::execute(vec!["b.txt".to_string()]).unwrap();
    lit::commands::commit::execute("second".to_string(), None).unwrap();

    let result = lit::commands::reflog::execute(None, 10);
    assert!(result.is_ok(), "Reflog should succeed: {:?}", result.err());
}

#[test]
fn test_reflog_with_count_limit() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    for i in 0..5 {
        create_file(
            temp.path(),
            &format!("f{}.txt", i),
            &format!("content {}", i),
        );
        lit::commands::add::execute(vec![format!("f{}.txt", i)]).unwrap();
        lit::commands::commit::execute(format!("commit {}", i), None).unwrap();
    }

    let result = lit::commands::reflog::execute(None, 2);
    assert!(result.is_ok(), "Reflog with count should succeed");
    let resp = result.unwrap();
    assert!(resp.entries.len() <= 2, "Should limit to 2 entries");
}

#[test]
fn test_reflog_specific_ref() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "aaa");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first".to_string(), None).unwrap();

    let result = lit::commands::reflog::execute(Some("main".to_string()), 10);
    assert!(result.is_ok(), "Reflog for 'main' should succeed");
}


#[test]
fn test_reflog_empty_repo() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // No commits — reflog should succeed with empty entries
    let result = lit::commands::reflog::execute(None, 10);
    assert!(result.is_ok(), "Reflog on empty repo should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(resp.entries.is_empty(), "Empty repo should have no reflog entries");
}

#[test]
fn test_reflog_nonexistent_ref() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    create_file(temp.path(), "a.txt", "content");
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first".to_string(), None).unwrap();

    // Request reflog for a nonexistent branch
    let result = lit::commands::reflog::execute(Some("nonexistent-branch".to_string()), 10);
    assert!(result.is_ok(), "Reflog for nonexistent ref should succeed");
    let resp = result.unwrap();
    assert!(resp.entries.is_empty(), "Nonexistent ref should have empty reflog");
}

#[test]
fn test_reflog_entry_ordering() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Directly write reflog entries to test ordering
    let hash_a = "a".repeat(192);
    let hash_b = "b".repeat(192);
    let hash_c = "c".repeat(192);
    lit::commands::reflog::append_reflog(temp.path(), "main", &hash_a, &hash_b, "commit", "first commit").unwrap();
    lit::commands::reflog::append_reflog(temp.path(), "main", &hash_b, &hash_c, "commit", "second commit").unwrap();

    let result = lit::commands::reflog::execute(Some("main".to_string()), 10).unwrap();
    assert_eq!(result.entries.len(), 2, "Should have 2 reflog entries");

    // Entries should be in reverse chronological order (most recent first)
    assert!(
        result.entries[0].message.contains("second"),
        "First entry should be 'second commit' but got: {}", result.entries[0].message
    );
    assert!(
        result.entries[1].message.contains("first"),
        "Second entry should be 'first commit' but got: {}", result.entries[1].message
    );
}