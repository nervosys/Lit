/// Command tests for `lit rotate-key`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use tempfile::TempDir;

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_rotate_key_without_encryption() {
    let temp = init_test_repo();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::rotate_key::rotate_key();
    assert!(
        result.is_err(),
        "Rotate key should fail when encryption is not enabled"
    );

    std::env::set_current_dir(original_dir).unwrap();
}
