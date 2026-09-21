//! Server accounts, roles and account lockout.
//!
//! `lit serve` historically accepted a single shared bearer token, which cannot
//! identify *who* acted. Attribution is the foundation every other control here
//! rests on: an audit record naming "the token" satisfies nothing. This module
//! gives the server real accounts.
//!
//! NIST SP 800-171r3 requirements implemented here:
//!
//! - `03.01.01` account management — create, enable, disable, remove accounts
//! - `03.01.02` access enforcement — role-gated authorization decisions
//! - `03.01.05` least privilege — four roles, lowest sufficient by default
//! - `03.05.01` user identification and authentication — unique named accounts
//! - `03.05.07` password management — length policy, salted PBKDF2 storage
//! - `03.05.08` unsuccessful logon attempts — windowed lockout
//!
//! Passwords are stored as PBKDF2-HMAC-SHA512 (NIST SP 800-132) at the same
//! 600,000 iterations the repository key derivation uses. The store never holds
//! a plaintext password, and comparison is constant-time.

use crate::crypto::encryption::{allow_replacement, restrict_to_owner};
use aes_gcm::aead::{rand_core::RngCore, OsRng};
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// PBKDF2 iterations, matching `crypto::encryption` (NIST SP 800-132).
const PBKDF2_ITERATIONS: u32 = 600_000;

/// Salt size in bytes (128 bits).
const SALT_SIZE: usize = 16;

/// Derived password verifier size in bytes (512 bits).
const VERIFIER_SIZE: usize = 64;

/// Minimum password length. 800-171r3 leaves the value to the organization;
/// this is SP 800-63B's memorized-secret guidance for a human-chosen password.
pub const MIN_PASSWORD_LEN: usize = 15;

/// Upper bound, so that a pathological input cannot turn PBKDF2 into a DoS.
pub const MAX_PASSWORD_LEN: usize = 256;

/// What a caller is trying to do, independent of which route expresses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Read repository state: status, log, diff, show, search.
    Read,
    /// Mutate repository state: add, commit, merge, branch, checkout.
    Write,
    /// Administer the server itself: accounts, sessions, audit.
    Administer,
}

/// Server roles, ordered least to most privileged.
///
/// Least privilege (`03.01.05`) is expressed by giving each account the lowest
/// role that lets it do its job; `Reader` is the default for new accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// May read repository state. Cannot mutate anything.
    Reader,
    /// May read and commit.
    Contributor,
    /// May read, commit, and perform history-altering operations.
    Maintainer,
    /// Full control, including account and session administration.
    Admin,
}

impl Role {
    /// Whether this role carries `permission`.
    pub fn grants(&self, permission: Permission) -> bool {
        match permission {
            Permission::Read => true,
            Permission::Write => *self >= Role::Contributor,
            Permission::Administer => *self == Role::Admin,
        }
    }

    /// Parse a role name, for CLI and config surfaces.
    pub fn parse(s: &str) -> Result<Role, String> {
        match s.to_ascii_lowercase().as_str() {
            "reader" => Ok(Role::Reader),
            "contributor" => Ok(Role::Contributor),
            "maintainer" => Ok(Role::Maintainer),
            "admin" => Ok(Role::Admin),
            other => Err(format!(
                "Unknown role '{}' (expected reader, contributor, maintainer, or admin)",
                other
            )),
        }
    }

    /// The wire/CLI name of this role.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Reader => "reader",
            Role::Contributor => "contributor",
            Role::Maintainer => "maintainer",
            Role::Admin => "admin",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Lockout policy for consecutive failed authentications (`03.05.08`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LockoutPolicy {
    /// Consecutive failures tolerated before the account locks.
    pub max_attempts: u32,
    /// Window, in seconds, over which failures are counted as consecutive.
    pub window_secs: i64,
    /// How long, in seconds, the account stays locked. Zero means until an
    /// administrator unlocks it, which is the stricter reading of the control.
    pub lockout_secs: i64,
}

impl Default for LockoutPolicy {
    fn default() -> Self {
        LockoutPolicy {
            max_attempts: 5,
            window_secs: 900,
            lockout_secs: 900,
        }
    }
}

/// A stored account. The password verifier and its salt are hex-encoded so the
/// file stays readable to an administrator auditing it by eye.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub role: Role,
    /// Hex-encoded PBKDF2 salt.
    salt: String,
    /// Hex-encoded PBKDF2-HMAC-SHA512 output.
    verifier: String,
    iterations: u32,
    #[serde(default)]
    pub disabled: bool,
    pub created: String,
    #[serde(default)]
    pub last_login: Option<String>,
    #[serde(default)]
    failed_attempts: u32,
    /// RFC 3339 instant of the first failure in the current window.
    #[serde(default)]
    first_failure: Option<String>,
    /// RFC 3339 instant until which authentication is refused.
    #[serde(default)]
    locked_until: Option<String>,
}

impl User {
    /// Whether the account is locked as of `now`.
    fn is_locked(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<chrono::DateTime<chrono::Utc>> {
        let until = self.locked_until.as_ref()?;
        let parsed = chrono::DateTime::parse_from_rfc3339(until).ok()?;
        let until_utc = parsed.with_timezone(&chrono::Utc);
        // A zero-duration policy stores a sentinel far in the future, so this
        // comparison covers "locked until an administrator intervenes" too.
        if until_utc > now {
            Some(until_utc)
        } else {
            None
        }
    }
}

/// The outcome of an authentication attempt. Callers audit every variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    /// Credentials verified; the account carries this role.
    Success { username: String, role: Role },
    /// No such account, or the wrong password. Deliberately indistinguishable.
    InvalidCredentials,
    /// Account locked by the lockout policy.
    Locked { until: String },
    /// Account exists but has been disabled by an administrator.
    Disabled,
}

/// A view of an account with no secret material, for listing over the API.
#[derive(Debug, Clone, Serialize)]
pub struct UserSummary {
    pub username: String,
    pub role: Role,
    pub disabled: bool,
    pub created: String,
    pub last_login: Option<String>,
    pub locked: bool,
}

/// The account database, persisted as JSON with owner-only permissions.
pub struct UserStore {
    path: PathBuf,
    users: HashMap<String, User>,
    policy: LockoutPolicy,
    /// The file's modification time and length as this instance last saw them.
    /// `None` when the file does not exist yet. See [`UserStore::refresh`].
    stamp: Option<(SystemTime, u64)>,
}

#[derive(Serialize, Deserialize, Default)]
struct StoreFile {
    #[serde(default)]
    users: Vec<User>,
}

/// The file's modification time and length, or `None` if it is absent.
///
/// Length is carried alongside the timestamp because filesystem timestamp
/// granularity is coarse enough — a second or worse on some filesystems — that
/// two edits within one tick would otherwise look identical.
fn stamp_of(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

impl UserStore {
    /// Open the store at `path`, creating an empty one if absent.
    pub fn open(path: &Path, policy: LockoutPolicy) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create server directory: {}", e))?;
        }

        let users = if path.exists() {
            // Tighten permissions before reading: a store left world-readable by
            // an earlier version should not stay that way just because nobody
            // wrote to it since.
            restrict_to_owner(path)?;
            let raw = fs::read_to_string(path)
                .map_err(|e| format!("Failed to read account store: {}", e))?;
            let parsed: StoreFile = serde_json::from_str(&raw)
                .map_err(|e| format!("Account store is not valid JSON: {}", e))?;
            parsed
                .users
                .into_iter()
                .map(|u| (u.username.to_ascii_lowercase(), u))
                .collect()
        } else {
            HashMap::new()
        };

        Ok(UserStore {
            path: path.to_path_buf(),
            users,
            policy,
            stamp: stamp_of(path),
        })
    }

    /// Re-read the store if the file changed underneath this instance.
    ///
    /// The server holds one `UserStore` for its whole lifetime, while
    /// `lit server user …` edits the same file from another process. Without
    /// this, an administrative change was invisible to the running server *and*
    /// was then erased by the server's next save — so a `disable` appeared to
    /// succeed while the account kept working. Called before reading the
    /// account set and before writing it.
    ///
    /// Residual race: two processes that write between one another's refresh
    /// and save can still lose an update. Closing that needs file locking.
    /// Administer a running server over its API rather than the CLI.
    pub fn refresh(&mut self) -> Result<(), String> {
        let current = stamp_of(&self.path);
        if current == self.stamp {
            return Ok(());
        }
        // The file went away. Keep what is in memory rather than silently
        // dropping every account — losing the store should not become a way to
        // disable authentication wholesale.
        if current.is_none() {
            return Ok(());
        }

        let raw = fs::read_to_string(&self.path)
            .map_err(|e| format!("Failed to re-read account store: {}", e))?;
        let parsed: StoreFile = serde_json::from_str(&raw)
            .map_err(|e| format!("Account store is not valid JSON: {}", e))?;
        self.users = parsed
            .users
            .into_iter()
            .map(|u| (u.username.to_ascii_lowercase(), u))
            .collect();
        self.stamp = current;
        Ok(())
    }

    /// Number of accounts in the store.
    pub fn len(&self) -> usize {
        self.users.len()
    }

    /// Whether the store holds no accounts. A server with none cannot be used.
    pub fn is_empty(&self) -> bool {
        self.users.is_empty()
    }

    /// Whether any enabled account holds `Admin`. Used to refuse the last
    /// administrator being removed or demoted, which would strand the server.
    fn enabled_admin_count(&self) -> usize {
        self.users
            .values()
            .filter(|u| u.role == Role::Admin && !u.disabled)
            .count()
    }

    /// Persist the store, with owner-only permissions.
    fn save(&mut self) -> Result<(), String> {
        let mut users: Vec<User> = self.users.values().cloned().collect();
        users.sort_by(|a, b| a.username.cmp(&b.username));
        let file = StoreFile { users };
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("Failed to serialize account store: {}", e))?;

        // Write to a temporary file in the same directory, restrict it, then
        // rename. Writing in place would leave a window in which the store is
        // truncated, and restricting after writing would leave one in which the
        // verifiers are readable — the pattern §3 of the handoff calls out.
        //
        // The temporary name carries the process id, because two processes
        // administering one store is a case this deliberately supports: a fixed
        // `users.json.tmp` would have them overwrite each other's half-written
        // file and race on the rename.
        let tmp = self
            .path
            .with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, json.as_bytes())
            .map_err(|e| format!("Failed to write account store: {}", e))?;
        restrict_to_owner(&tmp)?;

        // Windows refuses to rename onto a file it considers read-only, and the
        // restriction applied by the previous save is exactly that. Clearing it
        // first is what `EncryptionKey::save` does, for the same reason.
        allow_replacement(&self.path)?;
        if let Err(e) = fs::rename(&tmp, &self.path) {
            // Do not leave the temporary file behind holding every verifier.
            let _ = fs::remove_file(&tmp);
            return Err(format!("Failed to replace account store: {}", e));
        }
        restrict_to_owner(&self.path)?;
        // Record what we just wrote, so our own save is not mistaken for an
        // outside edit on the next refresh.
        self.stamp = stamp_of(&self.path);
        Ok(())
    }

    /// Check a candidate password against policy (`03.05.07`).
    pub fn check_password_policy(username: &str, password: &str) -> Result<(), String> {
        if password.len() < MIN_PASSWORD_LEN {
            return Err(format!(
                "Password must be at least {} characters",
                MIN_PASSWORD_LEN
            ));
        }
        if password.len() > MAX_PASSWORD_LEN {
            return Err(format!(
                "Password must be at most {} characters",
                MAX_PASSWORD_LEN
            ));
        }
        if password
            .to_ascii_lowercase()
            .contains(&username.to_ascii_lowercase())
        {
            return Err("Password must not contain the username".to_string());
        }
        if password
            .chars()
            .collect::<std::collections::HashSet<_>>()
            .len()
            < 5
        {
            return Err("Password must not be a short repeated sequence".to_string());
        }
        Ok(())
    }

    /// Derive a verifier from `password` and `salt`.
    fn derive(password: &str, salt: &[u8], iterations: u32) -> Zeroizing<Vec<u8>> {
        let mut out = Zeroizing::new(vec![0u8; VERIFIER_SIZE]);
        pbkdf2_hmac::<Sha512>(password.as_bytes(), salt, iterations, out.as_mut_slice());
        out
    }

    /// Create an account. Fails if the username is taken.
    pub fn add_user(&mut self, username: &str, password: &str, role: Role) -> Result<(), String> {
        self.refresh()?;
        let key = username.to_ascii_lowercase();
        if username.trim().is_empty() {
            return Err("Username must not be empty".to_string());
        }
        if !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '@')
        {
            return Err(
                "Username may contain only letters, digits, and the characters -_.@".to_string(),
            );
        }
        if self.users.contains_key(&key) {
            return Err(format!("Account '{}' already exists", username));
        }
        Self::check_password_policy(username, password)?;

        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);
        let verifier = Self::derive(password, &salt, PBKDF2_ITERATIONS);

        self.users.insert(
            key,
            User {
                username: username.to_string(),
                role,
                salt: hex::encode(salt),
                verifier: hex::encode(verifier.as_slice()),
                iterations: PBKDF2_ITERATIONS,
                disabled: false,
                created: chrono::Utc::now().to_rfc3339(),
                last_login: None,
                failed_attempts: 0,
                first_failure: None,
                locked_until: None,
            },
        );
        self.save()
    }

    /// Replace an account's password.
    pub fn set_password(&mut self, username: &str, password: &str) -> Result<(), String> {
        self.refresh()?;
        let key = username.to_ascii_lowercase();
        Self::check_password_policy(username, password)?;
        let user = self
            .users
            .get_mut(&key)
            .ok_or_else(|| format!("No such account '{}'", username))?;

        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);
        let verifier = Self::derive(password, &salt, PBKDF2_ITERATIONS);
        user.salt = hex::encode(salt);
        user.verifier = hex::encode(verifier.as_slice());
        user.iterations = PBKDF2_ITERATIONS;
        // A password change clears a lockout: the credential it locked is gone.
        user.failed_attempts = 0;
        user.first_failure = None;
        user.locked_until = None;
        self.save()
    }

    /// Change an account's role, refusing to strand the server without an admin.
    pub fn set_role(&mut self, username: &str, role: Role) -> Result<(), String> {
        self.refresh()?;
        let key = username.to_ascii_lowercase();
        let current = self
            .users
            .get(&key)
            .ok_or_else(|| format!("No such account '{}'", username))?;
        if current.role == Role::Admin && role != Role::Admin && self.enabled_admin_count() <= 1 {
            return Err("Refusing to demote the only enabled admin account".to_string());
        }
        if let Some(user) = self.users.get_mut(&key) {
            user.role = role;
        }
        self.save()
    }

    /// Enable or disable an account (`03.01.01`).
    pub fn set_disabled(&mut self, username: &str, disabled: bool) -> Result<(), String> {
        self.refresh()?;
        let key = username.to_ascii_lowercase();
        let current = self
            .users
            .get(&key)
            .ok_or_else(|| format!("No such account '{}'", username))?;
        if disabled && current.role == Role::Admin && self.enabled_admin_count() <= 1 {
            return Err("Refusing to disable the only enabled admin account".to_string());
        }
        if let Some(user) = self.users.get_mut(&key) {
            user.disabled = disabled;
        }
        self.save()
    }

    /// Remove an account.
    pub fn remove_user(&mut self, username: &str) -> Result<(), String> {
        self.refresh()?;
        let key = username.to_ascii_lowercase();
        let current = self
            .users
            .get(&key)
            .ok_or_else(|| format!("No such account '{}'", username))?;
        if current.role == Role::Admin && self.enabled_admin_count() <= 1 {
            return Err("Refusing to remove the only enabled admin account".to_string());
        }
        self.users.remove(&key);
        self.save()
    }

    /// Clear a lockout ahead of its expiry.
    pub fn unlock(&mut self, username: &str) -> Result<(), String> {
        self.refresh()?;
        let key = username.to_ascii_lowercase();
        let user = self
            .users
            .get_mut(&key)
            .ok_or_else(|| format!("No such account '{}'", username))?;
        user.failed_attempts = 0;
        user.first_failure = None;
        user.locked_until = None;
        self.save()
    }

    /// The account's current role and whether it is enabled, or `None` if the
    /// account no longer exists.
    ///
    /// This is what lets an established session be re-checked on every request.
    /// A session carries the role it was issued with; without consulting the
    /// store again, a revocation or a demotion took effect only when that
    /// session happened to expire — up to the absolute lifetime later.
    pub fn account_state(&mut self, username: &str) -> Option<(Role, bool)> {
        if let Err(e) = self.refresh() {
            eprintln!("WARNING: could not re-read the account store: {}", e);
        }
        self.users
            .get(&username.to_ascii_lowercase())
            .map(|u| (u.role, !u.disabled))
    }

    /// All accounts, without secret material.
    ///
    /// Takes `&mut self` so it can pick up outside edits first — a listing that
    /// showed a stale account set would be actively misleading to the
    /// administrator deciding what to change.
    pub fn list(&mut self) -> Vec<UserSummary> {
        if let Err(e) = self.refresh() {
            eprintln!("WARNING: could not re-read the account store: {}", e);
        }
        let now = chrono::Utc::now();
        let mut out: Vec<UserSummary> = self
            .users
            .values()
            .map(|u| UserSummary {
                username: u.username.clone(),
                role: u.role,
                disabled: u.disabled,
                created: u.created.clone(),
                last_login: u.last_login.clone(),
                locked: u.is_locked(now).is_some(),
            })
            .collect();
        out.sort_by(|a, b| a.username.cmp(&b.username));
        out
    }

    /// Verify a credential, applying and updating the lockout policy.
    ///
    /// An unknown username still pays the full PBKDF2 cost, so that response
    /// time does not reveal which accounts exist.
    pub fn authenticate(&mut self, username: &str, password: &str) -> AuthOutcome {
        // Pick up any administrative change made since the last call. A failure
        // here means the file is unreadable or corrupt; carry on with the
        // in-memory copy rather than locking everyone out, but say so, because
        // it means administrative changes are silently not arriving.
        if let Err(e) = self.refresh() {
            eprintln!("WARNING: could not re-read the account store: {}", e);
        }

        let now = chrono::Utc::now();
        let key = username.to_ascii_lowercase();

        let Some(user) = self.users.get(&key).cloned() else {
            let decoy = [0u8; SALT_SIZE];
            let _ = Self::derive(password, &decoy, PBKDF2_ITERATIONS);
            return AuthOutcome::InvalidCredentials;
        };

        if let Some(until) = user.is_locked(now) {
            return AuthOutcome::Locked {
                until: until.to_rfc3339(),
            };
        }
        if user.disabled {
            return AuthOutcome::Disabled;
        }

        let Ok(salt) = hex::decode(&user.salt) else {
            return AuthOutcome::InvalidCredentials;
        };
        let Ok(expected) = hex::decode(&user.verifier) else {
            return AuthOutcome::InvalidCredentials;
        };
        let candidate = Self::derive(password, &salt, user.iterations);
        let matches: bool = candidate.as_slice().ct_eq(expected.as_slice()).into();

        if matches {
            if let Some(u) = self.users.get_mut(&key) {
                u.failed_attempts = 0;
                u.first_failure = None;
                u.locked_until = None;
                u.last_login = Some(now.to_rfc3339());
            }
            let _ = self.save();
            AuthOutcome::Success {
                username: user.username,
                role: user.role,
            }
        } else {
            let outcome = self.record_failure(&key, now);
            let _ = self.save();
            outcome
        }
    }

    /// Count a failure and lock the account if the policy threshold is crossed.
    fn record_failure(&mut self, key: &str, now: chrono::DateTime<chrono::Utc>) -> AuthOutcome {
        let policy = self.policy;
        let Some(user) = self.users.get_mut(key) else {
            return AuthOutcome::InvalidCredentials;
        };

        // Failures older than the window do not count toward the threshold.
        let within_window = user
            .first_failure
            .as_ref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|t| (now - t.with_timezone(&chrono::Utc)).num_seconds() < policy.window_secs)
            .unwrap_or(false);

        if within_window {
            user.failed_attempts += 1;
        } else {
            user.failed_attempts = 1;
            user.first_failure = Some(now.to_rfc3339());
        }

        if user.failed_attempts >= policy.max_attempts {
            let until = if policy.lockout_secs > 0 {
                now + chrono::Duration::seconds(policy.lockout_secs)
            } else {
                // Locked until an administrator intervenes.
                now + chrono::Duration::days(3650)
            };
            user.locked_until = Some(until.to_rfc3339());
            return AuthOutcome::Locked {
                until: until.to_rfc3339(),
            };
        }

        AuthOutcome::InvalidCredentials
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store(dir: &TempDir, policy: LockoutPolicy) -> UserStore {
        UserStore::open(&dir.path().join("users.json"), policy).unwrap()
    }

    #[test]
    fn roles_grant_increasing_permission() {
        assert!(Role::Reader.grants(Permission::Read));
        assert!(!Role::Reader.grants(Permission::Write));
        assert!(Role::Contributor.grants(Permission::Write));
        assert!(!Role::Contributor.grants(Permission::Administer));
        assert!(Role::Maintainer.grants(Permission::Write));
        assert!(!Role::Maintainer.grants(Permission::Administer));
        assert!(Role::Admin.grants(Permission::Administer));
    }

    #[test]
    fn a_created_account_authenticates_and_a_wrong_password_does_not() {
        let dir = TempDir::new().unwrap();
        let mut s = store(&dir, LockoutPolicy::default());
        s.add_user("alice", "correct horse battery staple", Role::Contributor)
            .unwrap();

        match s.authenticate("alice", "correct horse battery staple") {
            AuthOutcome::Success { username, role } => {
                assert_eq!(username, "alice");
                assert_eq!(role, Role::Contributor);
            }
            other => panic!("expected success, got {:?}", other),
        }
        assert_eq!(
            s.authenticate("alice", "incorrect horse battery staple"),
            AuthOutcome::InvalidCredentials
        );
    }

    #[test]
    fn an_unknown_account_is_indistinguishable_from_a_wrong_password() {
        let dir = TempDir::new().unwrap();
        let mut s = store(&dir, LockoutPolicy::default());
        s.add_user("alice", "correct horse battery staple", Role::Reader)
            .unwrap();
        assert_eq!(
            s.authenticate("nobody", "correct horse battery staple"),
            AuthOutcome::InvalidCredentials
        );
    }

    #[test]
    fn consecutive_failures_lock_the_account() {
        let dir = TempDir::new().unwrap();
        let policy = LockoutPolicy {
            max_attempts: 3,
            window_secs: 900,
            lockout_secs: 900,
        };
        let mut s = store(&dir, policy);
        s.add_user("bob", "correct horse battery staple", Role::Reader)
            .unwrap();

        assert_eq!(
            s.authenticate("bob", "wrong"),
            AuthOutcome::InvalidCredentials
        );
        assert_eq!(
            s.authenticate("bob", "wrong"),
            AuthOutcome::InvalidCredentials
        );
        assert!(matches!(
            s.authenticate("bob", "wrong"),
            AuthOutcome::Locked { .. }
        ));
        // The correct password does not bypass the lock.
        assert!(matches!(
            s.authenticate("bob", "correct horse battery staple"),
            AuthOutcome::Locked { .. }
        ));

        s.unlock("bob").unwrap();
        assert!(matches!(
            s.authenticate("bob", "correct horse battery staple"),
            AuthOutcome::Success { .. }
        ));
    }

    #[test]
    fn the_store_round_trips_through_disk() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("users.json");
        {
            let mut s = UserStore::open(&path, LockoutPolicy::default()).unwrap();
            s.add_user("carol", "correct horse battery staple", Role::Admin)
                .unwrap();
        }
        let mut reopened = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        assert_eq!(reopened.len(), 1);
        assert!(matches!(
            reopened.authenticate("carol", "correct horse battery staple"),
            AuthOutcome::Success {
                role: Role::Admin,
                ..
            }
        ));
    }

    const PW: &str = "correct horse battery staple";

    /// The running server holds one `UserStore` for its lifetime while the CLI
    /// edits the same file from another process. These two tests are the ones
    /// that matter: before `refresh`, the server neither saw an administrative
    /// change nor left it on disk.
    #[test]
    fn an_account_disabled_from_outside_is_honoured_and_not_erased() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("users.json");

        // The "server": opened once, held across the whole test.
        let mut server = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        server.add_user("root", PW, Role::Admin).unwrap();
        server.add_user("bob", PW, Role::Reader).unwrap();
        assert!(matches!(
            server.authenticate("bob", PW),
            AuthOutcome::Success { .. }
        ));

        // The "CLI": a separate instance over the same file disables bob.
        {
            let mut cli = UserStore::open(&path, LockoutPolicy::default()).unwrap();
            cli.set_disabled("bob", true).unwrap();
        }

        // The server must honour it without being restarted...
        assert_eq!(server.authenticate("bob", PW), AuthOutcome::Disabled);

        // ...and must not have written its stale copy back over it.
        let mut fresh = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        assert_eq!(fresh.authenticate("bob", PW), AuthOutcome::Disabled);
    }

    #[test]
    fn an_account_created_outside_becomes_usable_without_a_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("users.json");

        let mut server = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        server.add_user("root", PW, Role::Admin).unwrap();

        {
            let mut cli = UserStore::open(&path, LockoutPolicy::default()).unwrap();
            cli.add_user("carol", PW, Role::Contributor).unwrap();
        }

        assert!(matches!(
            server.authenticate("carol", PW),
            AuthOutcome::Success {
                role: Role::Contributor,
                ..
            }
        ));
    }

    #[test]
    fn account_state_tracks_outside_disables_demotions_and_removals() {
        // This is what the server calls on every request to decide whether an
        // already-established session is still entitled to anything.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("users.json");

        let mut server = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        server.add_user("root", PW, Role::Admin).unwrap();
        server.add_user("bob", PW, Role::Maintainer).unwrap();
        assert_eq!(server.account_state("bob"), Some((Role::Maintainer, true)));

        {
            let mut cli = UserStore::open(&path, LockoutPolicy::default()).unwrap();
            cli.set_role("bob", Role::Reader).unwrap();
        }
        assert_eq!(
            server.account_state("bob"),
            Some((Role::Reader, true)),
            "a demotion made outside must be visible immediately"
        );

        {
            let mut cli = UserStore::open(&path, LockoutPolicy::default()).unwrap();
            cli.set_disabled("bob", true).unwrap();
        }
        assert_eq!(server.account_state("bob"), Some((Role::Reader, false)));

        {
            let mut cli = UserStore::open(&path, LockoutPolicy::default()).unwrap();
            cli.remove_user("bob").unwrap();
        }
        assert_eq!(server.account_state("bob"), None);
    }

    #[test]
    fn account_state_is_case_insensitive_like_authentication() {
        let dir = TempDir::new().unwrap();
        let mut s = store(&dir, LockoutPolicy::default());
        s.add_user("Alice", PW, Role::Contributor).unwrap();
        assert_eq!(s.account_state("alice"), Some((Role::Contributor, true)));
        assert_eq!(s.account_state("ALICE"), Some((Role::Contributor, true)));
        assert_eq!(s.account_state("nobody"), None);
    }

    #[test]
    fn a_deleted_store_does_not_wipe_the_accounts_in_memory() {
        // Losing the file should not become a way to turn authentication off.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("users.json");
        let mut server = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        server.add_user("root", PW, Role::Admin).unwrap();

        std::fs::remove_file(&path).unwrap();

        assert!(matches!(
            server.authenticate("root", PW),
            AuthOutcome::Success { .. }
        ));
    }

    #[test]
    fn the_plaintext_password_never_reaches_disk() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("users.json");
        let mut s = UserStore::open(&path, LockoutPolicy::default()).unwrap();
        s.add_user("dave", "correct horse battery staple", Role::Reader)
            .unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("correct horse battery staple"));
    }

    #[test]
    fn password_policy_rejects_weak_choices() {
        assert!(UserStore::check_password_policy("alice", "short").is_err());
        assert!(UserStore::check_password_policy("alice", "alice_alice_alice").is_err());
        assert!(UserStore::check_password_policy("alice", "aaaaaaaaaaaaaaaaaa").is_err());
        assert!(UserStore::check_password_policy("alice", "correct horse battery staple").is_ok());
    }

    #[test]
    fn the_last_admin_cannot_be_removed_demoted_or_disabled() {
        let dir = TempDir::new().unwrap();
        let mut s = store(&dir, LockoutPolicy::default());
        s.add_user("root", "correct horse battery staple", Role::Admin)
            .unwrap();
        assert!(s.remove_user("root").is_err());
        assert!(s.set_role("root", Role::Reader).is_err());
        assert!(s.set_disabled("root", true).is_err());

        // With a second admin present, the first may be removed.
        s.add_user("root2", "correct horse battery staple", Role::Admin)
            .unwrap();
        assert!(s.remove_user("root").is_ok());
    }

    #[test]
    fn a_disabled_account_cannot_authenticate() {
        let dir = TempDir::new().unwrap();
        let mut s = store(&dir, LockoutPolicy::default());
        s.add_user("root", "correct horse battery staple", Role::Admin)
            .unwrap();
        s.add_user("eve", "correct horse battery staple", Role::Reader)
            .unwrap();
        s.set_disabled("eve", true).unwrap();
        assert_eq!(
            s.authenticate("eve", "correct horse battery staple"),
            AuthOutcome::Disabled
        );
    }

    #[test]
    fn changing_a_password_clears_a_lockout() {
        let dir = TempDir::new().unwrap();
        let policy = LockoutPolicy {
            max_attempts: 2,
            window_secs: 900,
            lockout_secs: 900,
        };
        let mut s = store(&dir, policy);
        s.add_user("frank", "correct horse battery staple", Role::Reader)
            .unwrap();
        let _ = s.authenticate("frank", "wrong");
        assert!(matches!(
            s.authenticate("frank", "wrong"),
            AuthOutcome::Locked { .. }
        ));
        s.set_password("frank", "a different long passphrase")
            .unwrap();
        assert!(matches!(
            s.authenticate("frank", "a different long passphrase"),
            AuthOutcome::Success { .. }
        ));
    }
}
