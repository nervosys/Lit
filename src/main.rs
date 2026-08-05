// The CLI is a consumer of the library, not a second copy of it. Declaring
// these as modules here would compile every one of them a second time into
// this binary — and run their unit tests twice — so they are imported from
// `lit` instead.
use lit::{
    commands, config, core, crypto, errors, events, federation, formatter, identity, network,
    ontology, response,
};

use clap::{Parser, Subcommand};
use formatter::Format;
use std::process;

#[derive(Parser)]
#[command(name = "lit")]
#[command(about = "Lit - The agentic-first distributed version control system", long_about = None)]
// Takes the version from Cargo.toml, so `lit --version` and `-V` answer the
// question every CLI is expected to answer. Without this clap rejects both as
// unknown arguments, which left a released binary unable to say what it was.
#[command(version)]
struct Cli {
    /// Enable airgap mode (blocks all network protocols, allows only physical transports)
    #[arg(long, global = true)]
    airgapped: bool,

    /// Output as JSON (default for agents)
    #[arg(long, global = true)]
    json: bool,

    /// Output as human-readable text
    #[arg(long, global = true)]
    human: bool,

    /// Pretty-print (indent) JSON output. Token-heavy; for human inspection.
    #[arg(long, global = true, env = "LIT_PRETTY")]
    pretty: bool,

    /// Output format: json, human, msgpack
    #[arg(long, global = true, env = "LIT_OUTPUT")]
    output: Option<String>,

    /// Passphrase for encrypted repositories (avoids interactive prompt)
    #[arg(long, global = true, env = "LIT_PASSPHRASE", hide_env_values = true)]
    passphrase: Option<String>,

    /// Path to file containing passphrase (avoids interactive prompt)
    #[arg(long, global = true, env = "LIT_PASSPHRASE_FILE")]
    passphrase_file: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Lit repository
    Init {
        /// Create a bare repository
        #[arg(long)]
        bare: bool,

        /// Path to initialize (defaults to current directory)
        path: Option<String>,
    },

    /// Add file contents to the staging area
    Add {
        /// Files to add
        files: Vec<String>,
    },

    /// Record changes to the repository
    Commit {
        /// Commit message
        #[arg(short, long)]
        message: String,

        /// Author name (optional)
        #[arg(short, long)]
        author: Option<String>,

        /// Attach this commit to an active intent
        #[arg(long)]
        intent: Option<String>,
    },

    /// Show the working tree status
    Status,

    /// Show commit logs
    Log {
        /// Number of commits to show
        #[arg(short, long, default_value = "10")]
        count: usize,

        /// Show one line per commit
        #[arg(long)]
        oneline: bool,
    },

    /// List, create, or delete branches
    Branch {
        /// Branch name to create
        name: Option<String>,

        /// Delete branch
        #[arg(short, long)]
        delete: bool,

        /// List all branches
        #[arg(short, long)]
        all: bool,
    },

    /// Switch branches or restore working tree files
    Checkout {
        /// Branch or commit to checkout
        target: String,

        /// Create new branch
        #[arg(short, long)]
        b: bool,
    },

    /// Join two or more development histories together
    Merge {
        /// Branch to merge into current branch
        branch: String,

        /// Merge strategy (recursive, ours, theirs)
        #[arg(long, default_value = "recursive")]
        strategy: Option<String>,
    },

    /// Resolve merge conflicts
    Resolve {
        /// File to resolve
        file: Option<String>,

        /// Resolution strategy (ours or theirs)
        #[arg(long)]
        strategy: Option<String>,

        /// Resolve all conflicting files
        #[arg(long)]
        all: bool,

        /// Finalize merge after resolving all conflicts
        #[arg(long, name = "continue")]
        finish: bool,
    },

    /// Show various types of objects
    Show {
        /// Object hash or reference
        object: String,
    },

    /// Manage set of tracked repositories (LAN only)
    Remote {
        #[command(subcommand)]
        command: Option<RemoteCommands>,
    },

    /// Update remote refs along with associated objects (LAN only)
    Push {
        /// Remote name
        remote: String,

        /// Branch name
        branch: String,

        /// Force push
        #[arg(short, long)]
        force: bool,
    },

    /// Fetch from and integrate with another repository (LAN only)
    Pull {
        /// Remote name
        remote: String,

        /// Branch name
        branch: String,
    },

    /// Download objects and refs from a remote repository (LAN only)
    Fetch {
        /// Remote name
        remote: String,

        /// Branch name (fetch all if omitted)
        branch: Option<String>,
    },

    /// Clone a repository into a new directory (LAN only)
    Clone {
        /// Repository URL (must be LAN)
        url: String,

        /// Directory name
        directory: Option<String>,
    },

    /// Show configuration settings
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },

    /// Show changes between commits, working tree, and index
    Diff {
        /// Show staged changes (index vs HEAD)
        #[arg(long)]
        staged: bool,

        /// Show diffstat summary only
        #[arg(long)]
        stat: bool,

        /// Show word-level inline diff
        #[arg(long)]
        word_diff: bool,

        /// First ref (commit or branch)
        ref1: Option<String>,

        /// Second ref (commit or branch, defaults to HEAD)
        ref2: Option<String>,
    },

    /// Create, list, delete, or verify tags
    Tag {
        /// Tag name (for create/delete/verify)
        name: Option<String>,

        /// Create annotated tag
        #[arg(short, long)]
        annotate: bool,

        /// Tag message (implies annotated)
        #[arg(short, long)]
        message: Option<String>,

        /// Delete a tag
        #[arg(short, long)]
        delete: bool,

        /// Sign tag with post-quantum signature (ML-DSA-87)
        #[arg(long)]
        sign: bool,

        /// Verify tag signature
        #[arg(long)]
        verify: bool,

        /// List all tags
        #[arg(short, long)]
        list: bool,

        /// Target commit (defaults to HEAD)
        #[arg(long)]
        commit: Option<String>,
    },

    /// Rotate encryption passphrase
    RotateKey,

    /// Encrypt a repository created before encryption was enabled
    MigrateEncryption,

    // --- Phase 1.5-1.8 ---
    /// Stash changes temporarily
    Stash {
        #[command(subcommand)]
        command: Option<StashCommands>,
    },

    /// Reset current HEAD to a specified state
    Reset {
        /// Target commit
        target: String,

        /// Keep changes in staging
        #[arg(long)]
        soft: bool,

        /// Discard all changes
        #[arg(long)]
        hard: bool,
    },

    /// Revert a commit by creating a new inverse commit
    Revert {
        /// Commit hash to revert
        target: String,
    },

    /// Apply a commit from another branch
    CherryPick {
        /// Commit hash to cherry-pick
        target: String,
    },

    /// Rebase current branch onto another base
    Rebase {
        /// Base branch or commit
        base: String,

        /// Interactive rebase
        #[arg(short, long)]
        interactive: bool,

        /// Rebase onto a specific commit
        #[arg(long)]
        onto: Option<String>,

        /// Abort an in-progress rebase
        #[arg(long)]
        abort: bool,

        /// Continue a paused rebase
        #[arg(long, name = "continue")]
        cont: bool,
    },

    /// Show what revision and author last modified each line of a file
    Blame {
        /// File to blame
        file: String,
    },

    /// Binary search to find the commit that introduced a bug
    Bisect {
        #[command(subcommand)]
        command: Option<BisectCommands>,
    },

    /// Show reference log history
    Reflog {
        /// Reference name (default: HEAD)
        #[arg(long)]
        ref_name: Option<String>,

        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        count: usize,
    },

    // --- Phase 2: Agentic Features ---
    /// Execute multiple operations from JSONL stdin
    Batch {
        /// Stop on first failure
        #[arg(long)]
        atomic: bool,

        /// Validate without executing
        #[arg(long)]
        dry_run: bool,
    },

    /// Transaction operations (begin/commit/rollback)
    Tx {
        #[command(subcommand)]
        command: TxCommands,
    },

    /// Atomic add-all + commit in one step
    Snapshot {
        /// Commit message
        #[arg(short, long)]
        message: String,

        /// Author name
        #[arg(short, long)]
        author: Option<String>,

        /// Agent metadata as JSON string
        #[arg(long)]
        metadata: Option<String>,
    },

    /// Search file contents, commit messages, or metadata
    Search {
        /// Search query
        query: String,

        /// Search commit messages instead of files
        #[arg(long)]
        messages: bool,

        /// Search metadata (key=value)
        #[arg(long)]
        metadata: Option<String>,

        /// Maximum results
        #[arg(long, default_value = "100")]
        max_results: usize,
    },

    /// Monitor filesystem for changes (emit JSONL events)
    Watch {
        /// Debounce interval in milliseconds
        #[arg(long, default_value = "500")]
        debounce: u64,

        /// Filter pattern (glob)
        #[arg(long)]
        filter: Option<String>,
    },

    /// Run full repository integrity check
    Verify,

    // --- Phase 3: API Server & MCP ---
    /// Start the Lit REST API server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3000")]
        port: u16,

        /// Bearer token for authentication
        #[arg(long, env = "LIT_API_TOKEN", hide_env_values = true)]
        token: Option<String>,

        /// Use stdio transport (for SSH pipe mode)
        #[arg(long)]
        stdio: bool,

        /// Run as lit:// protocol daemon (TCP, port 9418 default)
        #[arg(long)]
        daemon: bool,
    },

    /// Start the MCP (Model Context Protocol) tool server
    McpServe {
        /// Use stdio transport (default)
        #[arg(long)]
        stdio: bool,

        /// Use HTTP transport on specified port
        #[arg(long)]
        port: Option<u16>,
    },

    /// Multi-agent swarm coordination
    Swarm {
        #[command(subcommand)]
        command: SwarmCommands,
    },

    // --- Phase 4: Git Interop ---
    /// Import a Git repository into Lit format
    ImportGit {
        /// Path to Git repository (directory containing .git)
        source: String,
    },

    /// Export a Lit repository to Git format
    ExportGit {
        /// Destination path for Git repository
        destination: String,
    },

    // --- Phase 5: Performance ---
    /// Garbage collection - pack loose objects
    Gc,

    /// Large File Storage operations
    Lfs {
        #[command(subcommand)]
        command: LfsCommands,
    },
    /// Output the Lit ontology for agent discovery
    Ontology,
    /// Generate JSON Schema from the ontology for agent SDK discovery
    Schema {
        /// Optional command ID to generate schema for a single command
        #[arg(long)]
        command: Option<String>,
    },

    // --- Sandbox ---
    /// Run repos in an isolated sandbox (restricted filesystem, env, and network)
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommands,
    },

    // --- Phase 6: Decentralized Identity & Federation ---
    /// Manage decentralized identity (DID)
    Did {
        #[command(subcommand)]
        command: DidCommands,
    },

    /// Issue and manage UCAN capability delegation tokens
    Ucan {
        #[command(subcommand)]
        command: UcanCommands,
    },

    /// Agent trust scoring
    Trust {
        #[command(subcommand)]
        command: TrustCommands,
    },

    /// Local-first issue tracker (stored as refs)
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },

    /// Local-first pull requests (stored as refs)
    Pr {
        #[command(subcommand)]
        command: PrCommands,
    },

    /// Event subscriptions and notifications
    Subscribe {
        #[command(subcommand)]
        command: SubscribeCommands,
    },

    /// Agent task delegation protocol
    Delegate {
        #[command(subcommand)]
        command: DelegateCommands,
    },

    /// Declare an intent — a scoped unit of agentic work
    Intent {
        #[command(subcommand)]
        command: IntentCommands,
    },

    /// Converge an intent's commits into the mainline
    Converge {
        /// Intent ID to converge
        intent_id: String,

        /// Strategy: auto (default), rebase, squash, accumulate
        #[arg(long, default_value = "auto")]
        strategy: Option<String>,

        /// Verify commit objects before converging
        #[arg(long)]
        verify: bool,

        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,

        /// Target branch (defaults to current branch)
        #[arg(long)]
        target: Option<String>,
    },

    // --- Generic Versioning & Datacenter ---
    /// Content type registry — register, detect, and manage content types
    /// for CAD, EDA, manuscripts, databases, scientific data, media, and more
    ContentType {
        #[command(subcommand)]
        command: ContentTypeCommands,
    },

    /// Datacenter deployment — cluster management, sharding, replication,
    /// health monitoring, and Prometheus-style metrics
    Datacenter {
        #[command(subcommand)]
        command: DatacenterCommands,
    },

    /// Agent profiles — domain-specific agent types with capabilities,
    /// trust levels, content type affinity, and resource limits
    AgentProfile {
        #[command(subcommand)]
        command: AgentProfileCommands,
    },

    /// Content-addressed federation and peer management
    Peer {
        #[command(subcommand)]
        command: PeerCommands,
    },

    // --- GitButler-parity features ---
    /// Amend the most recent commit with staged changes
    Amend {
        /// New commit message (keeps original if omitted)
        #[arg(short, long)]
        message: Option<String>,

        /// New author name (keeps original if omitted)
        #[arg(short, long)]
        author: Option<String>,
    },

    /// Reword the message of the most recent commit
    Reword {
        /// New commit message
        message: String,

        /// Target commit hash (defaults to HEAD)
        #[arg(long)]
        target: Option<String>,
    },

    /// Squash the last N commits into one
    Squash {
        /// Number of commits to squash
        count: usize,

        /// Squash commit message (auto-generated if omitted)
        #[arg(short, long)]
        message: Option<String>,
    },

    /// Uncommit the last commit, keeping changes in the working tree
    Uncommit {
        /// Discard the committed content instead of keeping it
        #[arg(long)]
        discard: bool,
    },

    /// Auto-assign working directory changes to the correct ancestor commits
    Absorb {
        /// Base branch/commit to stop at
        #[arg(long)]
        base: Option<String>,

        /// Show what would happen without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Operation log and undo timeline
    Undo {
        #[command(subcommand)]
        command: UndoCommands,
    },

    /// Stacked branch management
    Stack {
        #[command(subcommand)]
        command: StackCommands,
    },

    /// Parallel (virtual) branch workspace
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommands,
    },

    /// Remove empty branches
    Clean {
        /// Show what would be removed without removing
        #[arg(long)]
        dry_run: bool,
    },

    /// AI-assisted generation (commit messages, branch names, PR descriptions)
    Ai {
        #[command(subcommand)]
        command: AiCommands,
    },
}

#[derive(Subcommand)]
enum RemoteCommands {
    /// Add a remote
    Add {
        /// Remote name
        name: String,
        /// Remote URL (must be LAN)
        url: String,
    },

    /// Remove a remote
    Remove {
        /// Remote name
        name: String,
    },

    /// List remotes
    List {
        /// Show URLs
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show all configuration
    Show,

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
}

#[derive(Subcommand)]
enum StashCommands {
    /// Save changes to stash
    Push {
        /// Stash message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Pop the latest stash entry
    Pop,
    /// Apply a stash entry without removing it
    Apply {
        /// Stash index
        index: Option<usize>,
    },
    /// List all stash entries
    List,
    /// Drop a stash entry
    Drop {
        /// Stash index
        index: Option<usize>,
    },
}

#[derive(Subcommand)]
enum BisectCommands {
    /// Start bisecting
    Start,
    /// Mark current commit as good
    Good {
        /// Commit hash
        commit: String,
    },
    /// Mark current commit as bad
    Bad {
        /// Commit hash
        commit: String,
    },
    /// End bisect session
    Reset,
}

// Translating the parsed CLI into the library's own subcommand types.
//
// The enums above carry clap's derive and its help text; the library's
// counterparts are plain data, which is what keeps clap out of its public API.
// The two sets are deliberately identical in shape, so these are mechanical —
// and being exhaustive matches, a variant added on one side without the other
// fails to compile rather than going unnoticed.
//
// These are free functions rather than `From` impls because both the trait and
// the target type are foreign here, which the orphan rule forbids.

fn to_lib_remote(command: RemoteCommands) -> lit::RemoteCommands {
    match command {
        RemoteCommands::Add { name, url } => lit::RemoteCommands::Add { name, url },
        RemoteCommands::Remove { name } => lit::RemoteCommands::Remove { name },
        RemoteCommands::List { verbose } => lit::RemoteCommands::List { verbose },
    }
}

fn to_lib_config(command: ConfigCommands) -> lit::ConfigCommands {
    match command {
        ConfigCommands::Show => lit::ConfigCommands::Show,
        ConfigCommands::Get { key } => lit::ConfigCommands::Get { key },
        ConfigCommands::Set { key, value } => lit::ConfigCommands::Set { key, value },
    }
}

fn to_lib_stash(command: StashCommands) -> lit::StashCommands {
    match command {
        StashCommands::Push { message } => lit::StashCommands::Push { message },
        StashCommands::Pop => lit::StashCommands::Pop,
        StashCommands::Apply { index } => lit::StashCommands::Apply { index },
        StashCommands::List => lit::StashCommands::List,
        StashCommands::Drop { index } => lit::StashCommands::Drop { index },
    }
}

fn to_lib_bisect(command: BisectCommands) -> lit::BisectCommands {
    match command {
        BisectCommands::Start => lit::BisectCommands::Start,
        BisectCommands::Good { commit } => lit::BisectCommands::Good { commit },
        BisectCommands::Bad { commit } => lit::BisectCommands::Bad { commit },
        BisectCommands::Reset => lit::BisectCommands::Reset,
    }
}

#[derive(Subcommand)]
enum TxCommands {
    /// Begin a new transaction
    Begin,
    /// Commit the current transaction
    Commit,
    /// Rollback the current transaction
    Rollback,
}

#[derive(Subcommand)]
enum SwarmCommands {
    /// Register an agent in the swarm
    Register {
        /// Unique agent identifier
        agent_id: String,
    },
    /// List registered agents
    List,
    /// Acquire exclusive file lease
    LeaseAcquire {
        /// Agent ID
        #[arg(long)]
        agent: String,
        /// File path to lease
        #[arg(long)]
        path: String,
        /// Lease duration in seconds
        #[arg(long, default_value = "300")]
        duration: u64,
    },
    /// Release a file lease
    LeaseRelease {
        /// Agent ID
        #[arg(long)]
        agent: String,
        /// File path to release
        #[arg(long)]
        path: String,
    },
    /// List all active leases
    LeaseList,
}

#[derive(Subcommand)]
enum LfsCommands {
    /// Track file patterns for LFS
    Track {
        /// Glob patterns to track (e.g., "*.bin", "*.dat")
        patterns: Vec<String>,
    },
    /// Migrate existing large files to LFS
    Migrate {
        /// Size threshold in bytes (default: 10MB)
        #[arg(long)]
        threshold: Option<u64>,
    },
}

#[derive(Subcommand)]
enum SandboxCommands {
    /// Create a new sandbox from the current repo
    Init {
        /// Sandbox name (auto-generated if omitted)
        name: Option<String>,
    },
    /// Run a command inside a sandbox with restricted environment
    Run {
        /// Sandbox name
        name: String,
        /// Command and arguments to run (after --)
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    /// List all sandboxes
    List,
    /// Destroy a sandbox
    Destroy {
        /// Sandbox name
        name: String,
    },
}

#[derive(Subcommand)]
enum DidCommands {
    /// Generate a new DID identity
    Generate {
        /// Key method: ed25519 (default) or ml-dsa-87
        #[arg(long, default_value = "ed25519")]
        method: String,
    },
    /// Show current DID identity
    Show,
    /// Resolve a DID to its document
    Resolve {
        /// DID to resolve
        did: String,
    },
}

#[derive(Subcommand)]
enum UcanCommands {
    /// Issue a UCAN capability delegation token
    Issue {
        /// Audience DID (the agent receiving capabilities)
        audience: String,
        /// Resource (e.g., "repo:*", "branch:main", "file:src/*")
        #[arg(long)]
        resource: String,
        /// Action (e.g., "push", "commit", "merge")
        #[arg(long)]
        action: String,
        /// Duration in seconds (default: 3600)
        #[arg(long, default_value = "3600")]
        duration: i64,
    },
    /// List UCAN tokens
    List {
        /// Filter by audience DID
        audience: Option<String>,
    },
    /// Revoke a UCAN token
    Revoke {
        /// Token CID (or prefix)
        cid: String,
    },
}

#[derive(Subcommand)]
enum TrustCommands {
    /// Show trust score for an agent
    Show {
        /// Agent DID
        did: String,
    },
    /// List all tracked agents
    List,
    /// Show trust event history
    History {
        /// Agent DID
        did: String,
    },
}

#[derive(Subcommand)]
enum IssueCommands {
    /// Create a new issue
    Create {
        /// Issue title
        title: String,
        /// Issue body
        #[arg(long, default_value = "")]
        body: String,
        /// Labels
        #[arg(long)]
        label: Vec<String>,
    },
    /// List issues
    List {
        /// Filter by state: open, closed, all
        #[arg(long, default_value = "open")]
        state: String,
    },
    /// Show issue details
    Show {
        /// Issue ID
        id: u64,
    },
    /// Close an issue
    Close {
        /// Issue ID
        id: u64,
    },
    /// Add a comment to an issue
    Comment {
        /// Issue ID
        id: u64,
        /// Comment body
        body: String,
    },
}

#[derive(Subcommand)]
enum PrCommands {
    /// Create a new pull request
    Create {
        /// PR title
        title: String,
        /// PR body
        #[arg(long, default_value = "")]
        body: String,
        /// Source (head) branch
        #[arg(long)]
        head: String,
        /// Target (base) branch
        #[arg(long, default_value = "main")]
        base: String,
        /// Labels
        #[arg(long)]
        label: Vec<String>,
    },
    /// List pull requests
    List {
        /// Filter by state: open, merged, closed, all
        #[arg(long, default_value = "open")]
        state: String,
    },
    /// Show PR details
    Show {
        /// PR ID
        id: u64,
    },
    /// Merge a pull request
    Merge {
        /// PR ID
        id: u64,
    },
    /// Close a pull request
    Close {
        /// PR ID
        id: u64,
    },
    /// Add a comment to a PR
    Comment {
        /// PR ID
        id: u64,
        /// Comment body
        body: String,
    },
}

#[derive(Subcommand)]
enum SubscribeCommands {
    /// Subscribe to event types
    Add {
        /// Event types (e.g., commit-pushed, branch-created, issue-opened)
        event_types: Vec<String>,
        /// Optional branch filter
        #[arg(long)]
        branch: Option<String>,
    },
    /// List active subscriptions
    List,
    /// Remove a subscription
    Remove {
        /// Subscription ID
        id: String,
    },
    /// Read recent events
    Events {
        /// Filter by event type
        #[arg(long)]
        event_type: Option<String>,
        /// Max events to show
        #[arg(long, default_value = "50")]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum DelegateCommands {
    /// Create a new delegated task
    Create {
        /// Delegatee agent DID
        to: String,
        /// Task title
        title: String,
        /// Task description
        #[arg(long, default_value = "")]
        description: String,
        /// Priority: low, medium (default), high, critical
        #[arg(long, default_value = "medium")]
        priority: String,
        /// File/path scope
        #[arg(long)]
        scope: Vec<String>,
    },
    /// Accept a delegated task
    Accept {
        /// Task ID
        task_id: String,
    },
    /// Complete a delegated task
    Complete {
        /// Task ID
        task_id: String,
        /// Result summary
        result: String,
    },
    /// List delegated tasks
    List {
        /// Filter by agent DID
        #[arg(long)]
        agent: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
    },
    /// Show task details
    Show {
        /// Task ID
        task_id: String,
    },
}

#[derive(Subcommand)]
enum IntentCommands {
    /// Create a new intent with declared scope
    Create {
        /// Intent title
        title: String,
        /// Agent identifier (DID or name)
        #[arg(long)]
        agent: String,
        /// File/path scope patterns (e.g. src/auth/**)
        #[arg(long)]
        scope: Vec<String>,
        /// Priority: low, medium (default), high, critical
        #[arg(long, default_value = "medium")]
        priority: String,
        /// Parent intent ID for hierarchical decomposition
        #[arg(long)]
        parent: Option<String>,
        /// UCAN proof token
        #[arg(long)]
        ucan_proof: Option<String>,
    },
    /// List intents
    List {
        /// Filter by status: active, converged, abandoned
        #[arg(long)]
        status: Option<String>,
        /// Filter by agent
        #[arg(long)]
        agent: Option<String>,
    },
    /// Show details of an intent
    Show {
        /// Intent ID
        intent_id: String,
    },
    /// Close (abandon) an intent
    Close {
        /// Intent ID
        intent_id: String,
    },
}

#[derive(Subcommand)]
enum ContentTypeCommands {
    /// List all registered content types
    List {
        /// Filter by domain (e.g. cad, eda, manuscript, database, scientific, media)
        #[arg(long)]
        domain: Option<String>,
    },
    /// Show details of a content type
    Show {
        /// Content type ID (e.g. cad/step, eda/kicad-pcb, db/sqlite)
        type_id: String,
    },
    /// Register a custom content type
    Register {
        /// Unique type ID (e.g. custom/my-format)
        id: String,
        /// Human-readable name
        #[arg(long)]
        name: String,
        /// Domain: software, cad, eda, manuscript, database, scientific, media, geospatial, legal, financial, config, documentation
        #[arg(long)]
        domain: String,
        /// File extensions (comma-separated, without dot)
        #[arg(long, value_delimiter = ',')]
        extensions: Vec<String>,
        /// Diff strategy: text, binary, structural, semantic, opaque
        #[arg(long)]
        diff_strategy: Option<String>,
        /// Merge strategy: text-three-way, manual-resolve, schema-aware, component-level, append-only, last-writer-wins
        #[arg(long)]
        merge_strategy: Option<String>,
        /// Storage tier: standard, lfs, chunked, external
        #[arg(long)]
        storage_tier: Option<String>,
    },
    /// Detect content type of file(s)
    Detect {
        /// File path(s) to examine
        paths: Vec<String>,
    },
}

#[derive(Subcommand)]
enum DatacenterCommands {
    /// Show cluster status — nodes, shards, config summary
    Status,
    /// Register a new cluster node
    RegisterNode {
        /// Unique node ID
        node_id: String,
        /// Human-readable name
        #[arg(long)]
        name: String,
        /// Network endpoint (host:port or URL)
        #[arg(long)]
        endpoint: String,
        /// Region / availability zone
        #[arg(long)]
        region: String,
        /// Role: primary, replica, relay, observer
        #[arg(long)]
        role: Option<String>,
    },
    /// Remove a node from the cluster
    RemoveNode {
        /// Node ID to remove
        node_id: String,
    },
    /// Configure cluster settings
    Configure {
        /// Object replication factor (e.g. 3)
        #[arg(long)]
        replication_factor: Option<u32>,
        /// Number of virtual shards (e.g. 256)
        #[arg(long)]
        shard_count: Option<u32>,
        /// Shard strategy: consistent-hash, range-prefix, round-robin, domain-affinity
        #[arg(long)]
        shard_strategy: Option<String>,
        /// Replication mode: sync, async, semi-sync
        #[arg(long)]
        replication_mode: Option<String>,
        /// Connection pool size per node
        #[arg(long)]
        connection_pool_size: Option<u32>,
        /// Enable or disable Prometheus metrics endpoint
        #[arg(long)]
        metrics_enabled: Option<bool>,
        /// Metrics endpoint port
        #[arg(long)]
        metrics_port: Option<u16>,
        /// Write concern — nodes that must confirm a write
        #[arg(long)]
        write_concern: Option<u32>,
    },
    /// Run health checks on all registered nodes
    Health,
    /// Collect Prometheus-style metrics
    Metrics,
}

#[derive(Subcommand)]
enum AgentProfileCommands {
    /// List all agent profiles
    List {
        /// Filter by domain (e.g. software, cad, eda, writer, dba, reviewer)
        #[arg(long)]
        domain: Option<String>,
    },
    /// Show details of an agent profile
    Show {
        /// Profile ID (e.g. swe-default, cad-designer, dba)
        profile_id: String,
    },
    /// Register a custom agent profile
    Register {
        /// Unique profile ID
        profile_id: String,
        /// Human-readable name
        #[arg(long)]
        name: String,
        /// Domain: software, cad, eda, writer, dba, reviewer, ci, security, data-science, devops, qa, general
        #[arg(long)]
        domain: String,
        /// Capabilities (comma-separated): read, write, branch, merge, review, deploy, test, diff, lfs, intent, converge, orchestrate, etc.
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,
        /// Trust level: untrusted, limited, standard, elevated, admin
        #[arg(long)]
        trust_level: Option<String>,
        /// Supported content type IDs (comma-separated)
        #[arg(long, value_delimiter = ',')]
        content_types: Vec<String>,
        /// Allowed path patterns (comma-separated globs)
        #[arg(long, value_delimiter = ',')]
        allowed_paths: Vec<String>,
        /// Denied path patterns (comma-separated globs)
        #[arg(long, value_delimiter = ',')]
        denied_paths: Vec<String>,
    },
    /// List capabilities across domains
    Capabilities {
        /// Filter by domain
        #[arg(long)]
        domain: Option<String>,
    },
    /// Remove a custom agent profile
    Remove {
        /// Profile ID to remove
        profile_id: String,
    },
}

#[derive(Subcommand)]
enum PeerCommands {
    /// Add a federation peer
    Add {
        /// Peer DID
        did: String,
        /// Network endpoint
        #[arg(long)]
        endpoint: String,
        /// Peer public key (hex)
        #[arg(long)]
        public_key: String,
        /// Human-readable alias
        #[arg(long)]
        alias: Option<String>,
    },
    /// Remove a peer
    Remove {
        /// Peer DID
        did: String,
    },
    /// List all peers
    List,
    /// Show peer details
    Show {
        /// Peer DID
        did: String,
    },
    /// Sync with a peer
    Sync {
        /// Peer DID
        did: String,
    },
}

#[derive(Subcommand)]
enum UndoCommands {
    /// List recent operations
    List {
        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        count: usize,
    },
    /// Undo the last operation (or a specific one by ID)
    Revert {
        /// Operation ID to undo (defaults to most recent)
        id: Option<u64>,
    },
    /// Redo a previously undone operation
    Redo {
        /// Operation ID to redo (defaults to most recent undone)
        id: Option<u64>,
    },
}

#[derive(Subcommand)]
enum StackCommands {
    /// List all stacked branch chains
    List,
    /// Push a new branch onto the current branch (create a stacked branch)
    Push {
        /// Name for the new stacked branch
        name: String,
    },
    /// Restack all dependent branches after amending/editing commits
    Restack,
    /// Show the stack containing the current branch
    Show,
}

#[derive(Subcommand)]
enum WorkspaceCommands {
    /// List all virtual branches in the workspace
    List,
    /// Create a new virtual branch
    Create {
        /// Branch name
        name: String,
    },
    /// Apply a virtual branch to the workspace
    Apply {
        /// Branch name
        name: String,
    },
    /// Unapply a virtual branch from the workspace
    Unapply {
        /// Branch name
        name: String,
    },
    /// Move a file from one virtual branch to another
    MoveFile {
        /// File path
        file: String,
        /// Source branch
        #[arg(long)]
        from: String,
        /// Destination branch
        #[arg(long)]
        to: String,
    },
}

#[derive(Subcommand)]
enum AiCommands {
    /// Generate a commit message from staged changes
    CommitMessage {
        /// Additional context for the AI
        #[arg(long)]
        context: Option<String>,
    },
    /// Generate a branch name from a description
    BranchName {
        /// Description of the work
        description: String,
    },
    /// Generate a PR description from branch diff
    PrDescription {
        /// Head branch (defaults to current)
        #[arg(long)]
        head: Option<String>,
        /// Base branch (defaults to main)
        #[arg(long)]
        base: Option<String>,
    },
}

fn main() {
    // Use a thread with a larger stack to avoid stack overflow in debug builds.
    // The Commands enum + route_request match arms produce large stack frames
    // that exceed the default 8 MB stack in unoptimized debug builds on Windows.
    const STACK_SIZE: usize = 16 * 1024 * 1024; // 16 MB

    let builder = std::thread::Builder::new()
        .name("lit-main".to_string())
        .stack_size(STACK_SIZE);

    let handler = builder.spawn(run).expect("Failed to spawn main thread");

    if let Err(e) = handler.join() {
        // Re-panic on the main thread so the exit code is non-zero
        std::panic::resume_unwind(e);
    }
}

fn run() {
    // FIPS 140-3 IG 9.6: Run power-on self-tests before any crypto operations
    let crypto_config = crypto::CryptoConfig::load();
    if crypto_config.enable_self_tests {
        let mut fips = crypto::fips::FipsModule::new();
        if let Err(e) = fips.power_on_self_test() {
            eprintln!("FATAL: FIPS power-on self-test failed: {}", e);
            eprintln!("Cryptographic module integrity cannot be verified. Aborting.");
            process::exit(1);
        }
    }

    let cli = Cli::parse();

    // Enable airgap mode if flag is set
    if cli.airgapped {
        network::AirgapConfig::enable_airgap_mode();
        eprintln!("🔒 AIRGAP MODE ENABLED - Network protocols blocked");
    }

    // Set passphrase env var from CLI flags (so encryption functions pick it up)
    if let Some(passphrase) = &cli.passphrase {
        // SAFETY: Called in single-threaded main() before any thread spawning
        unsafe {
            std::env::set_var("LIT_PASSPHRASE", passphrase);
        }
    } else if let Some(file_path) = &cli.passphrase_file {
        // SAFETY: Called in single-threaded main() before any thread spawning
        unsafe {
            std::env::set_var("LIT_PASSPHRASE_FILE", file_path);
        }
    }

    // SECURITY: Clear passphrase from environment immediately after encryption
    // subsystem reads it, to minimize exposure window. Deferred to after command
    // dispatch so encryption functions can still read it.
    // We use a scope guard pattern: set a flag to clear on exit.
    struct PassphraseCleaner {
        clear_passphrase: bool,
        clear_passphrase_file: bool,
    }
    impl Drop for PassphraseCleaner {
        fn drop(&mut self) {
            // SAFETY: Called during main() cleanup, no other threads accessing env
            unsafe {
                if self.clear_passphrase {
                    std::env::remove_var("LIT_PASSPHRASE");
                }
                if self.clear_passphrase_file {
                    std::env::remove_var("LIT_PASSPHRASE_FILE");
                }
            }
        }
    }
    let _passphrase_cleaner = PassphraseCleaner {
        clear_passphrase: cli.passphrase.is_some(),
        clear_passphrase_file: cli.passphrase_file.is_some(),
    };

    // Load hierarchical config (user global -> repo-local -> env vars)
    let _config = config::LitConfig::load(core::find_repo_root().ok().as_deref());

    let format = Format::resolve(cli.json, cli.human, cli.output.as_deref(), cli.pretty);

    // Helper macro to run a command, render its response, and handle errors
    // Structured error output uses LitError for machine-readable codes and suggestions
    macro_rules! run {
        ($expr:expr) => {
            match $expr {
                Ok(resp) => {
                    let output = formatter::format_response(&resp, format);
                    use std::io::Write;
                    if std::io::stdout().write_all(&output).is_err() {
                        process::exit(1);
                    }
                    println!();
                }
                Err(e) => {
                    let output = formatter::format_error(&e, e.error_code(), format);
                    use std::io::Write;
                    let _ = std::io::stderr().write_all(&output);
                    let _ = std::io::stderr().write_all(b"\n");
                    process::exit(1);
                }
            }
        };
    }

    match cli.command {
        Commands::Init { bare, path } => run!(commands::init::execute(bare, path)),
        Commands::Add { files } => run!(commands::add::execute(files)),
        Commands::Commit {
            message,
            author,
            intent,
        } => {
            let result = commands::commit::execute(message, author);
            match result {
                Ok(ref resp) if intent.is_some() => {
                    let repo_root = core::find_repo_root().ok();
                    if let Some(root) = repo_root {
                        let _ = commands::intent::attach_commit(
                            &root,
                            intent.as_ref().unwrap(),
                            &resp.hash,
                        );
                    }
                    run!(result)
                }
                _ => run!(result),
            }
        }
        Commands::Status => run!(commands::status::execute()),
        Commands::Log { count, oneline } => run!(commands::log::execute(count, oneline)),
        Commands::Branch { name, delete, all } => {
            run!(commands::branch::execute(name, delete, all))
        }
        Commands::Checkout { target, b } => run!(commands::checkout::execute(target, b)),
        Commands::Merge { branch, strategy } => {
            run!(commands::merge::execute(branch, strategy))
        }
        Commands::Resolve {
            file,
            strategy,
            all,
            finish,
        } => run!(commands::resolve::execute(file, strategy, all, finish)),
        Commands::Show { object } => run!(commands::show::execute(object)),
        Commands::Remote { command } => run!(commands::remote::execute(command.map(to_lib_remote))),
        Commands::Push {
            remote,
            branch,
            force,
        } => run!(commands::push::execute(remote, branch, force)),
        Commands::Pull { remote, branch } => run!(commands::pull::execute(remote, branch)),
        Commands::Fetch { remote, branch } => run!(commands::fetch::execute(remote, branch)),
        Commands::Clone { url, directory } => run!(commands::clone::execute(url, directory)),
        Commands::Config { command } => run!(commands::config::execute(command.map(to_lib_config))),
        Commands::Diff {
            staged,
            stat,
            word_diff,
            ref1,
            ref2,
        } => run!(commands::diff::execute(staged, stat, word_diff, ref1, ref2)),
        Commands::Tag {
            name,
            message,
            annotate,
            delete,
            sign,
            verify,
            list,
            commit,
        } => run!(commands::tag::execute(
            name, message, annotate, delete, sign, verify, list, commit
        )),
        Commands::RotateKey => run!(commands::rotate_key::rotate_key()),
        Commands::MigrateEncryption => run!(commands::migrate_encryption::execute()),

        // Phase 1.5-1.8
        Commands::Stash { command } => run!(commands::stash::execute(command.map(to_lib_stash))),
        Commands::Reset { target, soft, hard } => {
            run!(commands::reset::execute(target, soft, hard))
        }
        Commands::Revert { target } => run!(commands::revert::execute(target)),
        Commands::CherryPick { target } => run!(commands::cherry_pick::execute(target)),
        Commands::Rebase {
            base,
            interactive,
            onto,
            abort,
            cont,
        } => {
            run!(commands::rebase::execute(
                base,
                interactive,
                onto,
                abort,
                cont
            ))
        }
        Commands::Blame { file } => run!(commands::blame::execute(file)),
        Commands::Bisect { command } => run!(commands::bisect::execute(command.map(to_lib_bisect))),
        Commands::Reflog { ref_name, count } => run!(commands::reflog::execute(ref_name, count)),

        // Phase 2
        Commands::Batch { atomic, dry_run } => run!(commands::batch::execute(atomic, dry_run)),
        Commands::Tx { command } => match command {
            TxCommands::Begin => run!(commands::transaction::execute_begin()),
            TxCommands::Commit => run!(commands::transaction::execute_commit_tx()),
            TxCommands::Rollback => run!(commands::transaction::execute_rollback()),
        },
        Commands::Snapshot {
            message,
            author,
            metadata,
        } => {
            let meta = match metadata
                .map(|s| serde_json::from_str::<serde_json::Value>(&s))
                .transpose()
            {
                Ok(v) => v,
                Err(e) => {
                    let lit_err =
                        errors::LitError::general(format!("Invalid metadata JSON: {}", e));
                    let output = formatter::format_error(&lit_err, "snapshot", format);
                    use std::io::Write;
                    let _ = std::io::stderr().write_all(&output);
                    let _ = std::io::stderr().write_all(b"\n");
                    process::exit(1);
                }
            };
            run!(commands::snapshot::execute(message, author, meta))
        }
        Commands::Search {
            query,
            messages,
            metadata,
            max_results,
        } => {
            run!(commands::search::execute(
                query,
                messages,
                metadata,
                max_results
            ))
        }
        Commands::Watch { debounce, filter } => run!(commands::watch::execute(debounce, filter)),
        Commands::Verify => run!(commands::verify::execute()),

        // Phase 3
        Commands::Serve {
            port,
            token,
            stdio,
            daemon,
        } => {
            if stdio {
                run!(commands::serve::execute_stdio())
            } else if daemon {
                run!(commands::serve::execute_daemon(port))
            } else {
                run!(commands::serve::execute(port, token))
            }
        }
        Commands::McpServe { stdio, port } => {
            if let Some(p) = port {
                run!(commands::mcp_serve::execute_http(p))
            } else {
                let _ = stdio;
                run!(commands::mcp_serve::execute_stdio())
            }
        }
        Commands::Swarm { command } => match command {
            SwarmCommands::Register { agent_id } => {
                run!(commands::swarm::execute_register(agent_id))
            }
            SwarmCommands::List => run!(commands::swarm::execute_list()),
            SwarmCommands::LeaseAcquire {
                agent,
                path,
                duration,
            } => {
                run!(commands::swarm::execute_lease_acquire(
                    agent, path, duration
                ))
            }
            SwarmCommands::LeaseRelease { agent, path } => {
                run!(commands::swarm::execute_lease_release(agent, path))
            }
            SwarmCommands::LeaseList => run!(commands::swarm::execute_lease_list()),
        },
        // Phase 4: Git Interop
        Commands::ImportGit { source } => run!(commands::import_git::execute(source)),
        Commands::ExportGit { destination } => run!(commands::export_git::execute(destination)),

        // Phase 5: Performance
        Commands::Gc => run!(commands::gc::execute()),
        Commands::Lfs { command } => match command {
            LfsCommands::Track { patterns } => run!(commands::lfs::execute_track(patterns)),
            LfsCommands::Migrate { threshold } => run!(commands::lfs::execute_migrate(threshold)),
        },

        Commands::Ontology => {
            let ont = ontology::get_ontology();
            let resp = response::OntologyResponse {
                ontology: serde_json::to_value(&ont).unwrap_or_default(),
            };
            let output = formatter::format_response(&resp, format);
            use std::io::Write;
            if std::io::stdout().write_all(&output).is_err() {
                process::exit(1);
            }
            println!();
        }

        Commands::Schema { command } => {
            let schema = if let Some(ref cmd_id) = command {
                match ontology::generate_command_schema(cmd_id) {
                    Some(s) => s,
                    None => {
                        let err = errors::LitError::general(format!("unknown command: {cmd_id}"));
                        let output = formatter::format_error(&err, "schema", format);
                        use std::io::Write;
                        let _ = std::io::stderr().write_all(&output);
                        eprintln!();
                        process::exit(1);
                    }
                }
            } else {
                ontology::generate_schemas()
            };
            let resp = response::SchemaResponse { schema };
            let output = formatter::format_response(&resp, format);
            use std::io::Write;
            if std::io::stdout().write_all(&output).is_err() {
                process::exit(1);
            }
            println!();
        }

        // Sandbox
        Commands::Sandbox { command } => match command {
            SandboxCommands::Init { name } => run!(commands::sandbox::execute_init(name)),
            SandboxCommands::Run { name, cmd } => run!(commands::sandbox::execute_run(name, cmd)),
            SandboxCommands::List => run!(commands::sandbox::execute_list()),
            SandboxCommands::Destroy { name } => run!(commands::sandbox::execute_destroy(name)),
        },

        // Phase 6: Decentralized Identity & Federation
        Commands::Did { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            match command {
                DidCommands::Generate { method } => {
                    let did_method = match method.as_str() {
                        "ml-dsa-87" | "mldsa87" => identity::did::DidMethod::MlDsa87,

                        _ => identity::did::DidMethod::Ed25519,
                    };

                    let keypair = identity::did::DidKeyPair::generate(did_method);

                    let doc = identity::did::DidDocument::from_keypair(&keypair);

                    run!(
                        identity::did::save_identity(&repo_root, &keypair).map(|()| {
                            response::DidResponse {
                                action: "generate".into(),

                                did: Some(keypair.did.clone()),

                                message: format!("DID identity generated: {}", keypair.did),

                                details: Some(serde_json::to_value(&doc).unwrap_or_default()),
                            }
                        })
                    )
                }

                DidCommands::Show => {
                    run!(identity::did::load_identity(&repo_root).map(|kp| {
                        let doc = identity::did::DidDocument::from_keypair(&kp);

                        response::DidResponse {
                            action: "show".into(),

                            did: Some(kp.did.clone()),

                            message: format!("Current identity: {}", kp.did),

                            details: Some(serde_json::to_value(&doc).unwrap_or_default()),
                        }
                    }))
                }

                DidCommands::Resolve { did } => {
                    run!(identity::did::resolve_did(&repo_root, &did).map(|doc| {
                        response::DidResponse {
                            action: "resolve".into(),

                            did: Some(did),

                            message: "DID resolved".into(),

                            details: Some(serde_json::to_value(&doc).unwrap_or_default()),
                        }
                    }))
                }
            }
        }

        Commands::Ucan { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            match command {
                UcanCommands::Issue {
                    audience,
                    resource,
                    action,
                    duration,
                } => {
                    run!(identity::did::load_identity(&repo_root).and_then(|kp| {
                        let cap = identity::ucan::Capability {
                            resource,

                            action,

                            caveats: None,
                        };

                        let mut token = identity::ucan::UcanToken::new(
                            kp.did.clone(),
                            audience,
                            vec![cap],
                            duration,
                        );

                        if let Some(ref sk) = kp.private_key {
                            token.sign(sk)?;
                        }

                        let cid = identity::ucan::save_token(&repo_root, &token)?;

                        Ok(response::UcanResponse {
                            action: "issue".into(),

                            token_cid: Some(cid),

                            message: "UCAN token issued".into(),

                            details: Some(serde_json::to_value(&token).unwrap_or_default()),
                        })
                    }))
                }

                UcanCommands::List { audience } => {
                    let aud = audience.unwrap_or_default();

                    run!(
                        identity::ucan::load_tokens_for(&repo_root, &aud).map(|tokens| {
                            response::UcanResponse {
                                action: "list".into(),

                                token_cid: None,

                                message: format!("{} token(s) found", tokens.len()),

                                details: Some(serde_json::to_value(&tokens).unwrap_or_default()),
                            }
                        })
                    )
                }

                UcanCommands::Revoke { cid } => {
                    run!(identity::ucan::revoke_token(&repo_root, &cid).map(|()| {
                        response::UcanResponse {
                            action: "revoke".into(),

                            token_cid: Some(cid),

                            message: "Token revoked".into(),

                            details: None,
                        }
                    }))
                }
            }
        }

        Commands::Trust { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            let engine = identity::trust::TrustEngine::new(&repo_root);

            match command {
                TrustCommands::Show { did } => {
                    run!(engine.get_score(&did).map(|score| {
                        response::TrustResponse {
                            action: "show".into(),

                            did: Some(score.did.clone()),

                            score: Some(score.score),

                            level: Some(score.level.to_string()),

                            message: format!("Trust score for {}: {:.1}", score.did, score.score),

                            details: Some(serde_json::to_value(&score).unwrap_or_default()),
                        }
                    }))
                }

                TrustCommands::List => {
                    run!(engine.list_agents().map(|agents| {
                        response::TrustResponse {
                            action: "list".into(),

                            did: None,

                            score: None,

                            level: None,

                            message: format!("{} agent(s) tracked", agents.len()),

                            details: Some(serde_json::to_value(&agents).unwrap_or_default()),
                        }
                    }))
                }

                TrustCommands::History { did } => {
                    run!(engine.get_score(&did).map(|score| {
                        response::TrustResponse {
                            action: "history".into(),

                            did: Some(score.did.clone()),

                            score: Some(score.score),

                            level: Some(score.level.to_string()),

                            message: format!("{} events for {}", score.events.len(), score.did),

                            details: Some(serde_json::to_value(&score.events).unwrap_or_default()),
                        }
                    }))
                }
            }
        }

        Commands::Issue { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            let author = identity::did::load_identity(&repo_root)
                .map(|kp| kp.did.clone())
                .unwrap_or_else(|_| "anonymous".to_string());

            match command {
                IssueCommands::Create { title, body, label } => {
                    run!(
                        commands::issue::create_issue(&repo_root, &title, &body, &author, label)
                            .map(|issue| {
                                response::IssueResponse {
                                    action: "create".into(),

                                    id: Some(issue.id),

                                    message: format!(
                                        "Issue #{} created: {}",
                                        issue.id, issue.title
                                    ),

                                    details: Some(serde_json::to_value(&issue).unwrap_or_default()),
                                }
                            })
                    )
                }

                IssueCommands::List { state } => {
                    let filter = match state.as_str() {
                        "closed" => Some(commands::issue::IssueState::Closed),

                        "all" => None,

                        _ => Some(commands::issue::IssueState::Open),
                    };

                    run!(
                        commands::issue::list_issues(&repo_root, filter).map(|issues| {
                            response::IssueResponse {
                                action: "list".into(),

                                id: None,

                                message: format!("{} issue(s)", issues.len()),

                                details: Some(serde_json::to_value(&issues).unwrap_or_default()),
                            }
                        })
                    )
                }

                IssueCommands::Show { id } => {
                    run!(commands::issue::get_issue(&repo_root, id).map(|issue| {
                        response::IssueResponse {
                            action: "show".into(),

                            id: Some(issue.id),

                            message: format!("#{}: {} [{}]", issue.id, issue.title, issue.state),

                            details: Some(serde_json::to_value(&issue).unwrap_or_default()),
                        }
                    }))
                }

                IssueCommands::Close { id } => {
                    run!(commands::issue::close_issue(&repo_root, id).map(|issue| {
                        response::IssueResponse {
                            action: "close".into(),

                            id: Some(issue.id),

                            message: format!("Issue #{} closed", issue.id),

                            details: None,
                        }
                    }))
                }

                IssueCommands::Comment { id, body } => {
                    run!(
                        commands::issue::comment_issue(&repo_root, id, &author, &body).map(
                            |issue| {
                                response::IssueResponse {
                                    action: "comment".into(),

                                    id: Some(issue.id),

                                    message: format!("Comment added to issue #{}", issue.id),

                                    details: None,
                                }
                            }
                        )
                    )
                }
            }
        }

        Commands::Pr { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            let author = identity::did::load_identity(&repo_root)
                .map(|kp| kp.did.clone())
                .unwrap_or_else(|_| "anonymous".to_string());

            match command {
                PrCommands::Create {
                    title,
                    body,
                    head,
                    base,
                    label,
                } => {
                    run!(commands::pr::create_pr(
                        &repo_root, &title, &body, &author, &head, &base, label
                    )
                    .map(|pr| {
                        response::PrResponse {
                            action: "create".into(),

                            id: Some(pr.id),

                            message: format!("PR #{} created: {}", pr.id, pr.title),

                            details: Some(serde_json::to_value(&pr).unwrap_or_default()),
                        }
                    }))
                }

                PrCommands::List { state } => {
                    let filter = match state.as_str() {
                        "merged" => Some(commands::pr::PrState::Merged),

                        "closed" => Some(commands::pr::PrState::Closed),

                        "all" => None,

                        _ => Some(commands::pr::PrState::Open),
                    };

                    run!(commands::pr::list_prs(&repo_root, filter).map(|prs| {
                        response::PrResponse {
                            action: "list".into(),

                            id: None,

                            message: format!("{} PR(s)", prs.len()),

                            details: Some(serde_json::to_value(&prs).unwrap_or_default()),
                        }
                    }))
                }

                PrCommands::Show { id } => {
                    run!(commands::pr::get_pr(&repo_root, id).map(|pr| {
                        response::PrResponse {
                            action: "show".into(),

                            id: Some(pr.id),

                            message: format!(
                                "#{}: {} [{} -> {}] {}",
                                pr.id, pr.title, pr.head, pr.base, pr.state
                            ),

                            details: Some(serde_json::to_value(&pr).unwrap_or_default()),
                        }
                    }))
                }

                PrCommands::Merge { id } => {
                    run!(commands::pr::merge_pr(&repo_root, id).map(|pr| {
                        response::PrResponse {
                            action: "merge".into(),

                            id: Some(pr.id),

                            message: format!("PR #{} merged", pr.id),

                            details: None,
                        }
                    }))
                }

                PrCommands::Close { id } => {
                    run!(commands::pr::close_pr(&repo_root, id).map(|pr| {
                        response::PrResponse {
                            action: "close".into(),

                            id: Some(pr.id),

                            message: format!("PR #{} closed", pr.id),

                            details: None,
                        }
                    }))
                }

                PrCommands::Comment { id, body } => {
                    run!(
                        commands::pr::comment_pr(&repo_root, id, &author, &body).map(|pr| {
                            response::PrResponse {
                                action: "comment".into(),

                                id: Some(pr.id),

                                message: format!("Comment added to PR #{}", pr.id),

                                details: None,
                            }
                        })
                    )
                }
            }
        }

        Commands::Subscribe { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            let subscriber = identity::did::load_identity(&repo_root)
                .map(|kp| kp.did.clone())
                .unwrap_or_else(|_| "local".to_string());

            match command {
                SubscribeCommands::Add {
                    event_types,
                    branch,
                } => {
                    let parsed: Result<Vec<events::subscription::EventType>, _> =
                        event_types.iter().map(|s| s.parse()).collect();

                    match parsed {
                        Ok(types) => {
                            run!(events::subscription::subscribe(
                                &repo_root,
                                &subscriber,
                                types,
                                branch
                            )
                            .map(|sub| {
                                response::SubscribeResponse {
                                    action: "add".into(),

                                    subscription_id: Some(sub.id.clone()),

                                    message: format!("Subscribed ({})", sub.id),

                                    details: Some(serde_json::to_value(&sub).unwrap_or_default()),
                                }
                            }))
                        }

                        Err(e) => {
                            let err =
                                errors::LitError::general(format!("Invalid event type: {}", e));

                            let output = formatter::format_error(&err, "subscribe", format);

                            use std::io::Write;

                            let _ = std::io::stderr().write_all(&output);

                            let _ = std::io::stderr().write_all(b"\n");

                            process::exit(1);
                        }
                    }
                }

                SubscribeCommands::List => {
                    run!(
                        events::subscription::list_subscriptions(&repo_root).map(|subs| {
                            response::SubscribeResponse {
                                action: "list".into(),

                                subscription_id: None,

                                message: format!("{} subscription(s)", subs.len()),

                                details: Some(serde_json::to_value(&subs).unwrap_or_default()),
                            }
                        })
                    )
                }

                SubscribeCommands::Remove { id } => {
                    let id_clone = id.clone();

                    run!(
                        events::subscription::unsubscribe(&repo_root, &id).map(|()| {
                            response::SubscribeResponse {
                                action: "remove".into(),

                                subscription_id: Some(id_clone),

                                message: "Subscription removed".into(),

                                details: None,
                            }
                        })
                    )
                }

                SubscribeCommands::Events { event_type, limit } => {
                    let et = event_type
                        .as_deref()
                        .map(|s| s.parse::<events::subscription::EventType>())
                        .transpose();

                    match et {
                        Ok(filter) => {
                            run!(events::subscription::read_events(
                                &repo_root,
                                filter.as_ref(),
                                limit
                            )
                            .map(|evts| {
                                response::SubscribeResponse {
                                    action: "events".into(),

                                    subscription_id: None,

                                    message: format!("{} event(s)", evts.len()),

                                    details: Some(serde_json::to_value(&evts).unwrap_or_default()),
                                }
                            }))
                        }

                        Err(e) => {
                            let err =
                                errors::LitError::general(format!("Invalid event type: {}", e));

                            let output = formatter::format_error(&err, "subscribe", format);

                            use std::io::Write;

                            let _ = std::io::stderr().write_all(&output);

                            let _ = std::io::stderr().write_all(b"\n");

                            process::exit(1);
                        }
                    }
                }
            }
        }

        Commands::Delegate { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            let my_did = identity::did::load_identity(&repo_root)
                .map(|kp| kp.did.clone())
                .unwrap_or_else(|_| "anonymous".to_string());

            match command {
                DelegateCommands::Create {
                    to,
                    title,
                    description,
                    priority,
                    scope,
                } => {
                    let prio = match priority.as_str() {
                        "low" => commands::delegate::TaskPriority::Low,

                        "high" => commands::delegate::TaskPriority::High,

                        "critical" => commands::delegate::TaskPriority::Critical,

                        _ => commands::delegate::TaskPriority::Medium,
                    };

                    run!(commands::delegate::create_task(
                        &repo_root,
                        &my_did,
                        &to,
                        &title,
                        &description,
                        prio,
                        scope,
                        None,
                        None,
                    )
                    .map(|task| {
                        response::DelegateResponse {
                            action: "create".into(),

                            task_id: Some(task.id.clone()),

                            message: format!(
                                "Task '{}' delegated to {}",
                                task.title, task.delegatee
                            ),

                            details: Some(serde_json::to_value(&task).unwrap_or_default()),
                        }
                    }))
                }

                DelegateCommands::Accept { task_id } => {
                    run!(commands::delegate::update_task_status(
                        &repo_root,
                        &task_id,
                        commands::delegate::TaskStatus::Accepted,
                        Some("Accepted".into()),
                    )
                    .map(|task| {
                        response::DelegateResponse {
                            action: "accept".into(),

                            task_id: Some(task.id.clone()),

                            message: format!("Task {} accepted", task.id),

                            details: None,
                        }
                    }))
                }

                DelegateCommands::Complete { task_id, result } => {
                    run!(
                        commands::delegate::complete_task(&repo_root, &task_id, &result).map(
                            |task| {
                                response::DelegateResponse {
                                    action: "complete".into(),

                                    task_id: Some(task.id.clone()),

                                    message: format!("Task {} completed", task.id),

                                    details: Some(serde_json::to_value(&task).unwrap_or_default()),
                                }
                            }
                        )
                    )
                }

                DelegateCommands::List { agent, status } => {
                    let st = status.map(|s| match s.as_str() {
                        "pending" => commands::delegate::TaskStatus::Pending,

                        "accepted" => commands::delegate::TaskStatus::Accepted,

                        "in-progress" => commands::delegate::TaskStatus::InProgress,

                        "completed" => commands::delegate::TaskStatus::Completed,

                        "failed" => commands::delegate::TaskStatus::Failed,

                        "rejected" => commands::delegate::TaskStatus::Rejected,

                        _ => commands::delegate::TaskStatus::Pending,
                    });

                    run!(
                        commands::delegate::list_tasks(&repo_root, agent.as_deref(), st).map(
                            |tasks| {
                                response::DelegateResponse {
                                    action: "list".into(),

                                    task_id: None,

                                    message: format!("{} task(s)", tasks.len()),

                                    details: Some(serde_json::to_value(&tasks).unwrap_or_default()),
                                }
                            }
                        )
                    )
                }

                DelegateCommands::Show { task_id } => {
                    run!(
                        commands::delegate::get_task(&repo_root, &task_id).map(|task| {
                            response::DelegateResponse {
                                action: "show".into(),

                                task_id: Some(task.id.clone()),

                                message: format!("{} [{}]", task.title, task.status),

                                details: Some(serde_json::to_value(&task).unwrap_or_default()),
                            }
                        })
                    )
                }
            }
        }

        Commands::Peer { command } => {
            let repo_root = core::find_repo_root()
                .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

            match command {
                PeerCommands::Add {
                    did,
                    endpoint,
                    public_key,
                    alias,
                } => {
                    let peer = federation::peers::PeerInfo {
                        did: did.clone(),

                        alias,

                        endpoint,

                        public_key_hex: public_key,

                        last_known_head: None,

                        last_sync: None,

                        reachable: false,

                        added: chrono::Utc::now().to_rfc3339(),
                    };

                    run!(federation::peers::add_peer(&repo_root, &peer).map(|()| {
                        response::FederationResponse {
                            action: "add".into(),

                            message: format!("Peer {} added", did),

                            details: Some(serde_json::to_value(&peer).unwrap_or_default()),
                        }
                    }))
                }

                PeerCommands::Remove { did } => {
                    let did_clone = did.clone();

                    run!(federation::peers::remove_peer(&repo_root, &did).map(|()| {
                        response::FederationResponse {
                            action: "remove".into(),

                            message: format!("Peer {} removed", did_clone),

                            details: None,
                        }
                    }))
                }

                PeerCommands::List => {
                    run!(federation::peers::list_peers(&repo_root).map(|peers| {
                        response::FederationResponse {
                            action: "list".into(),

                            message: format!("{} peer(s)", peers.len()),

                            details: Some(serde_json::to_value(&peers).unwrap_or_default()),
                        }
                    }))
                }

                PeerCommands::Show { did } => {
                    run!(federation::peers::get_peer(&repo_root, &did).map(|peer| {
                        response::FederationResponse {
                            action: "show".into(),

                            message: format!(
                                "Peer: {} ({})",
                                peer.did,
                                if peer.reachable {
                                    "reachable"
                                } else {
                                    "unreachable"
                                }
                            ),

                            details: Some(serde_json::to_value(&peer).unwrap_or_default()),
                        }
                    }))
                }

                PeerCommands::Sync { did } => {
                    let wants =
                        federation::peers::generate_want_list(&repo_root).unwrap_or_default();

                    run!(federation::peers::get_peer(&repo_root, &did).map(|_peer| {
                        response::FederationResponse {
                            action: "sync".into(),

                            message: format!("Sync with {} — {} objects wanted", did, wants.len()),

                            details: Some(serde_json::json!({"wants": wants})),
                        }
                    }))
                }
            }
        }

        Commands::Intent { command } => match command {
            IntentCommands::Create {
                title,
                agent,
                scope,
                priority,
                parent,
                ucan_proof,
            } => {
                let prio = match priority.as_str() {
                    "low" => commands::intent::IntentPriority::Low,
                    "high" => commands::intent::IntentPriority::High,
                    "critical" => commands::intent::IntentPriority::Critical,
                    _ => commands::intent::IntentPriority::Medium,
                };
                run!(commands::intent::execute_create(
                    title, agent, scope, prio, parent, ucan_proof
                ))
            }
            IntentCommands::List { status, agent } => {
                run!(commands::intent::execute_list(status, agent))
            }
            IntentCommands::Show { intent_id } => {
                run!(commands::intent::execute_show(intent_id))
            }
            IntentCommands::Close { intent_id } => {
                run!(commands::intent::execute_close(intent_id))
            }
        },

        Commands::Converge {
            intent_id,
            strategy,
            verify,
            dry_run,
            target,
        } => {
            run!(commands::converge::execute(
                intent_id, strategy, verify, dry_run, target
            ))
        }

        Commands::ContentType { command } => match command {
            ContentTypeCommands::List { domain } => {
                run!(commands::content_type::execute_list(domain))
            }
            ContentTypeCommands::Show { type_id } => {
                run!(commands::content_type::execute_show(type_id))
            }
            ContentTypeCommands::Register {
                id,
                name,
                domain,
                extensions,
                diff_strategy,
                merge_strategy,
                storage_tier,
            } => {
                run!(commands::content_type::execute_register(
                    id,
                    name,
                    domain,
                    extensions,
                    diff_strategy,
                    merge_strategy,
                    storage_tier,
                ))
            }
            ContentTypeCommands::Detect { paths } => {
                run!(commands::content_type::execute_detect(paths))
            }
        },

        Commands::Datacenter { command } => match command {
            DatacenterCommands::Status => {
                run!(commands::datacenter::execute_status())
            }
            DatacenterCommands::RegisterNode {
                node_id,
                name,
                endpoint,
                region,
                role,
            } => {
                run!(commands::datacenter::execute_register_node(
                    node_id, name, endpoint, region, role
                ))
            }
            DatacenterCommands::RemoveNode { node_id } => {
                run!(commands::datacenter::execute_remove_node(node_id))
            }
            DatacenterCommands::Configure {
                replication_factor,
                shard_count,
                shard_strategy,
                replication_mode,
                connection_pool_size,
                metrics_enabled,
                metrics_port,
                write_concern,
            } => {
                run!(commands::datacenter::execute_configure(
                    replication_factor,
                    shard_count,
                    shard_strategy,
                    replication_mode,
                    connection_pool_size,
                    metrics_enabled,
                    metrics_port,
                    write_concern,
                ))
            }
            DatacenterCommands::Health => {
                run!(commands::datacenter::execute_health())
            }
            DatacenterCommands::Metrics => {
                run!(commands::datacenter::execute_metrics())
            }
        },

        Commands::AgentProfile { command } => match command {
            AgentProfileCommands::List { domain } => {
                run!(commands::agent_profile::execute_list(domain))
            }
            AgentProfileCommands::Show { profile_id } => {
                run!(commands::agent_profile::execute_show(profile_id))
            }
            AgentProfileCommands::Register {
                profile_id,
                name,
                domain,
                capabilities,
                trust_level,
                content_types,
                allowed_paths,
                denied_paths,
            } => {
                run!(commands::agent_profile::execute_register(
                    profile_id,
                    name,
                    domain,
                    capabilities,
                    trust_level,
                    content_types,
                    allowed_paths,
                    denied_paths,
                ))
            }
            AgentProfileCommands::Capabilities { domain } => {
                run!(commands::agent_profile::execute_capabilities(domain))
            }
            AgentProfileCommands::Remove { profile_id } => {
                run!(commands::agent_profile::execute_remove(profile_id))
            }
        },

        // GitButler-parity features
        Commands::Amend { message, author } => run!(commands::amend::execute(message, author)),
        Commands::Reword { message, target } => run!(commands::reword::execute(message, target)),
        Commands::Squash { count, message } => run!(commands::squash::execute(count, message)),
        Commands::Uncommit { discard } => run!(commands::uncommit::execute(discard)),
        Commands::Absorb { base, dry_run } => run!(commands::absorb::execute(base, dry_run)),
        Commands::Clean { dry_run } => run!(commands::clean::execute(dry_run)),
        Commands::Undo { command } => match command {
            UndoCommands::List { count } => run!(commands::undo::execute_list(count)),
            UndoCommands::Revert { id } => run!(commands::undo::execute_undo(id)),
            UndoCommands::Redo { id } => run!(commands::undo::execute_redo(id)),
        },
        Commands::Stack { command } => match command {
            StackCommands::List => run!(commands::stack::execute_list()),
            StackCommands::Push { name } => run!(commands::stack::execute_push(name)),
            StackCommands::Restack => run!(commands::stack::execute_restack()),
            StackCommands::Show => run!(commands::stack::execute_show()),
        },
        Commands::Workspace { command } => match command {
            WorkspaceCommands::List => run!(commands::workspace::execute_list()),
            WorkspaceCommands::Create { name } => run!(commands::workspace::execute_create(name)),
            WorkspaceCommands::Apply { name } => run!(commands::workspace::execute_apply(name)),
            WorkspaceCommands::Unapply { name } => {
                run!(commands::workspace::execute_unapply(name))
            }
            WorkspaceCommands::MoveFile { file, from, to } => {
                run!(commands::workspace::execute_move_file(file, from, to))
            }
        },
        Commands::Ai { command } => match command {
            AiCommands::CommitMessage { context } => {
                run!(commands::ai::execute_commit_message(context))
            }
            AiCommands::BranchName { description } => {
                run!(commands::ai::execute_branch_name(description))
            }
            AiCommands::PrDescription { head, base } => {
                run!(commands::ai::execute_pr_description(head, base))
            }
        },
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    /// `lit --version` and `-V` must answer rather than error.
    ///
    /// The derive carried no `version` attribute, so clap treated both as
    /// unknown arguments and a released binary could not report which release
    /// it was. clap signals a version request by returning an error carrying
    /// `DisplayVersion`, which is the success case here.
    #[test]
    fn test_version_flag_reports_the_crate_version() {
        for flag in ["--version", "-V"] {
            // `Cli` is not Debug, so unwrap the Result by hand.
            let err = match Cli::try_parse_from(["lit", flag]) {
                Err(err) => err,
                Ok(_) => panic!("`lit {}` should be a version request", flag),
            };
            assert_eq!(
                err.kind(),
                clap::error::ErrorKind::DisplayVersion,
                "`lit {}` should be a version request, not a parse failure",
                flag
            );
            assert!(
                err.to_string().contains(env!("CARGO_PKG_VERSION")),
                "`lit {}` should name the crate version, got: {}",
                flag,
                err
            );
        }
    }
}
