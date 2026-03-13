/// Command tests for `lit init`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use std::fs;
use tempfile::TempDir;

#[test]
fn test_init_creates_repository_structure() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    // Execute init command
    let result = lit::commands::init::execute(false, Some(repo_path.clone()));
    assert!(result.is_ok(), "Init should succeed");

    let lit_dir = temp.path().join(".lit");

    // Verify directory structure
    assert!(lit_dir.exists(), ".lit directory should exist");
    assert!(
        lit_dir.join("objects").exists(),
        "objects directory should exist"
    );
    assert!(
        lit_dir.join("refs/heads").exists(),
        "refs/heads directory should exist"
    );
    assert!(
        lit_dir.join("refs/tags").exists(),
        "refs/tags directory should exist"
    );
    assert!(
        lit_dir.join("refs/remotes").exists(),
        "refs/remotes directory should exist"
    );

    // Verify files
    assert!(lit_dir.join("HEAD").exists(), "HEAD file should exist");
    assert!(lit_dir.join("config").exists(), "config file should exist");
    assert!(
        lit_dir.join("description").exists(),
        "description file should exist"
    );
    assert!(lit_dir.join("index").exists(), "index file should exist");
}

#[test]
fn test_init_creates_head_reference() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();

    let head_content = fs::read_to_string(temp.path().join(".lit/HEAD")).unwrap();
    assert!(
        head_content.contains("ref: refs/heads/main"),
        "HEAD should point to refs/heads/main"
    );
}

#[test]
fn test_init_creates_config() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();

    let config_content = fs::read_to_string(temp.path().join(".lit/config")).unwrap();
    assert!(
        config_content.contains("bare = false"),
        "Config should indicate non-bare repository"
    );
}

#[test]
fn test_init_bare_repository() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    let result = lit::commands::init::execute(true, Some(repo_path.clone()));
    assert!(result.is_ok(), "Bare init should succeed");

    let config_content = fs::read_to_string(temp.path().join(".lit/config")).unwrap();
    assert!(
        config_content.contains("bare = true"),
        "Config should indicate bare repository"
    );
}

#[test]
fn test_init_fails_if_already_initialized() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    // First init should succeed
    let result1 = lit::commands::init::execute(false, Some(repo_path.clone()));
    assert!(result1.is_ok(), "First init should succeed");

    // Second init should fail
    let result2 = lit::commands::init::execute(false, Some(repo_path.clone()));
    assert!(result2.is_err(), "Second init should fail");
    assert!(
        result2.unwrap_err().contains("already exists"),
        "Error should mention repository already exists"
    );
}

#[test]
fn test_init_in_current_directory() {
    let temp = TempDir::new().unwrap();

    // Change to temp directory
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Init without path (uses current directory)
    let result = lit::commands::init::execute(false, None);
    assert!(result.is_ok(), "Init in current directory should succeed");

    assert!(
        temp.path().join(".lit").exists(),
        ".lit should exist in current directory"
    );

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_init_creates_empty_index() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();

    // Load index and verify it's empty
    let index = lit::storage::Index::load(temp.path()).unwrap();
    assert_eq!(index.entries.len(), 0, "Index should be empty after init");
}

#[test]
fn test_init_creates_subdirectories() {
    let temp = TempDir::new().unwrap();
    let nested_path = temp.path().join("deep/nested/path");
    let repo_path = nested_path.to_str().unwrap().to_string();

    let result = lit::commands::init::execute(false, Some(repo_path.clone()));
    assert!(result.is_ok(), "Init should create nested directories");

    assert!(
        nested_path.join(".lit").exists(),
        ".lit should exist in nested path"
    );
}
