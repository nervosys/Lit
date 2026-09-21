//! Self-hosting support for `lit serve`.
//!
//! `lit serve` began as a development convenience: bound to loopback, one
//! shared bearer token, plaintext HTTP. That is a reasonable local tool and an
//! unreasonable server. This module supplies what a hosted deployment needs —
//! named accounts, roles, sessions that expire, TLS, a system use notification,
//! and an audit record for every decision.
//!
//! The control mapping, including the requirements Lit does *not* satisfy on
//! its own, is in `docs/NIST_800-171.md`. Deployment guidance is in
//! `docs/SELF_HOSTING.md`.
//!
//! ## Where authorization is decided
//!
//! In exactly one place: [`required_role`]. Every request is resolved to the
//! role it demands before it reaches a command, and a caller below that role is
//! refused. Route handlers do no authorization of their own, so a new route
//! that is not listed there fails closed as `Admin`.

pub mod audit;
pub mod auth;
pub mod banner;
pub mod proxy;
pub mod session;
pub mod tls;

use audit::{AuditRecord, AuditRecorder, ServerEvent};
use auth::{LockoutPolicy, Role, UserStore};
use banner::Banner;
use session::{SessionPolicy, SessionStore};
use std::path::PathBuf;
use std::sync::Mutex;
use tiny_http::Method;

/// Where server state lives, under the user's Lit directory.
pub fn default_server_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".lit").join("server"))
}

/// Everything the operator can configure about a hosted server.
#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// Address to bind, host and port.
    pub bind: String,
    /// TLS material. `None` serves plaintext, which is refused off loopback
    /// unless `allow_plaintext` is set.
    pub tls: Option<tls::TlsPaths>,
    /// Operator-supplied system use notification.
    pub banner_path: Option<PathBuf>,
    /// Account store location.
    pub users_path: PathBuf,
    /// Audit log location. `None` uses the default path.
    pub audit_path: Option<String>,
    /// Whether to write audit records at all.
    pub audit_enabled: bool,
    pub session_policy: SessionPolicy,
    pub lockout_policy: LockoutPolicy,
    /// Serve plaintext on a routable address anyway. Requires the operator to
    /// say so explicitly, and is recorded in the audit log at startup.
    pub allow_plaintext: bool,
    /// Addresses that are reverse proxies, and whose `X-Forwarded-For` may
    /// therefore be believed. Empty by default, which ignores the header: it is
    /// caller-supplied, and trusting it from anyone would let every client
    /// choose what its audit records say and which rate-limit bucket it lands
    /// in. See `server::proxy`.
    pub trusted_proxies: Vec<std::net::IpAddr>,
}

impl ServerOptions {
    /// Defaults for a local server: loopback, no TLS, auditing on.
    pub fn local(port: u16) -> Result<Self, String> {
        let dir = default_server_dir()?;
        Ok(ServerOptions {
            bind: format!("127.0.0.1:{}", port),
            tls: None,
            banner_path: None,
            users_path: dir.join("users.json"),
            audit_path: None,
            audit_enabled: true,
            session_policy: SessionPolicy::default(),
            lockout_policy: LockoutPolicy::default(),
            allow_plaintext: false,
            trusted_proxies: Vec::new(),
        })
    }

    /// Reject configurations that would transmit credentials in the clear.
    ///
    /// This is the one place the server refuses to start over a policy
    /// question. It is worth it: the failure mode it prevents — a server that
    /// works perfectly while publishing every session token to the network — is
    /// silent, and by the time it is noticed the credentials are already out.
    pub fn validate(&self) -> Result<(), String> {
        if self.tls.is_none() && !tls::is_loopback(&self.bind) && !self.allow_plaintext {
            return Err(format!(
                "Refusing to serve plaintext HTTP on {}, which is not a loopback address.\n\
                 Session tokens and passwords would cross the network unprotected \
                 (NIST SP 800-171r3 03.13.08).\n\
                 Supply --tls-cert and --tls-key, put a TLS-terminating proxy in front and bind \
                 loopback, or pass --allow-plaintext if this network is protected by other means.",
                self.bind
            ));
        }
        Ok(())
    }
}

/// The role a request must hold to be served.
///
/// Unlisted routes require `Admin`, so adding a route without considering its
/// authorization makes it unreachable rather than public.
pub fn required_role(method: &Method, path: &str) -> Role {
    match (method, path) {
        // Pre-authentication surfaces are handled before this function is
        // consulted; they appear here so the table is complete.
        (Method::Get, "/api/v1/banner") => Role::Reader,
        (Method::Post, "/api/v1/auth/login") => Role::Reader,

        // Anything a session holder may do with their own session.
        (Method::Post, "/api/v1/auth/logout") => Role::Reader,
        (Method::Get, "/api/v1/auth/whoami") => Role::Reader,

        // Reading repository state.
        (Method::Get, "/api/v1") | (Method::Get, "/api/v1/") => Role::Reader,
        (Method::Get, p)
            if p.starts_with("/api/v1/status")
                || p.starts_with("/api/v1/log")
                || p.starts_with("/api/v1/branches")
                || p.starts_with("/api/v1/diff")
                || p.starts_with("/api/v1/show/")
                || p.starts_with("/api/v1/tags")
                || p.starts_with("/api/v1/remotes")
                || p.starts_with("/api/v1/config")
                || p.starts_with("/api/v1/search")
                || p.starts_with("/api/v1/verify")
                || p.starts_with("/api/v1/ontology") =>
        {
            Role::Reader
        }

        // Adding to history.
        (Method::Post, "/api/v1/add")
        | (Method::Post, "/api/v1/commit")
        | (Method::Post, "/api/v1/snapshot") => Role::Contributor,

        // Moving or rewriting history, and changing what the working tree is.
        (Method::Post, "/api/v1/checkout")
        | (Method::Post, "/api/v1/merge")
        | (Method::Post, "/api/v1/branch") => Role::Maintainer,

        // Administering the server itself.
        (_, p) if p.starts_with("/api/v1/admin/") => Role::Admin,

        // Fail closed.
        _ => Role::Admin,
    }
}

/// Whether a route is reachable without a session.
pub fn is_public_route(method: &Method, path: &str) -> bool {
    matches!(
        (method, path),
        (Method::Get, "/api/v1/banner") | (Method::Post, "/api/v1/auth/login")
    )
}

/// The identity behind an authorized request.
#[derive(Debug, Clone)]
pub struct Caller {
    pub username: String,
    pub role: Role,
}

/// Shared, mutable server state.
///
/// The two stores are separately locked because they are contended on
/// different paths: every request touches sessions, while accounts are touched
/// only at login and by administration.
pub struct ServerContext {
    pub users: Mutex<UserStore>,
    pub sessions: Mutex<SessionStore>,
    pub recorder: AuditRecorder,
    pub banner: Banner,
    pub options: ServerOptions,
}

impl ServerContext {
    /// Build the server's state, or explain why it cannot be built.
    pub fn new(options: ServerOptions) -> Result<Self, String> {
        options.validate()?;

        let users = UserStore::open(&options.users_path, options.lockout_policy)?;
        if users.is_empty() {
            return Err(format!(
                "No accounts exist in {}.\n\
                 Create the first administrator with:  lit server user add <name> --role admin",
                options.users_path.display()
            ));
        }

        let banner = Banner::load(options.banner_path.as_deref())?;
        let recorder = if options.audit_enabled {
            AuditRecorder::enabled(options.audit_path.as_deref())?
        } else {
            AuditRecorder::disabled()
        };

        Ok(ServerContext {
            users: Mutex::new(users),
            sessions: Mutex::new(SessionStore::new(options.session_policy)),
            recorder,
            banner,
            options,
        })
    }

    /// Record the facts about how this server was started, so that a later
    /// reader of the log can tell which posture was in force.
    pub fn record_startup(&self) {
        let mut notes = Vec::new();
        notes.push(format!("bind={}", self.options.bind));
        notes.push(format!(
            "tls={}",
            if self.options.tls.is_some() {
                "on"
            } else {
                "off"
            }
        ));
        if self.options.tls.is_none() && self.options.allow_plaintext {
            notes.push("plaintext_override=yes".to_string());
        }
        if !self.banner.customized {
            notes.push("banner=default_placeholder".to_string());
        }
        if !self.options.trusted_proxies.is_empty() {
            notes.push(format!(
                "trusted_proxies={}",
                self.options.trusted_proxies.len()
            ));
        }
        self.recorder.record(
            ServerEvent::ServerStart,
            AuditRecord::success().object(notes.join(" ")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_routes_require_only_reader() {
        assert_eq!(required_role(&Method::Get, "/api/v1/status"), Role::Reader);
        assert_eq!(required_role(&Method::Get, "/api/v1/log"), Role::Reader);
        assert_eq!(
            required_role(&Method::Get, "/api/v1/show/abc123"),
            Role::Reader
        );
    }

    #[test]
    fn writing_requires_contributor_and_rewriting_requires_maintainer() {
        assert_eq!(
            required_role(&Method::Post, "/api/v1/commit"),
            Role::Contributor
        );
        assert_eq!(
            required_role(&Method::Post, "/api/v1/add"),
            Role::Contributor
        );
        assert_eq!(
            required_role(&Method::Post, "/api/v1/merge"),
            Role::Maintainer
        );
        assert_eq!(
            required_role(&Method::Post, "/api/v1/checkout"),
            Role::Maintainer
        );
    }

    #[test]
    fn administration_requires_admin() {
        assert_eq!(
            required_role(&Method::Get, "/api/v1/admin/users"),
            Role::Admin
        );
        assert_eq!(
            required_role(&Method::Post, "/api/v1/admin/users"),
            Role::Admin
        );
    }

    #[test]
    fn an_unlisted_route_fails_closed() {
        assert_eq!(
            required_role(&Method::Post, "/api/v1/brand-new"),
            Role::Admin
        );
        assert_eq!(
            required_role(&Method::Delete, "/api/v1/status"),
            Role::Admin
        );
    }

    #[test]
    fn a_reader_cannot_reach_a_write_route() {
        assert!(Role::Reader < required_role(&Method::Post, "/api/v1/commit"));
        assert!(Role::Contributor >= required_role(&Method::Post, "/api/v1/commit"));
        assert!(Role::Contributor < required_role(&Method::Post, "/api/v1/merge"));
    }

    #[test]
    fn only_the_banner_and_login_are_public() {
        assert!(is_public_route(&Method::Get, "/api/v1/banner"));
        assert!(is_public_route(&Method::Post, "/api/v1/auth/login"));
        assert!(!is_public_route(&Method::Get, "/api/v1/status"));
        assert!(!is_public_route(&Method::Post, "/api/v1/commit"));
        assert!(!is_public_route(&Method::Get, "/api/v1/auth/whoami"));
    }

    #[test]
    fn plaintext_off_loopback_is_refused_unless_overridden() {
        let mut opts = ServerOptions::local(8080).unwrap();
        opts.bind = "0.0.0.0:8080".to_string();
        let err = opts.validate().unwrap_err();
        assert!(err.contains("03.13.08"));

        opts.allow_plaintext = true;
        assert!(opts.validate().is_ok());
    }

    #[test]
    fn plaintext_on_loopback_is_allowed() {
        let opts = ServerOptions::local(8080).unwrap();
        assert!(opts.validate().is_ok());
    }
}
