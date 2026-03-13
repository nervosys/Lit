/// Command tests for `lit search`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use std::fs;
use tempfile::TempDir;

// Helper to initialize a test repository
fn init_test_repo() -> TempDir {
    let temp = tempfile::Builder::new()
        .prefix("lit_test_")
        .tempdir()
        .unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

// Helper to create a test file
fn create_file(dir: &std::path::Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

// Helper to create a commit
fn create_commit(repo_path: &std::path::Path, filename: &str, content: &str, message: &str) {
    create_file(repo_path, filename, content);

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(repo_path).unwrap();

    lit::commands::add::execute(vec![filename.to_string()]).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_search_file_contents() {
    let temp = init_test_repo();

    create_file(
        temp.path(),
        "hello.txt",
        "Hello World\nThis is a test file\nWith multiple lines",
    );
    create_file(temp.path(), "data.txt", "Some data\nHello again\nMore data");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::search::execute(
        "Hello".to_string(),
        false, // messages
        None,  // metadata_filter
        100,   // max_results
    );
    assert!(result.is_ok(), "Search should succeed");

    let response = result.unwrap();
    assert_eq!(response.match_type, "content");
    assert!(
        response.total >= 2,
        "Should find 'Hello' in at least 2 lines"
    );
    assert_eq!(response.query, "Hello");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_search_case_insensitive() {
    let temp = init_test_repo();

    create_file(
        temp.path(),
        "test.txt",
        "HELLO world\nhello WORLD\nHeLLo WoRLD",
    );

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::search::execute("hello".to_string(), false, None, 100);
    assert!(result.is_ok(), "Search should succeed");

    let response = result.unwrap();
    assert_eq!(
        response.total, 3,
        "Case-insensitive search should find all 3 lines"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_search_no_matches() {
    let temp = init_test_repo();

    create_file(temp.path(), "test.txt", "hello world");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result =
        lit::commands::search::execute("nonexistent_string_xyz".to_string(), false, None, 100);
    assert!(result.is_ok(), "Search with no matches should succeed");

    let response = result.unwrap();
    assert_eq!(response.total, 0, "Should find no matches");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_search_max_results() {
    let temp = init_test_repo();

    // Create file with many matching lines
    let content: String = (1..=20).map(|i| format!("match line {}\n", i)).collect();
    create_file(temp.path(), "many.txt", &content);

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::search::execute(
        "match".to_string(),
        false,
        None,
        5, // limit to 5 results
    );
    assert!(result.is_ok(), "Search with max_results should succeed");

    let response = result.unwrap();
    assert!(response.total <= 5, "Should respect max_results limit");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_search_commit_messages() {
    let temp = init_test_repo();

    create_commit(
        temp.path(),
        "file1.txt",
        "content1",
        "Fix critical bug in parser",
    );
    create_commit(temp.path(), "file2.txt", "content2", "Add new feature");
    create_commit(temp.path(), "file3.txt", "content3", "Fix another bug");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::search::execute(
        "Fix".to_string(),
        true, // search messages
        None,
        100,
    );
    assert!(result.is_ok(), "Message search should succeed");

    let response = result.unwrap();
    assert_eq!(response.match_type, "message");
    assert!(
        response.total >= 2,
        "Should find 'Fix' in at least 2 commit messages"
    );

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_search_commit_messages_no_commits() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::search::execute("anything".to_string(), true, None, 100);
    assert!(
        result.is_ok(),
        "Message search with no commits should succeed"
    );

    let response = result.unwrap();
    assert_eq!(response.total, 0, "Should find no matches with no commits");

    std::env::set_current_dir(original_dir).unwrap();
}
