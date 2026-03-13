/// Command tests for `lit pull`
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
fn test_pull_from_local_remote() {
    // Create and populate "remote" repo
    let remote = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(remote.path()).unwrap();
    create_file(remote.path(), "shared.txt", "shared content");
    lit::commands::add::execute(vec!["shared.txt".to_string()]).unwrap();
    lit::commands::commit::execute("remote commit".to_string(), None).unwrap();
    std::env::set_current_dir(&original_dir).unwrap();

    // Create local repo with its own commit, then add remote
    let local = init_test_repo();
    std::env::set_current_dir(local.path()).unwrap();
    create_file(local.path(), "local.txt", "local only");
    lit::commands::add::execute(vec!["local.txt".to_string()]).unwrap();
    lit::commands::commit::execute("local commit".to_string(), None).unwrap();

    let remote_url = remote.path().to_str().unwrap().to_string();
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: remote_url,
    }))
    .unwrap();

    let result =
        lit::commands::pull::execute("origin".to_string(), "main".to_string());
    assert!(result.is_ok(), "Pull should succeed: {:?}", result.err());

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_pull_nonexistent_remote() {
    let local = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(local.path()).unwrap();

    create_file(local.path(), "x.txt", "x");
    lit::commands::add::execute(vec!["x.txt".to_string()]).unwrap();
    lit::commands::commit::execute("c".to_string(), None).unwrap();

    let result =
        lit::commands::pull::execute("nonexistent".to_string(), "main".to_string());
    assert!(result.is_err(), "Pull from nonexistent remote should fail");

    std::env::set_current_dir(original_dir).unwrap();
}
