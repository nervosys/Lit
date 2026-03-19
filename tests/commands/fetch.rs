/// Command tests for `lit fetch`
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

fn setup_remote_pair() -> (TempDir, TempDir) {
    // Create "remote" repo with a commit
    let remote = init_test_repo();
    {
        let _cwd = super::test_helpers::CwdGuard::new(remote.path());
        create_file(remote.path(), "readme.txt", "hello from remote");
        lit::commands::add::execute(vec!["readme.txt".to_string()]).unwrap();
        lit::commands::commit::execute("remote initial".to_string(), None).unwrap();
    }

    // Create local repo and add remote
    let local = init_test_repo();
    {
        let _cwd = super::test_helpers::CwdGuard::new(local.path());
        let remote_url = remote.path().to_str().unwrap().to_string();
        lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
            name: "origin".to_string(),
            url: remote_url,
        }))
        .unwrap();
    }

    (remote, local)
}

#[test]
fn test_fetch_from_local_remote() {
    let (remote, local) = setup_remote_pair();
    let _keep_remote = remote; // prevent drop
    let _cwd = super::test_helpers::CwdGuard::new(local.path());

    let result = lit::commands::fetch::execute("origin".to_string(), Some("main".to_string()));
    assert!(result.is_ok(), "Fetch should succeed: {:?}", result.err());
    let resp = result.unwrap();
    assert!(
        resp.objects_transferred > 0,
        "Should transfer objects from remote"
    );
}

#[test]
fn test_fetch_nonexistent_remote() {
    let local = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(local.path());

    let result = lit::commands::fetch::execute("bogus".to_string(), None);
    assert!(result.is_err(), "Fetch from nonexistent remote should fail");
}
