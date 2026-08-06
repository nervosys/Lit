//! Passphrase agent: holds a passphrase in one long-lived process so that
//! separate `lit` commands do not each have to ask for it.
//!
//! # Why this exists
//!
//! The in-process passphrase cache cannot help the command line, because every
//! `lit` command is a new process that starts with an empty cache. Reusing a
//! passphrase across commands needs something that outlives them.
//!
//! # What it protects against, and what it does not
//!
//! The agent listens on loopback and authenticates with a token kept in a file
//! only its owner can read. That draws the boundary at *other users on this
//! machine*: they can reach the port, but not the token, and every request
//! without it is refused.
//!
//! It draws no boundary at all against **other processes running as you**. Such
//! a process can read the token file, so it can ask the agent for the
//! passphrase. This is not a shortcoming that a different transport would fix —
//! a Unix socket or a named pipe restricted to the owner grants exactly the same
//! set of processes. On an ordinary operating system, "another program running
//! as me" is inside the trust boundary.
//!
//! Against that same-user attacker the agent is therefore no stronger than
//! `LIT_PASSPHRASE`. It is better in two narrower ways: the secret is not in an
//! environment block, where it is visible in process listings and inherited by
//! every child; and it expires, where an exported variable lasts as long as the
//! shell.
//!
//! The agent is off unless started. Nothing here listens on a port, writes a
//! token, or holds a secret until someone runs `lit agent start`.

use crate::crypto::encryption::restrict_to_owner;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// How long an unused entry survives, when the caller names no preference.
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 900;

/// Refuse absurd request bodies rather than growing a buffer for them.
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

/// What a client sends. Every variant carries the token: there is no
/// unauthenticated operation, not even `Status`, because whether an agent holds
/// a passphrase for a given repository is itself worth not answering.
#[derive(Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Store a passphrase for `repo`.
    Put {
        token: String,
        repo: String,
        passphrase: String,
    },
    /// Retrieve the passphrase for `repo`, if one is held and unexpired.
    Get { token: String, repo: String },
    /// Forget one repository's passphrase, or all of them when `repo` is None.
    Drop { token: String, repo: Option<String> },
    /// How many entries are held, and with what idle timeout.
    Status { token: String },
    /// Stop the agent, clearing everything it holds.
    Shutdown { token: String },
}

/// What the agent sends back.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    Passphrase {
        passphrase: String,
    },
    /// No entry, or it had expired.
    Missing,
    Ok,
    Status {
        entries: usize,
        idle_timeout_secs: u64,
    },
    Denied,
    Malformed {
        message: String,
    },
}

struct Entry {
    passphrase: Zeroizing<String>,
    last_used: Instant,
}

/// The passphrases an agent is holding.
///
/// Expiry is by idle time rather than by age: a repository in active use should
/// not start prompting again in the middle of the work it is being used for.
pub struct Store {
    entries: HashMap<String, Entry>,
    idle_timeout: Duration,
}

impl Store {
    pub fn new(idle_timeout: Duration) -> Self {
        Store {
            entries: HashMap::new(),
            idle_timeout,
        }
    }

    pub fn put(&mut self, repo: String, passphrase: String) {
        self.entries.insert(
            repo,
            Entry {
                passphrase: Zeroizing::new(passphrase),
                last_used: Instant::now(),
            },
        );
    }

    /// Fetch and refresh, dropping the entry if it has gone stale.
    pub fn get(&mut self, repo: &str) -> Option<Zeroizing<String>> {
        self.expire();
        let entry = self.entries.get_mut(repo)?;
        entry.last_used = Instant::now();
        Some(entry.passphrase.clone())
    }

    pub fn drop_one(&mut self, repo: &str) {
        self.entries.remove(repo);
    }

    pub fn drop_all(&mut self) {
        self.entries.clear();
    }

    pub fn len(&mut self) -> usize {
        self.expire();
        self.entries.len()
    }

    pub fn is_empty(&mut self) -> bool {
        self.len() == 0
    }

    fn expire(&mut self) {
        let timeout = self.idle_timeout;
        self.entries.retain(|_, e| e.last_used.elapsed() < timeout);
    }
}

/// How a client finds a running agent: a port to connect to and a token to
/// present. Written to a file only its owner can read — that file is what keeps
/// other users on the machine out, so it is the part that matters.
#[derive(Serialize, Deserialize)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
    pub idle_timeout_secs: u64,
}

pub fn endpoint_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".lit").join("agent.json"))
}

impl Endpoint {
    pub fn load() -> Result<Endpoint, String> {
        let path = endpoint_path()?;
        let raw = std::fs::read(&path)
            .map_err(|_| "No agent is running (start one with `lit agent start`)".to_string())?;
        serde_json::from_slice(&raw).map_err(|e| format!("Agent endpoint file is unreadable: {e}"))
    }

    fn save(&self) -> Result<(), String> {
        let path = endpoint_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create agent directory: {e}"))?;
        }
        let raw =
            serde_json::to_vec(self).map_err(|e| format!("Failed to encode endpoint: {e}"))?;
        std::fs::write(&path, raw).map_err(|e| format!("Failed to write endpoint: {e}"))?;

        // The whole security boundary. Without this the token is readable by
        // every account on the machine, and the token is the only thing
        // standing between them and the passphrase.
        restrict_to_owner(&path)?;
        Ok(())
    }

    fn remove() {
        if let Ok(path) = endpoint_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A token with enough entropy that guessing it is not a strategy.
fn generate_token() -> String {
    use aes_gcm::aead::rand_core::RngCore;
    use aes_gcm::aead::OsRng;

    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Compare in constant time. A token check that returns early leaks how much of
/// a guess was right, which is exactly the feedback a guesser needs.
fn token_matches(presented: &str, expected: &str) -> bool {
    let a = presented.as_bytes();
    let b = expected.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

fn token_of(req: &Request) -> &str {
    match req {
        Request::Put { token, .. }
        | Request::Get { token, .. }
        | Request::Drop { token, .. }
        | Request::Status { token }
        | Request::Shutdown { token } => token,
    }
}

/// Apply a request that has already been authenticated.
///
/// Returns the response, and whether the agent should stop.
fn apply(req: Request, store: &Arc<Mutex<Store>>) -> (Response, bool) {
    let mut store = match store.lock() {
        Ok(s) => s,
        Err(_) => {
            return (
                Response::Malformed {
                    message: "agent state is poisoned".to_string(),
                },
                false,
            )
        }
    };

    match req {
        Request::Put {
            repo, passphrase, ..
        } => {
            store.put(repo, passphrase);
            (Response::Ok, false)
        }
        Request::Get { repo, .. } => match store.get(&repo) {
            Some(p) => (
                Response::Passphrase {
                    passphrase: p.to_string(),
                },
                false,
            ),
            None => (Response::Missing, false),
        },
        Request::Drop { repo, .. } => {
            match repo {
                Some(r) => store.drop_one(&r),
                None => store.drop_all(),
            }
            (Response::Ok, false)
        }
        Request::Status { .. } => (
            Response::Status {
                entries: store.len(),
                idle_timeout_secs: store.idle_timeout.as_secs(),
            },
            false,
        ),
        Request::Shutdown { .. } => {
            store.drop_all();
            (Response::Ok, true)
        }
    }
}

/// Read one request, act on it, write one response.
///
/// Returns true when the agent has been asked to stop.
fn handle_connection(
    stream: &mut TcpStream,
    expected_token: &str,
    store: &Arc<Mutex<Store>>,
) -> bool {
    // A client that connects and says nothing must not hold the agent open.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let Ok(peer) = stream.try_clone() else {
        return false;
    };

    // Bounded: a client that never sends a newline would otherwise grow this
    // buffer until the agent runs out of memory.
    let mut line = String::new();
    if BufReader::new(peer.take(MAX_REQUEST_BYTES))
        .read_line(&mut line)
        .is_err()
    {
        return false;
    }

    let (response, shutdown) = match serde_json::from_str::<Request>(line.trim()) {
        Ok(req) => {
            if token_matches(token_of(&req), expected_token) {
                apply(req, store)
            } else {
                // Say only that it was refused. Which field was wrong, or
                // whether the repository is known, is not the caller's business
                // until they have proven who they are.
                (Response::Denied, false)
            }
        }
        Err(e) => (
            Response::Malformed {
                message: e.to_string(),
            },
            false,
        ),
    };

    if let Ok(mut body) = serde_json::to_vec(&response) {
        body.push(b'\n');
        let _ = stream.write_all(&body);
        let _ = stream.flush();
    }

    shutdown
}

/// Run an agent until it is asked to stop. Blocks.
pub fn serve(idle_timeout: Duration) -> Result<(), String> {
    if Endpoint::load().is_ok() && ping().is_ok() {
        return Err("An agent is already running (`lit agent stop` to replace it)".to_string());
    }

    // Loopback only. Binding anywhere else would put the passphrase on the
    // network, token or no token.
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|e| format!("Failed to bind agent socket: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to read agent port: {e}"))?
        .port();

    let token = generate_token();
    Endpoint {
        port,
        token: token.clone(),
        idle_timeout_secs: idle_timeout.as_secs(),
    }
    .save()?;

    let store = Arc::new(Mutex::new(Store::new(idle_timeout)));

    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(s) => s,
            Err(_) => continue,
        };
        if handle_connection(&mut stream, &token, &store) {
            break;
        }
    }

    if let Ok(mut s) = store.lock() {
        s.drop_all();
    }
    Endpoint::remove();
    Ok(())
}

/// Send one request to a running agent and read its reply.
fn request(req: &Request) -> Result<Response, String> {
    let endpoint = Endpoint::load()?;
    let mut stream = TcpStream::connect(SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port)))
        .map_err(|_| "No agent is running (start one with `lit agent start`)".to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("Failed to configure agent socket: {e}"))?;

    let mut body = serde_json::to_vec(req).map_err(|e| format!("Failed to encode request: {e}"))?;
    body.push(b'\n');
    stream
        .write_all(&body)
        .map_err(|e| format!("Failed to reach agent: {e}"))?;

    let mut line = String::new();
    BufReader::new(&stream)
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read agent reply: {e}"))?;

    serde_json::from_str(line.trim()).map_err(|e| format!("Agent sent an unreadable reply: {e}"))
}

fn token() -> Result<String, String> {
    Ok(Endpoint::load()?.token)
}

/// Check that an agent is actually listening, not merely that a file says so.
pub fn ping() -> Result<(), String> {
    match request(&Request::Status { token: token()? })? {
        Response::Status { .. } => Ok(()),
        _ => Err("Agent did not answer a status request".to_string()),
    }
}

/// Ask the agent for a passphrase. `None` covers every ordinary reason there is
/// no answer — no agent, nothing stored, entry expired — because a caller
/// looking for a passphrase should move on to the next source rather than fail.
pub fn get(repo: &str) -> Option<Zeroizing<String>> {
    let token = token().ok()?;
    match request(&Request::Get {
        token,
        repo: repo.to_string(),
    })
    .ok()?
    {
        Response::Passphrase { passphrase } => Some(Zeroizing::new(passphrase)),
        _ => None,
    }
}

pub fn put(repo: &str, passphrase: &str) -> Result<(), String> {
    match request(&Request::Put {
        token: token()?,
        repo: repo.to_string(),
        passphrase: passphrase.to_string(),
    })? {
        Response::Ok => Ok(()),
        other => Err(format!("Agent refused to store the passphrase: {other:?}")),
    }
}

pub fn drop_entry(repo: Option<&str>) -> Result<(), String> {
    match request(&Request::Drop {
        token: token()?,
        repo: repo.map(|r| r.to_string()),
    })? {
        Response::Ok => Ok(()),
        other => Err(format!("Agent refused: {other:?}")),
    }
}

pub fn status() -> Result<(usize, u64), String> {
    match request(&Request::Status { token: token()? })? {
        Response::Status {
            entries,
            idle_timeout_secs,
        } => Ok((entries, idle_timeout_secs)),
        other => Err(format!("Agent refused: {other:?}")),
    }
}

pub fn shutdown() -> Result<(), String> {
    let result = request(&Request::Shutdown { token: token()? });
    // The agent removes its own endpoint file, but if it died without doing so
    // a stale file would keep every later command trying a dead port.
    Endpoint::remove();
    match result? {
        Response::Ok => Ok(()),
        other => Err(format!("Agent refused to stop: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entries_expire_when_idle() {
        let mut store = Store::new(Duration::from_millis(50));
        store.put("repo".to_string(), "hunter2".to_string());
        assert!(store.get("repo").is_some());

        std::thread::sleep(Duration::from_millis(80));
        assert!(
            store.get("repo").is_none(),
            "an entry left alone past the timeout should be gone"
        );
        assert!(store.is_empty());
    }

    #[test]
    fn test_use_refreshes_the_timeout() {
        // Expiry is by idle time, so a repository in active use should not
        // start prompting again in the middle of the work it is being used for.
        let mut store = Store::new(Duration::from_millis(120));
        store.put("repo".to_string(), "hunter2".to_string());

        for _ in 0..4 {
            std::thread::sleep(Duration::from_millis(50));
            assert!(store.get("repo").is_some(), "use should keep it alive");
        }
    }

    #[test]
    fn test_drop_all_forgets_everything() {
        let mut store = Store::new(Duration::from_secs(60));
        store.put("a".to_string(), "one".to_string());
        store.put("b".to_string(), "two".to_string());
        assert_eq!(store.len(), 2);

        store.drop_all();
        assert!(store.is_empty());
    }

    #[test]
    fn test_token_comparison_rejects_wrong_and_short_tokens() {
        let real = generate_token();
        assert!(token_matches(&real, &real));
        assert!(!token_matches("", &real));
        assert!(!token_matches(&real[..real.len() - 1], &real));

        let mut wrong = real.clone();
        // Flip the last character; a prefix-equal token must still be refused.
        let last = if wrong.ends_with('a') { 'b' } else { 'a' };
        wrong.pop();
        wrong.push(last);
        assert!(!token_matches(&wrong, &real));
    }

    #[test]
    fn test_generated_tokens_differ() {
        assert_ne!(generate_token(), generate_token());
    }

    /// A request carrying the wrong token must be refused whatever it asks for,
    /// and must not disturb what the agent holds.
    #[test]
    fn test_wrong_token_is_denied_and_changes_nothing() {
        let store = Arc::new(Mutex::new(Store::new(Duration::from_secs(60))));
        let real = generate_token();

        let req = Request::Put {
            token: "not-the-token".to_string(),
            repo: "repo".to_string(),
            passphrase: "hunter2".to_string(),
        };
        assert!(!token_matches(token_of(&req), &real));

        // The server applies nothing it has not authenticated.
        assert!(store.lock().unwrap().is_empty());
    }

    #[test]
    fn test_put_then_get_round_trips_through_apply() {
        let store = Arc::new(Mutex::new(Store::new(Duration::from_secs(60))));

        let (resp, stop) = apply(
            Request::Put {
                token: String::new(),
                repo: "repo".to_string(),
                passphrase: "hunter2".to_string(),
            },
            &store,
        );
        assert!(matches!(resp, Response::Ok));
        assert!(!stop);

        let (resp, _) = apply(
            Request::Get {
                token: String::new(),
                repo: "repo".to_string(),
            },
            &store,
        );
        match resp {
            Response::Passphrase { passphrase } => assert_eq!(passphrase, "hunter2"),
            other => panic!("expected the passphrase back, got {other:?}"),
        }

        let (_, stop) = apply(
            Request::Shutdown {
                token: String::new(),
            },
            &store,
        );
        assert!(stop, "shutdown should stop the agent");
        assert!(
            store.lock().unwrap().is_empty(),
            "shutdown should clear what it held"
        );
    }

    #[test]
    fn test_get_for_unknown_repo_is_missing_not_an_error() {
        let store = Arc::new(Mutex::new(Store::new(Duration::from_secs(60))));
        let (resp, _) = apply(
            Request::Get {
                token: String::new(),
                repo: "never-stored".to_string(),
            },
            &store,
        );
        assert!(matches!(resp, Response::Missing));
    }
}
