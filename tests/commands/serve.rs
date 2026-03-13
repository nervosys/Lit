/// Command tests for `lit serve`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
///
/// The serve command starts a blocking HTTP server, so we test configuration
/// and error paths rather than the running server.
use tempfile::TempDir;

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_serve_starts_on_available_port() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Use a high port to avoid conflicts; the server blocks, so spawn in a thread
    // and give it a moment to bind, then drop.
    let handle = std::thread::spawn(|| {
        lit::commands::serve::execute(18384, Some("test-token".to_string()))
    });

    // Give the server a moment to start
    std::thread::sleep(std::time::Duration::from_millis(200));

    // The server is blocking, so we can't wait for it. Just verify the thread is
    // running (it didn't immediately error out).
    assert!(!handle.is_finished(), "Server should still be running");

    std::env::set_current_dir(original_dir).unwrap();
    // Thread is left running; it will be cleaned up when the test process exits.
}
