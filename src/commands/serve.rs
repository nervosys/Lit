use crate::commands;
use crate::core::{find_repo_root, Object, ObjectHash};
use crate::response::{CommandResponse, ServeResponse};
use crate::storage::ObjectStore;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::net::{IpAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tiny_http::{Header, Method, Response, Server, StatusCode};

/// Maximum request body size (1 MB)
const MAX_BODY_SIZE: usize = 1_048_576;

/// Maximum requests per IP per window
const RATE_LIMIT_MAX_REQUESTS: u32 = 100;

/// Rate limit window duration in seconds
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Per-IP rate limiter using a sliding window counter
pub(crate) struct RateLimiter {
    clients: HashMap<IpAddr, (Instant, u32)>,
}

impl RateLimiter {
    pub(crate) fn new() -> Self {
        RateLimiter {
            clients: HashMap::new(),
        }
    }

    /// Check whether a request from `ip` should be allowed.
    /// Returns `true` if allowed, `false` if rate-limited.
    pub(crate) fn check(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(RATE_LIMIT_WINDOW_SECS);

        let entry = self.clients.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            // Reset window
            *entry = (now, 1);
            true
        } else if entry.1 < RATE_LIMIT_MAX_REQUESTS {
            entry.1 += 1;
            true
        } else {
            false
        }
    }
}

pub fn execute(port: u16, token: Option<String>) -> Result<ServeResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    execute_at(port, token, repo_root)
}

/// Like `execute`, but takes an explicit repo root instead of searching
/// from the current directory. Useful for tests that cannot rely on cwd.
pub fn execute_at(
    port: u16,
    token: Option<String>,
    repo_root: std::path::PathBuf,
) -> Result<ServeResponse, crate::errors::LitError> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&bind_addr)
        .map_err(|e| format!("Failed to start server on {}: {}", bind_addr, e))?;

    eprintln!("Lit API server listening on http://{}", bind_addr);
    eprintln!("Repository: {}", repo_root.display());
    if token.is_some() {
        eprintln!("Authentication: Bearer token required");
    }
    eprintln!("Press Ctrl+C to stop");

    let mut rate_limiter = RateLimiter::new();

    for mut request in server.incoming_requests() {
        // Rate limiting
        if let Some(ip) = request.remote_addr().map(|a| a.ip()) {
            if !rate_limiter.check(ip) {
                let body = r#"{"status":"error","error":{"message":"Rate limit exceeded"}}"#;
                let resp = Response::from_string(body)
                    .with_status_code(StatusCode(429))
                    .with_header(json_content_type());
                let _ = request.respond(resp);
                continue;
            }
        }

        // Authenticate if token is set
        if let Some(ref expected_token) = token {
            let auth_header = request.headers().iter().find(|h| {
                let name = h.field.as_str().to_string();
                name.eq_ignore_ascii_case("authorization")
            });

            let authorized = match auth_header {
                Some(h) => {
                    let val = h.value.as_str();
                    val.starts_with("Bearer ")
                        && subtle::ConstantTimeEq::ct_eq(
                            &val.as_bytes()[7..],
                            expected_token.as_bytes(),
                        )
                        .into()
                }
                None => false,
            };

            if !authorized {
                let body = r#"{"status":"error","error":{"message":"Unauthorized"}}"#;
                let resp = Response::from_string(body)
                    .with_status_code(StatusCode(401))
                    .with_header(json_content_type());
                let _ = request.respond(resp);
                continue;
            }
        }

        let method = request.method().clone();
        let url = request.url().to_string();

        // Read body before routing so request is available for respond
        let body_str = read_body(&mut request).unwrap_or_default();
        let result = route_request(method, &url, &body_str, &repo_root);

        match result {
            Ok((status, body)) => {
                let resp = Response::from_string(body)
                    .with_status_code(StatusCode(status))
                    .with_header(json_content_type());
                let _ = request.respond(resp);
            }
            Err(e) => {
                // SECURITY: Log internal message server-side, return generic message to client
                eprintln!("API error: {}", e.internal_message());
                let body = serde_json::json!({
                    "status": "error",
                    "error": {"message": e.user_message()}
                })
                .to_string();
                let resp = Response::from_string(body)
                    .with_status_code(StatusCode(500))
                    .with_header(json_content_type());
                let _ = request.respond(resp);
            }
        }
    }

    Ok(ServeResponse {
        message: "Server stopped".to_string(),
    })
}

/// Execute the server in stdio pipe mode.
/// Reads newline-delimited JSON requests from stdin, routes them through
/// `route_request`, and writes JSON responses to stdout. Used by SSH transport.
pub fn execute_stdio() -> Result<ServeResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let reader = stdin.lock();
    let mut writer = stdout.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break, // EOF or pipe closed
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp =
                    serde_json::json!({"status": 400, "body": format!("Invalid JSON: {}", e)});
                let _ = writeln!(writer, "{}", resp);
                let _ = writer.flush();
                continue;
            }
        };

        let method_str = req.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
        let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let body = req.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let method = match method_str.to_uppercase().as_str() {
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            _ => Method::Get,
        };

        let (status, response_body) = match route_request(method, path, body, &repo_root) {
            Ok((s, b)) => (s, b),
            Err(e) => {
                // SECURITY: Log internal message server-side, return generic message to client
                eprintln!("Stdio API error: {}", e.internal_message());
                let err_body = serde_json::json!({
                    "status": "error",
                    "error": {"message": e.user_message()}
                })
                .to_string();
                (500, err_body)
            }
        };

        let resp = serde_json::json!({"status": status, "body": response_body});
        let _ = writeln!(writer, "{}", resp);
        let _ = writer.flush();
    }

    Ok(ServeResponse {
        message: "Stdio server stopped".to_string(),
    })
}

/// Execute the server as a lit:// protocol TCP daemon.
/// Accepts TCP connections and handles each with the same newline-delimited
/// JSON protocol as stdio mode. Used by the `lit://` native transport.
pub fn execute_daemon(port: u16) -> Result<ServeResponse, crate::errors::LitError> {
    let repo_root = find_repo_root()?;
    // SECURITY: Bind to localhost only — network exposure requires explicit reverse proxy
    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&bind_addr)
        .map_err(|e| format!("Failed to bind lit:// daemon on {}: {}", bind_addr, e))?;

    eprintln!("Lit daemon listening on lit://127.0.0.1:{}", port);
    eprintln!("Repository: {}", repo_root.display());
    eprintln!("Press Ctrl+C to stop");

    let rate_limiter = Arc::new(Mutex::new(RateLimiter::new()));

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Accept error: {}", e);
                continue;
            }
        };

        // Rate limit per peer IP
        if let Ok(addr) = stream.peer_addr() {
            if let Ok(mut rl) = rate_limiter.lock() {
                if !rl.check(addr.ip()) {
                    // Silently drop over-limit connections
                    continue;
                }
            }
        }

        let repo = repo_root.clone();
        std::thread::spawn(move || {
            handle_daemon_connection(stream, &repo);
        });
    }

    Ok(ServeResponse {
        message: "Daemon stopped".to_string(),
    })
}

/// Handle a single lit:// daemon TCP connection
fn handle_daemon_connection(stream: std::net::TcpStream, repo_root: &std::path::Path) {
    let reader_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let reader = std::io::BufReader::new(reader_stream);
    let mut writer = std::io::BufWriter::new(stream);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp =
                    serde_json::json!({"status": 400, "body": format!("Invalid JSON: {}", e)});
                let _ = writeln!(writer, "{}", resp);
                let _ = writer.flush();
                continue;
            }
        };

        let method_str = req.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
        let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let body = req.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let method = match method_str.to_uppercase().as_str() {
            "GET" => Method::Get,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "DELETE" => Method::Delete,
            _ => Method::Get,
        };

        let (status, response_body) = match route_request(method, path, body, repo_root) {
            Ok((s, b)) => (s, b),
            Err(e) => {
                let err_body = serde_json::json!({
                    "status": "error",
                    "error": {"message": e.internal_message()}
                })
                .to_string();
                (500, err_body)
            }
        };

        let resp = serde_json::json!({"status": status, "body": response_body});
        if writeln!(writer, "{}", resp).is_err() {
            break;
        }
        if writer.flush().is_err() {
            break;
        }
    }
}

fn json_content_type() -> Header {
    Header::from_bytes("Content-Type", "application/json").unwrap()
}

fn read_body(request: &mut tiny_http::Request) -> Result<String, crate::errors::LitError> {
    let content_length = request.body_length().unwrap_or(0);
    if content_length > MAX_BODY_SIZE {
        return Err("Request body too large".into());
    }
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .map_err(|e| format!("Failed to read request body: {}", e))?;
    Ok(body)
}

fn route_request(
    method: Method,
    url: &str,
    body: &str,
    repo_root: &std::path::Path,
) -> Result<(u16, String), crate::errors::LitError> {
    let path = url.split('?').next().unwrap_or(url);

    match (method, path) {
        // Discovery
        (Method::Get, "/api/v1") | (Method::Get, "/api/v1/") => {
            let info = serde_json::json!({
                "name": "lit",
                "version": env!("CARGO_PKG_VERSION"),
                "api_version": "v1",
                "endpoints": [
                    "GET  /api/v1/status",
                    "GET  /api/v1/log?count=N",
                    "GET  /api/v1/branches",
                    "GET  /api/v1/diff?staged=bool",
                    "GET  /api/v1/show/:ref",
                    "GET  /api/v1/tags",
                    "GET  /api/v1/remotes",
                    "GET  /api/v1/config",
                    "GET  /api/v1/search?q=query&messages=bool",
                    "GET  /api/v1/verify",
                    "GET  /api/v1/ontology",
                    "POST /api/v1/add",
                    "POST /api/v1/commit",
                    "POST /api/v1/snapshot",
                    "POST /api/v1/checkout",
                    "POST /api/v1/merge",
                    "POST /api/v1/branch",
                ]
            });
            Ok((200, serde_json::to_string_pretty(&info).unwrap()))
        }

        // GET endpoints
        (Method::Get, "/api/v1/status") => {
            let resp = commands::status::execute()?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/log") => {
            let count = parse_query_param(url, "count")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(10);
            let oneline = parse_query_param(url, "oneline")
                .map(|s| s == "true")
                .unwrap_or(false);
            let resp = commands::log::execute(count, oneline)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/branches") => {
            let resp = commands::branch::execute(None, false, true)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/diff") => {
            let staged = parse_query_param(url, "staged")
                .map(|s| s == "true")
                .unwrap_or(false);
            let stat = parse_query_param(url, "stat")
                .map(|s| s == "true")
                .unwrap_or(false);
            let resp = commands::diff::execute(staged, stat, false, None, None)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, p) if p.starts_with("/api/v1/show/") => {
            let object = &p["/api/v1/show/".len()..];
            if object.is_empty() {
                return Ok((400, r#"{"status":"error","error":{"message":"Missing object ref"}}"#.to_string()));
            }
            if !is_valid_ref(object) {
                return Ok((400, r#"{"status":"error","error":{"message":"Invalid object ref"}}"#.to_string()));
            }
            let resp = commands::show::execute(object.to_string())?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/tags") => {
            let resp = commands::tag::execute(None, None, false, false, false, false, true, None)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/remotes") => {
            let resp = commands::remote::execute(Some(crate::RemoteCommands::List { verbose: true }))?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/config") => {
            let resp = commands::config::execute(Some(crate::ConfigCommands::Show))?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/search") => {
            let query = parse_query_param(url, "q").unwrap_or_default();
            if query.is_empty() {
                return Ok((400, r#"{"status":"error","error":{"message":"Missing query parameter 'q'"}}"#.to_string()));
            }
            let messages = parse_query_param(url, "messages")
                .map(|s| s == "true")
                .unwrap_or(false);
            let metadata = parse_query_param(url, "metadata");
            let max = parse_query_param(url, "max")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(100);
            let resp = commands::search::execute(query, messages, metadata, max)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/verify") => {
            let resp = commands::verify::execute()?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Get, "/api/v1/ontology") => {
            let resp = crate::ontology::get_ontology();
            Ok((200, serde_json::to_string_pretty(&resp).unwrap()))
        }

        // POST endpoints
        (Method::Post, "/api/v1/add") => {
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let files: Vec<String> = payload
                .get("files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if files.is_empty() {
                return Ok((400, r#"{"status":"error","error":{"message":"Missing 'files' array"}}"#.to_string()));
            }
            let resp = commands::add::execute(files)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Post, "/api/v1/commit") => {
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let message = payload
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'message' field")?
                .to_string();
            let author = payload
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::commit::execute(message, author)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Post, "/api/v1/snapshot") => {
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let message = payload
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'message' field")?
                .to_string();
            let author = payload
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let metadata = payload.get("metadata").cloned();
            let resp = commands::snapshot::execute(message, author, metadata)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Post, "/api/v1/checkout") => {
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let target = payload
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'target' field")?
                .to_string();
            let create = payload
                .get("create")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = commands::checkout::execute(target, create)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Post, "/api/v1/merge") => {
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let branch = payload
                .get("branch")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'branch' field")?
                .to_string();
            let strategy = payload
                .get("strategy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::merge::execute(branch, strategy)?;
            Ok((200, resp.to_json_output()))
        }

        (Method::Post, "/api/v1/branch") => {
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let delete = payload
                .get("delete")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = commands::branch::execute(name, delete, false)?;
            Ok((200, resp.to_json_output()))
        }

        // ── Transport API endpoints ──
        // List refs (branches + tags)
        (Method::Get, "/api/v1/transport/refs") => {
            let kind = parse_query_param(url, "kind").unwrap_or_else(|| "all".to_string());
            let mut refs = Vec::new();
            if kind == "all" || kind == "heads" {
                if let Ok(head_refs) = crate::core::refs::list_refs(repo_root, "heads") {
                    for r in head_refs {
                        refs.push(serde_json::json!({"kind": "heads", "name": r.name, "hash": r.hash}));
                    }
                }
            }
            if kind == "all" || kind == "tags" {
                if let Ok(tag_refs) = crate::core::refs::list_refs(repo_root, "tags") {
                    for r in tag_refs {
                        refs.push(serde_json::json!({"kind": "tags", "name": r.name, "hash": r.hash}));
                    }
                }
            }
            Ok((200, serde_json::json!({"refs": refs}).to_string()))
        }

        // Read HEAD
        (Method::Get, "/api/v1/transport/head") => {
            let head = std::fs::read_to_string(repo_root.join(".lit").join("HEAD"))
                .map_err(|e| format!("Failed to read HEAD: {}", e))?;
            Ok((200, serde_json::json!({"head": head.trim()}).to_string()))
        }

        // Read a specific ref
        (Method::Get, p) if p.starts_with("/api/v1/transport/refs/heads/") => {
            let branch = &p["/api/v1/transport/refs/heads/".len()..];
            if !is_valid_ref(branch) {
                return Ok((400, r#"{"status":"error","error":{"message":"Invalid branch name"}}"#.to_string()));
            }
            let hash = crate::core::refs::read_ref(repo_root, &format!("heads/{}", branch))?;
            Ok((200, serde_json::json!({"branch": branch, "hash": hash}).to_string()))
        }

        // Check if object exists
        (Method::Get, p) if p.starts_with("/api/v1/transport/objects/") && p.ends_with("/exists") => {
            let hash_str = &p["/api/v1/transport/objects/".len()..p.len() - "/exists".len()];
            if !is_valid_hex_hash(hash_str) {
                return Ok((400, r#"{"status":"error","error":{"message":"Invalid object hash"}}"#.to_string()));
            }
            let store = ObjectStore::new(repo_root);
            let exists = store.exists(&ObjectHash::from_hex(hash_str.to_string()));
            Ok((200, serde_json::json!({"hash": hash_str, "exists": exists}).to_string()))
        }

        // Download a single object (serialized bytes, base64 encoded)
        (Method::Get, p) if p.starts_with("/api/v1/transport/objects/") => {
            let hash_str = &p["/api/v1/transport/objects/".len()..];
            if !is_valid_hex_hash(hash_str) {
                return Ok((400, r#"{"status":"error","error":{"message":"Invalid object hash"}}"#.to_string()));
            }
            let store = ObjectStore::new(repo_root);
            let hash = ObjectHash::from_hex(hash_str.to_string());
            let obj = store.read(&hash)?;
            let data = obj.to_bytes();
            use std::io::Write as _;
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(&data).map_err(|e| format!("Compress error: {}", e))?;
            let compressed = encoder.finish().map_err(|e| format!("Compress error: {}", e))?;
            let b64 = base64_encode(&compressed);
            Ok((200, serde_json::json!({"hash": hash_str, "data": b64}).to_string()))
        }

        // Batch upload objects
        (Method::Post, "/api/v1/transport/objects") => {
            let store = ObjectStore::new(repo_root);
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let objects = payload.get("objects")
                .and_then(|v| v.as_array())
                .ok_or("Missing 'objects' array")?;
            let mut written = 0;
            for entry in objects {
                let b64_data = entry.get("data")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'data' field in object entry")?;
                let compressed = base64_decode(b64_data)?;
                use std::io::Read as _;
                let mut decoder = flate2::read::ZlibDecoder::new(&compressed[..]);
                let mut raw = Vec::new();
                decoder.read_to_end(&mut raw)
                    .map_err(|e| format!("Decompress error: {}", e))?;
                let obj = Object::from_bytes(&raw)?;
                store.write(&obj)?;
                written += 1;
            }
            Ok((200, serde_json::json!({"written": written}).to_string()))
        }

        // Update a branch ref
        (Method::Put, p) if p.starts_with("/api/v1/transport/refs/heads/") => {
            let branch = &p["/api/v1/transport/refs/heads/".len()..];
            if !is_valid_ref(branch) {
                return Ok((400, r#"{"status":"error","error":{"message":"Invalid branch name"}}"#.to_string()));
            }
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let hash = payload.get("hash")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'hash' field")?;
            let force = payload.get("force")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Fast-forward check unless force
            if !force {
                if let Ok(current) = crate::core::refs::read_ref(repo_root, &format!("heads/{}", branch)) {
                    let store = ObjectStore::new(repo_root);
                    let old_hash = ObjectHash::from_hex(current);
                    let new_hash = ObjectHash::from_hex(hash.to_string());
                    let is_ff = crate::core::merge::is_ancestor(&store, &old_hash, &new_hash)?;
                    if !is_ff {
                        return Ok((409, serde_json::json!({
                            "status": "error",
                            "error": {"message": "Non-fast-forward update rejected. Use force=true."}
                        }).to_string()));
                    }
                }
            }

            crate::core::refs::write_ref(repo_root, &format!("heads/{}", branch), hash)?;
            Ok((200, serde_json::json!({"branch": branch, "hash": hash, "updated": true}).to_string()))
        }

        // Server-side graph walk (negotiate)
        (Method::Post, "/api/v1/transport/negotiate") => {
            let store = ObjectStore::new(repo_root);
            let payload: serde_json::Value = serde_json::from_str(body)
                .map_err(|e| format!("Invalid JSON: {}", e))?;
            let wants: Vec<String> = payload.get("wants")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let haves: Vec<String> = payload.get("haves")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let known: HashSet<String> = haves.into_iter().collect();
            let mut all_needed = Vec::new();
            for want in &wants {
                let hash = ObjectHash::from_hex(want.clone());
                let needed = crate::network::transport::walk_commit_graph(&store, &hash, &known)?;
                for h in needed {
                    let s = h.as_str().to_string();
                    if !all_needed.contains(&s) {
                        all_needed.push(s);
                    }
                }
            }
            Ok((200, serde_json::json!({"needed": all_needed}).to_string()))
        }

        _ => Ok((
            404,
            r#"{"status":"error","error":{"message":"Not found. GET /api/v1 for available endpoints."}}"#
                .to_string(),
        )),
    }
}

fn url_decode(s: &str) -> String {
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                result.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            result.push(b' ');
        } else {
            result.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(result).unwrap_or_default()
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k == key {
                return Some(url_decode(v));
            }
        }
    }
    None
}

/// Validate a ref name (branch, tag) — alphanumeric, hyphens, underscores, dots, slashes.
/// Rejects empty strings, leading/trailing slashes, double dots, and path traversal.
fn is_valid_ref(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 {
        return false;
    }
    if name.contains("..") || name.contains("//") || name.starts_with('/') || name.ends_with('/') {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/')
}

/// Validate a hex-encoded object hash (up to 192 hex characters for SHA3-512+BLAKE3 composite).
fn is_valid_hex_hash(s: &str) -> bool {
    !s.is_empty() && s.len() <= 192 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Base64 encode bytes (standard alphabet, no padding)
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Base64 decode string to bytes
fn base64_decode(input: &str) -> Result<Vec<u8>, crate::errors::LitError> {
    fn val(c: u8) -> Result<u32, crate::errors::LitError> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("Invalid base64 character: {}", c as char).into()),
        }
    }
    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            return Err("Invalid base64 length".into());
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = val(chunk[2])?;
        let d = val(chunk[3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}
