/// Integration tests for HTTPS transport
///
/// These tests start a `lit serve` instance in a background thread and exercise
/// the full push/fetch/clone lifecycle over HTTP transport.
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid interference.
use std::fs;
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Find a free port by binding to port 0
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Start lit serve in background thread, returns port and a shutdown flag.
/// The server thread cannot be cleanly stopped (tiny_http is blocking), so we
/// just let it die when the test process exits. The returned `Arc<AtomicBool>`
/// is a placeholder for signalling intent.
fn start_server(repo_path: &std::path::Path, token: Option<String>) -> (u16, Arc<AtomicBool>) {
    let port = free_port();
    let running = Arc::new(AtomicBool::new(true));
    let repo = repo_path.to_path_buf();
    let tok = token.clone();

    thread::spawn(move || {
        let _ = lit::commands::serve::execute_at(port, tok, repo);
    });

    // Give the server time to start
    thread::sleep(Duration::from_millis(300));

    (port, running)
}

/// Helper: init a repo and make a commit so there's content to push/pull
fn init_repo_with_commit(dir: &std::path::Path) -> String {
    let _cwd = super::test_helpers::CwdGuard::new(dir);

    lit::commands::init::execute(false, None).unwrap();

    // Create a file and commit
    fs::write(dir.join("hello.txt"), "Hello, world!\n").unwrap();
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    let commit_resp = lit::commands::commit::execute(
        "Initial commit".to_string(),
        Some("Test <test@test.com>".to_string()),
    )
    .unwrap();

    commit_resp.hash
}

/// Helper: init a bare-ish repo (just init, no commits)
fn init_bare_repo(dir: &std::path::Path) {
    let _cwd = super::test_helpers::CwdGuard::new(dir);
    lit::commands::init::execute(false, None).unwrap();
}

// ---------------------------------------------------------------------------
// Transport API: List refs
// ---------------------------------------------------------------------------

#[test]
fn test_http_list_refs_empty() {
    let server_dir = TempDir::new().unwrap();
    init_bare_repo(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    let refs = lit::network::https::list_refs_http(&base_url, "heads", None).unwrap();
    assert!(refs.is_empty(), "Bare repo should have no refs");
}

#[test]
fn test_http_list_refs_with_branches() {
    let server_dir = TempDir::new().unwrap();
    init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    let refs = lit::network::https::list_refs_http(&base_url, "heads", None).unwrap();
    assert!(!refs.is_empty(), "Should have at least one branch");
    assert!(
        refs.iter().any(|r| r.name == "main"),
        "Should have main branch"
    );
}

#[test]
fn test_http_read_head() {
    let server_dir = TempDir::new().unwrap();
    init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    let head = lit::network::https::read_head_http(&base_url, None).unwrap();
    assert!(
        head.contains("main"),
        "HEAD should reference main, got: {}",
        head
    );
}

#[test]
fn test_http_read_ref() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    let hash = lit::network::https::read_ref_http(&base_url, "main", None).unwrap();
    assert_eq!(hash, commit_hash, "Branch ref should match commit hash");
}

// ---------------------------------------------------------------------------
// Transport API: Bearer token auth
// ---------------------------------------------------------------------------

#[test]
fn test_http_auth_required_rejected_without_token() {
    let server_dir = TempDir::new().unwrap();
    init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), Some("secret-token".to_string()));
    let base_url = format!("http://127.0.0.1:{}", port);

    let result = lit::network::https::list_refs_http(&base_url, "heads", None);
    assert!(result.is_err(), "Should fail without token");
    let err = result.unwrap_err();
    assert!(
        err.contains("401") || err.contains("Unauthorized"),
        "Should get 401, got: {}",
        err
    );
}

#[test]
fn test_http_auth_accepted_with_correct_token() {
    let server_dir = TempDir::new().unwrap();
    init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), Some("secret-token".to_string()));
    let base_url = format!("http://127.0.0.1:{}", port);

    let refs =
        lit::network::https::list_refs_http(&base_url, "heads", Some("secret-token")).unwrap();
    assert!(!refs.is_empty(), "Should succeed with correct token");
}

#[test]
fn test_http_auth_rejected_with_wrong_token() {
    let server_dir = TempDir::new().unwrap();
    init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), Some("secret-token".to_string()));
    let base_url = format!("http://127.0.0.1:{}", port);

    let result = lit::network::https::list_refs_http(&base_url, "heads", Some("wrong-token"));
    assert!(result.is_err(), "Should fail with wrong token");
}

// ---------------------------------------------------------------------------
// Transport API: Object transfer
// ---------------------------------------------------------------------------

#[test]
fn test_http_negotiate_and_download_objects() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    // Negotiate: we want the commit, we have nothing
    let needed =
        lit::network::https::negotiate_http(&base_url, &[commit_hash.clone()], &[], None).unwrap();
    assert!(
        !needed.is_empty(),
        "Should need at least the commit + tree + blob"
    );

    // Download into a fresh repo
    let client_dir = TempDir::new().unwrap();
    init_bare_repo(client_dir.path());
    let client_store = lit::storage::ObjectStore::new(client_dir.path());

    let downloaded =
        lit::network::https::download_objects_http(&base_url, &client_store, &needed, None)
            .unwrap();
    assert_eq!(
        downloaded,
        needed.len(),
        "Should download all needed objects"
    );

    // Verify objects exist in client store
    for hash in &needed {
        assert!(
            client_store.exists(hash),
            "Object {} should exist in client",
            hash.as_str()
        );
    }
}

#[test]
fn test_http_upload_objects() {
    let server_dir = TempDir::new().unwrap();
    init_bare_repo(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    // Create a client with content
    let client_dir = TempDir::new().unwrap();
    let commit_hash = init_repo_with_commit(client_dir.path());
    let client_store = lit::storage::ObjectStore::new(client_dir.path());

    // Walk commit graph to collect all objects
    let needed = lit::network::transport::walk_commit_graph(
        &client_store,
        &lit::core::ObjectHash::from_hex(commit_hash.clone()),
        &std::collections::HashSet::new(),
    )
    .unwrap();

    let uploaded =
        lit::network::https::upload_objects_http(&base_url, &client_store, &needed, None).unwrap();
    assert_eq!(uploaded, needed.len(), "Should upload all objects");

    // Update the branch ref on the server
    lit::network::https::update_ref_http(&base_url, "main", &commit_hash, false, None).unwrap();

    // Verify the server has the ref
    let server_hash = lit::network::https::read_ref_http(&base_url, "main", None).unwrap();
    assert_eq!(server_hash, commit_hash);
}

// ---------------------------------------------------------------------------
// RemoteRepo abstraction: HTTP variant
// ---------------------------------------------------------------------------

#[test]
fn test_remote_repo_open_http() {
    let remote = lit::network::transport::RemoteRepo::open("http://127.0.0.1:9999").unwrap();
    match remote {
        lit::network::transport::RemoteRepo::Http { base_url, .. } => {
            assert_eq!(base_url, "http://127.0.0.1:9999");
        }
        _ => panic!("Should be Http variant"),
    }
}

#[test]
fn test_remote_repo_http_list_branches() {
    let server_dir = TempDir::new().unwrap();
    init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    let remote = lit::network::transport::RemoteRepo::open(&base_url).unwrap();
    let branches = remote.list_branches().unwrap();
    assert!(
        branches.iter().any(|(name, _)| name == "main"),
        "Should list main branch"
    );
}

// ---------------------------------------------------------------------------
// Full push-over-HTTP workflow
// ---------------------------------------------------------------------------

#[test]
fn test_push_over_http() {
    let server_dir = TempDir::new().unwrap();
    init_bare_repo(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);

    // Create client repo with a commit
    let client_dir = TempDir::new().unwrap();
    let _commit_hash = init_repo_with_commit(client_dir.path());

    let _cwd = super::test_helpers::CwdGuard::new(client_dir.path());

    // Add HTTP remote
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: format!("http://127.0.0.1:{}", port),
    }))
    .unwrap();

    // Disable airgap for this test
    lit::network::AirgapConfig::disable_airgap_mode();

    // Push to HTTP remote
    let result = lit::commands::push::execute("origin".to_string(), "main".to_string(), false);

    let resp = result.unwrap();
    assert!(resp.updated, "Push should report as updated");
    assert!(resp.objects_transferred > 0, "Should transfer objects");
}

// ---------------------------------------------------------------------------
// Full fetch-over-HTTP workflow
// ---------------------------------------------------------------------------

#[test]
fn test_fetch_over_http() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);

    // Create a fresh client repo
    let client_dir = TempDir::new().unwrap();
    init_bare_repo(client_dir.path());

    let _cwd = super::test_helpers::CwdGuard::new(client_dir.path());

    // Add HTTP remote
    lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
        name: "origin".to_string(),
        url: format!("http://127.0.0.1:{}", port),
    }))
    .unwrap();

    lit::network::AirgapConfig::disable_airgap_mode();

    let result = lit::commands::fetch::execute("origin".to_string(), Some("main".to_string()));

    let resp = result.unwrap();
    assert!(resp.objects_transferred > 0, "Should transfer objects");

    // Verify remote-tracking ref was updated
    let tracking_hash =
        lit::core::refs::read_ref(client_dir.path(), "remotes/origin/main").unwrap();
    assert_eq!(tracking_hash, commit_hash);
}

// ---------------------------------------------------------------------------
// Full clone-over-HTTP workflow
// ---------------------------------------------------------------------------

#[test]
fn test_clone_over_http() {
    let server_dir = TempDir::new().unwrap();
    let _commit_hash = init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);

    let workspace = TempDir::new().unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(workspace.path());

    lit::network::AirgapConfig::disable_airgap_mode();

    let result = lit::commands::clone::execute(
        format!("http://127.0.0.1:{}", port),
        Some("cloned-repo".to_string()),
    );

    let resp = result.unwrap();
    assert_eq!(resp.directory, "cloned-repo");
    assert!(resp.objects_transferred > 0, "Should transfer objects");
    assert!(
        resp.branches_cloned.contains(&"main".to_string()),
        "Should clone main branch"
    );

    // Verify working tree was checked out
    let cloned_path = workspace.path().join("cloned-repo");
    assert!(
        cloned_path.join("hello.txt").exists(),
        "Working tree should be checked out"
    );
    let content = fs::read_to_string(cloned_path.join("hello.txt")).unwrap();
    assert_eq!(content, "Hello, world!\n");
}

// ---------------------------------------------------------------------------
// Push then fetch roundtrip
// ---------------------------------------------------------------------------

#[test]
fn test_push_fetch_roundtrip_over_http() {
    let server_dir = TempDir::new().unwrap();
    init_bare_repo(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let http_url = format!("http://127.0.0.1:{}", port);

    // Client A: commit and push
    let client_a = TempDir::new().unwrap();
    let commit_hash = init_repo_with_commit(client_a.path());

    {
        let _cwd = super::test_helpers::CwdGuard::new(client_a.path());

        lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
            name: "origin".to_string(),
            url: http_url.clone(),
        }))
        .unwrap();

        lit::network::AirgapConfig::disable_airgap_mode();
        lit::commands::push::execute("origin".to_string(), "main".to_string(), false).unwrap();
    }

    // Client B: fetch from same server
    let client_b = TempDir::new().unwrap();
    init_bare_repo(client_b.path());

    {
        let _cwd = super::test_helpers::CwdGuard::new(client_b.path());

        lit::commands::remote::execute(Some(lit::RemoteCommands::Add {
            name: "origin".to_string(),
            url: http_url,
        }))
        .unwrap();

        let fetch_resp =
            lit::commands::fetch::execute("origin".to_string(), Some("main".to_string())).unwrap();

        assert!(fetch_resp.objects_transferred > 0);
    }

    // Verify client B has the same commit
    let tracking = lit::core::refs::read_ref(client_b.path(), "remotes/origin/main").unwrap();
    assert_eq!(tracking, commit_hash);
}

// ---------------------------------------------------------------------------
// Update ref with fast-forward check
// ---------------------------------------------------------------------------

#[test]
fn test_http_update_ref_fast_forward_check() {
    let server_dir = TempDir::new().unwrap();
    let first_hash = init_repo_with_commit(server_dir.path());

    let (port, _flag) = start_server(server_dir.path(), None);
    let base_url = format!("http://127.0.0.1:{}", port);

    // Attempt to update main to a hash that is not a descendant
    // This should fail with a non-fast-forward error
    let fake_hash = "a".repeat(192);
    let result = lit::network::https::update_ref_http(&base_url, "main", &fake_hash, false, None);
    // The server should reject this since the fake hash doesn't exist as an object
    // (either an error reading the object, or the ff check fails)
    assert!(result.is_err(), "Non-ff update should fail");

    // Force update should work
    let force_result =
        lit::network::https::update_ref_http(&base_url, "main", &first_hash, true, None);
    assert!(force_result.is_ok(), "Force update should succeed");
}
