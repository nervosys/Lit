/// Integration tests for lit:// protocol transport (TCP daemon mode)
///
/// These tests spawn `lit serve --daemon` as a background thread, then
/// connect via TCP using `LitConnection::open_local()`. This tests the
/// full TCP transport layer without requiring external infrastructure.
///
/// NOTE: Each test uses a unique port to allow parallel execution within
/// this module. Tests that modify cwd use `--test-threads=1`.
use std::collections::HashSet;
use std::fs;
use std::net::TcpListener;
use tempfile::TempDir;

/// Find an available TCP port by binding to port 0
fn find_available_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to port 0");
    listener.local_addr().unwrap().port()
}

/// Start a lit daemon in the background and wait for it to be ready
fn start_daemon(repo_path: &std::path::Path, port: u16) -> std::thread::JoinHandle<()> {
    // Find the lit binary
    let current_exe = std::env::current_exe().unwrap();
    let exe_dir = current_exe.parent().unwrap();
    let lit_exe = if exe_dir.ends_with("deps") {
        exe_dir
            .parent()
            .unwrap()
            .join("lit")
            .with_extension(std::env::consts::EXE_EXTENSION)
    } else {
        exe_dir
            .join("lit")
            .with_extension(std::env::consts::EXE_EXTENSION)
    };

    let repo = repo_path.to_path_buf();
    let handle = std::thread::spawn(move || {
        let mut child = std::process::Command::new(&lit_exe)
            .arg("serve")
            .arg("--daemon")
            .arg("--port")
            .arg(port.to_string())
            .current_dir(&repo)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn lit serve --daemon");

        // Keep the thread alive until test completes; when unparked
        // we kill and wait on the child to avoid zombie processes.
        std::thread::park();
        let _ = child.kill();
        let _ = child.wait();
    });

    // Wait for the daemon to start accepting connections
    let addr = format!("127.0.0.1:{}", port);
    for _ in 0..50 {
        if std::net::TcpStream::connect(&addr).is_ok() {
            return handle;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("Daemon did not start within 5 seconds on port {}", port);
}

/// Helper: init a repo and make a commit so there's content to push/pull
fn lit_init_repo_with_commit(dir: &std::path::Path) -> String {
    let _cwd = super::test_helpers::CwdGuard::new(dir);

    lit::commands::init::execute(false, None).unwrap();

    fs::write(dir.join("hello.txt"), "Hello from lit://!\n").unwrap();
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    let commit_resp = lit::commands::commit::execute(
        "Initial commit".to_string(),
        Some("Test <test@test.com>".to_string()),
    )
    .unwrap();

    commit_resp.hash
}

/// Helper: init a bare-ish repo (just init, no commits)
fn lit_init_bare_repo(dir: &std::path::Path) {
    let _cwd = super::test_helpers::CwdGuard::new(dir);
    lit::commands::init::execute(false, None).unwrap();
}

// ---------------------------------------------------------------------------
// lit:// URL parsing
// ---------------------------------------------------------------------------

#[test]
fn test_lit_url_detection() {
    assert!(lit::network::lit_protocol::is_lit_url(
        "lit://example.com/repo"
    ));
    assert!(lit::network::lit_protocol::is_lit_url(
        "lit://host:9418/path"
    ));
    assert!(!lit::network::lit_protocol::is_lit_url(
        "http://example.com/repo"
    ));
    assert!(!lit::network::lit_protocol::is_lit_url(
        "ssh://example.com/repo"
    ));
    assert!(!lit::network::lit_protocol::is_lit_url("/local/path"));
}

#[test]
fn test_parse_lit_url_with_port() {
    let parsed =
        lit::network::lit_protocol::parse_lit_url("lit://host.com:1234/path/to/repo").unwrap();
    assert_eq!(parsed.host, "host.com");
    assert_eq!(parsed.port, 1234);
    assert_eq!(parsed.path, "/path/to/repo");
}

#[test]
fn test_parse_lit_url_default_port() {
    let parsed = lit::network::lit_protocol::parse_lit_url("lit://host.com/path/to/repo").unwrap();
    assert_eq!(parsed.host, "host.com");
    assert_eq!(parsed.port, 9418);
    assert_eq!(parsed.path, "/path/to/repo");
}

#[test]
fn test_parse_lit_url_invalid() {
    assert!(lit::network::lit_protocol::parse_lit_url("http://example.com").is_err());
    assert!(lit::network::lit_protocol::parse_lit_url("lit://host.com").is_err()); // no path
    assert!(lit::network::lit_protocol::parse_lit_url("lit:///path").is_err()); // empty host
}

// ---------------------------------------------------------------------------
// TCP transport: List refs
// ---------------------------------------------------------------------------

#[test]
fn test_lit_tcp_list_refs_empty() {
    let server_dir = TempDir::new().unwrap();
    lit_init_bare_repo(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();
    let refs = lit::network::lit_protocol::list_refs_lit(&mut conn, "heads").unwrap();
    assert!(refs.is_empty(), "Bare repo should have no refs");

    daemon.thread().unpark();
}

#[test]
fn test_lit_tcp_list_refs_with_branches() {
    let server_dir = TempDir::new().unwrap();
    lit_init_repo_with_commit(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();
    let refs = lit::network::lit_protocol::list_refs_lit(&mut conn, "heads").unwrap();
    assert!(!refs.is_empty(), "Should have at least one branch");
    assert!(
        refs.iter().any(|r| r.name == "main"),
        "Should have main branch"
    );

    daemon.thread().unpark();
}

// ---------------------------------------------------------------------------
// TCP transport: Read HEAD and refs
// ---------------------------------------------------------------------------

#[test]
fn test_lit_tcp_read_head() {
    let server_dir = TempDir::new().unwrap();
    lit_init_repo_with_commit(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();
    let head = lit::network::lit_protocol::read_head_lit(&mut conn).unwrap();
    assert!(
        head.contains("main"),
        "HEAD should reference main, got: {}",
        head
    );

    daemon.thread().unpark();
}

#[test]
fn test_lit_tcp_read_ref() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = lit_init_repo_with_commit(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();
    let hash = lit::network::lit_protocol::read_ref_lit(&mut conn, "main").unwrap();
    assert_eq!(hash, commit_hash, "Branch ref should match commit hash");

    daemon.thread().unpark();
}

// ---------------------------------------------------------------------------
// TCP transport: Negotiate and download objects
// ---------------------------------------------------------------------------

#[test]
fn test_lit_tcp_negotiate_and_download() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = lit_init_repo_with_commit(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();

    // Negotiate: we want the commit, we have nothing
    let needed = lit::network::lit_protocol::negotiate_lit(
        &mut conn,
        std::slice::from_ref(&commit_hash),
        &[],
    )
    .unwrap();
    assert!(
        !needed.is_empty(),
        "Should need at least the commit + tree + blob"
    );

    // Download into a fresh repo
    let client_dir = TempDir::new().unwrap();
    lit_init_bare_repo(client_dir.path());
    let client_store = lit::storage::ObjectStore::new(client_dir.path());

    let downloaded =
        lit::network::lit_protocol::download_objects_lit(&mut conn, &client_store, &needed)
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

    daemon.thread().unpark();
}

// ---------------------------------------------------------------------------
// TCP transport: Upload objects
// ---------------------------------------------------------------------------

#[test]
fn test_lit_tcp_upload_objects() {
    let server_dir = TempDir::new().unwrap();
    lit_init_bare_repo(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();

    // Create a client with content
    let client_dir = TempDir::new().unwrap();
    let commit_hash = lit_init_repo_with_commit(client_dir.path());
    let client_store = lit::storage::ObjectStore::new(client_dir.path());

    // Walk commit graph to collect all objects
    let needed = lit::network::transport::walk_commit_graph(
        &client_store,
        &lit::core::ObjectHash::from_hex(commit_hash.clone()),
        &HashSet::new(),
    )
    .unwrap();

    let uploaded =
        lit::network::lit_protocol::upload_objects_lit(&mut conn, &client_store, &needed).unwrap();
    assert_eq!(uploaded, needed.len(), "Should upload all objects");

    // Update the branch ref on the server via connection
    lit::network::lit_protocol::update_ref_lit(&mut conn, "main", &commit_hash, false).unwrap();

    // Verify the server has the ref
    let server_hash = lit::network::lit_protocol::read_ref_lit(&mut conn, "main").unwrap();
    assert_eq!(server_hash, commit_hash);

    daemon.thread().unpark();
}

// ---------------------------------------------------------------------------
// TCP transport: Multiple requests over same connection (session persistence)
// ---------------------------------------------------------------------------

#[test]
fn test_lit_tcp_multiple_requests() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = lit_init_repo_with_commit(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();

    // First request
    let refs = lit::network::lit_protocol::list_refs_lit(&mut conn, "heads").unwrap();
    assert!(!refs.is_empty());

    // Second request on same connection
    let head = lit::network::lit_protocol::read_head_lit(&mut conn).unwrap();
    assert!(head.contains("main"));

    // Third request on same connection
    let hash = lit::network::lit_protocol::read_ref_lit(&mut conn, "main").unwrap();
    assert_eq!(hash, commit_hash);

    daemon.thread().unpark();
}

// ---------------------------------------------------------------------------
// Full push-fetch roundtrip over lit://
// ---------------------------------------------------------------------------

#[test]
fn test_push_fetch_roundtrip_over_lit() {
    // Server repo (empty)
    let server_dir = TempDir::new().unwrap();
    lit_init_bare_repo(server_dir.path());
    let port = find_available_port();
    let daemon = start_daemon(server_dir.path(), port);

    // Client A: create repo with commit
    let client_a = TempDir::new().unwrap();
    let commit_hash = lit_init_repo_with_commit(client_a.path());

    // Push from client A to server via lit://
    {
        let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();
        let client_store = lit::storage::ObjectStore::new(client_a.path());

        // Walk to find all objects
        let objects = lit::network::transport::walk_commit_graph(
            &client_store,
            &lit::core::ObjectHash::from_hex(commit_hash.clone()),
            &HashSet::new(),
        )
        .unwrap();

        // Upload objects
        let uploaded =
            lit::network::lit_protocol::upload_objects_lit(&mut conn, &client_store, &objects)
                .unwrap();
        assert!(uploaded > 0);

        // Update ref
        lit::network::lit_protocol::update_ref_lit(&mut conn, "main", &commit_hash, false).unwrap();
    }

    // Client B: fetch from server via lit://
    {
        let client_b = TempDir::new().unwrap();
        lit_init_bare_repo(client_b.path());
        let client_store = lit::storage::ObjectStore::new(client_b.path());

        let mut conn = lit::network::lit_protocol::LitConnection::open_local(port).unwrap();

        // Get server's main ref
        let server_hash = lit::network::lit_protocol::read_ref_lit(&mut conn, "main").unwrap();
        assert_eq!(server_hash, commit_hash);

        // Negotiate
        let needed = lit::network::lit_protocol::negotiate_lit(
            &mut conn,
            std::slice::from_ref(&server_hash),
            &[],
        )
        .unwrap();

        // Download
        let downloaded =
            lit::network::lit_protocol::download_objects_lit(&mut conn, &client_store, &needed)
                .unwrap();
        assert!(downloaded > 0);

        // Verify objects match
        for hash in &needed {
            assert!(
                client_store.exists(hash),
                "Object {} should exist in client B",
                hash.as_str()
            );
        }
    }

    daemon.thread().unpark();
}
