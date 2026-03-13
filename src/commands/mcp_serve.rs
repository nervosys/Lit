use crate::commands;
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
pub fn execute_stdio() -> Result<McpServeResponse, String> {
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
pub fn execute_http(port: u16) -> Result<McpServeResponse, String> {
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
        if request.as_reader().take(MAX_BODY_SIZE as u64).read_to_string(&mut body).is_err() {
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
                            "text": e
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
                        message: e,
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
    vec![
        McpTool {
            name: "lit_status".to_string(),
            description: "Show repository status: current branch, staged/modified/untracked files"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        McpTool {
            name: "lit_diff".to_string(),
            description: "Show changes between working tree, index, and commits".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "staged": { "type": "boolean", "description": "Show staged changes (index vs HEAD)" },
                    "stat": { "type": "boolean", "description": "Show summary statistics only" }
                },
                "required": []
            }),
        },
        McpTool {
            name: "lit_log".to_string(),
            description: "Show commit history".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "count": { "type": "integer", "description": "Number of commits to show", "default": 10 }
                },
                "required": []
            }),
        },
        McpTool {
            name: "lit_commit".to_string(),
            description: "Record staged changes as a new commit".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Commit message" },
                    "author": { "type": "string", "description": "Author name (optional)" }
                },
                "required": ["message"]
            }),
        },
        McpTool {
            name: "lit_add".to_string(),
            description: "Stage files for the next commit".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "File paths to stage"
                    }
                },
                "required": ["files"]
            }),
        },
        McpTool {
            name: "lit_branch".to_string(),
            description: "List, create, or delete branches".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Branch name (omit to list)" },
                    "delete": { "type": "boolean", "description": "Delete the branch" }
                },
                "required": []
            }),
        },
        McpTool {
            name: "lit_checkout".to_string(),
            description: "Switch branches or restore working tree files".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Branch or commit to checkout" },
                    "create": { "type": "boolean", "description": "Create new branch" }
                },
                "required": ["target"]
            }),
        },
        McpTool {
            name: "lit_merge".to_string(),
            description: "Merge a branch into the current branch".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "branch": { "type": "string", "description": "Branch to merge" },
                    "strategy": { "type": "string", "description": "Merge strategy: recursive, ours, theirs" }
                },
                "required": ["branch"]
            }),
        },
        McpTool {
            name: "lit_search".to_string(),
            description: "Search file contents, commit messages, or metadata".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "messages": { "type": "boolean", "description": "Search commit messages instead of file contents" },
                    "metadata": { "type": "string", "description": "Search metadata key=value" },
                    "max_results": { "type": "integer", "description": "Maximum results", "default": 100 }
                },
                "required": ["query"]
            }),
        },
        McpTool {
            name: "lit_snapshot".to_string(),
            description: "Atomic add-all + commit in one step (preferred for agents)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Commit message" },
                    "author": { "type": "string", "description": "Author name" },
                    "metadata": { "type": "object", "description": "Agent metadata JSON (agent_id, task_id, confidence, etc.)" }
                },
                "required": ["message"]
            }),
        },
        McpTool {
            name: "lit_show".to_string(),
            description: "Show contents of a commit, tree, or blob object".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "object": { "type": "string", "description": "Object hash or reference (branch name, HEAD, tag)" }
                },
                "required": ["object"]
            }),
        },
        McpTool {
            name: "lit_verify".to_string(),
            description: "Run full repository integrity check".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

fn call_tool(name: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    match name {
        "lit_status" => {
            let resp = commands::status::execute()?;
            serde_json::to_value(&resp).map_err(|e| e.to_string())
        }
        "lit_diff" => {
            let staged = args
                .get("staged")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let stat = args.get("stat").and_then(|v| v.as_bool()).unwrap_or(false);
            let resp = commands::diff::execute(staged, stat, false, None, None)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string())
        }
        "lit_log" => {
            let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let resp = commands::log::execute(count, false)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string())
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
            serde_json::to_value(&resp).map_err(|e| e.to_string())
        }
        "lit_add" => {
            let files: Vec<String> = args
                .get("files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            if files.is_empty() {
                return Err("Missing 'files' array".to_string());
            }
            let resp = commands::add::execute(files)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string())
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
            serde_json::to_value(&resp).map_err(|e| e.to_string())
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
            serde_json::to_value(&resp).map_err(|e| e.to_string())
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
            serde_json::to_value(&resp).map_err(|e| e.to_string())
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
            serde_json::to_value(&resp).map_err(|e| e.to_string())
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
            serde_json::to_value(&resp).map_err(|e| e.to_string())
        }
        "lit_show" => {
            let object = args
                .get("object")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'object'")?
                .to_string();
            let resp = commands::show::execute(object)?;
            serde_json::to_value(&resp).map_err(|e| e.to_string())
        }
        "lit_verify" => {
            let resp = commands::verify::execute()?;
            serde_json::to_value(&resp).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn read_resource(uri: &str) -> Result<String, String> {
    match uri {
        "lit://status" => {
            let resp = commands::status::execute()?;
            serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
        }
        "lit://branches" => {
            let resp = commands::branch::execute(None, false, true)?;
            serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
        }
        "lit://log" => {
            let resp = commands::log::execute(50, false)?;
            serde_json::to_string_pretty(&resp).map_err(|e| e.to_string())
        }
        "lit://ontology" => {
            let ontology = crate::ontology::get_ontology();
            serde_json::to_string_pretty(&ontology).map_err(|e| e.to_string())
        }
        _ => Err(format!("Unknown resource: {}", uri)),
    }
}
