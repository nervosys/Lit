/// Command tests for `lit export-git`
use std::fs;
use tempfile::TempDir;

#[test]
fn test_export_git_creates_git_structure() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create a file and commit
    fs::write(temp.path().join("test.txt"), "hello world\n").unwrap();
    lit::commands::add::execute(vec!["test.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial commit".to_string(), None).unwrap();

    // Export to git
    let git_dest = temp.path().join("exported.git");
    let result = lit::commands::export_git::execute(git_dest.to_str().unwrap().to_string());

    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok(), "Export should succeed: {:?}", result);
    let response = result.unwrap();

    assert!(git_dest.join("HEAD").exists(), "HEAD should exist");
    assert!(git_dest.join("objects").exists(), "objects dir should exist");
    assert!(response.objects_exported > 0, "Should export at least one object");
}

#[test]
fn test_export_git_roundtrip_preserves_content() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    fs::write(temp.path().join("hello.txt"), "hello world\n").unwrap();
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    lit::commands::commit::execute("test commit".to_string(), None).unwrap();

    let git_dest = temp.path().join("out.git");
    let result = lit::commands::export_git::execute(git_dest.to_str().unwrap().to_string());

    std::env::set_current_dir(&original_dir).unwrap();

    assert!(result.is_ok(), "Export should succeed: {:?}", result);
    let response = result.unwrap();
    assert!(response.refs_exported > 0, "Should export at least one ref");
}
