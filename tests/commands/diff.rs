/// Integration tests for `lit diff`, including --word-diff mode
use std::fs;
use tempfile::TempDir;

fn setup_test_env() -> TempDir {
    TempDir::new().unwrap()
}

#[test]
fn test_diff_no_changes() {
    let temp = setup_test_env();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::init::execute(false, None).unwrap();
    fs::write("hello.txt", "hello world\n").unwrap();
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    let resp = lit::commands::diff::execute(false, false, false, None, None).unwrap();
    assert!(
        resp.files.is_empty(),
        "No changes should produce empty diff"
    );
}

#[test]
fn test_diff_detects_modification() {
    let temp = setup_test_env();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::init::execute(false, None).unwrap();
    fs::write("file.txt", "line one\nline two\n").unwrap();
    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    // Modify and stage
    fs::write("file.txt", "line one\nline changed\n").unwrap();
    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();

    let resp = lit::commands::diff::execute(true, false, false, None, None).unwrap();
    assert_eq!(resp.files_changed, 1);
    assert!(resp.total_additions > 0 || resp.total_deletions > 0);
}

#[test]
fn test_diff_word_diff_flag_accepted() {
    let temp = setup_test_env();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::init::execute(false, None).unwrap();
    fs::write("doc.txt", "the quick brown fox\n").unwrap();
    lit::commands::add::execute(vec!["doc.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial".to_string(), None).unwrap();

    fs::write("doc.txt", "the slow brown fox\n").unwrap();
    lit::commands::add::execute(vec!["doc.txt".to_string()]).unwrap();

    // word_diff=true should work without errors
    let resp = lit::commands::diff::execute(true, false, true, None, None).unwrap();
    assert_eq!(resp.files_changed, 1);
    assert!(resp.word_diff, "Response should reflect word_diff=true");
}

#[test]
fn test_diff_stat_only() {
    let temp = setup_test_env();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::init::execute(false, None).unwrap();
    fs::write("a.txt", "content\n").unwrap();
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
    lit::commands::commit::execute("first".to_string(), None).unwrap();

    fs::write("a.txt", "new content\n").unwrap();
    lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();

    let resp = lit::commands::diff::execute(true, true, false, None, None).unwrap();
    assert!(resp.stat_only);
    assert!(!resp.stats.is_empty());
}