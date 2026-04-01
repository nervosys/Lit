/// Command tests for `lit watch`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
///
/// The watch command runs an infinite polling loop, so we test it by spawning
/// in a thread and verifying it starts.
use std::fs;
use tempfile::TempDir;

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_watch_starts_polling() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create a file so the watcher has something to scan
    fs::write(temp.path().join("watched.txt"), "content").unwrap();

    let handle = std::thread::spawn(|| lit::commands::watch::execute(500, None));

    std::thread::sleep(std::time::Duration::from_millis(300));

    assert!(
        !handle.is_finished(),
        "Watch should still be running (infinite loop)"
    );
}
