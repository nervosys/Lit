/// Integration tests for SSH transport (pipe mode)
///
/// These tests use `SshPipe::open_local()` which spawns `lit serve --stdio`
/// directly, bypassing actual SSH. This tests the full pipe protocol and
/// transport logic without requiring SSH infrastructure.
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid interference.
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;

/// Helper: init a repo and make a commit so there's content to push/pull
fn ssh_init_repo_with_commit(dir: &std::path::Path) -> String {
    let _cwd = super::test_helpers::CwdGuard::new(dir);

    lit::commands::init::execute(false, None).unwrap();

    fs::write(dir.join("hello.txt"), "Hello from SSH!\n").unwrap();
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    let commit_resp = lit::commands::commit::execute(
        "Initial commit".to_string(),
        Some("Test <test@test.com>".to_string()),
    )
    .unwrap();

    commit_resp.hash
}

/// Helper: init a bare-ish repo (just init, no commits)
fn ssh_init_bare_repo(dir: &std::path::Path) {
    let _cwd = super::test_helpers::CwdGuard::new(dir);
    lit::commands::init::execute(false, None).unwrap();
}

// ---------------------------------------------------------------------------
// SSH URL parsing
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_url_detection() {
    assert!(lit::network::ssh::is_ssh_url("ssh://example.com/repo"));
    assert!(lit::network::ssh::is_ssh_url("ssh://user@host:22/path"));
    assert!(lit::network::ssh::is_ssh_url("git@github.com:user/repo"));
    assert!(!lit::network::ssh::is_ssh_url("http://example.com/repo"));
    assert!(!lit::network::ssh::is_ssh_url("https://example.com/repo"));
    assert!(!lit::network::ssh::is_ssh_url("/local/path"));
    assert!(!lit::network::ssh::is_ssh_url("file:///local/path"));
}

#[test]
fn test_parse_ssh_url_standard() {
    let parsed = lit::network::ssh::parse_ssh_url("ssh://user@host.com:2222/path/to/repo").unwrap();
    assert_eq!(parsed.user, Some("user".to_string()));
    assert_eq!(parsed.host, "host.com");
    assert_eq!(parsed.port, Some(2222));
    assert_eq!(parsed.path, "/path/to/repo");
}

#[test]
fn test_parse_ssh_url_no_user() {
    let parsed = lit::network::ssh::parse_ssh_url("ssh://host.com/path/to/repo").unwrap();
    assert_eq!(parsed.user, None);
    assert_eq!(parsed.host, "host.com");
    assert_eq!(parsed.port, None);
    assert_eq!(parsed.path, "/path/to/repo");
}

#[test]
fn test_parse_ssh_url_scp_style() {
    let parsed = lit::network::ssh::parse_ssh_url("git@github.com:user/repo").unwrap();
    assert_eq!(parsed.user, Some("git".to_string()));
    assert_eq!(parsed.host, "github.com");
    assert_eq!(parsed.port, None);
    assert_eq!(parsed.path, "user/repo");
}

#[test]
fn test_parse_ssh_url_invalid() {
    assert!(lit::network::ssh::parse_ssh_url("http://example.com").is_err());
    assert!(lit::network::ssh::parse_ssh_url("ssh://host.com").is_err()); // no path
    assert!(lit::network::ssh::parse_ssh_url("/local/path").is_err());
}

// ---------------------------------------------------------------------------
// Pipe transport: List refs
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_pipe_list_refs_empty() {
    let server_dir = TempDir::new().unwrap();
    ssh_init_bare_repo(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    let refs = lit::network::ssh::list_refs_ssh(&mut pipe, "heads").unwrap();
    assert!(refs.is_empty(), "Bare repo should have no refs");
}

#[test]
fn test_ssh_pipe_list_refs_with_branches() {
    let server_dir = TempDir::new().unwrap();
    ssh_init_repo_with_commit(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    let refs = lit::network::ssh::list_refs_ssh(&mut pipe, "heads").unwrap();
    assert!(!refs.is_empty(), "Should have at least one branch");
    assert!(
        refs.iter().any(|r| r.name == "main"),
        "Should have main branch"
    );
}

#[test]
fn test_ssh_pipe_read_head() {
    let server_dir = TempDir::new().unwrap();
    ssh_init_repo_with_commit(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    let head = lit::network::ssh::read_head_ssh(&mut pipe).unwrap();
    assert!(
        head.contains("main"),
        "HEAD should reference main, got: {}",
        head
    );
}

#[test]
fn test_ssh_pipe_read_ref() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = ssh_init_repo_with_commit(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    let hash = lit::network::ssh::read_ref_ssh(&mut pipe, "main").unwrap();
    assert_eq!(hash, commit_hash, "Branch ref should match commit hash");
}

// ---------------------------------------------------------------------------
// Pipe transport: Negotiate and download objects
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_pipe_negotiate_and_download() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = ssh_init_repo_with_commit(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    // Negotiate: we want the commit, we have nothing
    let needed = lit::network::ssh::negotiate_ssh(&mut pipe, std::slice::from_ref(&commit_hash), &[]).unwrap();
    assert!(
        !needed.is_empty(),
        "Should need at least the commit + tree + blob"
    );

    // Download into a fresh repo
    let client_dir = TempDir::new().unwrap();
    ssh_init_bare_repo(client_dir.path());
    let client_store = lit::storage::ObjectStore::new(client_dir.path());

    let downloaded =
        lit::network::ssh::download_objects_ssh(&mut pipe, &client_store, &needed).unwrap();
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

// ---------------------------------------------------------------------------
// Pipe transport: Upload objects
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_pipe_upload_objects() {
    let server_dir = TempDir::new().unwrap();
    ssh_init_bare_repo(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    // Create a client with content
    let client_dir = TempDir::new().unwrap();
    let commit_hash = ssh_init_repo_with_commit(client_dir.path());
    let client_store = lit::storage::ObjectStore::new(client_dir.path());

    // Walk commit graph to collect all objects
    let needed = lit::network::transport::walk_commit_graph(
        &client_store,
        &lit::core::ObjectHash::from_hex(commit_hash.clone()),
        &HashSet::new(),
    )
    .unwrap();

    let uploaded =
        lit::network::ssh::upload_objects_ssh(&mut pipe, &client_store, &needed).unwrap();
    assert_eq!(uploaded, needed.len(), "Should upload all objects");

    // Update the branch ref on the server via pipe
    lit::network::ssh::update_ref_ssh(&mut pipe, "main", &commit_hash, false).unwrap();

    // Verify the server has the ref
    let server_hash = lit::network::ssh::read_ref_ssh(&mut pipe, "main").unwrap();
    assert_eq!(server_hash, commit_hash);
}

// ---------------------------------------------------------------------------
// Pipe transport: Multiple requests over same pipe (session persistence)
// ---------------------------------------------------------------------------

#[test]
fn test_ssh_pipe_multiple_requests() {
    let server_dir = TempDir::new().unwrap();
    let commit_hash = ssh_init_repo_with_commit(server_dir.path());

    let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

    // First request
    let refs = lit::network::ssh::list_refs_ssh(&mut pipe, "heads").unwrap();
    assert!(!refs.is_empty());

    // Second request on same pipe
    let head = lit::network::ssh::read_head_ssh(&mut pipe).unwrap();
    assert!(head.contains("main"));

    // Third request on same pipe
    let hash = lit::network::ssh::read_ref_ssh(&mut pipe, "main").unwrap();
    assert_eq!(hash, commit_hash);
}

// ---------------------------------------------------------------------------
// Full push-over-pipe workflow
// ---------------------------------------------------------------------------

#[test]
fn test_push_fetch_roundtrip_over_pipe() {
    // Server repo (empty)
    let server_dir = TempDir::new().unwrap();
    ssh_init_bare_repo(server_dir.path());

    // Client A: create repo with commit
    let client_a = TempDir::new().unwrap();
    let commit_hash = ssh_init_repo_with_commit(client_a.path());

    // Push from client A to server via pipe
    {
        let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();
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
            lit::network::ssh::upload_objects_ssh(&mut pipe, &client_store, &objects).unwrap();
        assert!(uploaded > 0);

        // Update ref
        lit::network::ssh::update_ref_ssh(&mut pipe, "main", &commit_hash, false).unwrap();
    }

    // Client B: fetch from server via pipe
    {
        let client_b = TempDir::new().unwrap();
        ssh_init_bare_repo(client_b.path());
        let client_store = lit::storage::ObjectStore::new(client_b.path());

        let mut pipe = lit::network::ssh::SshPipe::open_local(server_dir.path()).unwrap();

        // Get server's main ref
        let server_hash = lit::network::ssh::read_ref_ssh(&mut pipe, "main").unwrap();
        assert_eq!(server_hash, commit_hash);

        // Negotiate
        let needed =
            lit::network::ssh::negotiate_ssh(&mut pipe, std::slice::from_ref(&server_hash), &[]).unwrap();

        // Download
        let downloaded =
            lit::network::ssh::download_objects_ssh(&mut pipe, &client_store, &needed).unwrap();
        assert_eq!(downloaded, needed.len());

        // Verify the commit object exists
        assert!(client_store.exists(&lit::core::ObjectHash::from_hex(commit_hash.clone())));
    }
}
