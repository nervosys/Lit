/// Command tests for `lit remote`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use tempfile::TempDir;

// Helper to initialize a test repository
fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_remote_list_empty() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::remote::execute(None);
    assert!(result.is_ok(), "Remote list on empty config should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_add() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///path/to/repo".to_string(),
    }));
    assert!(result.is_ok(), "Remote add should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_add_multiple() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///path/to/repo1".to_string(),
    }))
    .unwrap();

    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "upstream".to_string(),
        url: "file:///path/to/repo2".to_string(),
    }))
    .unwrap();

    // List should show both
    let result = lit::commands::remote::execute(None);
    assert!(result.is_ok(), "Remote list should show multiple remotes");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_remove() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Add a remote first
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///path/to/repo".to_string(),
    }))
    .unwrap();

    // Remove it
    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::Remove {
        name: "origin".to_string(),
    }));
    assert!(result.is_ok(), "Remote remove should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_remove_nonexistent() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::Remove {
        name: "nonexistent".to_string(),
    }));
    assert!(result.is_err(), "Remote remove nonexistent should fail");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_list_verbose() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Add a remote
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///path/to/repo".to_string(),
    }))
    .unwrap();

    // List with verbose
    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::List { verbose: true }));
    assert!(result.is_ok(), "Remote list verbose should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_list_non_verbose() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Add a remote
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///path/to/repo".to_string(),
    }))
    .unwrap();

    // List without verbose
    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::List { verbose: false }));
    assert!(result.is_ok(), "Remote list should succeed");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_config_persistence() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    // Add a remote
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///path/to/repo".to_string(),
    }))
    .unwrap();

    // Verify remotes file exists
    let remotes_file = temp.path().join(".lit/remotes");
    assert!(remotes_file.exists(), "Remotes config file should exist");

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_remote_with_network_share_url() {
    let temp = init_test_repo();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "share".to_string(),
        url: "//server/share/repo".to_string(),
    }));
    // Should succeed (validation happens during push/pull)
    assert!(
        result.is_ok(),
        "Remote add with network share URL should succeed: {:?}",
        result.err()
    );

    std::env::set_current_dir(original_dir).unwrap();
}
