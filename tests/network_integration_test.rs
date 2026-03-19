/// Network integration tests for Lit VCS
///
/// Tests push/pull transport validation, remote configuration, airgap rules,
/// and file-based transport operations using local file paths as remotes.
use std::fs;
use tempfile::TempDir;

mod cwd_guard {
    use std::cell::Cell;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard};

    static CWD_MUTEX: Mutex<()> = Mutex::new(());
    thread_local! { static CWD_LOCKED: Cell<bool> = const { Cell::new(false) }; }

    pub struct CwdGuard {
        original: std::path::PathBuf,
        _lock: Option<MutexGuard<'static, ()>>,
    }
    impl CwdGuard {
        pub fn new(path: &Path) -> Self {
            let already_held = CWD_LOCKED.with(|c| c.get());
            if already_held {
                std::env::set_current_dir(path).unwrap();
                return CwdGuard {
                    original: std::path::PathBuf::new(),
                    _lock: None,
                };
            }
            let lock = CWD_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            CWD_LOCKED.with(|c| c.set(true));
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            CwdGuard {
                original,
                _lock: Some(lock),
            }
        }
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            if self._lock.is_some() {
                let _ = std::env::set_current_dir(&self.original);
                CWD_LOCKED.with(|c| c.set(false));
            }
        }
    }
}

/// Helper: create a minimal .lit repo skeleton.
fn init_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".lit/objects")).unwrap();
    fs::create_dir_all(dir.join(".lit/refs/heads")).unwrap();
    fs::write(dir.join(".lit/HEAD"), "ref: refs/heads/main\n").unwrap();
}

/// Helper: stage a file in the index.
#[allow(dead_code)]
fn stage_file(repo: &std::path::Path, name: &str, content: &str) {
    fs::write(repo.join(name), content).unwrap();
    let blob = lit::core::Blob::new(content.as_bytes().to_vec());
    let obj = lit::core::Object::Blob(blob);
    let store = lit::storage::ObjectStore::new(repo);
    let hash = store.write(&obj).unwrap();

    let mut index = lit::storage::Index::load(repo).unwrap();
    index.add(
        name.to_string(),
        hash.as_str().to_string(),
        "100644".to_string(),
    );
    index.save(repo).unwrap();
}

// ---------------------------------------------------------------------------
// Push / Pull transport validation
// ---------------------------------------------------------------------------

#[test]
fn test_push_requires_remote_configured() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    let result = lit::commands::push::execute("origin".to_string(), "main".to_string(), false);

    // Should fail because no remote is configured
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.contains("No remotes configured") || err.contains("remote"),
        "Error should mention missing remote config, got: {}",
        err
    );
}

#[test]
fn test_pull_requires_remote_configured() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    let result = lit::commands::pull::execute("origin".to_string(), "main".to_string());

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.contains("No remotes configured") || err.contains("remote"),
        "Error should mention missing remote config, got: {}",
        err
    );
}

#[test]
fn test_push_with_file_remote() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    // Add a file:// remote (not a valid lit repo)
    let remote_dir = TempDir::new().unwrap();
    let remote_url = format!(
        "file:///{}",
        remote_dir.path().to_string_lossy().replace('\\', "/")
    );

    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: remote_url,
    }))
    .unwrap();

    // Push should fail because remote is not a lit repo
    let result = lit::commands::push::execute("origin".to_string(), "main".to_string(), false);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.contains("not appear to be a Lit repository")
            || err.contains("Cannot resolve")
            || err.contains("Branch"),
        "Should fail with path resolution error. Got: {}",
        err
    );
}

#[test]
fn test_pull_with_file_remote() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    let remote_dir = TempDir::new().unwrap();
    let remote_url = format!(
        "file:///{}",
        remote_dir.path().to_string_lossy().replace('\\', "/")
    );

    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: remote_url,
    }))
    .unwrap();

    let result = lit::commands::pull::execute("origin".to_string(), "main".to_string());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.contains("not appear to be a Lit repository")
            || err.contains("Cannot resolve")
            || err.contains("Branch"),
        "Should fail with path resolution error. Got: {}",
        err
    );
}

// ---------------------------------------------------------------------------
// Network share remote
// ---------------------------------------------------------------------------

#[test]
fn test_push_with_network_share_remote() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    // Ensure airgap global flag is off (after CwdGuard to avoid racing other airgap tests)
    lit::network::AirgapConfig::disable_airgap_mode();

    // Save current airgap config and write a clean one
    let home = dirs::home_dir().unwrap();
    let airgap_path = home.join(".lit").join("airgap.toml");
    let backup = if airgap_path.exists() {
        Some(fs::read_to_string(&airgap_path).unwrap())
    } else {
        None
    };
    fs::create_dir_all(airgap_path.parent().unwrap()).unwrap();
    fs::write(&airgap_path, "enabled = false\nstrict_mode = false\n").unwrap();

    // Add a UNC path remote
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "share".to_string(),
        url: "//server/share/repo.lit".to_string(),
    }))
    .unwrap();

    let result = lit::commands::push::execute("share".to_string(), "main".to_string(), false);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    // Should either be "not implemented" or a transport validation issue, not a panic
    assert!(
        err.contains("not yet fully implemented")
            || err.contains("Push")
            || err.contains("transport")
            || err.contains("network"),
        "Should produce a sensible error, got: {}",
        err
    );

    // Restore original airgap config
    match backup {
        Some(content) => fs::write(&airgap_path, content).unwrap(),
        None => {
            let _ = fs::remove_file(&airgap_path);
        }
    }
}

// ---------------------------------------------------------------------------
// Airgap transport validation
// ---------------------------------------------------------------------------

#[test]
fn test_airgap_blocks_http_remote_on_push() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    // Write a remote config manually with an http URL
    let remotes_json = r#"{"remotes":{"bad":{"url":"http://evil.com/repo"}}}"#;
    fs::write(tmp.path().join(".lit/remotes"), remotes_json).unwrap();

    // Enable airgap mode
    lit::network::AirgapConfig::enable_airgap_mode();

    let result = lit::commands::push::execute("bad".to_string(), "main".to_string(), false);
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.to_lowercase().contains("http")
            || err.to_lowercase().contains("blocked")
            || err.to_lowercase().contains("not allowed")
            || err.to_lowercase().contains("airgap")
            || err.to_lowercase().contains("transport"),
        "Airgap mode should block HTTP transport, got: {}",
        err
    );

    // Clean up global state
    lit::network::AirgapConfig::disable_airgap_mode();
}

#[test]
fn test_airgap_blocks_http_remote_on_pull() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    let remotes_json = r#"{"remotes":{"bad":{"url":"https://evil.com/repo"}}}"#;
    fs::write(tmp.path().join(".lit/remotes"), remotes_json).unwrap();

    lit::network::AirgapConfig::enable_airgap_mode();

    let result = lit::commands::pull::execute("bad".to_string(), "main".to_string());
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.to_lowercase().contains("https")
            || err.to_lowercase().contains("blocked")
            || err.to_lowercase().contains("not allowed")
            || err.to_lowercase().contains("airgap")
            || err.to_lowercase().contains("transport"),
        "Airgap mode should block HTTPS transport, got: {}",
        err
    );

    lit::network::AirgapConfig::disable_airgap_mode();
}

#[test]
fn test_airgap_allows_file_remote_on_push() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    let remote_dir = TempDir::new().unwrap();
    let remote_url = format!(
        "file:///{}",
        remote_dir.path().to_string_lossy().replace('\\', "/")
    );

    let remotes_json = format!(r#"{{"remotes":{{"usb":{{"url":"{}"}}}}}}"#, remote_url);
    fs::write(tmp.path().join(".lit/remotes"), remotes_json).unwrap();

    lit::network::AirgapConfig::enable_airgap_mode();

    let result = lit::commands::push::execute("usb".to_string(), "main".to_string(), false);
    // Should NOT fail with a transport error — only with repo/branch resolution
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.contains("not appear to be a Lit repository")
            || err.contains("Cannot resolve")
            || err.contains("Branch"),
        "file:// transport should be allowed in airgap mode, got: {}",
        err
    );

    lit::network::AirgapConfig::disable_airgap_mode();
}

// ---------------------------------------------------------------------------
// Remote configuration persistence
// ---------------------------------------------------------------------------

#[test]
fn test_remote_config_roundtrip() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    // Add several remotes
    for i in 0..5 {
        lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
            name: format!("remote-{}", i),
            url: format!("file:///mnt/usb{}/repo", i),
        }))
        .unwrap();
    }

    // Verify the .lit/remotes file is valid JSON
    let content = fs::read_to_string(tmp.path().join(".lit/remotes")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let remotes = parsed["remotes"].as_object().unwrap();
    assert_eq!(remotes.len(), 5);

    // Remove one and verify
    lit::commands::remote::execute(Some(lit::RemoteCommands::Remove {
        name: "remote-2".to_string(),
    }))
    .unwrap();

    let content = fs::read_to_string(tmp.path().join(".lit/remotes")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let remotes = parsed["remotes"].as_object().unwrap();
    assert_eq!(remotes.len(), 4);
    assert!(!remotes.contains_key("remote-2"));
}

#[test]
fn test_remote_add_duplicate_fails() {
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let _cwd = cwd_guard::CwdGuard::new(tmp.path());

    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///repo".to_string(),
    }))
    .unwrap();

    let result = lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: "file:///other".to_string(),
    }));
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err = err.internal_message();
    assert!(
        err.contains("already exists"),
        "Should reject duplicate remote name, got: {}",
        err
    );
}

// ---------------------------------------------------------------------------
// Network validator (LAN-only rules)
// ---------------------------------------------------------------------------

#[test]
fn test_network_validator_allows_private_ips() {
    let validator = lit::network::NetworkValidator::new().unwrap();
    // 10.x.x.x, 172.16-31.x.x, 192.168.x.x should be allowed
    assert!(validator.validate_url("lit://10.0.0.1/repo").is_ok());
    assert!(validator.validate_url("lit://192.168.1.100/repo").is_ok());
    assert!(validator.validate_url("lit://172.16.0.1/repo").is_ok());
}

#[test]
fn test_network_validator_blocks_public_ips() {
    let validator = lit::network::NetworkValidator::new().unwrap();
    // Public IPs should be blocked
    let result = validator.validate_url("lit://8.8.8.8/repo");
    assert!(result.is_err(), "Public IP 8.8.8.8 should be blocked");
}

#[test]
fn test_network_validator_blocks_non_lit_protocol() {
    let validator = lit::network::NetworkValidator::new().unwrap();
    let result = validator.validate_url("git://192.168.1.1/repo");
    assert!(result.is_err(), "git:// protocol should be rejected");

    let result = validator.validate_url("https://192.168.1.1/repo");
    assert!(result.is_err(), "https:// protocol should be rejected");
}

#[test]
fn test_network_validator_accepts_only_lit_protocol() {
    let validator = lit::network::NetworkValidator::new().unwrap();
    let result = validator.validate_url("lit://192.168.1.1/repo");
    assert!(result.is_ok(), "lit:// with private IP should be accepted");
}
