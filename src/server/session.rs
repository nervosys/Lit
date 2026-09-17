//! Bearer sessions with inactivity and absolute lifetime limits.
//!
//! A session is created by a successful authentication and identified by a
//! 256-bit random token. The token is never stored in the clear: the store
//! keeps only its SHA3-512 digest, so a copy of server memory or of a future
//! on-disk session file does not yield usable credentials.
//!
//! NIST SP 800-171r3 requirements implemented here:
//!
//! - `03.01.11` session termination — inactivity and absolute lifetime limits
//! - `03.05.01` user identification — every request resolves to a named account
//!
//! `03.01.10` (device lock) has no analogue in a headless service and is
//! documented as not applicable rather than claimed; see docs/NIST_800-171.md.

use crate::server::auth::Role;
use aes_gcm::aead::{rand_core::RngCore, OsRng};
use serde::Serialize;
use sha3::{Digest, Sha3_512};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Token size in bytes (256 bits).
const TOKEN_SIZE: usize = 32;

/// Why a session is no longer valid. Callers audit each variant distinctly:
/// an expiry is routine, a reuse of an unknown token may not be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    /// No session with that token, or it was already terminated.
    Unknown,
    /// Terminated because it sat idle longer than the inactivity limit.
    IdleTimeout,
    /// Terminated because it reached its absolute lifetime.
    LifetimeExceeded,
}

impl SessionError {
    /// A stable code for audit records and API error bodies.
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionError::Unknown => "unknown_session",
            SessionError::IdleTimeout => "idle_timeout",
            SessionError::LifetimeExceeded => "lifetime_exceeded",
        }
    }
}

/// Session lifetime policy.
#[derive(Debug, Clone, Copy)]
pub struct SessionPolicy {
    /// Terminate after this much inactivity.
    pub idle_timeout: Duration,
    /// Terminate this long after creation regardless of activity.
    pub max_lifetime: Duration,
    /// Refuse to create more than this many concurrent sessions per account.
    pub max_per_user: usize,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        SessionPolicy {
            idle_timeout: Duration::from_secs(900),
            max_lifetime: Duration::from_secs(8 * 3600),
            max_per_user: 8,
        }
    }
}

/// An authenticated session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub username: String,
    pub role: Role,
    pub created: Instant,
    pub last_seen: Instant,
    pub remote: Option<IpAddr>,
    /// RFC 3339 creation time, for audit records and the API.
    pub created_rfc3339: String,
}

/// A session as reported over the API, with no token material.
#[derive(Debug, Clone, Serialize)]
pub struct SessionSummary {
    pub username: String,
    pub role: Role,
    pub created: String,
    pub idle_secs: u64,
    pub remote: Option<String>,
}

/// The set of live sessions.
pub struct SessionStore {
    /// Keyed by the hex SHA3-512 digest of the bearer token.
    sessions: HashMap<String, Session>,
    policy: SessionPolicy,
}

impl SessionStore {
    pub fn new(policy: SessionPolicy) -> Self {
        SessionStore {
            sessions: HashMap::new(),
            policy,
        }
    }

    /// Digest a bearer token for lookup. Never store the token itself.
    fn digest(token: &str) -> String {
        let mut hasher = Sha3_512::new();
        hasher.update(token.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Create a session for an authenticated account, returning the bearer
    /// token. The token is the only copy; the store keeps a digest.
    pub fn create(&mut self, username: &str, role: Role, remote: Option<IpAddr>) -> String {
        self.reap();

        // Enforce the per-account concurrency cap by evicting the oldest.
        let mut owned: Vec<(String, Instant)> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.username == username)
            .map(|(k, s)| (k.clone(), s.created))
            .collect();
        if owned.len() >= self.policy.max_per_user {
            owned.sort_by_key(|(_, created)| *created);
            let excess = owned.len() + 1 - self.policy.max_per_user;
            for (key, _) in owned.into_iter().take(excess) {
                self.sessions.remove(&key);
            }
        }

        let mut raw = [0u8; TOKEN_SIZE];
        OsRng.fill_bytes(&mut raw);
        let token = hex::encode(raw);
        let now = Instant::now();

        self.sessions.insert(
            Self::digest(&token),
            Session {
                username: username.to_string(),
                role,
                created: now,
                last_seen: now,
                remote,
                created_rfc3339: chrono::Utc::now().to_rfc3339(),
            },
        );
        token
    }

    /// Validate a bearer token and mark the session active.
    ///
    /// A session that has passed either limit is removed here, so that the
    /// timeout is enforced on use and not merely on a sweep.
    pub fn validate(&mut self, token: &str) -> Result<Session, SessionError> {
        let key = Self::digest(token);
        let now = Instant::now();

        let Some(session) = self.sessions.get(&key) else {
            return Err(SessionError::Unknown);
        };

        if now.duration_since(session.created) >= self.policy.max_lifetime {
            self.sessions.remove(&key);
            return Err(SessionError::LifetimeExceeded);
        }
        if now.duration_since(session.last_seen) >= self.policy.idle_timeout {
            self.sessions.remove(&key);
            return Err(SessionError::IdleTimeout);
        }

        let session = self
            .sessions
            .get_mut(&key)
            .expect("session present under the same lock");
        session.last_seen = now;
        Ok(session.clone())
    }

    /// Terminate one session by its token. Returns the account it belonged to.
    pub fn terminate(&mut self, token: &str) -> Option<String> {
        self.sessions
            .remove(&Self::digest(token))
            .map(|s| s.username)
    }

    /// Terminate every session belonging to `username`, returning how many.
    /// Used when an account is disabled, deleted, or has its role reduced, so
    /// that a privilege change takes effect immediately rather than at expiry.
    pub fn terminate_user(&mut self, username: &str) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, s| s.username != username);
        before - self.sessions.len()
    }

    /// Drop sessions that have passed either limit.
    pub fn reap(&mut self) -> usize {
        let now = Instant::now();
        let idle = self.policy.idle_timeout;
        let lifetime = self.policy.max_lifetime;
        let before = self.sessions.len();
        self.sessions.retain(|_, s| {
            now.duration_since(s.created) < lifetime && now.duration_since(s.last_seen) < idle
        });
        before - self.sessions.len()
    }

    /// Live sessions, for administrative listing.
    pub fn list(&self) -> Vec<SessionSummary> {
        let now = Instant::now();
        let mut out: Vec<SessionSummary> = self
            .sessions
            .values()
            .map(|s| SessionSummary {
                username: s.username.clone(),
                role: s.role,
                created: s.created_rfc3339.clone(),
                idle_secs: now.duration_since(s.last_seen).as_secs(),
                remote: s.remote.map(|ip| ip.to_string()),
            })
            .collect();
        out.sort_by(|a, b| a.created.cmp(&b.created));
        out
    }

    /// Number of live sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether no sessions are live.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(idle_secs: u64, life_secs: u64) -> SessionPolicy {
        SessionPolicy {
            idle_timeout: Duration::from_secs(idle_secs),
            max_lifetime: Duration::from_secs(life_secs),
            max_per_user: 8,
        }
    }

    #[test]
    fn a_created_session_validates_and_carries_its_role() {
        let mut store = SessionStore::new(policy(900, 3600));
        let token = store.create("alice", Role::Maintainer, None);
        let session = store.validate(&token).expect("session should validate");
        assert_eq!(session.username, "alice");
        assert_eq!(session.role, Role::Maintainer);
    }

    #[test]
    fn an_unknown_token_is_rejected() {
        let mut store = SessionStore::new(SessionPolicy::default());
        assert_eq!(store.validate("not-a-token"), Err(SessionError::Unknown));
    }

    #[test]
    fn the_raw_token_is_not_used_as_the_key() {
        let mut store = SessionStore::new(SessionPolicy::default());
        let token = store.create("alice", Role::Reader, None);
        assert!(!store.sessions.contains_key(&token));
        assert!(store.sessions.contains_key(&SessionStore::digest(&token)));
    }

    #[test]
    fn an_idle_session_is_terminated_on_use() {
        // A zero-length idle limit makes any subsequent use "idle", which is
        // the boundary the comparison has to get right.
        let mut store = SessionStore::new(policy(0, 3600));
        let token = store.create("alice", Role::Reader, None);
        assert_eq!(store.validate(&token), Err(SessionError::IdleTimeout));
        // The session is gone, not merely refused.
        assert_eq!(store.validate(&token), Err(SessionError::Unknown));
    }

    #[test]
    fn a_session_past_its_absolute_lifetime_is_terminated() {
        let mut store = SessionStore::new(policy(900, 0));
        let token = store.create("alice", Role::Reader, None);
        assert_eq!(store.validate(&token), Err(SessionError::LifetimeExceeded));
    }

    #[test]
    fn terminating_a_user_drops_every_session_they_hold() {
        let mut store = SessionStore::new(SessionPolicy::default());
        let a = store.create("alice", Role::Reader, None);
        let b = store.create("alice", Role::Reader, None);
        let c = store.create("bob", Role::Reader, None);
        assert_eq!(store.terminate_user("alice"), 2);
        assert_eq!(store.validate(&a), Err(SessionError::Unknown));
        assert_eq!(store.validate(&b), Err(SessionError::Unknown));
        assert!(store.validate(&c).is_ok());
    }

    #[test]
    fn explicit_termination_invalidates_the_token() {
        let mut store = SessionStore::new(SessionPolicy::default());
        let token = store.create("alice", Role::Reader, None);
        assert_eq!(store.terminate(&token).as_deref(), Some("alice"));
        assert_eq!(store.validate(&token), Err(SessionError::Unknown));
    }

    #[test]
    fn the_per_account_session_cap_evicts_the_oldest() {
        let mut store = SessionStore::new(SessionPolicy {
            idle_timeout: Duration::from_secs(900),
            max_lifetime: Duration::from_secs(3600),
            max_per_user: 2,
        });
        let first = store.create("alice", Role::Reader, None);
        let second = store.create("alice", Role::Reader, None);
        let third = store.create("alice", Role::Reader, None);
        assert_eq!(store.validate(&first), Err(SessionError::Unknown));
        assert!(store.validate(&second).is_ok());
        assert!(store.validate(&third).is_ok());
    }

    #[test]
    fn two_sessions_never_share_a_token() {
        let mut store = SessionStore::new(SessionPolicy::default());
        let a = store.create("alice", Role::Reader, None);
        let b = store.create("bob", Role::Reader, None);
        assert_ne!(a, b);
        assert_eq!(a.len(), TOKEN_SIZE * 2);
    }
}
