#![allow(dead_code)]
mod commands;
mod config;
mod core;
mod crypto;
mod errors;
mod formatter;
mod network;
mod ontology;
mod response;
mod storage;

use clap::{Parser, Subcommand};
use formatter::Format;
use std::process;

#[derive(Parser)]
#[command(name = "lit")]
#[command(about = "Lit - The agentic-first distributed version control system", long_about = None)]
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

fn main() {
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

    let format = Format::resolve(cli.json, cli.human, cli.output.as_deref());

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
        Commands::Commit { message, author } => run!(commands::commit::execute(message, author)),
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
        Commands::Remote { command } => run!(commands::remote::execute(command)),
        Commands::Push {
            remote,
            branch,
            force,
        } => run!(commands::push::execute(remote, branch, force)),
        Commands::Pull { remote, branch } => run!(commands::pull::execute(remote, branch)),
        Commands::Fetch { remote, branch } => run!(commands::fetch::execute(remote, branch)),
        Commands::Clone { url, directory } => run!(commands::clone::execute(url, directory)),
        Commands::Config { command } => run!(commands::config::execute(command)),
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

        // Phase 1.5-1.8
        Commands::Stash { command } => run!(commands::stash::execute(command)),
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
        Commands::Bisect { command } => run!(commands::bisect::execute(command)),
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
    }
}
