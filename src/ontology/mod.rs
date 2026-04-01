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
        TypeDef {
            id: "TreeEntry".to_string(),
            name: "Tree Entry".to_string(),
            description: "A single entry in a tree object, mapping a name to an object hash".to_string(),
            properties: vec![
                prop("mode", "string", "File mode: 100644 (normal), 100755 (executable), 040000 (directory)", true),
                prop("name", "string", "File or directory name", true),
                prop("hash", "ObjectHash", "Hash of the referenced blob or tree", true),
                prop("object_type", "string", "Object type: 'blob' or 'tree'", true),
            ],
        },
        TypeDef {
            id: "PQSignature".to_string(),
            name: "Post-Quantum Signature".to_string(),
            description: "ML-DSA-87 (Dilithium5) digital signature for quantum-resistant authentication".to_string(),
            properties: vec![
                prop("algorithm", "string", "Signature algorithm identifier (ML-DSA-87)", true),
                prop("signature", "bytes", "Raw signature bytes", true),
            ],
        },
        TypeDef {
            id: "PQKeyPair".to_string(),
            name: "Post-Quantum Key Pair".to_string(),
            description: "ML-DSA-87 key pair for signing and verification".to_string(),
            properties: vec![
                prop("public_key", "bytes", "Public key for verification", true),
                prop("secret_key", "bytes", "Secret key for signing (stored securely)", true),
            ],
        },
        TypeDef {
            id: "IndexEntry".to_string(),
            name: "Index Entry".to_string(),
            description: "A staged file in the index (staging area), mapping a path to its object hash".to_string(),
            properties: vec![
                prop("path", "string", "Relative file path", true),
                prop("hash", "string", "Object hash of the staged content", true),
                prop("mode", "string", "File mode (100644, 100755, etc.)", true),
            ],
        },
        TypeDef {
            id: "EncryptionConfig".to_string(),
            name: "Encryption Configuration".to_string(),
            description: "Repository encryption settings using AES-256-GCM with PBKDF2 key derivation".to_string(),
            properties: vec![
                prop("enabled", "boolean", "Whether encryption is active", true),
                prop("algorithm", "string", "Encryption algorithm (AES-256-GCM)", true),
                prop("salt", "bytes", "Random salt for key derivation", true),
                prop("kdf_iterations", "integer", "PBKDF2 iteration count", true),
            ],
        },
        TypeDef {
            id: "LfsPointer".to_string(),
            name: "LFS Pointer".to_string(),
            description: "Lightweight pointer replacing large file content, referencing the actual data stored separately".to_string(),
            properties: vec![
                prop("version", "string", "LFS pointer format version", true),
                prop("oid", "string", "Object identifier (sha3-blake3:hash)", true),
                prop("size", "integer", "Original file size in bytes", true),
            ],
        },
        TypeDef {
            id: "PackFile".to_string(),
            name: "Pack File".to_string(),
            description: "LITP-format pack file containing multiple compressed objects with CRC32 integrity".to_string(),
            properties: vec![
                prop("magic", "string", "File magic bytes: LITP", true),
                prop("version", "integer", "Pack format version", true),
                prop("object_count", "integer", "Number of objects in the pack", true),
            ],
        },
        TypeDef {
            id: "TransactionState".to_string(),
            name: "Transaction State".to_string(),
            description: "Write-ahead log state for transactional operations with rollback support".to_string(),
            properties: vec![
                prop("tx_id", "string", "Unique transaction identifier", true),
                prop("started_at", "integer", "Unix timestamp when transaction began", true),
                prop("operations", "object[]", "List of operations within the transaction", true),
            ],
        },
        TypeDef {
            id: "DiffHunk".to_string(),
            name: "Diff Hunk".to_string(),
            description: "A contiguous block of changes between two versions of a file".to_string(),
            properties: vec![
                prop("old_start", "integer", "Starting line number in original", true),
                prop("old_count", "integer", "Number of lines in original", true),
                prop("new_start", "integer", "Starting line number in modified", true),
                prop("new_count", "integer", "Number of lines in modified", true),
                prop("lines", "DiffLine[]", "Individual line changes", true),
            ],
        },
        TypeDef {
            id: "DiffLine".to_string(),
            name: "Diff Line".to_string(),
            description: "A single line in a diff hunk with its change kind".to_string(),
            properties: vec![
                prop("kind", "string", "Change type: context, add, or remove", true),
                prop("content", "string", "Line content", true),
            ],
        },
        TypeDef {
            id: "RemoteConfig".to_string(),
            name: "Remote Configuration".to_string(),
            description: "Named remote repository URL configuration stored in .lit/remotes".to_string(),
            properties: vec![
                prop("name", "string", "Remote name (e.g., 'origin')", true),
                prop("url", "string", "Remote repository URL", true),
            ],
        },
        TypeDef {
            id: "ReflogEntry".to_string(),
            name: "Reflog Entry".to_string(),
            description: "A single entry in a reference log, recording a ref state transition".to_string(),
            properties: vec![
                prop("index", "integer", "Entry index (0 = most recent)", true),
                prop("old_hash", "ObjectHash", "Previous ref target", true),
                prop("new_hash", "ObjectHash", "New ref target", true),
                prop("action", "string", "Action that caused the change (commit, checkout, merge, etc.)", true),
                prop("message", "string", "Description of the change", true),
                prop("timestamp", "integer", "Unix timestamp", true),
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
        cmd("swarm lease-release", "Release File Lease", "swarm", "Release an exclusive write lease on a file",
            vec![
                param("agent_id", "string", "Agent releasing the lease", true, None),
                param("path", "string", "File path to release", true, None),
            ],
            "SwarmResponse", vec!["Removes lease file"], vec!["Lease must be held by this agent"],
            vec![ex("Release lease", "lit swarm lease-release --agent claude-1 --path src/main.rs",
                serde_json::json!({"agent_id": "claude-1", "path": "src/main.rs"}))],
            vec![], vec!["swarm lease-acquire"], true, false),
        cmd("swarm list", "List Agents", "swarm", "List all registered agents in the swarm",
            vec![],
            "SwarmResponse", vec![], vec![],
            vec![ex("List agents", "lit swarm list", serde_json::json!({}))],
            vec![], vec!["swarm register"], true, true),
        cmd("swarm lease-list", "List Leases", "swarm", "List all active file leases across all agents",
            vec![],
            "SwarmResponse", vec![], vec![],
            vec![ex("List leases", "lit swarm lease-list", serde_json::json!({}))],
            vec![], vec![], true, true),
        // Remote & collaboration
        cmd("remote", "Manage Remotes", "collaboration", "Add, remove, or list remote repository URLs (LAN only)",
            vec![
                param("command", "string", "Subcommand: add, remove, list", true, None),
                param("name", "string", "Remote name (for add/remove)", false, None),
                param("url", "string", "Remote URL (for add)", false, None),
                param("verbose", "boolean", "Show URLs in list", false, Some("false")),
            ],
            "RemoteResponse", vec!["May modify .lit/remotes config"], vec![],
            vec![
                ex("List remotes", "lit remote list", serde_json::json!({"command": "list"})),
                ex("Add remote", "lit remote add origin smb://server/repo", serde_json::json!({"command": "add", "name": "origin", "url": "smb://server/repo"})),
            ],
            vec!["push", "pull", "fetch"], vec!["init", "clone"], true, true),
        cmd("clone", "Clone Repository", "collaboration", "Clone a remote repository into a new local directory (LAN only)",
            vec![
                param("url", "string", "Repository URL (must be LAN)", true, None),
                param("directory", "string", "Destination directory name", false, None),
            ],
            "CloneResponse", vec!["Creates new directory", "Downloads all objects and refs"], vec![],
            vec![ex("Clone", "lit clone smb://server/repo myrepo", serde_json::json!({"url": "smb://server/repo", "directory": "myrepo"}))],
            vec!["status", "log", "checkout"], vec![], false, false),
        cmd("fetch", "Fetch Remote", "collaboration", "Download objects and refs from a remote without merging",
            vec![
                param("remote", "string", "Remote name", true, None),
                param("branch", "string", "Specific branch to fetch (omit for all)", false, None),
            ],
            "FetchResponse", vec!["Updates remote-tracking refs"], vec!["Remote must be configured"],
            vec![ex("Fetch all", "lit fetch origin", serde_json::json!({"remote": "origin"}))],
            vec!["merge", "log"], vec![], false, true),
        // Configuration
        cmd("config", "Configuration", "configuration", "Show, get, or set repository and global configuration values",
            vec![
                param("command", "string", "Subcommand: show, get, set", true, None),
                param("key", "string", "Configuration key (for get/set)", false, None),
                param("value", "string", "Configuration value (for set)", false, None),
            ],
            "ConfigResponse", vec!["May modify .lit/config"], vec![],
            vec![
                ex("Show all config", "lit config show", serde_json::json!({"command": "show"})),
                ex("Get value", "lit config get core.bare", serde_json::json!({"command": "get", "key": "core.bare"})),
            ],
            vec![], vec!["init"], true, true),
        // Tagging
        cmd("tag", "Manage Tags", "branching", "Create, list, delete, sign, or verify tags. Supports annotated and post-quantum-signed tags (ML-DSA-87)",
            vec![
                param("name", "string", "Tag name", false, None),
                param("annotate", "boolean", "Create annotated tag", false, Some("false")),
                param("message", "string", "Tag message (implies annotated)", false, None),
                param("delete", "boolean", "Delete the named tag", false, Some("false")),
                param("sign", "boolean", "Sign tag with ML-DSA-87", false, Some("false")),
                param("verify", "boolean", "Verify tag signature", false, Some("false")),
                param("list", "boolean", "List all tags", false, Some("false")),
                param("commit", "string", "Target commit (defaults to HEAD)", false, None),
            ],
            "TagResponse", vec!["May create or delete refs/tags/"], vec![],
            vec![
                ex("Create annotated tag", "lit tag v1.0 -a -m 'Release 1.0'", serde_json::json!({"name": "v1.0", "annotate": true, "message": "Release 1.0"})),
                ex("List tags", "lit tag --list", serde_json::json!({"list": true})),
                ex("Sign tag", "lit tag v1.0 --sign -m 'Signed release'", serde_json::json!({"name": "v1.0", "sign": true, "message": "Signed release"})),
            ],
            vec!["push"], vec!["commit"], true, true),
        // History manipulation
        cmd("stash", "Stash Changes", "history", "Save, restore, list, or drop temporarily stashed changes",
            vec![
                param("command", "string", "Subcommand: push, pop, apply, list, drop", true, None),
                param("message", "string", "Stash message (for push)", false, None),
                param("index", "integer", "Stash index (for apply/drop)", false, None),
            ],
            "StashResponse", vec!["May modify .lit/stash and working tree"], vec![],
            vec![
                ex("Save changes", "lit stash push -m 'WIP'", serde_json::json!({"command": "push", "message": "WIP"})),
                ex("Restore latest", "lit stash pop", serde_json::json!({"command": "pop"})),
                ex("List stashes", "lit stash list", serde_json::json!({"command": "list"})),
            ],
            vec!["checkout", "commit"], vec![], false, false),
        cmd("reset", "Reset HEAD", "history", "Reset current HEAD to a specified state. Supports soft (HEAD only), mixed (HEAD + index), and hard (HEAD + index + working tree)",
            vec![
                param("target", "string", "Target commit hash or HEAD~N expression", true, None),
                param("soft", "boolean", "Keep changes in staging area", false, Some("false")),
                param("hard", "boolean", "Discard all changes (index + working tree)", false, Some("false")),
            ],
            "ResetResponse", vec!["Updates HEAD", "May modify index and working tree"], vec![],
            vec![
                ex("Soft reset", "lit reset HEAD~1 --soft", serde_json::json!({"target": "HEAD~1", "soft": true})),
                ex("Hard reset", "lit reset HEAD~3 --hard", serde_json::json!({"target": "HEAD~3", "hard": true})),
            ],
            vec!["status", "log"], vec!["log", "commit"], false, false),
        cmd("revert", "Revert Commit", "history", "Create a new inverse commit that undoes the changes from a specified commit",
            vec![param("target", "string", "Commit hash to revert", true, None)],
            "RevertResponse", vec!["Creates inverse commit"], vec!["Target commit must exist"],
            vec![ex("Revert commit", "lit revert abc123", serde_json::json!({"target": "abc123"}))],
            vec!["push", "log"], vec!["log"], false, false),
        cmd("cherry-pick", "Cherry-Pick Commit", "history", "Apply the changes from a specific commit onto the current branch",
            vec![param("target", "string", "Commit hash to cherry-pick", true, None)],
            "CherryPickResponse", vec!["Creates new commit with applied changes"], vec!["Target commit must exist"],
            vec![ex("Cherry-pick", "lit cherry-pick abc123", serde_json::json!({"target": "abc123"}))],
            vec!["push", "log"], vec!["log", "checkout"], false, false),
        cmd("rebase", "Rebase Branch", "history", "Reapply commits from the current branch onto a new base. Supports interactive mode with todo editing",
            vec![
                param("base", "string", "Base branch or commit to rebase onto", true, None),
                param("interactive", "boolean", "Interactive rebase with todo list", false, Some("false")),
                param("onto", "string", "Specific commit to rebase onto", false, None),
                param("abort", "boolean", "Abort an in-progress rebase", false, Some("false")),
                param("continue", "boolean", "Continue a paused rebase", false, Some("false")),
            ],
            "RebaseResponse", vec!["Rewrites commit history", "Updates HEAD"], vec!["Working tree must be clean"],
            vec![
                ex("Rebase onto main", "lit rebase main", serde_json::json!({"base": "main"})),
                ex("Interactive rebase", "lit rebase main --interactive", serde_json::json!({"base": "main", "interactive": true})),
            ],
            vec!["push --force", "log"], vec!["checkout"], false, false),
        cmd("blame", "Blame File", "history", "Show what revision and author last modified each line of a file",
            vec![param("file", "string", "File path to blame", true, None)],
            "BlameResponse", vec![], vec!["File must exist in repository"],
            vec![ex("Blame file", "lit blame src/main.rs", serde_json::json!({"file": "src/main.rs"}))],
            vec!["show", "log"], vec![], true, true),
        cmd("bisect", "Binary Search", "history", "Binary search through commit history to find the commit that introduced a bug",
            vec![
                param("command", "string", "Subcommand: start, good, bad, reset", true, None),
                param("commit", "string", "Commit hash (for good/bad)", false, None),
            ],
            "BisectResponse", vec!["Updates HEAD to test commits", "Saves state to .lit/bisect.json"], vec![],
            vec![
                ex("Start bisect", "lit bisect start", serde_json::json!({"command": "start"})),
                ex("Mark good", "lit bisect good abc123", serde_json::json!({"command": "good", "commit": "abc123"})),
            ],
            vec!["bisect good", "bisect bad", "bisect reset"], vec![], false, false),
        cmd("reflog", "Reference Log", "history", "Show the history of reference changes (HEAD updates, branch moves, etc.)",
            vec![
                param("ref_name", "string", "Reference name (default: HEAD)", false, Some("HEAD")),
                param("count", "integer", "Number of entries to show", false, Some("20")),
            ],
            "ReflogResponse", vec![], vec![],
            vec![ex("Show reflog", "lit reflog", serde_json::json!({}))],
            vec!["reset", "checkout"], vec![], true, true),
        cmd("resolve", "Resolve Conflicts", "branching", "Resolve merge conflicts using a specified strategy or finalize a merge after manual resolution",
            vec![
                param("file", "string", "Specific file to resolve", false, None),
                param("strategy", "string", "Resolution strategy: ours or theirs", false, None),
                param("all", "boolean", "Resolve all conflicting files", false, Some("false")),
                param("finish", "boolean", "Finalize merge after resolving all conflicts", false, Some("false")),
            ],
            "ResolveResponse", vec!["Modifies conflicting files", "May create merge commit"], vec!["Merge conflicts must exist"],
            vec![
                ex("Resolve all with ours", "lit resolve --all --strategy ours", serde_json::json!({"all": true, "strategy": "ours"})),
                ex("Finish merge", "lit resolve --continue", serde_json::json!({"finish": true})),
            ],
            vec!["commit", "push"], vec!["merge", "pull"], false, false),
        // Monitoring
        cmd("watch", "Watch Filesystem", "agent", "Monitor the working tree for file changes and emit a continuous stream of JSONL events",
            vec![
                param("debounce", "integer", "Debounce interval in milliseconds", false, Some("500")),
                param("filter", "string", "Glob pattern to filter watched files", false, None),
            ],
            "WatchResponse (continuous JSONL stream)", vec![], vec![],
            vec![ex("Watch with filter", "lit watch --filter '*.rs'", serde_json::json!({"filter": "*.rs"}))],
            vec!["snapshot"], vec![], true, true),
        // Transactions
        cmd("tx begin", "Begin Transaction", "agent", "Start a new transaction with write-ahead log for crash recovery",
            vec![],
            "TransactionResponse", vec!["Creates .lit/transaction.json and .lit/transaction.lock"], vec!["No other transaction active"],
            vec![ex("Begin transaction", "lit tx begin", serde_json::json!({}))],
            vec!["tx commit", "tx rollback"], vec![], false, false),
        cmd("tx commit", "Commit Transaction", "agent", "Commit the current transaction, finalizing all operations within it",
            vec![],
            "TransactionResponse", vec!["Removes transaction lock"], vec!["Transaction must be active"],
            vec![ex("Commit transaction", "lit tx commit", serde_json::json!({}))],
            vec![], vec!["tx begin"], false, false),
        cmd("tx rollback", "Rollback Transaction", "agent", "Rollback the current transaction, undoing all operations within it",
            vec![],
            "TransactionResponse", vec!["Restores pre-transaction state", "Removes transaction files"], vec!["Transaction must be active"],
            vec![ex("Rollback", "lit tx rollback", serde_json::json!({}))],
            vec![], vec!["tx begin"], false, false),
        // Server / API
        cmd("serve", "REST API Server", "server", "Start the Lit REST API server with optional bearer token authentication. Supports HTTP, stdio, and lit:// daemon modes",
            vec![
                param("port", "integer", "Port to listen on", false, Some("3000")),
                param("token", "string", "Bearer token for authentication (or LIT_API_TOKEN env)", false, None),
                param("stdio", "boolean", "Use stdio transport (for SSH pipe mode)", false, Some("false")),
                param("daemon", "boolean", "Run as lit:// protocol daemon (TCP, port 9418)", false, Some("false")),
            ],
            "ServeResponse", vec!["Starts long-running HTTP/TCP server"], vec!["Repository must be initialized"],
            vec![ex("Start server", "lit serve --port 3000 --token secret", serde_json::json!({"port": 3000, "token": "secret"}))],
            vec![], vec!["init"], false, false),
        cmd("mcp-serve", "MCP Tool Server", "server", "Start the Model Context Protocol (MCP) tool server for LLM agent integration. Exposes lit.* tools via JSON-RPC 2.0",
            vec![
                param("stdio", "boolean", "Use stdio transport (default)", false, Some("true")),
                param("port", "integer", "Use HTTP transport on specified port", false, None),
            ],
            "McpServeResponse", vec!["Starts long-running MCP server"], vec!["Repository must be initialized"],
            vec![
                ex("MCP stdio", "lit mcp-serve --stdio", serde_json::json!({"stdio": true})),
                ex("MCP HTTP", "lit mcp-serve --port 3001", serde_json::json!({"port": 3001})),
            ],
            vec![], vec!["init"], false, false),
        // Git interop
        cmd("import-git", "Import Git Repository", "interop", "Import a Git repository into Lit format, converting SHA-1 objects to SHA3-512+BLAKE3 composite hashes",
            vec![param("source", "string", "Path to Git repository (directory containing .git)", true, None)],
            "ImportGitResponse", vec!["Creates Lit objects from Git objects", "Creates Lit refs from Git refs"], vec!["Source must be a valid Git repository"],
            vec![ex("Import", "lit import-git /path/to/git-repo", serde_json::json!({"source": "/path/to/git-repo"}))],
            vec!["log", "status", "verify"], vec![], false, false),
        cmd("export-git", "Export to Git", "interop", "Export a Lit repository to Git format, converting composite hashes back to SHA-1",
            vec![param("destination", "string", "Destination path for the Git repository", true, None)],
            "ExportGitResponse", vec!["Creates bare Git repository at destination"], vec!["Lit repository must have commits"],
            vec![ex("Export", "lit export-git /path/to/output", serde_json::json!({"destination": "/path/to/output"}))],
            vec![], vec!["commit"], false, false),
        // Performance
        cmd("gc", "Garbage Collection", "performance", "Pack loose objects into pack files (LITP format) with CRC32 integrity, reducing disk usage and improving read performance",
            vec![],
            "GcResponse", vec!["Creates pack files", "Removes packed loose objects"], vec!["Repository must be initialized"],
            vec![ex("Run GC", "lit gc", serde_json::json!({}))],
            vec!["verify"], vec![], false, false),
        cmd("lfs track", "LFS Track Patterns", "performance", "Track file patterns for Large File Storage, writing rules to .litattributes",
            vec![param("patterns", "string[]", "Glob patterns to track (e.g., '*.bin', '*.dat')", true, None)],
            "LfsTrackResponse", vec!["Modifies .litattributes"], vec![],
            vec![ex("Track binaries", "lit lfs track '*.bin' '*.dat'", serde_json::json!({"patterns": ["*.bin", "*.dat"]}))],
            vec!["lfs migrate", "add"], vec!["init"], true, true),
        cmd("lfs migrate", "LFS Migrate", "performance", "Migrate existing large files to LFS pointer format, replacing content with lightweight references",
            vec![param("threshold", "integer", "Size threshold in bytes (default: 10MB)", false, Some("10485760"))],
            "LfsMigrateResponse", vec!["Replaces large blobs with LFS pointers"], vec!["LFS patterns must be configured"],
            vec![ex("Migrate large files", "lit lfs migrate --threshold 5242880", serde_json::json!({"threshold": 5242880}))],
            vec!["commit"], vec!["lfs track"], false, false),
        // Security
        cmd("rotate-key", "Rotate Encryption Key", "security", "Re-encrypt all repository objects and refs with a new passphrase. Prompts for old and new passphrases interactively",
            vec![],
            "RotateKeyResponse", vec!["Re-encrypts all objects, index, and refs"], vec!["Repository must be encrypted"],
            vec![ex("Rotate key", "lit rotate-key", serde_json::json!({}))],
            vec!["verify"], vec![], false, false),
        // Discovery
        cmd("ontology", "Show Ontology", "agent", "Output the complete Lit ontology as structured JSON for autonomous agent discovery. Includes all commands, types, workflows, protocols, and error categories",
            vec![],
            "OntologyResponse", vec![], vec![],
            vec![ex("Get ontology", "lit ontology", serde_json::json!({}))],
            vec![], vec![], true, true),
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
        Workflow {
            id: "git-migration".to_string(),
            name: "Git Migration".to_string(),
            description: "Import an existing Git repository into Lit format and verify integrity".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "import-git", "Import Git objects and refs into Lit", false),
                wstep(2, "verify", "Verify integrity of imported data", false),
                wstep(3, "log", "Review imported commit history", true),
                wstep(4, "branch --all", "List imported branches", true),
            ],
        },
        Workflow {
            id: "agent-code-review".to_string(),
            name: "Agent Code Review".to_string(),
            description: "Review changes on a branch, provide feedback via commits".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "log", "Review recent commit history", false),
                wstep(2, "diff", "Examine changes in detail", false),
                wstep(3, "blame", "Check authorship of specific files", true),
                wstep(4, "search", "Search for patterns or issues", true),
                wstep(5, "snapshot", "Commit review annotations as metadata", true),
            ],
        },
        Workflow {
            id: "agent-bisect".to_string(),
            name: "Automated Bug Bisection".to_string(),
            description: "Binary search through commit history to find the commit that introduced a regression".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "bisect start", "Begin bisection", false),
                wstep(2, "bisect bad", "Mark the known-bad commit", false),
                wstep(3, "bisect good", "Mark a known-good commit", false),
                wstep(4, "verify", "Test current commit (repeat until found)", false),
                wstep(5, "bisect reset", "End bisection session", false),
            ],
        },
        Workflow {
            id: "encrypted-repo".to_string(),
            name: "Encrypted Repository".to_string(),
            description: "Work with an encrypted repository using passphrase-based access".to_string(),
            agent_optimized: true,
            steps: vec![
                wstep(1, "init", "Initialize repository", false),
                wstep(2, "config set", "Configure encryption settings", false),
                wstep(3, "snapshot", "Create encrypted commits (passphrase via --passphrase or LIT_PASSPHRASE)", false),
                wstep(4, "rotate-key", "Periodically rotate encryption passphrase", true),
                wstep(5, "verify", "Verify encrypted integrity", true),
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
                "REPO_NOT_FOUND",
                "Not a Lit repository",
                true,
                "Run 'lit init' to create one",
            ),
            errc(
                "REPO_CORRUPT",
                "Repository data is corrupt or inconsistent",
                false,
                "Run 'lit verify' to diagnose — may require re-clone",
            ),
            errc(
                "NO_COMMITS",
                "No commits in repository",
                true,
                "Create files and run 'lit snapshot -m \"initial\"'",
            ),
            errc(
                "MERGE_CONFLICT",
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
                "REF_NOT_FOUND",
                "Branch, tag, or ref does not exist",
                true,
                "Run 'lit branch --all' or 'lit tag --list' to list available refs",
            ),
            errc(
                "REF_CONFLICT",
                "Reference already exists or conflicts with another",
                true,
                "Use a different name or delete the existing ref first",
            ),
            errc(
                "OBJECT_NOT_FOUND",
                "Object hash not found in store",
                false,
                "The object may be corrupt or missing — run 'lit verify'",
            ),
            errc(
                "INDEX_LOCKED",
                "Index is locked by another operation",
                true,
                "Wait for the other operation to complete or remove .lit/index.lock",
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
                "TRANSPORT_DENIED",
                "Network transport blocked or unavailable",
                true,
                "Disable --airgapped flag, use local file:// URLs, or check remote configuration",
            ),
            errc(
                "AUTH_FAILED",
                "Authentication failed for remote operation",
                true,
                "Check Bearer token (--token or LIT_API_TOKEN env) or verify credentials",
            ),
            errc(
                "CRYPTO_ERROR",
                "Encryption or decryption operation failed",
                true,
                "Verify passphrase is correct (--passphrase or LIT_PASSPHRASE env)",
            ),
            errc(
                "INVALID_INPUT",
                "Invalid argument or parameter value",
                true,
                "Check command help with 'lit <command> --help'",
            ),
            errc(
                "IO_ERROR",
                "File system read/write error",
                false,
                "Check file permissions and disk space",
            ),
            errc(
                "CONFIG_ERROR",
                "Configuration file is missing or malformed",
                true,
                "Run 'lit config show' to inspect or 'lit init' to recreate defaults",
            ),
            errc(
                "NOT_IMPLEMENTED",
                "Feature is not yet implemented",
                false,
                "This feature is planned for a future release",
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

// ============================================================================
// JSON Schema Generation
// ============================================================================

/// Map an ontology type string to a JSON Schema type
fn ontology_type_to_schema(type_name: &str) -> serde_json::Value {
    match type_name {
        "string" | "String" => serde_json::json!({ "type": "string" }),
        "boolean" | "bool" => serde_json::json!({ "type": "boolean" }),
        "integer" | "usize" | "i64" | "u64" | "i32" | "u32" => {
            serde_json::json!({ "type": "integer" })
        }
        "number" | "f64" | "f32" => serde_json::json!({ "type": "number" }),
        t if t.starts_with("array<") && t.ends_with('>') => {
            let inner = &t[6..t.len() - 1];
            serde_json::json!({
                "type": "array",
                "items": ontology_type_to_schema(inner)
            })
        }
        t if t.starts_with("optional<") && t.ends_with('>') => {
            let inner = &t[9..t.len() - 1];
            let mut schema = ontology_type_to_schema(inner);
            // Make nullable by allowing null
            if let Some(obj) = schema.as_object_mut() {
                if let Some(serde_json::Value::String(ty)) = obj.get("type").cloned() {
                    obj.insert("type".to_string(), serde_json::json!([ty, "null"]));
                }
            }
            schema
        }
        // Named types become $ref
        other => serde_json::json!({ "$ref": format!("#/$defs/{other}") }),
    }
}

/// Generate a JSON Schema for a single `TypeDef`
fn type_def_to_schema(td: &TypeDef) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for prop in &td.properties {
        let mut prop_schema = ontology_type_to_schema(&prop.type_name);
        if let Some(obj) = prop_schema.as_object_mut() {
            obj.insert(
                "description".to_string(),
                serde_json::Value::String(prop.description.clone()),
            );
        }
        properties.insert(prop.name.clone(), prop_schema);
        if prop.required {
            required.push(serde_json::Value::String(prop.name.clone()));
        }
    }

    serde_json::json!({
        "type": "object",
        "description": td.description,
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

/// Generate a JSON Schema for a single command's input parameters
fn command_input_schema(cmd: &CommandDef) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for param in &cmd.parameters {
        let mut param_schema = ontology_type_to_schema(&param.type_name);
        if let Some(obj) = param_schema.as_object_mut() {
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
        properties.insert(param.name.clone(), param_schema);
        if param.required {
            required.push(serde_json::Value::String(param.name.clone()));
        }
    }

    serde_json::json!({
        "type": "object",
        "description": format!("Input parameters for '{}'", cmd.name),
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

/// Generate the full JSON Schema document for the Lit ontology.
///
/// Produces a standard JSON Schema (draft 2020-12) with:
/// - `$defs` containing a schema for every ontology type
/// - `commands` containing input schemas for every command
pub fn generate_schemas() -> serde_json::Value {
    let ont = get_ontology();

    // Build $defs from types
    let mut defs = serde_json::Map::new();
    for td in &ont.types {
        defs.insert(td.id.clone(), type_def_to_schema(td));
    }

    // Build command schemas
    let mut commands = serde_json::Map::new();
    for cmd in &ont.commands {
        commands.insert(
            cmd.id.clone(),
            serde_json::json!({
                "description": cmd.description,
                "category": cmd.category,
                "input": command_input_schema(cmd),
                "returns": cmd.returns,
                "idempotent": cmd.idempotent,
                "safe": cmd.safe,
                "side_effects": cmd.side_effects,
                "preconditions": cmd.preconditions,
            }),
        );
    }

    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://lit-vcs.dev/schema/v1",
        "title": "Lit VCS Schema",
        "description": "JSON Schema for the Lit version control system — types and command interfaces for agent discovery",
        "version": ont.version,
        "$defs": defs,
        "commands": commands
    })
}

/// Generate a JSON Schema for a single command by its ID.
///
/// Returns `None` if the command is not found.
pub fn generate_command_schema(command_id: &str) -> Option<serde_json::Value> {
    let ont = get_ontology();
    ont.commands.iter().find(|c| c.id == command_id).map(|cmd| {
        serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": format!("https://lit-vcs.dev/schema/v1/commands/{}", cmd.id),
            "title": format!("lit {}", cmd.name),
            "description": cmd.description,
            "input": command_input_schema(cmd),
            "returns": cmd.returns,
            "idempotent": cmd.idempotent,
            "safe": cmd.safe,
        })
    })
}
