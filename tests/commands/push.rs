/// Command tests for `lit push`
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
fn test_push_to_local_remote() {
    // Create "remote" repo (bare-like, needs at least init)
    let remote = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();

    // Create local repo with a commit
    let local = init_test_repo();
    std::env::set_current_dir(local.path()).unwrap();
    create_file(local.path(), "file.txt", "push content");
    lit::commands::add::execute(vec!["file.txt".to_string()]).unwrap();
    lit::commands::commit::execute("local commit".to_string(), None).unwrap();

    // Add remote
    let remote_url = remote.path().to_str().unwrap().to_string();
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: remote_url,
    }))
    .unwrap();

    let result =
        lit::commands::push::execute("origin".to_string(), "main".to_string(), false);
    assert!(result.is_ok(), "Push should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(resp.objects_transferred > 0, "Should transfer objects");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_push_nonexistent_remote() {
    let local = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(local.path()).unwrap();

    create_file(local.path(), "x.txt", "data");
    lit::commands::add::execute(vec!["x.txt".to_string()]).unwrap();
    lit::commands::commit::execute("c1".to_string(), None).unwrap();

    let result =
        lit::commands::push::execute("nonexistent".to_string(), "main".to_string(), false);
    assert!(result.is_err(), "Push to nonexistent remote should fail");

    std::env::set_current_dir(original_dir).unwrap();
}
