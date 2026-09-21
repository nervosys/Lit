//! The hardened, self-hostable server and its administration commands.
//!
//! `lit serve` remains what it was: a loopback development server with an
//! optional shared token. `lit server serve` is the one intended to be hosted —
//! it authenticates named accounts, authorizes by role, expires sessions, can
//! terminate TLS, and writes an audit record for every decision it makes.
//!
//! The two are separate commands rather than flags on one because they have
//! different defaults and different failure modes, and conflating them is how a
//! development server ends up on a routable address.
//!
//! See `docs/NIST_800-171.md` for the control mapping and `docs/SELF_HOSTING.md`
//! for deployment.

use crate::commands::serve::{json_content_type, read_body, route_request, RateLimiter};
use crate::core::find_repo_root;
use crate::errors::LitError;
use crate::response::{ServeResponse, ServerAdminResponse};
use crate::server::audit::{AuditRecord, ServerEvent};
use crate::server::auth::{AuthOutcome, LockoutPolicy, Role, UserStore};
use crate::server::session::SessionError;
use crate::server::tls::TlsMaterial;
use crate::server::{
    default_server_dir, is_public_route, required_role, Caller, ServerContext, ServerOptions,
};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tiny_http::{Method, Response, Server, StatusCode};

/// Reply helper: a JSON body with a status code.
fn json_response(status: u16, body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(json_content_type())
}

/// Longest username accepted at the login route.
///
/// Anything longer cannot be an account, because `UserStore::add_user` would
/// never have created it — so hashing it is wasted work, and recording it is
/// worse than wasted. See [`audit_subject`].
const MAX_LOGIN_USERNAME: usize = 64;

/// What to record as the subject of a failed login.
///
/// The attempted username belongs in the record: `03.03.02` is about knowing
/// who tried, and a failed-login trail without names is not worth keeping. But
/// this string is caller-supplied, and the commonest way for it to be wrong is
/// a password typed into the username field — which would then sit in a log
/// that is retained, forwarded to a SIEM, and read by administrators.
///
/// Anything not shaped like an account name cannot be one, so it is recorded as
/// a marker rather than verbatim. That covers the mistyped-password case for
/// any password containing a space or a symbol outside the account charset, and
/// bounds what an anonymous caller can write into the log.
///
/// Line-format injection is separately impossible: records are serialized with
/// `serde_json`, which escapes newlines and quotes, so a crafted username
/// cannot forge a second `timestamp | event | json | hmac` line.
fn audit_subject(username: &str) -> String {
    let shaped = !username.is_empty()
        && username.len() <= MAX_LOGIN_USERNAME
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'));
    if shaped {
        username.to_string()
    } else {
        "<malformed-username>".to_string()
    }
}

/// A structured error body. Messages here are deliberately terse: they go to
/// unauthenticated callers, so they say what to do without saying why it failed.
fn error_body(message: &str) -> String {
    serde_json::json!({
        "status": "error",
        "error": { "message": message }
    })
    .to_string()
}

/// Stops a running server. Cloneable, and safe to call from another thread.
///
/// The loop ends after the request it is currently serving, so a shutdown is
/// not a connection reset for whoever is mid-request.
#[derive(Clone)]
pub struct ShutdownHandle(Arc<Server>);

impl ShutdownHandle {
    pub fn shutdown(&self) {
        self.0.unblock();
    }
}

/// A server that has bound its port but is not yet serving.
///
/// Binding and running are separate because the bind is the part that fails —
/// a taken port, unreadable TLS material, an empty account store — and a caller
/// needs to hear about that before anything claims to be listening. It is also
/// what makes the server testable: bind port 0, ask which port you got, drive
/// real requests, then stop it.
pub struct BoundServer {
    server: Arc<Server>,
    context: ServerContext,
    repo_root: PathBuf,
}

impl BoundServer {
    /// The address actually bound, which is how a caller learns the port when
    /// it asked for 0.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.server.server_addr().to_ip()
    }

    /// A handle that stops [`BoundServer::run`].
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle(Arc::clone(&self.server))
    }

    /// Tell the operator what was started, and what about it is weak.
    pub fn announce(&self) {
        let context = &self.context;
        let scheme = if context.options.tls.is_some() {
            "https"
        } else {
            "http"
        };
        eprintln!(
            "Lit server listening on {}://{}",
            scheme, context.options.bind
        );
        eprintln!("Repository: {}", self.repo_root.display());
        eprintln!(
            "Accounts:   {} ({} configured)",
            context.options.users_path.display(),
            context.users.lock().map(|u| u.len()).unwrap_or(0)
        );
        if context.options.tls.is_none() {
            eprintln!("WARNING: TLS is off. Credentials cross the wire in the clear.");
        }
        if !context.banner.customized {
            eprintln!(
                "WARNING: serving the default placeholder system use notification. \
                 Set server.banner_path (NIST SP 800-171r3 03.01.09)."
            );
        }
        if !context.recorder.is_enabled() {
            eprintln!("WARNING: audit logging is disabled (NIST SP 800-171r3 03.03.01).");
        }
        eprintln!("Press Ctrl+C to stop");
    }
}

/// Start the hardened server, serving until it is stopped.
pub fn execute_serve(options: ServerOptions) -> Result<ServeResponse, LitError> {
    let repo_root = find_repo_root()?;
    let bound = bind(options, repo_root)?;
    bound.announce();
    bound.run()
}

/// Bind the server's port and build its state, without serving anything yet.
pub fn bind(options: ServerOptions, repo_root: PathBuf) -> Result<BoundServer, LitError> {
    let context = ServerContext::new(options).map_err(LitError::Config)?;

    // Bind before announcing anything, so a port conflict is not reported as a
    // running server.
    let server = match &context.options.tls {
        Some(paths) => {
            let material = TlsMaterial::load(paths).map_err(LitError::Config)?;
            Server::https(
                &context.options.bind,
                tiny_http::SslConfig {
                    certificate: material.certificate,
                    private_key: material.private_key,
                },
            )
            .map_err(|e| {
                LitError::Network(format!(
                    "Failed to start HTTPS server on {}: {}",
                    context.options.bind, e
                ))
            })?
        }
        None => Server::http(&context.options.bind).map_err(|e| {
            LitError::Network(format!(
                "Failed to start server on {}: {}",
                context.options.bind, e
            ))
        })?,
    };

    Ok(BoundServer {
        server: Arc::new(server),
        context,
        repo_root,
    })
}

impl BoundServer {
    /// Serve until [`ShutdownHandle::shutdown`] is called.
    pub fn run(self) -> Result<ServeResponse, LitError> {
        // Destructured so the request loop below reads against plain locals.
        let BoundServer {
            server,
            context,
            repo_root,
        } = self;

        context.record_startup();
        let mut rate_limiter = RateLimiter::new();

        for mut request in server.incoming_requests() {
            let remote: Option<IpAddr> = request.remote_addr().map(|a| a.ip());
            let method = request.method().clone();
            let url = request.url().to_string();
            let path = url.split('?').next().unwrap_or(&url).to_string();

            // 1. Rate limiting, before any work is done on the caller's behalf.
            if let Some(ip) = remote {
                if !rate_limiter.check(ip) {
                    context.recorder.record(
                        ServerEvent::RateLimited,
                        AuditRecord::failure("rate_limit_exceeded")
                            .source(remote)
                            .object(path.clone()),
                    );
                    let _ = request.respond(json_response(429, error_body("Rate limit exceeded")));
                    continue;
                }
            }

            // 2. Body, subject to the size cap.
            let body = match read_body(&mut request) {
                Ok(b) => b,
                Err(e) => {
                    context.recorder.record(
                        ServerEvent::RequestRejected,
                        AuditRecord::failure(e.to_string())
                            .source(remote)
                            .object(path.clone()),
                    );
                    let _ = request.respond(json_response(413, error_body("Request rejected")));
                    continue;
                }
            };

            // 3. Public routes: the banner, and login itself.
            if is_public_route(&method, &path) {
                let response = if method == Method::Get {
                    json_response(200, context.banner.to_json())
                } else {
                    handle_login(&context, &body, remote)
                };
                let _ = request.respond(response);
                continue;
            }

            // 4. Everything else needs a live session.
            let token = match bearer_token(&request) {
                Some(t) => t,
                None => {
                    context.recorder.record(
                        ServerEvent::SessionInvalid,
                        AuditRecord::failure("missing_bearer_token")
                            .source(remote)
                            .object(path.clone()),
                    );
                    let _ =
                        request.respond(json_response(401, error_body("Authentication required")));
                    continue;
                }
            };

            let caller = {
                let mut sessions = match context.sessions.lock() {
                    Ok(s) => s,
                    Err(poisoned) => poisoned.into_inner(),
                };
                match sessions.validate(&token) {
                    Ok(session) => Caller {
                        username: session.username,
                        role: session.role,
                    },
                    Err(e) => {
                        drop(sessions);
                        context.recorder.record(
                            ServerEvent::SessionInvalid,
                            AuditRecord::failure(e.as_str())
                                .source(remote)
                                .object(path.clone()),
                        );
                        let message = match e {
                            SessionError::IdleTimeout | SessionError::LifetimeExceeded => {
                                "Session expired; authenticate again"
                            }
                            SessionError::Unknown => "Authentication required",
                        };
                        let _ = request.respond(json_response(401, error_body(message)));
                        continue;
                    }
                }
            };

            // 4b. Re-check the account behind the session, every request.
            //
            // A session carries the role it was issued with. Without consulting the
            // store again, an account disabled, demoted, or removed from the CLI
            // kept its open session working until that session happened to expire —
            // up to the absolute lifetime, eight hours on the defaults. Revocation
            // has to bite now, not eventually, so the current role wins over the
            // one the session was minted with.
            let caller = {
                let state = {
                    let mut users = match context.users.lock() {
                        Ok(u) => u,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    users.account_state(&caller.username)
                };
                match state {
                    Some((role, true)) => Caller {
                        username: caller.username,
                        role,
                    },
                    other => {
                        let reason = if other.is_none() {
                            "account_removed"
                        } else {
                            "account_disabled"
                        };
                        {
                            let mut sessions = match context.sessions.lock() {
                                Ok(s) => s,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            sessions.terminate_user(&caller.username);
                        }
                        context.recorder.record(
                            ServerEvent::AccessDenied,
                            AuditRecord::failure(reason)
                                .subject(caller.username.clone())
                                .source(remote)
                                .object(format!("{:?} {}", method, path)),
                        );
                        let _ = request.respond(json_response(
                            403,
                            error_body("Account is no longer authorized"),
                        ));
                        continue;
                    }
                }
            };

            // 5. Authorization, decided in one place for every route.
            let needed = required_role(&method, &path);
            if caller.role < needed {
                context.recorder.record(
                    ServerEvent::AccessDenied,
                    AuditRecord::failure(format!("requires_{}", needed))
                        .subject(caller.username.clone())
                        .source(remote)
                        .object(format!("{:?} {}", method, path)),
                );
                let _ = request.respond(json_response(403, error_body("Insufficient privilege")));
                continue;
            }

            // 6. Session-scoped and administrative routes, then the repository API.
            let response = if path == "/api/v1/auth/logout" {
                handle_logout(&context, &token, &caller, remote)
            } else if path == "/api/v1/auth/whoami" {
                json_response(200, whoami_body(&caller))
            } else if path.starts_with("/api/v1/admin/") {
                handle_admin(&context, &method, &path, &body, &caller, remote)
            } else {
                match route_request(method.clone(), &url, &body, &repo_root) {
                    Ok((status, body)) => {
                        context.recorder.record(
                            ServerEvent::ApiRequest,
                            AuditRecord::success()
                                .subject(caller.username.clone())
                                .source(remote)
                                .object(format!("{:?} {} -> {}", method, path, status)),
                        );
                        json_response(status, body)
                    }
                    Err(e) => {
                        // The internal message stays server-side; the caller gets
                        // the sanitized one. Both reach the audit log, because an
                        // assessor reading it needs the detail.
                        eprintln!("API error: {}", e.internal_message());
                        context.recorder.record(
                            ServerEvent::ApiRequest,
                            AuditRecord::failure(e.internal_message())
                                .subject(caller.username.clone())
                                .source(remote)
                                .object(format!("{:?} {}", method, path)),
                        );
                        json_response(500, error_body(e.user_message()))
                    }
                }
            };
            let _ = request.respond(response);
        }

        context
            .recorder
            .record(ServerEvent::ServerStop, AuditRecord::success());
        Ok(ServeResponse {
            message: "Server stopped".to_string(),
        })
    }
}

/// Extract a bearer token from the Authorization header.
fn bearer_token(request: &tiny_http::Request) -> Option<String> {
    request
        .headers()
        .iter()
        .find(|h| {
            h.field
                .as_str()
                .as_str()
                .eq_ignore_ascii_case("authorization")
        })
        .and_then(|h| h.value.as_str().strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// `POST /api/v1/auth/login`
fn handle_login(
    context: &ServerContext,
    body: &str,
    remote: Option<IpAddr>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let payload: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return json_response(400, error_body("Invalid JSON")),
    };
    let username = payload
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let password = payload
        .get("password")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    if username.is_empty() || password.is_empty() {
        return json_response(
            400,
            error_body("Both 'username' and 'password' are required"),
        );
    }

    // Reject oversized credentials before doing any work on them. Neither can
    // belong to a real account, and without this an anonymous caller could
    // write close to the 1 MB body cap into the audit log on every failed
    // attempt — filling the disk that the audit trail depends on, one rejected
    // login at a time.
    if username.len() > MAX_LOGIN_USERNAME || password.len() > crate::server::auth::MAX_PASSWORD_LEN
    {
        context.recorder.record(
            ServerEvent::AuthFailure,
            AuditRecord::failure("oversized_credential")
                .source(remote)
                .object("/api/v1/auth/login"),
        );
        return json_response(400, error_body("Invalid credentials"));
    }

    let outcome = {
        let mut users = match context.users.lock() {
            Ok(u) => u,
            Err(poisoned) => poisoned.into_inner(),
        };
        users.authenticate(&username, &password)
    };

    match outcome {
        AuthOutcome::Success { username, role } => {
            let token = {
                let mut sessions = match context.sessions.lock() {
                    Ok(s) => s,
                    Err(poisoned) => poisoned.into_inner(),
                };
                sessions.create(&username, role, remote)
            };
            context.recorder.record(
                ServerEvent::AuthSuccess,
                AuditRecord::success()
                    .subject(username.clone())
                    .source(remote)
                    .object("/api/v1/auth/login"),
            );
            let idle = context.options.session_policy.idle_timeout.as_secs();
            let lifetime = context.options.session_policy.max_lifetime.as_secs();
            json_response(
                200,
                serde_json::json!({
                    "status": "ok",
                    "token": token,
                    "username": username,
                    "role": role.as_str(),
                    "idle_timeout_secs": idle,
                    "max_lifetime_secs": lifetime,
                })
                .to_string(),
            )
        }
        AuthOutcome::Locked { until } => {
            context.recorder.record(
                ServerEvent::AuthLockout,
                AuditRecord::failure(format!("locked_until={}", until))
                    .subject(audit_subject(&username))
                    .source(remote)
                    .object("/api/v1/auth/login"),
            );
            // The caller is told the account is locked: withholding it just
            // produces support tickets, and an attacker who triggered the
            // lockout already knows.
            json_response(423, error_body("Account locked; contact an administrator"))
        }
        AuthOutcome::Disabled => {
            context.recorder.record(
                ServerEvent::AuthFailure,
                AuditRecord::failure("account_disabled")
                    .subject(audit_subject(&username))
                    .source(remote)
                    .object("/api/v1/auth/login"),
            );
            json_response(403, error_body("Account disabled"))
        }
        AuthOutcome::InvalidCredentials => {
            context.recorder.record(
                ServerEvent::AuthFailure,
                AuditRecord::failure("invalid_credentials")
                    .subject(audit_subject(&username))
                    .source(remote)
                    .object("/api/v1/auth/login"),
            );
            json_response(401, error_body("Invalid credentials"))
        }
    }
}

/// `POST /api/v1/auth/logout`
fn handle_logout(
    context: &ServerContext,
    token: &str,
    caller: &Caller,
    remote: Option<IpAddr>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    {
        let mut sessions = match context.sessions.lock() {
            Ok(s) => s,
            Err(poisoned) => poisoned.into_inner(),
        };
        sessions.terminate(token);
    }
    context.recorder.record(
        ServerEvent::SessionEnd,
        AuditRecord::success()
            .subject(caller.username.clone())
            .source(remote)
            .object("/api/v1/auth/logout"),
    );
    json_response(
        200,
        r#"{"status":"ok","message":"Session terminated"}"#.to_string(),
    )
}

/// `GET /api/v1/auth/whoami`
fn whoami_body(caller: &Caller) -> String {
    use crate::server::auth::Permission;
    let mut permissions = Vec::new();
    for (name, permission) in [
        ("read", Permission::Read),
        ("write", Permission::Write),
        ("administer", Permission::Administer),
    ] {
        if caller.role.grants(permission) {
            permissions.push(name);
        }
    }
    serde_json::json!({
        "status": "ok",
        "username": caller.username,
        "role": caller.role.as_str(),
        "permissions": permissions,
    })
    .to_string()
}

/// Administrative routes under `/api/v1/admin/`. Reaching here means the
/// caller already holds `Admin`.
fn handle_admin(
    context: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    caller: &Caller,
    remote: Option<IpAddr>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let audit = |outcome: Result<&str, String>, object: String| match outcome {
        Ok(_) => context.recorder.record(
            ServerEvent::AccountChange,
            AuditRecord::success()
                .subject(caller.username.clone())
                .source(remote)
                .object(object),
        ),
        Err(reason) => context.recorder.record(
            ServerEvent::AccountChange,
            AuditRecord::failure(reason)
                .subject(caller.username.clone())
                .source(remote)
                .object(object),
        ),
    };

    match (method, path) {
        (Method::Get, "/api/v1/admin/users") => {
            let mut users = match context.users.lock() {
                Ok(u) => u,
                Err(poisoned) => poisoned.into_inner(),
            };
            let list = serde_json::to_value(users.list()).unwrap_or(serde_json::Value::Null);
            json_response(
                200,
                serde_json::json!({"status": "ok", "users": list}).to_string(),
            )
        }

        (Method::Get, "/api/v1/admin/sessions") => {
            let sessions = match context.sessions.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            let list = serde_json::to_value(sessions.list()).unwrap_or(serde_json::Value::Null);
            json_response(
                200,
                serde_json::json!({"status": "ok", "sessions": list}).to_string(),
            )
        }

        (Method::Post, "/api/v1/admin/users") => {
            let payload: serde_json::Value = match serde_json::from_str(body) {
                Ok(v) => v,
                Err(_) => return json_response(400, error_body("Invalid JSON")),
            };
            let username = payload
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let password = payload
                .get("password")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let role = payload
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("reader");
            let role = match Role::parse(role) {
                Ok(r) => r,
                Err(e) => return json_response(400, error_body(&e)),
            };
            let result = {
                let mut users = match context.users.lock() {
                    Ok(u) => u,
                    Err(poisoned) => poisoned.into_inner(),
                };
                users.add_user(username, password, role)
            };
            match result {
                Ok(()) => {
                    audit(
                        Ok("created"),
                        format!("create user={} role={}", username, role),
                    );
                    json_response(
                        201,
                        serde_json::json!({"status":"ok","message":"Account created"}).to_string(),
                    )
                }
                Err(e) => {
                    audit(Err(e.clone()), format!("create user={}", username));
                    json_response(400, error_body(&e))
                }
            }
        }

        (Method::Delete, p) if p.starts_with("/api/v1/admin/users/") => {
            let username = p.trim_start_matches("/api/v1/admin/users/").to_string();
            let result = {
                let mut users = match context.users.lock() {
                    Ok(u) => u,
                    Err(poisoned) => poisoned.into_inner(),
                };
                users.remove_user(&username)
            };
            match result {
                Ok(()) => {
                    // Removing an account must also remove its reach: a live
                    // session would otherwise outlive the account it belongs to.
                    let terminated = {
                        let mut sessions = match context.sessions.lock() {
                            Ok(s) => s,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        sessions.terminate_user(&username)
                    };
                    audit(
                        Ok("removed"),
                        format!(
                            "remove user={} sessions_terminated={}",
                            username, terminated
                        ),
                    );
                    json_response(
                        200,
                        serde_json::json!({"status":"ok","message":"Account removed",
                                           "sessions_terminated": terminated})
                        .to_string(),
                    )
                }
                Err(e) => {
                    audit(Err(e.clone()), format!("remove user={}", username));
                    json_response(400, error_body(&e))
                }
            }
        }

        _ => json_response(404, error_body("Unknown administrative route")),
    }
}

// ---------------------------------------------------------------------------
// Administration from the CLI
// ---------------------------------------------------------------------------

/// Where the account store lives unless the operator says otherwise.
fn users_path(explicit: Option<PathBuf>) -> Result<PathBuf, LitError> {
    match explicit {
        Some(p) => Ok(p),
        None => Ok(default_server_dir()
            .map_err(LitError::Config)?
            .join("users.json")),
    }
}

fn open_store(path: Option<PathBuf>) -> Result<UserStore, LitError> {
    let path = users_path(path)?;
    UserStore::open(&path, LockoutPolicy::default()).map_err(LitError::Config)
}

/// `lit server user add`
pub fn user_add(
    username: String,
    password: String,
    role: String,
    store_path: Option<PathBuf>,
) -> Result<ServerAdminResponse, LitError> {
    let role = Role::parse(&role).map_err(LitError::Config)?;
    let mut store = open_store(store_path)?;
    store
        .add_user(&username, &password, role)
        .map_err(LitError::Config)?;
    Ok(ServerAdminResponse {
        action: "user-add".to_string(),
        message: format!("Created account '{}' with role {}", username, role),
        users: None,
    })
}

/// `lit server user list`
pub fn user_list(store_path: Option<PathBuf>) -> Result<ServerAdminResponse, LitError> {
    let mut store = open_store(store_path)?;
    let users = store.list();
    let count = users.len();
    Ok(ServerAdminResponse {
        action: "user-list".to_string(),
        message: format!("{} account(s)", count),
        users: Some(serde_json::to_value(users).unwrap_or(serde_json::Value::Null)),
    })
}

/// `lit server user role`
pub fn user_role(
    username: String,
    role: String,
    store_path: Option<PathBuf>,
) -> Result<ServerAdminResponse, LitError> {
    let role = Role::parse(&role).map_err(LitError::Config)?;
    let mut store = open_store(store_path)?;
    store.set_role(&username, role).map_err(LitError::Config)?;
    Ok(ServerAdminResponse {
        action: "user-role".to_string(),
        message: format!(
            "Account '{}' now holds role {}. Existing sessions keep their previous role \
             until they expire; restart the server to end them immediately.",
            username, role
        ),
        users: None,
    })
}

/// `lit server user password`
pub fn user_password(
    username: String,
    password: String,
    store_path: Option<PathBuf>,
) -> Result<ServerAdminResponse, LitError> {
    let mut store = open_store(store_path)?;
    store
        .set_password(&username, &password)
        .map_err(LitError::Config)?;
    Ok(ServerAdminResponse {
        action: "user-password".to_string(),
        message: format!(
            "Password changed for '{}'; any lockout is cleared",
            username
        ),
        users: None,
    })
}

/// `lit server user disable` / `enable`
pub fn user_set_disabled(
    username: String,
    disabled: bool,
    store_path: Option<PathBuf>,
) -> Result<ServerAdminResponse, LitError> {
    let mut store = open_store(store_path)?;
    store
        .set_disabled(&username, disabled)
        .map_err(LitError::Config)?;
    Ok(ServerAdminResponse {
        action: if disabled {
            "user-disable"
        } else {
            "user-enable"
        }
        .to_string(),
        message: format!(
            "Account '{}' {}",
            username,
            if disabled { "disabled" } else { "enabled" }
        ),
        users: None,
    })
}

/// `lit server user remove`
pub fn user_remove(
    username: String,
    store_path: Option<PathBuf>,
) -> Result<ServerAdminResponse, LitError> {
    let mut store = open_store(store_path)?;
    store.remove_user(&username).map_err(LitError::Config)?;
    Ok(ServerAdminResponse {
        action: "user-remove".to_string(),
        message: format!("Removed account '{}'", username),
        users: None,
    })
}

/// `lit server user unlock`
pub fn user_unlock(
    username: String,
    store_path: Option<PathBuf>,
) -> Result<ServerAdminResponse, LitError> {
    let mut store = open_store(store_path)?;
    store.unlock(&username).map_err(LitError::Config)?;
    Ok(ServerAdminResponse {
        action: "user-unlock".to_string(),
        message: format!("Cleared the lockout on '{}'", username),
        users: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn the_cli_creates_lists_and_removes_accounts() {
        let dir = TempDir::new().unwrap();
        let path = Some(dir.path().join("users.json"));

        user_add(
            "root".into(),
            "correct horse battery staple".into(),
            "admin".into(),
            path.clone(),
        )
        .unwrap();
        user_add(
            "reader".into(),
            "correct horse battery staple".into(),
            "reader".into(),
            path.clone(),
        )
        .unwrap();

        let listed = user_list(path.clone()).unwrap();
        assert_eq!(listed.message, "2 account(s)");

        user_remove("reader".into(), path.clone()).unwrap();
        assert_eq!(user_list(path.clone()).unwrap().message, "1 account(s)");
    }

    #[test]
    fn an_unknown_role_name_is_refused_before_the_store_is_touched() {
        let dir = TempDir::new().unwrap();
        let path = Some(dir.path().join("users.json"));
        assert!(user_add(
            "root".into(),
            "correct horse battery staple".into(),
            "superuser".into(),
            path.clone(),
        )
        .is_err());
        assert!(!dir.path().join("users.json").exists());
    }

    #[test]
    fn a_well_formed_username_is_audited_verbatim() {
        assert_eq!(audit_subject("alice"), "alice");
        assert_eq!(audit_subject("ci-bot_1.x@example"), "ci-bot_1.x@example");
    }

    #[test]
    fn a_password_typed_into_the_username_field_is_not_stored() {
        // The case this exists for: most passwords carry a space or a symbol
        // outside the account charset, and an audit log is retained and
        // forwarded.
        assert_eq!(
            audit_subject("correct horse battery staple"),
            "<malformed-username>"
        );
        assert_eq!(audit_subject("hunter2!"), "<malformed-username>");
    }

    #[test]
    fn an_oversized_or_empty_username_is_not_stored() {
        assert_eq!(audit_subject(""), "<malformed-username>");
        assert_eq!(
            audit_subject(&"a".repeat(MAX_LOGIN_USERNAME + 1)),
            "<malformed-username>"
        );
        // The boundary itself is still a plausible account name.
        let at_limit = "a".repeat(MAX_LOGIN_USERNAME);
        assert_eq!(audit_subject(&at_limit), at_limit);
    }

    #[test]
    fn a_username_cannot_forge_a_second_audit_line() {
        // Serialization is what protects the `ts | event | json | hmac` format.
        // A crafted username must come back escaped, on one line.
        let crafted = "a\n2099-01-01T00:00:00Z | AUTH_SUCCESS | {} | deadbeef";
        let record = AuditRecord::failure("invalid_credentials").subject(audit_subject(crafted));
        let json = serde_json::to_string(&record).unwrap();
        assert!(
            !json.contains('\n'),
            "record must stay on one line: {}",
            json
        );
        // This particular input is malformed anyway, so it never reaches the log.
        assert!(json.contains("<malformed-username>"));

        // Even a shaped name is escaped rather than trusted.
        let record = AuditRecord::failure("invalid_credentials").subject("alice");
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains('\n'));
    }

    #[test]
    fn whoami_reports_the_permissions_the_role_carries() {
        let reader = Caller {
            username: "r".into(),
            role: Role::Reader,
        };
        let body = whoami_body(&reader);
        assert!(body.contains("\"read\""));
        assert!(!body.contains("\"write\""));

        let admin = Caller {
            username: "a".into(),
            role: Role::Admin,
        };
        let body = whoami_body(&admin);
        assert!(body.contains("\"administer\""));
    }

    #[test]
    fn a_server_with_no_accounts_refuses_to_start() {
        let dir = TempDir::new().unwrap();
        let mut options = ServerOptions::local(0).unwrap();
        options.users_path = dir.path().join("users.json");
        options.audit_enabled = false;
        // `ServerContext` holds the account store, so it is deliberately not
        // `Debug`; take the error rather than unwrapping through it.
        let err = match ServerContext::new(options) {
            Ok(_) => panic!("a server with no accounts should refuse to start"),
            Err(e) => e,
        };
        assert!(err.contains("No accounts exist"));
        assert!(err.contains("lit server user add"));
    }
}
