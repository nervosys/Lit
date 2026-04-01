/// Command tests for `lit config`
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
fn test_config_show() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Show));
    assert!(result.is_ok(), "Config show should succeed");
}

#[test]
fn test_config_show_no_command() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Calling with None should default to show
    let result = lit::commands::config::execute(None);
    assert!(
        result.is_ok(),
        "Config with no command should default to show"
    );
}

#[test]
fn test_config_get_airgap_enabled() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Get {
        key: "airgap.enabled".to_string(),
    }));
    assert!(result.is_ok(), "Config get airgap.enabled should succeed");
}

#[test]
fn test_config_get_airgap_strict_mode() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Get {
        key: "airgap.strict_mode".to_string(),
    }));
    assert!(
        result.is_ok(),
        "Config get airgap.strict_mode should succeed"
    );
}

#[test]
fn test_config_get_unknown_key() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Get {
        key: "unknown.key".to_string(),
    }));
    assert!(result.is_err(), "Config get with unknown key should fail");
    let error = result.unwrap_err();
    let error = error.internal_message();
    assert!(
        error.contains("Unknown configuration key"),
        "Error should mention unknown key"
    );
}

#[test]
fn test_config_set_airgap_enabled_true() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "airgap.enabled".to_string(),
        value: "true".to_string(),
    }));
    assert!(
        result.is_ok(),
        "Config set airgap.enabled to true should succeed"
    );

    // Verify it was set
    let get_result = lit::commands::config::execute(Some(lit::ConfigCommands::Get {
        key: "airgap.enabled".to_string(),
    }));
    assert!(
        get_result.is_ok(),
        "Should be able to get the value after setting"
    );

    // Clean up: disable airgap mode AND restore config file to avoid affecting other tests
    lit::network::AirgapConfig::disable_airgap_mode();
    lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "airgap.enabled".to_string(),
        value: "false".to_string(),
    }))
    .ok();
}

#[test]
fn test_config_set_airgap_enabled_false() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "airgap.enabled".to_string(),
        value: "false".to_string(),
    }));
    assert!(
        result.is_ok(),
        "Config set airgap.enabled to false should succeed"
    );
}

#[test]
fn test_config_set_airgap_strict_mode() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "airgap.strict_mode".to_string(),
        value: "true".to_string(),
    }));
    assert!(
        result.is_ok(),
        "Config set airgap.strict_mode should succeed"
    );

    // Clean up: disable airgap mode AND restore config file to avoid affecting other tests
    lit::network::AirgapConfig::disable_airgap_mode();
    lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "airgap.strict_mode".to_string(),
        value: "false".to_string(),
    }))
    .ok();
}

#[test]
fn test_config_set_invalid_boolean() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "airgap.enabled".to_string(),
        value: "invalid".to_string(),
    }));
    assert!(
        result.is_err(),
        "Config set with invalid boolean should fail"
    );
    let error = result.unwrap_err();
    let error = error.internal_message();
    assert!(
        error.contains("Invalid boolean"),
        "Error should mention invalid boolean"
    );
}

#[test]
fn test_config_set_unsupported_key() {
    let temp = init_test_repo();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::config::execute(Some(lit::ConfigCommands::Set {
        key: "unsupported.key".to_string(),
        value: "value".to_string(),
    }));
    assert!(
        result.is_err(),
        "Config set with unsupported key should fail"
    );
    let error = result.unwrap_err();
    let error = error.internal_message();
    assert!(
        error.contains("not supported"),
        "Error should mention not supported"
    );
}
