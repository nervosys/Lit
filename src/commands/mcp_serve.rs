use crate::commands;
use crate::commands::serve::RateLimiter;
use crate::ontology;
use crate::response::McpServeResponse;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Read, Write};

const MAX_BODY_SIZE: usize = 1_048_576; // 1 MB

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

/// MCP tool definition
#[derive(Debug, Serialize)]
struct McpTool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

/// Run MCP server over stdio (JSON-RPC 2.0)
pub fn execute_stdio() -> Result<McpServeResponse, crate::errors::LitError> {
    let stdin = io::stdin();
    let stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                write_response(&stdout, &resp);
                continue;
            }
        };

        if request.jsonrpc != "2.0" {
            let resp = JsonRpcResponse {
                jsonrpc: "2.0",
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32600,
                    message: "Invalid Request: jsonrpc must be '2.0'".to_string(),
                    data: None,
                }),
            };
            write_response(&stdout, &resp);
            continue;
        }

        let response = handle_mcp_method(&request);
        write_response(&stdout, &response);
    }

    Ok(McpServeResponse {
        transport: "stdio".to_string(),
        message: "MCP server stopped".to_string(),
    })
}

/// Run MCP server over HTTP (uses tiny_http)
/// SECURITY: Binds to 127.0.0.1 only — localhost binding provides implicit
/// authentication since only local processes can connect. For remote access,
/// use a reverse proxy with proper authentication.
pub fn execute_http(port: u16) -> Result<McpServeResponse, crate::errors::LitError> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let server = tiny_http::Server::http(&bind_addr)
        .map_err(|e| format!("Failed to start MCP HTTP server on {}: {}", bind_addr, e))?;

    eprintln!("Lit MCP server (HTTP) on http://{}", bind_addr);
    eprintln!("Press Ctrl+C to stop");

    let mut rate_limiter = RateLimiter::new();

    for mut request in server.incoming_requests() {
        // Rate limiting
        if let Some(ip) = request.remote_addr().map(|a| a.ip()) {
            if !rate_limiter.check(ip) {
                let _ = request.respond(tiny_http::Response::from_string(
                    r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32000,"message":"Rate limit exceeded"}}"#,
                ));
                continue;
            }
        }

        let content_length = request.body_length().unwrap_or(0);
        if content_length > MAX_BODY_SIZE {
            let _ = request.respond(tiny_http::Response::from_string(
                r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Request body too large"}}"#,
            ));
            continue;
        }

        let mut body = String::new();
        if request
            .as_reader()
            .take(MAX_BODY_SIZE as u64)
            .read_to_string(&mut body)
            .is_err()
        {
            continue;
        }

        let rpc_req: JsonRpcRequest = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let json = serde_json::to_string(&resp).unwrap_or_default();
                let http_resp = tiny_http::Response::from_string(json).with_header(
                    tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
                );
                let _ = request.respond(http_resp);
                continue;
            }
        };

        let rpc_resp = handle_mcp_method(&rpc_req);
        let json = serde_json::to_string(&rpc_resp).unwrap_or_default();
        let http_resp = tiny_http::Response::from_string(json).with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json").unwrap(),
        );
        let _ = request.respond(http_resp);
    }

    Ok(McpServeResponse {
        transport: "http".to_string(),
        message: "MCP server stopped".to_string(),
    })
}

fn write_response(stdout: &io::Stdout, resp: &JsonRpcResponse) {
    let json = serde_json::to_string(resp).unwrap_or_default();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{}", json);
    let _ = out.flush();
}

fn handle_mcp_method(req: &JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        // MCP initialization
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false }
                },
                "serverInfo": {
                    "name": "lit-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        },

        "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(serde_json::json!({})),
            error: None,
        },

        // List available tools
        "tools/list" => {
            let tools = get_mcp_tools();
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: req.id.clone(),
                result: Some(serde_json::json!({ "tools": tools })),
                error: None,
            }
        }

        // Call a tool
        "tools/call" => {
            let tool_name = req
                .params
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = req
                .params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::json!({}));

            match call_tool(tool_name, &arguments) {
                Ok(result) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: req.id.clone(),
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&result).unwrap_or_default()
                        }]
                    })),
                    error: None,
                },
                Err(e) => {
                    // SECURITY: Log internal details server-side, return sanitized message to client (FINDING-002)
                    eprintln!("MCP tools/call error: {}", e.internal_message());
                    JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: req.id.clone(),
                        result: Some(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": e.user_message().to_string()
                            }],
                            "isError": true
                        })),
                        error: None,
                    }
                }
            }
        }

        // List resources
        "resources/list" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(serde_json::json!({
                "resources": [
                    {
                        "uri": "lit://status",
                        "name": "Repository Status",
                        "description": "Current repository status including branch, staged, modified, and untracked files",
                        "mimeType": "application/json"
                    },
                    {
                        "uri": "lit://branches",
                        "name": "Branches",
                        "description": "List of all branches in the repository",
                        "mimeType": "application/json"
                    },
                    {
                        "uri": "lit://log",
                        "name": "Commit History",
                        "description": "Recent commit history",
                        "mimeType": "application/json"
                    },
                    {
                        "uri": "lit://ontology",
                        "name": "Lit Ontology",
                        "description": "Machine-readable ontology for agent discovery",
                        "mimeType": "application/json"
                    },
                    {
                        "uri": "lit://schema",
                        "name": "JSON Schema",
                        "description": "JSON Schema (draft 2020-12) generated from the Lit ontology for input validation",
                        "mimeType": "application/schema+json"
                    }
                ]
            })),
            error: None,
        },

        // Read a resource
        "resources/read" => {
            let uri = req.params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            match read_resource(uri) {
                Ok(content) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: req.id.clone(),
                    result: Some(serde_json::json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": content
                        }]
                    })),
                    error: None,
                },
                Err(e) => {
                    // SECURITY: Log internal details server-side, return sanitized message to client (FINDING-002)
                    eprintln!("MCP resources/read error: {}", e.internal_message());
                    JsonRpcResponse {
                        jsonrpc: "2.0",
                        id: req.id.clone(),
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32602,
                            message: e.user_message().to_string(),
                            data: None,
                        }),
                    }
                }
            }
        }

        _ => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
                data: None,
            }),
        },
    }
}

fn get_mcp_tools() -> Vec<McpTool> {
    // MCP tool name → ontology command ID
    let tool_commands: &[(&str, &str, &str)] = &[
        // Core read
        (
            "lit_status",
            "status",
            "Show repository status: current branch, staged/modified/untracked files",
        ),
        (
            "lit_diff",
            "diff",
            "Show changes between working tree, index, and commits",
        ),
        ("lit_log", "log", "Show commit history"),
        (
            "lit_show",
            "show",
            "Show contents of a commit, tree, or blob object",
        ),
        (
            "lit_blame",
            "blame",
            "Show what revision and author last modified each line of a file",
        ),
        (
            "lit_reflog",
            "reflog",
            "Show reference log history (HEAD updates, branch moves)",
        ),
        // Core write
        ("lit_add", "add", "Stage files for the next commit"),
        (
            "lit_commit",
            "commit",
            "Record staged changes as a new commit",
        ),
        (
            "lit_snapshot",
            "snapshot",
            "Atomic add-all + commit in one step (preferred for agents)",
        ),
        // Branching
        ("lit_branch", "branch", "List, create, or delete branches"),
        (
            "lit_checkout",
            "checkout",
            "Switch branches or restore working tree files",
        ),
        (
            "lit_merge",
            "merge",
            "Merge a branch into the current branch",
        ),
        (
            "lit_resolve",
            "resolve",
            "Resolve merge conflicts using a specified strategy",
        ),
        (
            "lit_rebase",
            "rebase",
            "Rebase current branch onto another base",
        ),
        (
            "lit_cherry_pick",
            "cherry-pick",
            "Apply a commit from another branch",
        ),
        (
            "lit_revert",
            "revert",
            "Revert a commit by creating a new inverse commit",
        ),
        (
            "lit_reset",
            "reset",
            "Reset current HEAD to a specified state",
        ),
        ("lit_stash", "stash", "Stash changes temporarily"),
        ("lit_tag", "tag", "Create, list, delete, or verify tags"),
        // Remote
        ("lit_push", "push", "Push commits to a remote repository"),
        (
            "lit_pull",
            "pull",
            "Fetch from and integrate with a remote repository",
        ),
        (
            "lit_fetch",
            "fetch",
            "Download objects and refs from a remote repository",
        ),
        (
            "lit_clone",
            "clone",
            "Clone a repository into a new directory",
        ),
        // Search & agent
        (
            "lit_search",
            "search",
            "Search file contents, commit messages, or metadata",
        ),
        (
            "lit_verify",
            "verify",
            "Run full repository integrity check",
        ),
        (
            "lit_gc",
            "gc",
            "Garbage collection — pack loose objects into pack files",
        ),
        // Init & config
        ("lit_init", "init", "Initialize a new Lit repository"),
        (
            "lit_config",
            "config",
            "Show or modify configuration settings",
        ),
        // Discovery
        (
            "lit_ontology",
            "ontology",
            "Output the complete Lit ontology for agent discovery",
        ),
        (
            "lit_schema",
            "schema",
            "Generate JSON Schema from the ontology",
        ),
    ];

    let ont = ontology::get_ontology();

    tool_commands
        .iter()
        .map(|(tool_name, cmd_id, description)| {
            // Use ontology-generated schema if available, fall back to empty object
            let input_schema = ont
                .commands
                .iter()
                .find(|c| c.id == *cmd_id)
                .map(|cmd| {
                    let mut properties = serde_json::Map::new();
                    let mut required = Vec::new();
                    for param in &cmd.parameters {
                        let mut prop = match param.type_name.as_str() {
                            "string" | "String" => serde_json::json!({ "type": "string" }),
                            "boolean" | "bool" => serde_json::json!({ "type": "boolean" }),
                            "integer" | "usize" => serde_json::json!({ "type": "integer" }),
                            t if t.starts_with("array<") => serde_json::json!({
                                "type": "array",
                                "items": { "type": "string" }
                            }),
                            _ => serde_json::json!({ "type": "string" }),
                        };
                        if let Some(obj) = prop.as_object_mut() {
                            obj.insert(
                                "description".to_string(),
                                serde_json::Value::String(param.description.clone()),
                            );
                            if let Some(ref default) = param.default {
                                obj.insert(
                                    "default".to_string(),
                                    serde_json::Value::String(default.clone()),
                                );
                            }
                        }
                        properties.insert(param.name.clone(), prop);
                        if param.required {
                            required.push(serde_json::Value::String(param.name.clone()));
                        }
                    }
                    serde_json::json!({
                        "type": "object",
                        "properties": properties,
                        "required": required
                    })
                })
                .unwrap_or_else(|| {
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    })
                });

            McpTool {
                name: tool_name.to_string(),
                description: description.to_string(),
                input_schema,
            }
        })
        .collect()
}

fn call_tool(
    name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, crate::errors::LitError> {
    match name {
        "lit_status" => {
            let resp = commands::status::execute()?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_diff" => {
            let staged = args
                .get("staged")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let stat = args.get("stat").and_then(|v| v.as_bool()).unwrap_or(false);
            let resp = commands::diff::execute(staged, stat, false, None, None)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_log" => {
            let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let resp = commands::log::execute(count, false)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_commit" => {
            let message = args
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'message'")?
                .to_string();
            let author = args
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::commit::execute(message, author)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_add" => {
            let files: Vec<String> = args
                .get("files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if files.is_empty() {
                return Err("Missing 'files' array".into());
            }
            let resp = commands::add::execute(files)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_branch" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let delete = args
                .get("delete")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let list = name.is_none() && !delete;
            let resp = commands::branch::execute(name, delete, list)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_checkout" => {
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'target'")?
                .to_string();
            let create = args
                .get("create")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = commands::checkout::execute(target, create)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_merge" => {
            let branch = args
                .get("branch")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'branch'")?
                .to_string();
            let strategy = args
                .get("strategy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::merge::execute(branch, strategy)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_search" => {
            let query = args
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'query'")?
                .to_string();
            let messages = args
                .get("messages")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let metadata = args
                .get("metadata")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let max = args
                .get("max_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(100) as usize;
            let resp = commands::search::execute(query, messages, metadata, max)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_snapshot" => {
            let message = args
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'message'")?
                .to_string();
            let author = args
                .get("author")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let metadata = args.get("metadata").cloned();
            let resp = commands::snapshot::execute(message, author, metadata)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_show" => {
            let object = args
                .get("object")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'object'")?
                .to_string();
            let resp = commands::show::execute(object)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_verify" => {
            let resp = commands::verify::execute()?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_init" => {
            let bare = args.get("bare").and_then(|v| v.as_bool()).unwrap_or(false);
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::init::execute(bare, path)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_tag" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let message = args
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let annotate = args
                .get("annotate")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let delete = args
                .get("delete")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let sign = args.get("sign").and_then(|v| v.as_bool()).unwrap_or(false);
            let verify = args
                .get("verify")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let list = args.get("list").and_then(|v| v.as_bool()).unwrap_or(false);
            let commit = args
                .get("commit")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::tag::execute(
                name, message, annotate, delete, sign, verify, list, commit,
            )?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_push" => {
            let remote = args
                .get("remote")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'remote'")?
                .to_string();
            let branch = args
                .get("branch")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'branch'")?
                .to_string();
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            let resp = commands::push::execute(remote, branch, force)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_pull" => {
            let remote = args
                .get("remote")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'remote'")?
                .to_string();
            let branch = args
                .get("branch")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'branch'")?
                .to_string();
            let resp = commands::pull::execute(remote, branch)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_fetch" => {
            let remote = args
                .get("remote")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'remote'")?
                .to_string();
            let branch = args
                .get("branch")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::fetch::execute(remote, branch)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_clone" => {
            let url = args
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'url'")?
                .to_string();
            let directory = args
                .get("directory")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let resp = commands::clone::execute(url, directory)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_stash" => {
            // Default to stash push with optional message
            let message = args
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("push");
            let stash_cmd = match action {
                "pop" => Some(crate::StashCommands::Pop),
                "list" => Some(crate::StashCommands::List),
                "apply" => {
                    let index = args
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .map(|i| i as usize);
                    Some(crate::StashCommands::Apply { index })
                }
                "drop" => {
                    let index = args
                        .get("index")
                        .and_then(|v| v.as_u64())
                        .map(|i| i as usize);
                    Some(crate::StashCommands::Drop { index })
                }
                _ => Some(crate::StashCommands::Push { message }),
            };
            let resp = commands::stash::execute(stash_cmd)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_reset" => {
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'target'")?
                .to_string();
            let soft = args.get("soft").and_then(|v| v.as_bool()).unwrap_or(false);
            let hard = args.get("hard").and_then(|v| v.as_bool()).unwrap_or(false);
            let resp = commands::reset::execute(target, soft, hard)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_revert" => {
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'target'")?
                .to_string();
            let resp = commands::revert::execute(target)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_cherry_pick" => {
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'target'")?
                .to_string();
            let resp = commands::cherry_pick::execute(target)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_rebase" => {
            let base = args
                .get("base")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'base'")?
                .to_string();
            let interactive = args
                .get("interactive")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let onto = args
                .get("onto")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let abort = args.get("abort").and_then(|v| v.as_bool()).unwrap_or(false);
            let cont = args
                .get("continue")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = commands::rebase::execute(base, interactive, onto, abort, cont)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_blame" => {
            let file = args
                .get("file")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'file'")?
                .to_string();
            let resp = commands::blame::execute(file)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_reflog" => {
            let ref_name = args
                .get("ref_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
            let resp = commands::reflog::execute(ref_name, count)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_resolve" => {
            let file = args
                .get("file")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let strategy = args
                .get("strategy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
            let finish = args
                .get("finish")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resp = commands::resolve::execute(file, strategy, all, finish)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_gc" => {
            let resp = commands::gc::execute()?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_config" => {
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("show");
            let config_cmd = match action {
                "get" => {
                    let key = args
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing 'key'")?
                        .to_string();
                    Some(crate::ConfigCommands::Get { key })
                }
                "set" => {
                    let key = args
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing 'key'")?
                        .to_string();
                    let value = args
                        .get("value")
                        .and_then(|v| v.as_str())
                        .ok_or("Missing 'value'")?
                        .to_string();
                    Some(crate::ConfigCommands::Set { key, value })
                }
                _ => Some(crate::ConfigCommands::Show),
            };
            let resp = commands::config::execute(config_cmd)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string().into())
        }
        "lit_ontology" => {
            let ont = crate::ontology::get_ontology();
            serde_json::to_value(&ont).map_err(|e| e.to_string().into())
        }
        "lit_schema" => {
            let command = args.get("command").and_then(|v| v.as_str());
            let schema = if let Some(cmd_id) = command {
                match crate::ontology::generate_command_schema(cmd_id) {
                    Some(s) => s,
                    None => return Err(format!("Unknown command: {}", cmd_id).into()),
                }
            } else {
                crate::ontology::generate_schemas()
            };
            Ok(schema)
        }
        _ => Err(format!("Unknown tool: {}", name).into()),
    }
}

fn read_resource(uri: &str) -> Result<String, crate::errors::LitError> {
    match uri {
        "lit://status" => {
            let resp = commands::status::execute()?;
            serde_json::to_string_pretty(&resp).map_err(|e| e.to_string().into())
        }
        "lit://branches" => {
            let resp = commands::branch::execute(None, false, true)?;
            serde_json::to_string_pretty(&resp).map_err(|e| e.to_string().into())
        }
        "lit://log" => {
            let resp = commands::log::execute(50, false)?;
            serde_json::to_string_pretty(&resp).map_err(|e| e.to_string().into())
        }
        "lit://ontology" => {
            let ontology = crate::ontology::get_ontology();
            serde_json::to_string_pretty(&ontology).map_err(|e| e.to_string().into())
        }
        "lit://schema" => {
            let schema = ontology::generate_schemas();
            serde_json::to_string_pretty(&schema).map_err(|e| e.to_string().into())
        }
        _ => Err(format!("Unknown resource: {}", uri).into()),
    }
}
