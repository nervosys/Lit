/// Command tests for `lit lfs`
use std::fs;
use tempfile::TempDir;

#[test]
fn test_lfs_track_creates_litattributes() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::lfs::execute_track(vec!["*.bin".to_string(), "*.dat".to_string()]);

    assert!(result.is_ok(), "LFS track should succeed: {:?}", result);
    let response = result.unwrap();
    assert_eq!(response.patterns.len(), 2, "Should add 2 patterns");

    let attrs_path = temp.path().join(".litattributes");
    assert!(attrs_path.exists(), ".litattributes should exist");

    let content = fs::read_to_string(&attrs_path).unwrap();
    assert!(content.contains("*.bin"), "Should contain *.bin pattern");
    assert!(content.contains("*.dat"), "Should contain *.dat pattern");
}

#[test]
fn test_lfs_track_deduplicates_patterns() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    lit::commands::lfs::execute_track(vec!["*.bin".to_string()]).unwrap();
    let result = lit::commands::lfs::execute_track(vec!["*.bin".to_string(), "*.dat".to_string()]);

    assert!(result.is_ok(), "Second track should succeed: {:?}", result);
    let response = result.unwrap();
    // Should have both patterns but no duplicates
    assert_eq!(response.patterns.len(), 2, "Should have 2 unique patterns");
}

#[test]
fn test_lfs_migrate_empty_repo() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::lfs::execute_migrate(Some(1024));

    assert!(result.is_ok(), "LFS migrate should succeed: {:?}", result);
    let response = result.unwrap();
    assert_eq!(
        response.files_migrated, 0,
        "Empty repo should migrate 0 files"
    );
}
