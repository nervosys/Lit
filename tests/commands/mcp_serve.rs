/// Command tests for `lit mcp-serve`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
///
/// The mcp-serve command starts a blocking JSON-RPC server. We test the HTTP
/// mode by spawning in a thread and verifying it starts without errors.
use tempfile::TempDir;

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_mcp_serve_http_starts() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let handle = std::thread::spawn(|| {
        lit::commands::mcp_serve::execute_http(19384)
    });

    std::thread::sleep(std::time::Duration::from_millis(200));

    assert!(!handle.is_finished(), "MCP HTTP server should still be running");
}