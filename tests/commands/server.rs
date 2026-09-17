//! End-to-end tests for `lit server serve`.
//!
//! These drive a **real server over a real socket**: bind port 0, learn the
//! port, send actual HTTP, assert on the status codes a client would see. That
//! matters because every control in `docs/NIST_800-171.md` is a claim about what
//! happens to a request, and the unit tests only cover the pieces in isolation —
//! they cannot catch a route wired to the wrong check, or an authorization
//! decision that never runs.
//!
//! Each test gets its own port, its own account store and its own audit log, so
//! they are independent and need no `--test-threads=1`.
//!
//! One shared thing remains: `AuditLog` keeps its HMAC signing key at a fixed
//! path under `~/.lit`, created on first use. If these tests ever turn flaky in
//! a way that points at the audit log, that key is the first thing to suspect —
//! it is the only state they do not own.
//!
//! **What is deliberately not covered here:** the routes that read or write the
//! repository (`/status`, `/log`, `/commit`, …). Those reach `route_request`,
//! which resolves the repository from the process's current working directory
//! rather than from the `repo_root` it is handed — a pre-existing quirk
//! inherited from `lit serve`, noted in `docs/HANDOFF.md` §0. Exercising them
//! here would make these tests order-dependent for no gain, because the
//! security surface below sits entirely *in front* of that call: an
//! unauthorized request is refused before `route_request` is ever reached.

use lit::commands::server::{bind, BoundServer, ShutdownHandle};
use lit::server::auth::{LockoutPolicy, Role, UserStore};
use lit::server::session::SessionPolicy;
use lit::server::ServerOptions;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

const PW_ADMIN: &str = "a very long administrator password";
const PW_READER: &str = "a very long reader password";

/// A server running in a background thread, stopped when the test ends.
struct TestServer {
    base: String,
    shutdown: ShutdownHandle,
    thread: Option<std::thread::JoinHandle<()>>,
    users_path: PathBuf,
    /// Held so the temporary directory outlives the server.
    _dir: TempDir,
}

impl TestServer {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    /// A second `UserStore` over the same file, standing in for the CLI acting
    /// against a running server.
    fn cli(&self) -> UserStore {
        UserStore::open(&self.users_path, LockoutPolicy::default()).unwrap()
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.shutdown();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Start a server with one admin and one reader account.
fn start(customize: impl FnOnce(&mut ServerOptions)) -> TestServer {
    let dir = TempDir::new().unwrap();
    let users_path = dir.path().join("users.json");

    {
        let mut store = UserStore::open(&users_path, LockoutPolicy::default()).unwrap();
        store.add_user("admin1", PW_ADMIN, Role::Admin).unwrap();
        store.add_user("read1", PW_READER, Role::Reader).unwrap();
    }

    let mut options = ServerOptions::local(0).unwrap();
    // Port 0 lets the OS pick a free port, so tests never collide.
    options.bind = "127.0.0.1:0".to_string();
    options.users_path = users_path.clone();
    // Keep each test's audit trail to itself, and out of ~/.lit.
    options.audit_path = Some(dir.path().join("audit.log").to_string_lossy().into_owned());
    customize(&mut options);

    let bound: BoundServer = bind(options, dir.path().to_path_buf()).expect("server should bind");
    let addr = bound.local_addr().expect("an IP port");
    let shutdown = bound.shutdown_handle();
    let thread = std::thread::spawn(move || {
        let _ = bound.run();
    });

    TestServer {
        base: format!("http://{}", addr),
        shutdown,
        thread: Some(thread),
        users_path,
        _dir: dir,
    }
}

/// Status code of a GET, with an optional bearer token.
fn get(url: &str, token: Option<&str>) -> u16 {
    let mut req = ureq::get(url);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {}", t));
    }
    match req.call() {
        Ok(r) => r.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(e) => panic!("transport error for {}: {}", url, e),
    }
}

/// Status code and body of a POST.
fn post(url: &str, token: Option<&str>, body: serde_json::Value) -> (u16, String) {
    let mut req = ureq::post(url);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {}", t));
    }
    match req.send_json(body) {
        Ok(r) => (r.status(), r.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(code, r)) => (code, r.into_string().unwrap_or_default()),
        Err(e) => panic!("transport error for {}: {}", url, e),
    }
}

fn delete(url: &str, token: Option<&str>) -> u16 {
    let mut req = ureq::delete(url);
    if let Some(t) = token {
        req = req.set("Authorization", &format!("Bearer {}", t));
    }
    match req.call() {
        Ok(r) => r.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(e) => panic!("transport error for {}: {}", url, e),
    }
}

/// Log in and return the bearer token.
fn login(server: &TestServer, username: &str, password: &str) -> String {
    let (status, body) = post(
        &server.url("/api/v1/auth/login"),
        None,
        serde_json::json!({"username": username, "password": password}),
    );
    assert_eq!(status, 200, "login should succeed, body: {}", body);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
    parsed["token"].as_str().expect("a token").to_string()
}

// ---------------------------------------------------------------------------
// 03.01.09 — system use notification
// ---------------------------------------------------------------------------

#[test]
fn the_banner_is_served_before_authentication() {
    let server = start(|_| {});
    let response = ureq::get(&server.url("/api/v1/banner"))
        .call()
        .expect("the banner must not require a session");
    assert_eq!(response.status(), 200);
    let body = response.into_string().unwrap();
    assert!(body.contains("03.01.09"));
    assert!(
        body.contains("\"customized\":false"),
        "the shipped default must announce itself as a placeholder: {}",
        body
    );
}

#[test]
fn an_operator_banner_replaces_the_placeholder() {
    let dir = TempDir::new().unwrap();
    let banner = dir.path().join("banner.txt");
    std::fs::write(&banner, "AUTHORIZED USE ONLY - Contoso Federal").unwrap();

    let server = start(|o| o.banner_path = Some(banner.clone()));
    let body = ureq::get(&server.url("/api/v1/banner"))
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    assert!(body.contains("Contoso Federal"));
    assert!(body.contains("\"customized\":true"));
}

// ---------------------------------------------------------------------------
// 03.05.01 — identification and authentication
// ---------------------------------------------------------------------------

#[test]
fn an_unauthenticated_request_is_refused() {
    let server = start(|_| {});
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), None), 401);
}

#[test]
fn a_bad_password_is_refused_and_a_good_one_yields_a_session() {
    let server = start(|_| {});
    let (status, _) = post(
        &server.url("/api/v1/auth/login"),
        None,
        serde_json::json!({"username": "read1", "password": "wrong"}),
    );
    assert_eq!(status, 401);

    let token = login(&server, "read1", PW_READER);
    assert_eq!(token.len(), 64, "a 256-bit token, hex encoded");
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);
}

#[test]
fn a_forged_token_is_refused() {
    let server = start(|_| {});
    let forged = "0".repeat(64);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&forged)), 401);
}

#[test]
fn whoami_reports_the_account_and_its_permissions() {
    let server = start(|_| {});
    let token = login(&server, "read1", PW_READER);
    let body = ureq::get(&server.url("/api/v1/auth/whoami"))
        .set("Authorization", &format!("Bearer {}", token))
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    assert!(body.contains("\"username\":\"read1\""));
    assert!(body.contains("\"role\":\"reader\""));
    assert!(body.contains("\"read\""));
    assert!(!body.contains("\"write\""));
}

// ---------------------------------------------------------------------------
// 03.01.02 / 03.01.05 / 03.01.07 — access enforcement and least privilege
// ---------------------------------------------------------------------------

#[test]
fn a_reader_cannot_write_and_cannot_administer() {
    let server = start(|_| {});
    let token = login(&server, "read1", PW_READER);

    // Refused on privilege, before the request ever reaches the repository.
    let (status, _) = post(
        &server.url("/api/v1/commit"),
        Some(&token),
        serde_json::json!({"message": "nope"}),
    );
    assert_eq!(status, 403, "a reader must not be able to commit");

    assert_eq!(get(&server.url("/api/v1/admin/users"), Some(&token)), 403);
}

#[test]
fn an_admin_can_administer() {
    let server = start(|_| {});
    let token = login(&server, "admin1", PW_ADMIN);
    assert_eq!(get(&server.url("/api/v1/admin/users"), Some(&token)), 200);
    assert_eq!(get(&server.url("/api/v1/admin/sessions"), Some(&token)), 200);
}

#[test]
fn an_unlisted_route_fails_closed_even_for_a_reader() {
    let server = start(|_| {});
    let token = login(&server, "read1", PW_READER);
    // Not in the route table, so it requires Admin and a reader is refused.
    assert_eq!(get(&server.url("/api/v1/does-not-exist"), Some(&token)), 403);
}

// ---------------------------------------------------------------------------
// 03.01.11 — session termination
// ---------------------------------------------------------------------------

#[test]
fn logging_out_invalidates_the_token() {
    let server = start(|_| {});
    let token = login(&server, "read1", PW_READER);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);

    let (status, _) = post(
        &server.url("/api/v1/auth/logout"),
        Some(&token),
        serde_json::json!({}),
    );
    assert_eq!(status, 200);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 401);
}

#[test]
fn an_idle_session_is_refused_once_the_timeout_has_passed() {
    let server = start(|o| {
        o.session_policy = SessionPolicy {
            idle_timeout: Duration::from_millis(150),
            max_lifetime: Duration::from_secs(3600),
            ..SessionPolicy::default()
        };
    });
    let token = login(&server, "read1", PW_READER);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);

    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        get(&server.url("/api/v1/auth/whoami"), Some(&token)),
        401,
        "an idle session must be refused, not merely swept later"
    );
}

// ---------------------------------------------------------------------------
// 03.05.08 — unsuccessful logon attempts
// ---------------------------------------------------------------------------

#[test]
fn repeated_failures_lock_the_account_and_the_right_password_does_not_help() {
    let server = start(|o| {
        o.lockout_policy = LockoutPolicy {
            max_attempts: 3,
            window_secs: 900,
            lockout_secs: 900,
        };
    });

    for _ in 0..2 {
        let (status, _) = post(
            &server.url("/api/v1/auth/login"),
            None,
            serde_json::json!({"username": "read1", "password": "wrong"}),
        );
        assert_eq!(status, 401);
    }

    let (status, _) = post(
        &server.url("/api/v1/auth/login"),
        None,
        serde_json::json!({"username": "read1", "password": "wrong"}),
    );
    assert_eq!(status, 423, "the threshold attempt should report a lock");

    let (status, _) = post(
        &server.url("/api/v1/auth/login"),
        None,
        serde_json::json!({"username": "read1", "password": PW_READER}),
    );
    assert_eq!(status, 423, "a lock must not be bypassable with the real password");
}

// ---------------------------------------------------------------------------
// 03.01.01 / 03.09.02 — account management and personnel termination
//
// This is the section that matters most: it is what an assessor tests, and the
// behaviour it pins down was broken twice during development.
// ---------------------------------------------------------------------------

#[test]
fn an_account_created_by_the_cli_can_log_in_without_a_restart() {
    let server = start(|_| {});
    server
        .cli()
        .add_user("late", PW_READER, Role::Reader)
        .unwrap();

    let token = login(&server, "late", PW_READER);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);
}

#[test]
fn disabling_an_account_from_the_cli_kills_the_session_it_already_holds() {
    let server = start(|_| {});
    let token = login(&server, "read1", PW_READER);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);

    server.cli().set_disabled("read1", true).unwrap();

    assert_eq!(
        get(&server.url("/api/v1/auth/whoami"), Some(&token)),
        403,
        "a revoked account must lose its open session on the next request, \
         not when the session happens to expire"
    );
    // And it cannot simply log in again.
    let (status, _) = post(
        &server.url("/api/v1/auth/login"),
        None,
        serde_json::json!({"username": "read1", "password": PW_READER}),
    );
    assert_eq!(status, 403);
}

#[test]
fn removing_an_account_from_the_cli_kills_the_session_it_already_holds() {
    let server = start(|_| {});
    let token = login(&server, "read1", PW_READER);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);

    server.cli().remove_user("read1").unwrap();

    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 403);
}

#[test]
fn demoting_an_account_downgrades_the_session_it_already_holds() {
    // Demotion is checked on the admin surface rather than on a maintainer
    // route, because every maintainer route reaches the repository and so
    // depends on the process's working directory. `/admin/users` needs `admin`
    // and touches nothing but the account store, which makes it the honest
    // place to observe a privilege change taking effect.
    let server = start(|_| {});
    server
        .cli()
        .add_user("second", PW_ADMIN, Role::Admin)
        .unwrap();

    let token = login(&server, "second", PW_ADMIN);
    assert_eq!(
        get(&server.url("/api/v1/admin/users"), Some(&token)),
        200,
        "an admin should reach the admin surface"
    );

    // Demote from the CLI while the session is open. `admin1` is still an
    // admin, so this is not the last-admin case.
    server.cli().set_role("second", Role::Reader).unwrap();

    assert_eq!(
        get(&server.url("/api/v1/admin/users"), Some(&token)),
        403,
        "the open session must drop to the account's current role, \
         not keep the one it was issued with"
    );
}

#[test]
fn an_admin_can_create_and_delete_accounts_over_the_api() {
    let server = start(|_| {});
    let admin = login(&server, "admin1", PW_ADMIN);

    let (status, _) = post(
        &server.url("/api/v1/admin/users"),
        Some(&admin),
        serde_json::json!({"username": "api1", "password": PW_READER, "role": "contributor"}),
    );
    assert_eq!(status, 201);

    // The new account works immediately.
    let token = login(&server, "api1", PW_READER);
    assert_eq!(get(&server.url("/api/v1/auth/whoami"), Some(&token)), 200);

    // Deleting it terminates its session.
    assert_eq!(
        delete(&server.url("/api/v1/admin/users/api1"), Some(&admin)),
        200
    );

    // 401 here, where the CLI tests above expect 403, and the difference is
    // real rather than incidental. Revocation reaches an open session by two
    // different routes:
    //
    //   - over the API, the delete runs inside the server, so it removes the
    //     session outright. The token is then simply unknown: 401, "authenticate
    //     again", which is the honest answer.
    //   - from the CLI, the server cannot touch its own session table, so the
    //     session survives until the per-request re-check notices the account
    //     has gone and refuses it: 403.
    //
    // Both end with the caller locked out on the next request. Asserting the
    // exact code on each keeps the two mechanisms from quietly collapsing into
    // one, which is how the second of them would get lost.
    assert_eq!(
        get(&server.url("/api/v1/auth/whoami"), Some(&token)),
        401,
        "an API delete terminates the session, so the token is unknown"
    );
}

#[test]
fn a_weak_password_is_refused_over_the_api() {
    let server = start(|_| {});
    let admin = login(&server, "admin1", PW_ADMIN);
    let (status, body) = post(
        &server.url("/api/v1/admin/users"),
        Some(&admin),
        serde_json::json!({"username": "weak", "password": "short", "role": "reader"}),
    );
    assert_eq!(status, 400);
    assert!(body.contains("15 characters"), "body: {}", body);
}

// ---------------------------------------------------------------------------
// 03.03.01 / 03.03.02 — audit
// ---------------------------------------------------------------------------

#[test]
fn every_decision_reaches_the_audit_log_attributed() {
    let dir = TempDir::new().unwrap();
    let users_path = dir.path().join("users.json");
    let audit_path = dir.path().join("audit.log");
    {
        let mut store = UserStore::open(&users_path, LockoutPolicy::default()).unwrap();
        store.add_user("admin1", PW_ADMIN, Role::Admin).unwrap();
        store.add_user("read1", PW_READER, Role::Reader).unwrap();
    }

    let mut options = ServerOptions::local(0).unwrap();
    options.bind = "127.0.0.1:0".to_string();
    options.users_path = users_path;
    options.audit_path = Some(audit_path.to_string_lossy().into_owned());

    let bound = bind(options, dir.path().to_path_buf()).unwrap();
    let addr = bound.local_addr().unwrap();
    let shutdown = bound.shutdown_handle();
    let thread = std::thread::spawn(move || {
        let _ = bound.run();
    });
    let base = format!("http://{}", addr);

    // One failure, one success, one refusal on privilege.
    let _ = post(
        &format!("{}/api/v1/auth/login", base),
        None,
        serde_json::json!({"username": "read1", "password": "wrong"}),
    );
    let (_, body) = post(
        &format!("{}/api/v1/auth/login", base),
        None,
        serde_json::json!({"username": "read1", "password": PW_READER}),
    );
    let token = serde_json::from_str::<serde_json::Value>(&body).unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let _ = get(&format!("{}/api/v1/admin/users", base), Some(&token));

    shutdown.shutdown();
    let _ = thread.join();

    let log = std::fs::read_to_string(&audit_path).expect("an audit log");

    assert!(log.contains("SERVER_START"));
    assert!(log.contains("AUTH_FAILURE"), "a failed login must be recorded");
    assert!(log.contains("AUTH_SUCCESS"));
    assert!(
        log.contains("ACCESS_DENIED"),
        "a refusal on privilege must be recorded"
    );
    assert!(log.contains("SERVER_STOP"));

    // 03.03.02: the record names who, from where, and what — not just what.
    assert!(
        log.contains("\"subject\":\"read1\""),
        "records must be attributed to an account, log:\n{}",
        log
    );
    assert!(log.contains("\"source\":\"127.0.0.1\""));
    assert!(log.contains("/api/v1/admin/users"));

    // Every line carries its HMAC, which is what makes tampering detectable.
    for line in log.lines().filter(|l| !l.trim().is_empty()) {
        let fields: Vec<&str> = line.split(" | ").collect();
        assert_eq!(fields.len(), 4, "malformed audit line: {}", line);
        assert_eq!(fields[3].len(), 64, "expected a hex HMAC: {}", line);
    }
}

// ---------------------------------------------------------------------------
// 03.13.08 — transmission confidentiality
// ---------------------------------------------------------------------------

#[test]
fn binding_a_routable_address_without_tls_is_refused() {
    let dir = TempDir::new().unwrap();
    let users_path = dir.path().join("users.json");
    {
        let mut store = UserStore::open(&users_path, LockoutPolicy::default()).unwrap();
        store.add_user("admin1", PW_ADMIN, Role::Admin).unwrap();
    }

    let mut options = ServerOptions::local(0).unwrap();
    options.bind = "0.0.0.0:0".to_string();
    options.users_path = users_path.clone();
    options.audit_enabled = false;

    // The refusal happens at bind, so nothing ever listens.
    //
    // `Display` on `LitError` is deliberately sanitized — it says only
    // "Configuration error" — so the detail has to come from
    // `internal_message`, which is the half that never reaches a client.
    let err = match bind(options.clone(), dir.path().to_path_buf()) {
        Ok(_) => panic!("plaintext on a routable address should be refused"),
        Err(e) => e.internal_message().to_string(),
    };
    assert!(err.contains("03.13.08"), "error should cite the control: {}", err);

    // And the override is honoured when the operator asks for it explicitly.
    let mut allowed = options;
    allowed.allow_plaintext = true;
    assert!(bind(allowed, dir.path().to_path_buf()).is_ok());
}

#[test]
fn a_server_with_no_accounts_refuses_to_bind() {
    let dir = TempDir::new().unwrap();
    let mut options = ServerOptions::local(0).unwrap();
    options.bind = "127.0.0.1:0".to_string();
    options.users_path = dir.path().join("users.json");
    options.audit_enabled = false;

    match bind(options, dir.path().to_path_buf()) {
        Ok(_) => panic!("a server with no accounts should refuse to start"),
        Err(e) => {
            let msg = e.internal_message();
            assert!(msg.contains("No accounts exist"), "{}", msg);
            assert!(
                msg.contains("lit server user add"),
                "the refusal should say how to fix it: {}",
                msg
            );
        }
    }
}
