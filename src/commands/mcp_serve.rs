use crate::commands;
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
pub fn execute_http(port: u16) -> Result<McpServeResponse, crate::errors::LitError> {
    let bind_addr = format!("127.0.0.1:{}", port);
    let server = tiny_http::Server::http(&bind_addr)
        .map_err(|e| format!("Failed to start MCP HTTP server on {}: {}", bind_addr, e))?;

    eprintln!("Lit MCP server (HTTP) on http://{}", bind_addr);
    eprintln!("Press Ctrl+C to stop");

    for mut request in server.incoming_requests() {
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
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: req.id.clone(),
                    result: Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": e.internal_message().to_string()
                        }],
                        "isError": true
                    })),
                    error: None,
                },
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
                Err(e) => JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: req.id.clone(),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: e.internal_message().to_string(),
                        data: None,
                    }),
                },
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
            "lit_commit",
            "commit",
            "Record staged changes as a new commit",
        ),
        ("lit_add", "add", "Stage files for the next commit"),
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
            "lit_search",
            "search",
            "Search file contents, commit messages, or metadata",
        ),
        (
            "lit_snapshot",
            "snapshot",
            "Atomic add-all + commit in one step (preferred for agents)",
        ),
        (
            "lit_show",
            "show",
            "Show contents of a commit, tree, or blob object",
        ),
        (
            "lit_verify",
            "verify",
            "Run full repository integrity check",
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
