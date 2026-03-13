/// Command tests for `lit tag`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use std::fs;
use tempfile::TempDir;

// Helper to initialize a test repository
fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

// Helper to create a test file
fn create_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

// Helper to create a commit and return its hash
fn create_commit(
    repo_path: &std::path::Path,
    filename: &str,
    content: &str,
    message: &str,
) -> String {
    create_file(repo_path, filename, content);

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_path).unwrap();

    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();

    let commit_hash = fs::read_to_string(repo_path.join(".lit/refs/heads/main"))
        .unwrap()
        .trim()
        .to_string();

    std::env::set_current_dir(original_dir).unwrap();

    commit_hash
}

#[test]
fn test_tag_create_lightweight() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::tag::execute(
        Some("v1.0".to_string()),
        None,  // message
        false, // annotate
        false, // delete
        false, // sign
        false, // verify
        false, // list
        None,  // commit
    );
    assert!(result.is_ok(), "Creating lightweight tag should succeed");

    // Verify tag ref exists
    let tag_ref = fs::read_to_string(temp.path().join(".lit/refs/tags/v1.0"));
    assert!(tag_ref.is_ok(), "Tag ref file should exist");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_tag_create_annotated() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::tag::execute(
        Some("v2.0".to_string()),
        Some("Release v2.0".to_string()), // message
        true,                             // annotate
        false,                            // delete
        false,                            // sign
        false,                            // verify
        false,                            // list
        None,                             // commit
    );
    assert!(result.is_ok(), "Creating annotated tag should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_tag_list_empty() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::tag::execute(None, None, false, false, false, false, true, None);
    assert!(result.is_ok(), "Listing tags should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_tag_list_after_create() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create two tags
    lit::commands::tag::execute(
        Some("v1.0".to_string()),
        None,
        false,
        false,
        false,
        false,
        false,
        None,
    )
    .unwrap();
    lit::commands::tag::execute(
        Some("v2.0".to_string()),
        None,
        false,
        false,
        false,
        false,
        false,
        None,
    )
    .unwrap();

    // List tags
    let result = lit::commands::tag::execute(None, None, false, false, false, false, true, None);
    assert!(result.is_ok(), "Listing tags should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_tag_delete() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create then delete
    lit::commands::tag::execute(
        Some("v1.0".to_string()),
        None,
        false,
        false,
        false,
        false,
        false,
        None,
    )
    .unwrap();

    let result = lit::commands::tag::execute(
        Some("v1.0".to_string()),
        None,
        false,
        true,
        false,
        false,
        false,
        None,
    );
    assert!(result.is_ok(), "Deleting tag should succeed");

    // Tag ref should be gone
    assert!(
        !temp.path().join(".lit/refs/tags/v1.0").exists(),
        "Tag ref should be deleted"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_tag_duplicate_name_fails() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Create tag
    lit::commands::tag::execute(
        Some("v1.0".to_string()),
        None,
        false,
        false,
        false,
        false,
        false,
        None,
    )
    .unwrap();

    // Try to create same tag again
    let result = lit::commands::tag::execute(
        Some("v1.0".to_string()),
        None,
        false,
        false,
        false,
        false,
        false,
        None,
    );
    assert!(result.is_err(), "Duplicate tag should fail");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_tag_signed_creates_pq_signature() {
    let temp = init_test_repo();

    create_commit(temp.path(), "test.txt", "hello", "Initial commit");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::tag::execute(
        Some("v3.0-signed".to_string()),
        Some("Signed release".to_string()),
        true,  // annotate
        false, // delete
        true,  // sign
        false, // verify
        false, // list
        None,  // commit
    );
    assert!(result.is_ok(), "Creating signed tag should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}
