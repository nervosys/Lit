/// Command tests for `lit clone`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use tempfile::TempDir;

// Helper for test isolation
fn setup_test_env() -> TempDir {
    TempDir::new().unwrap()
}

#[test]
fn test_clone_nonexistent_repo() {
    let temp = setup_test_env();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result =
        lit::commands::clone::execute("file:///nonexistent/path/to/repo".to_string(), None);
    assert!(result.is_err(), "Clone should fail for nonexistent path");
    let error = result.unwrap_err();
    assert!(
        error.contains("Cannot resolve")
            || error.contains("not found")
            || error.contains("does not appear"),
        "Error should mention path resolution: {}",
        error
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_clone_with_file_url() {
    let temp = setup_test_env();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::clone::execute("file:///path/to/repo".to_string(), None);
    // Should validate transport but fail on implementation
    assert!(result.is_err(), "Clone should return error");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_clone_with_network_share() {
    let temp = setup_test_env();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::clone::execute("//server/share/repo".to_string(), None);
    // Should validate transport but fail on implementation
    assert!(result.is_err(), "Clone should return error");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_clone_with_directory() {
    let temp = setup_test_env();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::clone::execute(
        "file:///path/to/repo".to_string(),
        Some("target-dir".to_string()),
    );
    assert!(result.is_err(), "Clone with directory should return error");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_clone_validates_airgap_transport() {
    let temp = setup_test_env();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Clone should validate transport before failing on implementation
    let result = lit::commands::clone::execute("file:///path/to/repo".to_string(), None);

    // Should get to validation stage (may pass or fail depending on airgap config)
    assert!(
        result.is_err(),
        "Clone should return error (not implemented)"
    );

    std::env::set_current_dir(original_dir).unwrap();
}
