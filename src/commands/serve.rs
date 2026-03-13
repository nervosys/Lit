use crate::commands;
use crate::core::find_repo_root;
use crate::response::{CommandResponse, ServeResponse};
use tiny_http::{Header, Method, Response, Server, StatusCode};

/// Maximum request body size (1 MB)
const MAX_BODY_SIZE: usize = 1_048_576;

pub fn execute(port: u16, token: Option<String>) -> Result<ServeResponse, String> {
    let repo_root = find_repo_root()?;

    let bind_addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&bind_addr)
        .map_err(|e| format!("Failed to start server on {}: {}", bind_addr, e))?;

    eprintln!("Lit API server listening on http://{}", bind_addr);
    eprintln!("Repository: {}", repo_root.display());
    if token.is_some() {
        eprintln!("Authentication: Bearer token required");
    }
    eprintln!("Press Ctrl+C to stop");

    for mut request in server.incoming_requests() {
        // Authenticate if token is set
        if let Some(ref expected_token) = token {
            let auth_header = request
                .headers()
                .iter()
                .find(|h| {
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
        let result = route_request(method, &url, &body_str);

        match result {
            Ok((status, body)) => {
                let resp = Response::from_string(body)
                    .with_status_code(StatusCode(status))
                    .with_header(json_content_type());
                let _ = request.respond(resp);
            }
            Err(e) => {
                let body = serde_json::json!({
                    "status": "error",
                    "error": {"message": e}
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

fn json_content_type() -> Header {
    Header::from_bytes("Content-Type", "application/json").unwrap()
}

fn read_body(request: &mut tiny_http::Request) -> Result<String, String> {
    let content_length = request.body_length().unwrap_or(0);
    if content_length > MAX_BODY_SIZE {
        return Err("Request body too large".to_string());
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
) -> Result<(u16, String), String> {
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
            if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
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
