//! Lit Ontology — machine-readable capability schema for autonomous agent discovery.
//!
//! Provides a structured representation of all Lit commands, types, workflows,
//! and interaction protocols so that autonomous agents can discover and use Lit
//! without prior training or documentation.

use serde::Serialize;

/// Top-level ontology for the Lit version control system
#[derive(Debug, Serialize)]
pub struct LitOntology {
    #[serde(rename = "@context")]
    pub context: OntologyContext,
    #[serde(rename = "@type")]
    pub type_name: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub capabilities: Vec<Capability>,
    pub types: Vec<TypeDef>,
    pub commands: Vec<CommandDef>,
    pub workflows: Vec<Workflow>,
    pub protocols: Protocols,
    pub errors: ErrorOntology,
}

#[derive(Debug, Serialize)]
pub struct OntologyContext {
    pub lit: &'static str,
    pub schema: &'static str,
    pub vcs: &'static str,
    pub mcp: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct TypeDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub properties: Vec<PropertyDef>,
}

#[derive(Debug, Serialize)]
pub struct PropertyDef {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Serialize)]
pub struct CommandDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub parameters: Vec<ParamDef>,
    pub returns: String,
    pub side_effects: Vec<String>,
    pub preconditions: Vec<String>,
    pub examples: Vec<Example>,
    /// Commands that commonly follow this one
    pub follows: Vec<String>,
    /// Commands that commonly precede this one
    pub preceded_by: Vec<String>,
    pub idempotent: bool,
    pub safe: bool,
}

#[derive(Debug, Serialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub description: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Example {
    pub description: String,
    pub cli: String,
    pub json: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub agent_optimized: bool,
}

#[derive(Debug, Serialize)]
pub struct WorkflowStep {
    pub order: usize,
    pub command: String,
    pub description: String,
    pub optional: bool,
}

#[derive(Debug, Serialize)]
pub struct Protocols {
    pub cli: CliProtocol,
    pub rest: RestProtocol,
    pub mcp: McpProtocol,
    pub batch: BatchProtocol,
}

#[derive(Debug, Serialize)]
pub struct CliProtocol {
    pub binary: &'static str,
    pub global_flags: Vec<FlagDef>,
    pub output_formats: Vec<&'static str>,
    pub default_format: &'static str,
}

#[derive(Debug, Serialize)]
pub struct FlagDef {
    pub flag: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct RestProtocol {
    pub base_path: &'static str,
    pub auth: Vec<&'static str>,
    pub content_type: &'static str,
}

#[derive(Debug, Serialize)]
pub struct McpProtocol {
    pub protocol_version: &'static str,
    pub transports: Vec<McpTransport>,
    pub tool_prefix: &'static str,
}

#[derive(Debug, Serialize)]
pub struct McpTransport {
    pub name: String,
    pub command: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct BatchProtocol {
    pub format: &'static str,
    pub input: &'static str,
    pub flags: Vec<FlagDef>,
}

#[derive(Debug, Serialize)]
pub struct ErrorOntology {
    pub format: ErrorFormat,
    pub categories: Vec<ErrorCategory>,
}

#[derive(Debug, Serialize)]
pub struct ErrorFormat {
    pub json_envelope: serde_json::Value,
    pub fields: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorCategory {
    pub code: String,
    pub description: String,
    pub recoverable: bool,
    pub suggested_action: String,
}

/// Build and return the complete Lit ontology
pub fn get_ontology() -> LitOntology {
    LitOntology {
        context: OntologyContext {
            lit: "https://lit-vcs.dev/ontology/v1#",
            schema: "https://schema.org/",
            vcs: "https://lit-vcs.dev/ontology/vcs#",
            mcp: "https://modelcontextprotocol.io/schema/v1#",
        },
        type_name: "VersionControlSystem",
        name: "Lit",
        version: env!("CARGO_PKG_VERSION"),
        description: "Agentic-first distributed version control system. A complete Git replacement designed for AI agents first and humans second. Features post-quantum cryptography (ML-DSA-87, ML-KEM), FIPS 140-2 compliance, structured JSON I/O, batch mode, transactions, and MCP integration.",
        capabilities: build_capabilities(),
        types: build_types(),
        commands: build_commands(),
        workflows: build_workflows(),
        protocols: build_protocols(),
        errors: build_errors(),
    }
}

fn build_capabilities() -> Vec<Capability> {
    vec![
        cap("version-control", "Distributed Version Control", "Full DAG-based version control with branches, merges, commits, and tags"),
        cap("post-quantum-crypto", "Post-Quantum Cryptography", "ML-DSA-87 (Dilithium5) signatures and ML-KEM (Kyber) key encapsulation for quantum-resistant security"),
        cap("fips-compliance", "FIPS 140-2 Compliance", "SHA3-512, AES-256-GCM, HMAC-SHA256, PBKDF2 key derivation with secure zeroization"),
        cap("structured-io", "Structured I/O", "All commands produce structured JSON output by default, with human-readable alternative"),
        cap("batch-mode", "Batch Operations", "Execute multiple operations from JSONL stdin with atomic and dry-run modes"),
        cap("transactions", "Transaction Support", "Begin/commit/rollback with write-ahead log for crash recovery"),
        cap("agent-metadata", "Agent Metadata", "First-class metadata field on commits for agent_id, task_id, confidence, intent, tool_versions"),
        cap("search", "Full-Text Search", "Search file contents, commit messages, and agent metadata"),
        cap("snapshot", "Atomic Snapshots", "Single-command add-all + commit for agent workflows"),
        cap("integrity-verification", "Repository Verification", "Full integrity check of objects, refs, DAG connectivity, and index"),
        cap("mcp-server", "MCP Tool Server", "Model Context Protocol server for LLM agent integration (stdio and HTTP transports)"),
        cap("rest-api", "REST API Server", "HTTP API for remote repository operations with bearer token authentication"),
        cap("swarm-coordination", "Multi-Agent Swarm", "Agent registration, branch namespacing, and file lease system for concurrent agent collaboration"),
        cap("airgap-mode", "Air-Gap Mode", "Blocks all network protocols, allows only physical/local transports for secure environments"),
        cap("encryption-at-rest", "Encryption at Rest", "AES-256-GCM encryption for all repository objects and refs"),
    ]
}

fn build_types() -> Vec<TypeDef> {
    vec![
        TypeDef {
            id: "ObjectHash".to_string(),
            name: "Object Hash".to_string(),
            description:
                "192-character hex string: SHA3-512 (128 chars) + BLAKE3 (64 chars) concatenated"
                    .to_string(),
            properties: vec![
                prop(
                    "sha3_512",
                    "string",
                    "First 128 hex characters — SHA3-512 digest",
                    true,
                ),
                prop(
                    "blake3",
                    "string",
                    "Last 64 hex characters — BLAKE3 digest",
                    true,
                ),
            ],
        },
        TypeDef {
            id: "Commit".to_string(),
            name: "Commit Object".to_string(),
            description: "A snapshot of the repository state at a point in time".to_string(),
            properties: vec![
                prop("tree", "ObjectHash", "Hash of the root tree object", true),
                prop(
                    "parents",
                    "ObjectHash[]",
                    "Parent commit hashes (empty for initial commit)",
                    true,
                ),
                prop("author", "string", "Author identity", true),
                prop(
                    "timestamp",
                    "integer",
                    "Unix timestamp (seconds since epoch)",
                    true,
                ),
                prop("message", "string", "Commit message", true),
                prop(
                    "metadata",
                    "object|null",
                    "Optional JSON metadata (agent_id, task_id, confidence, etc.)",
                    false,
                ),
                prop(
                    "signature",
                    "PQSignature|null",
                    "Optional ML-DSA-87 signature",
                    false,
                ),
            ],
        },
        TypeDef {
            id: "Tree".to_string(),
            name: "Tree Object".to_string(),
            description: "A directory listing mapping names to object hashes".to_string(),
            properties: vec![prop(
                "entries",
                "TreeEntry[]",
                "List of entries (files and subdirectories)",
                true,
            )],
        },
        TypeDef {
            id: "Blob".to_string(),
            name: "Blob Object".to_string(),
            description: "File contents stored as a compressed byte sequence".to_string(),
            properties: vec![prop("data", "bytes", "Compressed file content", true)],
        },
        TypeDef {
            id: "Tag".to_string(),
            name: "Tag Object".to_string(),
            description: "A named reference to a specific commit, optionally signed".to_string(),
            properties: vec![
                prop("name", "string", "Tag name", true),
                prop("target", "ObjectHash", "The tagged commit hash", true),
                prop("tagger", "string", "Author of the tag", true),
                prop("message", "string", "Tag message", true),
                prop(
                    "signature",
                    "PQSignature|null",
                    "Optional ML-DSA-87 signature",
                    false,
                ),
            ],
        },
        TypeDef {
            id: "Branch".to_string(),
            name: "Branch Reference".to_string(),
            description: "A mutable named pointer to a commit hash, stored in .lit/refs/heads/"
                .to_string(),
            properties: vec![
                prop("name", "string", "Branch name", true),
                prop(
                    "target",
                    "ObjectHash",
                    "The commit hash this branch points to",
                    true,
                ),
            ],
        },
        TypeDef {
            id: "AgentMetadata".to_string(),
            name: "Agent Metadata".to_string(),
            description: "Structured metadata attached to commits by autonomous agents".to_string(),
            properties: vec![
                prop(
                    "agent_id",
                    "string",
                    "Unique identifier of the agent that created the commit",
                    false,
                ),
                prop(
                    "agent_model",
                    "string",
                    "Model name/version (e.g., 'claude-opus-4-20250514', 'gpt-4o')",
                    false,
                ),
                prop(
                    "task_id",
                    "string",
                    "Identifier for the task or work item being addressed",
                    false,
                ),
                prop(
                    "confidence",
                    "number",
                    "Agent's self-assessed confidence in the changes (0.0 - 1.0)",
                    false,
                ),
                prop(
                    "intent",
                    "string",
                    "Human-readable description of what the agent intended to do",
                    false,
                ),
                prop(
                    "tool_versions",
                    "object",
                    "Versions of tools used (e.g., compiler, linter)",
                    false,
                ),
                prop(
                    "parent_task",
                    "string",
                    "Reference to a parent task for hierarchical workflows",
                    false,
                ),
                prop(
                    "session_id",
                    "string",
                    "Conversation or session identifier",
                    false,
                ),
            ],
        },
        TypeDef {
            id: "FileLease".to_string(),
            name: "File Lease".to_string(),
            description: "Exclusive write lock on a file for swarm coordination".to_string(),
            properties: vec![
                prop("agent_id", "string", "The agent holding the lease", true),
                prop("path", "string", "File path the lease covers", true),
                prop(
                    "acquired_at",
                    "integer",
                    "Unix timestamp when lease was acquired",
                    true,
                ),
                prop(
                    "expires_at",
                    "integer",
                    "Unix timestamp when lease expires",
                    true,
                ),
            ],
        },
    ]
}

fn build_commands() -> Vec<CommandDef> {
    vec![
        // Core VCS
        cmd("init", "Initialize Repository", "core", "Create a new Lit repository in the current or specified directory",
            vec![
                param("bare", "boolean", "Create a bare repository (no working tree)", false, None),
                param("path", "string", "Directory path (defaults to current directory)", false, None),
            ],
            "InitResponse", vec!["Creates .lit/ directory structure"], vec![],
            vec![ex("Initialize", "lit init", serde_json::json!({"bare": false}))],
            vec!["add", "config"], vec![], true, true),
        cmd("add", "Stage Files", "core", "Add file contents to the staging area (index)",
            vec![param("files", "string[]", "File paths to stage", true, None)],
            "AddResponse", vec!["Modifies .lit/index"], vec!["Repository must be initialized"],
            vec![ex("Stage files", "lit add src/main.rs", serde_json::json!({"files": ["src/main.rs"]}))],
            vec!["commit", "status"], vec!["init", "checkout"], false, true),
        cmd("commit", "Create Commit", "core", "Record staged changes as a new commit object",
            vec![
                param("message", "string", "Commit message describing the changes", true, None),
                param("author", "string", "Author name", false, None),
            ],
            "CommitResponse", vec!["Creates commit and tree objects", "Updates HEAD ref"], vec!["Files must be staged"],
            vec![ex("Commit", "lit commit -m 'fix bug'", serde_json::json!({"message": "fix bug"}))],
            vec!["push", "log", "status"], vec!["add"], false, false),
        cmd("status", "Show Status", "core", "Show the working tree status: branch, staged, modified, and untracked files",
            vec![],
            "StatusResponse", vec![], vec!["Repository must be initialized"],
            vec![ex("Check status", "lit status", serde_json::json!({}))],
            vec!["add", "commit", "diff"], vec![], true, true),
        cmd("log", "Show History", "core", "Display commit history from HEAD or a specified ref",
            vec![
                param("count", "integer", "Number of commits to show", false, Some("10")),
                param("oneline", "boolean", "Compact one-line format", false, Some("false")),
            ],
            "LogResponse", vec![], vec!["Repository must have commits"],
            vec![ex("Recent history", "lit log -n 5", serde_json::json!({"count": 5}))],
            vec!["show", "diff"], vec!["commit"], true, true),
        cmd("diff", "Show Changes", "core", "Show differences between working tree, index, and commits",
            vec![
                param("staged", "boolean", "Compare index to HEAD", false, Some("false")),
                param("stat", "boolean", "Show statistics only", false, Some("false")),
                param("ref1", "string", "First reference", false, None),
                param("ref2", "string", "Second reference", false, None),
            ],
            "DiffResponse", vec![], vec![],
            vec![ex("Working tree diff", "lit diff", serde_json::json!({}))],
            vec!["add", "commit"], vec![], true, true),
        cmd("show", "Show Object", "core", "Display contents of a commit, tree, or blob object",
            vec![param("object", "string", "Object hash or ref name", true, None)],
            "ShowResponse", vec![], vec![],
            vec![ex("Show commit", "lit show HEAD", serde_json::json!({"object": "HEAD"}))],
            vec![], vec!["log"], true, true),
        // Branching
        cmd("branch", "Manage Branches", "branching", "List, create, or delete branches",
            vec![
                param("name", "string", "Branch name to create", false, None),
                param("delete", "boolean", "Delete the named branch", false, Some("false")),
                param("all", "boolean", "List all branches", false, Some("false")),
            ],
            "BranchResponse", vec!["May create or delete refs"], vec![],
            vec![ex("List branches", "lit branch --all", serde_json::json!({"all": true}))],
            vec!["checkout"], vec![], true, true),
        cmd("checkout", "Switch Branch", "branching", "Switch to a different branch or restore working tree files",
            vec![
                param("target", "string", "Branch name or commit hash", true, None),
                param("b", "boolean", "Create and switch to new branch", false, Some("false")),
            ],
            "CheckoutResponse", vec!["Updates working tree", "Updates HEAD"], vec![],
            vec![ex("Switch branch", "lit checkout main", serde_json::json!({"target": "main"}))],
            vec!["add", "commit", "merge"], vec!["branch"], false, false),
        cmd("merge", "Merge Branches", "branching", "Merge another branch into the current branch",
            vec![
                param("branch", "string", "Branch to merge", true, None),
                param("strategy", "string", "Merge strategy (recursive, ours, theirs)", false, Some("recursive")),
            ],
            "MergeResponse", vec!["May create merge commit", "May produce conflicts"], vec!["Must be on a branch"],
            vec![ex("Merge feature", "lit merge feature-x", serde_json::json!({"branch": "feature-x"}))],
            vec!["resolve", "commit", "push"], vec!["checkout", "pull"], false, false),
        // Collaboration
        cmd("push", "Push Changes", "collaboration", "Upload local commits to a remote repository (LAN only)",
            vec![
                param("remote", "string", "Remote name", true, None),
                param("branch", "string", "Branch name", true, None),
                param("force", "boolean", "Force push", false, Some("false")),
            ],
            "PushResponse", vec!["Updates remote refs"], vec!["Remote must be configured"],
            vec![ex("Push to origin", "lit push origin main", serde_json::json!({"remote": "origin", "branch": "main"}))],
            vec![], vec!["commit", "merge"], false, false),
        cmd("pull", "Pull Changes", "collaboration", "Fetch and merge changes from a remote repository",
            vec![
                param("remote", "string", "Remote name", true, None),
                param("branch", "string", "Branch name", true, None),
            ],
            "PullResponse", vec!["Updates local refs and working tree"], vec!["Remote must be configured"],
            vec![ex("Pull from origin", "lit pull origin main", serde_json::json!({"remote": "origin", "branch": "main"}))],
            vec!["merge", "commit"], vec![], false, false),
        // Agent-optimized
        cmd("snapshot", "Atomic Snapshot", "agent", "Stage all files and commit in one atomic operation — the preferred agent workflow",
            vec![
                param("message", "string", "Commit message", true, None),
                param("author", "string", "Author name", false, None),
                param("metadata", "AgentMetadata", "Agent metadata JSON object", false, None),
            ],
            "SnapshotResponse", vec!["Stages all files", "Creates commit"], vec![],
            vec![ex("Agent snapshot", "lit snapshot -m 'implement feature X' --metadata '{\"agent_id\":\"claude-1\",\"confidence\":0.95}'",
                serde_json::json!({"message": "implement feature X", "metadata": {"agent_id": "claude-1", "confidence": 0.95}}))],
            vec!["push", "log"], vec![], false, false),
        cmd("batch", "Batch Operations", "agent", "Execute multiple operations from JSONL on stdin",
            vec![
                param("atomic", "boolean", "Stop on first failure, skip remaining", false, Some("false")),
                param("dry_run", "boolean", "Validate without executing", false, Some("false")),
            ],
            "BatchResponse", vec!["Depends on operations"], vec![],
            vec![ex("Batch", "echo '{\"command\":\"status\"}' | lit batch", serde_json::json!({"atomic": false}))],
            vec![], vec![], false, false),
        cmd("search", "Search Repository", "agent", "Full-text search across file contents, commit messages, or agent metadata",
            vec![
                param("query", "string", "Search query string", true, None),
                param("messages", "boolean", "Search commit messages", false, Some("false")),
                param("metadata", "string", "Search metadata (key=value)", false, None),
                param("max_results", "integer", "Maximum results to return", false, Some("100")),
            ],
            "SearchResponse", vec![], vec![],
            vec![ex("Search files", "lit search 'TODO'", serde_json::json!({"query": "TODO"}))],
            vec![], vec![], true, true),
        cmd("verify", "Verify Integrity", "agent", "Run full repository integrity check — objects, refs, DAG, index",
            vec![],
            "VerifyResponse", vec![], vec!["Repository must be initialized"],
            vec![ex("Verify", "lit verify", serde_json::json!({}))],
            vec![], vec![], true, true),
        // Swarm
        cmd("swarm register", "Register Agent", "swarm", "Register an agent for multi-agent coordination with branch namespacing",
            vec![param("agent_id", "string", "Unique agent identifier", true, None)],
            "SwarmResponse", vec!["Creates agent namespace in refs"], vec![],
            vec![ex("Register", "lit swarm register claude-1", serde_json::json!({"agent_id": "claude-1"}))],
            vec!["swarm lease-acquire"], vec![], true, false),
        cmd("swarm lease-acquire", "Acquire File Lease", "swarm", "Acquire exclusive write access to a file for a specified duration",
            vec![
                param("agent_id", "string", "Agent requesting the lease", true, None),
                param("path", "string", "File path to lease", true, None),
                param("duration", "integer", "Lease duration in seconds", false, Some("300")),
            ],
            "SwarmResponse", vec!["Creates lease file"], vec!["Agent must be registered"],
            vec![ex("Acquire lease", "lit swarm lease-acquire --agent claude-1 --path src/main.rs --duration 300",
                serde_json::json!({"agent_id": "claude-1", "path": "src/main.rs", "duration": 300}))],
            vec!["swarm lease-release"], vec!["swarm register"], false, false),
    ]
}

fn build_workflows() -> Vec<Workflow> {
    vec![
        Workflow {
            id: "agent-basic".to_string(),
            name: "Basic Agent Workflow".to_string(),
            description: "The simplest agent workflow: make changes, snapshot, push".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "status", "Check current repository state", false),
                wstep(
                    2,
                    "snapshot",
                    "Stage all changes and commit atomically",
                    false,
                ),
                wstep(3, "push", "Push to remote", true),
            ],
        },
        Workflow {
            id: "agent-branch".to_string(),
            name: "Agent Branch Workflow".to_string(),
            description: "Create a feature branch, make changes, merge back".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(
                    1,
                    "checkout -b",
                    "Create and switch to feature branch",
                    false,
                ),
                wstep(2, "snapshot", "Make changes and commit", false),
                wstep(3, "checkout", "Switch back to main branch", false),
                wstep(4, "merge", "Merge feature branch", false),
                wstep(5, "push", "Push merged changes", true),
            ],
        },
        Workflow {
            id: "agent-batch".to_string(),
            name: "Batch Operation Workflow".to_string(),
            description: "Submit multiple operations as JSONL for batch execution".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(
                    1,
                    "batch --dry-run",
                    "Validate operations without executing",
                    true,
                ),
                wstep(2, "batch --atomic", "Execute operations atomically", false),
            ],
        },
        Workflow {
            id: "agent-transaction".to_string(),
            name: "Transaction Workflow".to_string(),
            description: "Group operations with rollback support".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "tx begin", "Start a new transaction", false),
                wstep(
                    2,
                    "add/commit/...",
                    "Perform operations within the transaction",
                    false,
                ),
                wstep(
                    3,
                    "tx commit",
                    "Commit the transaction (or tx rollback to undo)",
                    false,
                ),
            ],
        },
        Workflow {
            id: "swarm-collaboration".to_string(),
            name: "Multi-Agent Collaboration".to_string(),
            description:
                "Multiple agents working on the same repository with lease-based coordination"
                    .to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "swarm register", "Register agent identity", false),
                wstep(
                    2,
                    "swarm lease-acquire",
                    "Acquire exclusive lease on files to edit",
                    false,
                ),
                wstep(3, "checkout -b", "Create agent-namespaced branch", false),
                wstep(4, "snapshot", "Make and commit changes", false),
                wstep(5, "swarm lease-release", "Release file leases", false),
                wstep(
                    6,
                    "push",
                    "Push agent branch for coordinator to merge",
                    false,
                ),
            ],
        },
        Workflow {
            id: "verify-and-fix".to_string(),
            name: "Verify and Fix".to_string(),
            description: "Check repository integrity and take corrective action if needed"
                .to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "verify", "Run full integrity check", false),
                wstep(
                    2,
                    "search",
                    "Search for related issues if verification fails",
                    true,
                ),
                wstep(3, "snapshot", "Commit fixes if any were applied", true),
            ],
        },
    ]
}

fn build_protocols() -> Protocols {
    Protocols {
        cli: CliProtocol {
            binary: "lit",
            global_flags: vec![
                flag("--json", "Output as JSON (default)"),
                flag("--human", "Output as human-readable text"),
                flag("--airgapped", "Enable air-gap mode (block network)"),
                flag(
                    "--passphrase <PASSPHRASE>",
                    "Encryption passphrase (or LIT_PASSPHRASE env)",
                ),
                flag(
                    "--passphrase-file <PATH>",
                    "Path to passphrase file (or LIT_PASSPHRASE_FILE env)",
                ),
            ],
            output_formats: vec!["json", "human"],
            default_format: "json",
        },
        rest: RestProtocol {
            base_path: "/api/v1",
            auth: vec!["Bearer token", "None (localhost only)"],
            content_type: "application/json",
        },
        mcp: McpProtocol {
            protocol_version: "2024-11-05",
            tool_prefix: "lit_",
            transports: vec![
                McpTransport {
                    name: "stdio".to_string(),
                    command: "lit mcp-serve --stdio".to_string(),
                    description: "JSON-RPC 2.0 over stdin/stdout — standard MCP transport"
                        .to_string(),
                },
                McpTransport {
                    name: "http".to_string(),
                    command: "lit mcp-serve --port 3001".to_string(),
                    description: "JSON-RPC 2.0 over HTTP POST".to_string(),
                },
            ],
        },
        batch: BatchProtocol {
            format: "JSONL",
            input: "stdin",
            flags: vec![
                flag("--atomic", "Stop on first failure"),
                flag("--dry-run", "Validate without executing"),
            ],
        },
    }
}

fn build_errors() -> ErrorOntology {
    ErrorOntology {
        format: ErrorFormat {
            json_envelope: serde_json::json!({
                "status": "error",
                "command": "<command_name>",
                "error": {
                    "code": "<error_code>",
                    "message": "<human_readable_message>",
                    "suggestions": ["<recovery_hint>"]
                }
            }),
            fields: vec![
                "status".to_string(),
                "command".to_string(),
                "error.code".to_string(),
                "error.message".to_string(),
                "error.suggestions".to_string(),
            ],
        },
        categories: vec![
            errc(
                "NOT_REPO",
                "Not a Lit repository",
                true,
                "Run 'lit init' to create one",
            ),
            errc(
                "NO_COMMITS",
                "No commits in repository",
                true,
                "Create files and run 'lit snapshot -m \"initial\"'",
            ),
            errc(
                "CONFLICT",
                "Merge conflict detected",
                true,
                "Use 'lit resolve' or 'lit resolve --all --strategy ours'",
            ),
            errc(
                "NOTHING_STAGED",
                "No files staged for commit",
                true,
                "Run 'lit add <files>' or use 'lit snapshot' instead",
            ),
            errc(
                "BRANCH_NOT_FOUND",
                "Branch does not exist",
                true,
                "Run 'lit branch --all' to list available branches",
            ),
            errc(
                "OBJECT_NOT_FOUND",
                "Object hash not found in store",
                false,
                "The object may be corrupt or missing — run 'lit verify'",
            ),
            errc(
                "TX_IN_PROGRESS",
                "Another transaction is active",
                true,
                "Run 'lit tx rollback' to abort the existing transaction",
            ),
            errc(
                "LEASE_HELD",
                "File lease held by another agent",
                true,
                "Wait for lease expiration or coordinate with the holding agent",
            ),
            errc(
                "NETWORK_BLOCKED",
                "Network access blocked in airgap mode",
                true,
                "Disable --airgapped flag or use local file:// URLs",
            ),
        ],
    }
}

// Builder helpers
fn cap(id: &str, name: &str, desc: &str) -> Capability {
    Capability {
        id: id.to_string(),
        name: name.to_string(),
        description: desc.to_string(),
    }
}

fn prop(name: &str, type_name: &str, desc: &str, required: bool) -> PropertyDef {
    PropertyDef {
        name: name.to_string(),
        type_name: type_name.to_string(),
        description: desc.to_string(),
        required,
    }
}

fn param(
    name: &str,
    type_name: &str,
    desc: &str,
    required: bool,
    default: Option<&str>,
) -> ParamDef {
    ParamDef {
        name: name.to_string(),
        type_name: type_name.to_string(),
        description: desc.to_string(),
        required,
        default: default.map(|s| s.to_string()),
    }
}

fn ex(desc: &str, cli: &str, json: serde_json::Value) -> Example {
    Example {
        description: desc.to_string(),
        cli: cli.to_string(),
        json,
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd(
    id: &str,
    name: &str,
    category: &str,
    desc: &str,
    parameters: Vec<ParamDef>,
    returns: &str,
    side_effects: Vec<&str>,
    preconditions: Vec<&str>,
    examples: Vec<Example>,
    follows: Vec<&str>,
    preceded_by: Vec<&str>,
    idempotent: bool,
    safe: bool,
) -> CommandDef {
    CommandDef {
        id: id.to_string(),
        name: name.to_string(),
        category: category.to_string(),
        description: desc.to_string(),
        parameters,
        returns: returns.to_string(),
        side_effects: side_effects.into_iter().map(|s| s.to_string()).collect(),
        preconditions: preconditions.into_iter().map(|s| s.to_string()).collect(),
        examples,
        follows: follows.into_iter().map(|s| s.to_string()).collect(),
        preceded_by: preceded_by.into_iter().map(|s| s.to_string()).collect(),
        idempotent,
        safe,
    }
}

fn wstep(order: usize, command: &str, description: &str, optional: bool) -> WorkflowStep {
    WorkflowStep {
        order,
        command: command.to_string(),
        description: description.to_string(),
        optional,
    }
}

fn flag(f: &str, desc: &str) -> FlagDef {
    FlagDef {
        flag: f.to_string(),
        description: desc.to_string(),
    }
}

fn errc(code: &str, desc: &str, recoverable: bool, action: &str) -> ErrorCategory {
    ErrorCategory {
        code: code.to_string(),
        description: desc.to_string(),
        recoverable,
        suggested_action: action.to_string(),
    }
}
