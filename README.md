# Lit — The Agentic-First Distributed Version Control System

**The world's first version control system designed for AI agents first and humans second.**

Lit is a complete Git replacement written in Rust — 50 commands, 30 MCP tools, post-quantum cryptographic security, sandboxed execution, and structured machine-readable output. Every interface is designed for autonomous agent workflows, with human-friendly output available via a single flag.

## Why Lit?

Git was designed in 2005 for human developers using terminals. Every interface — output formatting, error messages, interactive prompts, conflict markers — assumes a human is reading and responding. AI agents must parse Git's freeform text output with fragile regex, work around interactive prompts, and translate `<<<<<<<` conflict markers into something actionable.

**Lit inverts this.** Every command emits structured JSON by default. Errors include machine-actionable codes and remediation hints. Merge conflicts are structured objects, not text markers. Batch operations accept JSONL on stdin. Nothing ever prompts for input. Humans get the same power through a `--human` flag.

|                         | Git                         | Lit                                      |
| ----------------------- | --------------------------- | ---------------------------------------- |
| **Designed for**        | Human developers            | AI agents (humans supported)             |
| **Default output**      | Freeform text               | Structured JSON                          |
| **Error handling**      | Freeform strings            | Typed error codes + remediation          |
| **Batch operations**    | Shell scripting             | Native JSONL batch mode                  |
| **API access**          | Third-party wrappers        | Built-in REST + MCP + `lit://` + stdio   |
| **Merge conflicts**     | `<<<<<<<` markers           | Structured conflict objects              |
| **Agent metadata**      | Commit message conventions  | First-class metadata field               |
| **Agent coordination**  | None                        | Swarm registration, file leasing         |
| **Agent workflow**      | Branch → PR → merge         | Intent → Commit → Converge               |
| **Identity**            | None                        | DID-based identity, UCAN delegation      |
| **Trust scoring**       | None                        | Reputation tracking per agent            |
| **Issues & PRs**        | None built-in               | Local-first, stored as git refs          |
| **Federation**          | Centralized (GitHub/GitLab) | Content-addressed peer-to-peer           |
| **Sandboxing**          | None                        | Process isolation with env/fs/net fences |
| **Cryptography**        | SHA-1 / SHA-256             | SHA3-512 + BLAKE3 (quantum-resistant)    |
| **Signatures**          | GPG / SSH                   | ML-DSA-87 (NIST FIPS 204)                |
| **Encryption**          | None built-in               | AES-256-GCM at rest, TLS 1.3 in transit  |
| **Compliance**          | None                        | FIPS 140-3, auto self-test at startup    |
| **Interactive prompts** | Frequent                    | Never (zero-prompt design)               |

## Features

- **52 CLI commands** — full Git-equivalent workflow plus agent-native extensions
- **30 MCP tools** — LLM agents interact via Model Context Protocol tool calls
- **Decentralized identity** -- DID-based identity with Ed25519 and ML-DSA-87 post-quantum keys
- **UCAN capability delegation** -- fine-grained, cryptographically signed permission tokens
- **Agent trust scoring** -- reputation tracking with event-driven trust levels
- **Local-first issues & PRs** -- issue tracker and pull requests stored as git refs
- **Event subscriptions** -- subscribe to repository events (commits, branches, merges)
- **Agent task delegation** -- structured protocol for delegating work between agents
- **Content-addressed federation** -- peer-to-peer repository sync with want-list negotiation
- **4 transport protocols** — HTTPS, SSH, `lit://` (custom TCP), stdio pipe
- **Sandboxed execution** — run untrusted code in isolated environments with filesystem, environment, and network fences
- **Intent → Commit → Converge** — agentic workflow replacing branch/PR with scoped intents, commit attachment, and trust-gated convergence
- **Swarm coordination** — multi-agent registration, file leasing, and conflict-free concurrent access
- **Post-quantum signatures** — ML-DSA-87 (NIST FIPS 204, Security Level 5)
- **FIPS 140-3 compliance** — AES-256-GCM, PBKDF2-HMAC-SHA512, automatic Known Answer Tests at startup
- **Airgap mode** — complete network isolation for classified environments (USB, file shares only)
- **Atomic transactions** — begin/commit/rollback multi-operation sequences
- **Large File Storage** — LFS tracking and migration for binary assets
- **Git interop** — bidirectional import/export with existing Git repositories
- **Command ontology** — machine-readable type graph for agent discovery and SDK generation
- **JSON Schema** — auto-generated draft 2020-12 schemas for all command inputs/outputs
- **Per-IP rate limiting** — sliding window throttle (100 req/60s) on all server endpoints
- **Tamper-evident audit logs** — HMAC-SHA256 signed, append-only operation logs

## Installation

```bash
cargo build --release
cargo install --path .
```

Pre-built binaries for Linux, macOS, and Windows are available on the [Releases](https://github.com/nervosys/Lit/releases) page.

## Quick Start

### For Agents (JSON output — default)

```bash
# Initialize
lit init
# → {"status":"ok","command":"init","path":"/path/to/repo"}

# Stage and commit
lit add src/
lit commit -m "implement feature X"
# → {"status":"ok","command":"commit","hash":"abc123...","tree":"def456..."}

# Check status
lit status
# → {"status":"ok","command":"status","branch":"main","staged":[],"modified":[],"clean":true}

# Batch operations via JSONL stdin
echo '{"op":"add","files":["src/a.rs"]}
{"op":"commit","message":"fix bug"}' | lit batch
```

### For Humans

```bash
# Use --human or -H for familiar output
lit status --human
lit log --human
lit diff --human

# Or set globally
export LIT_OUTPUT=human
```

## Commands (52)

### Version Control

```bash
lit init [--bare] [path]           # Initialize repository
lit add <files...>                 # Stage files
lit commit -m <message> [--intent <id>]  # Record changes (optionally attach to intent)
lit status                         # Show working tree status
lit log [--count N]                # Show commit history
lit diff [--word-diff] [--stat]    # Show changes (structured hunks)
lit show <object>                  # Inspect any object
lit branch [name] [--delete]       # Manage branches
lit checkout <target> [-b]         # Switch branches
lit merge <branch>                 # Merge branches (3-way)
lit resolve <file> --strategy=X    # Programmatic conflict resolution
lit tag <name> [--sign]            # Create tags (with optional PQ signatures)
lit stash [push|pop|apply|list|drop] # Stash work-in-progress
lit reset [--soft|--mixed|--hard]  # Move HEAD
lit revert <commit>                # Create inverse commit
lit cherry-pick <commit>           # Apply specific commits
lit rebase <base>                  # Replay commits
lit blame <file>                   # Line-by-line authorship
lit bisect <start|good|bad|reset>  # Binary search for bug-introducing commit
lit reflog                         # Reference update history
```

### Remote Operations

```bash
lit remote add <name> <url>        # Add remote
lit push <remote> <branch>         # Push to remote (--force available)
lit pull <remote> <branch>         # Fetch and merge
lit clone <url> [directory]        # Clone repository
lit fetch <remote> [branch]        # Download without merging
```

Supported transports: HTTPS, SSH, `lit://` (custom TCP), `file://`, USB/removable media, network shares.

### Agentic Commands

```bash
lit batch                          # Execute JSONL operations from stdin
lit serve --port 3000              # Launch HTTP REST API server
lit serve --stdio                  # Launch stdio JSON pipe server
lit serve --daemon                 # Launch lit:// TCP daemon
lit mcp-serve --stdio              # Launch MCP tool server (stdio)
lit mcp-serve --port 8385          # Launch MCP tool server (HTTP)
lit tx <begin|commit|rollback>     # Atomic transaction mode
lit snapshot -m <msg>              # Add-all + commit in one step
lit search <query>                 # Full-text search across history
lit watch                          # Emit filesystem change events as JSON
lit ontology                       # Output command ontology for agent discovery
lit schema [--command <id>]        # JSON Schema (draft 2020-12) from ontology
```

### Swarm Coordination

```bash
lit swarm register <agent-id>      # Register an agent in the swarm
lit swarm list                     # List registered agents
lit swarm lease-acquire --agent <id> --path <file> --duration 300
                                   # Acquire exclusive file lease (seconds)
lit swarm lease-release --agent <id> --path <file>
                                   # Release a file lease
lit swarm lease-list               # List all active leases
```

### Sandbox

Run untrusted code in process-isolated environments with filesystem, environment, and network fences:

```bash
lit sandbox init [name]            # Create sandbox from working tree
lit sandbox run <name> -- <cmd>    # Run command inside sandbox
lit sandbox list                   # List all sandboxes
lit sandbox destroy <name>         # Remove a sandbox
```

Isolation layers:

| Layer       | Protection                                               |
| ----------- | -------------------------------------------------------- |
| Filesystem  | Working tree copied into `.lit/sandboxes/<name>/`        |
| Environment | `env_clear()` — only minimal vars exposed                |
| Home / Temp | HOME, USERPROFILE, TEMP, TMP redirected to sandbox dir   |
| PATH        | Restricted to system directories only                    |
| Network     | `LIT_AIRGAPPED=1` — blocks all network protocols         |
| Git config  | `GIT_CONFIG_NOSYSTEM=1` — prevents config file leaks     |
| Credentials | Cleared — no cloud tokens, SSH keys, or API keys present |

### Identity & Trust

```bash
lit did generate [--method ed25519|ml-dsa-87]  # Generate DID identity
lit did show                                   # Show current identity
lit did resolve <did>                          # Resolve a DID to its document
lit ucan issue <audience> --resource <r> --action <a>  # Issue UCAN token
lit ucan list [audience]                       # List UCAN tokens
lit ucan revoke <cid>                          # Revoke a UCAN token
lit trust show <did>                           # Show agent trust score
lit trust list                                 # List all tracked agents
lit trust history <did>                        # Show trust event history
```

### Issues & Pull Requests

```bash
lit issue create <title> [--body <b>] [--label <l>]    # Create issue
lit issue list [--state open|closed|all]                # List issues
lit issue show <id>                                     # Show issue
lit issue close <id>                                    # Close issue
lit issue comment <id> <body>                           # Comment on issue
lit pr create <title> --head <branch> [--base main]     # Create PR
lit pr list [--state open|merged|closed|all]             # List PRs
lit pr show <id>                                         # Show PR
lit pr merge <id>                                        # Merge PR
lit pr close <id>                                        # Close PR
lit pr comment <id> <body>                               # Comment on PR
```

### Intent → Commit → Converge

Declare scoped intents, attach commits, and converge work — replacing the branch → PR → merge ceremony:

```bash
lit intent create <title> --agent <id> --scope <paths...>  # Declare a scoped unit of work
lit intent list [--status active] [--agent <id>]           # List intents
lit intent show <intent-id>                                # Show intent details
lit intent close <intent-id>                               # Abandon an intent
lit commit -m <msg> --intent <intent-id>                   # Attach commit to intent
lit converge <intent-id> [--strategy auto|squash|rebase|accumulate]
                                                           # Merge intent into mainline
lit converge <intent-id> --dry-run                         # Preview convergence
lit converge <intent-id> --verify                          # Verify commit objects first
```

Intents auto-acquire swarm leases for their scope, detect scope conflicts with other active intents, and support hierarchical decomposition (parent/child intents). The `accumulate` strategy waits for all child intents to converge before merging the parent.

### Events & Delegation

```bash
lit subscribe add <event-types...> [--branch <b>]  # Subscribe to events
lit subscribe list                                  # List subscriptions
lit subscribe remove <id>                           # Remove subscription
lit subscribe events [--event-type <t>] [--limit N] # Read recent events
lit delegate create <to> <title> [--priority high]  # Delegate task to agent
lit delegate accept <task-id>                        # Accept delegated task
lit delegate complete <task-id> <result>              # Complete task
lit delegate list [--agent <did>] [--status <s>]      # List tasks
lit delegate show <task-id>                            # Show task details
```

### Federation

```bash
lit peer add <did> --endpoint <url> --public-key <hex>  # Add peer
lit peer remove <did>                                    # Remove peer
lit peer list                                            # List all peers
lit peer show <did>                                      # Show peer details
lit peer sync <did>                                      # Sync with peer
```

### Large File Storage

```bash
lit lfs track <patterns...>        # Track file patterns (e.g., "*.bin")
lit lfs migrate [--threshold N]    # Migrate existing large files to LFS
```

### Maintenance & Migration

```bash
lit verify                         # Full repository integrity check
lit gc                             # Garbage collection — pack loose objects
lit import-git <source>            # Import from Git repository
lit export-git <destination>       # Export to Git format
lit rotate-key                     # Rotate encryption passphrase
lit config [get|set|show]          # Manage configuration settings
```

## Structured Errors

All commands return typed `LitError` values with machine-actionable codes, not freeform strings:

```json
{
  "status": "error",
  "command": "commit",
  "code": "REPO_NOT_FOUND",
  "message": "Not a lit repository (or any parent up to /)",
  "suggestions": ["Run 'lit init' to create a new repository"]
}
```

Error codes: `ENCRYPTION_ERROR`, `IO_ERROR`, `CONFIG_ERROR`, `NETWORK_ERROR`, `REPO_NOT_FOUND`, `OBJECT_NOT_FOUND`, `INDEX_ERROR`, `MERGE_CONFLICT`, `AUTH_FAILURE`, `INVALID_INPUT`, `PERMISSION_DENIED`, `TIMEOUT`, `PROTOCOL_ERROR`, `INTERNAL_ERROR`, `GENERAL_ERROR`.

## Ontology & JSON Schema

Lit ships a built-in ontology — a typed knowledge graph of every command, type, relationship, and classification in the system. Agents use this for self-discovery, SDK generation, and input validation.

```bash
# Machine-readable ontology (JSON)
lit ontology

# Full JSON Schema (draft 2020-12) — all types and command interfaces
lit schema

# Single command schema
lit schema --command commit
# → {"$schema": "https://json-schema.org/draft/2020-12/schema",
#    "input": {"properties": {"message": {"type": "string"}, ...}}}
```

The schema is auto-generated from the ontology — types map to `$defs`, commands map to input/output schemas with metadata (idempotent, safe, side_effects, preconditions). See [ONTOLOGY.md](docs/ONTOLOGY.md) for the complete human-readable reference.

## API Server

Four server modes for different integration patterns:

```bash
# HTTP REST API — for agent swarms and CI/CD
lit serve --port 3000 --token $LIT_TOKEN

# stdio JSON pipe — for direct process integration
lit serve --stdio

# lit:// TCP daemon — for custom protocol clients
lit serve --daemon --port 9418
```

```bash
# Example: agent commits via REST
curl -X POST http://localhost:3000/api/v1/commit \
  -H "Authorization: Bearer $LIT_TOKEN" \
  -d '{"message":"implement feature X","author":"agent-7","sign":true}'
```

All server endpoints enforce per-IP rate limiting (100 requests per 60-second sliding window). Body size is capped at 1 MB.

## MCP Server

Lit exposes itself as an MCP (Model Context Protocol) tool server, enabling LLM agents to interact with repositories directly through tool calls:

```bash
lit mcp-serve --stdio      # For stdio-based MCP clients (VS Code, Claude Desktop)
lit mcp-serve --port 8385  # For HTTP-based MCP clients
```

**Tools (30):** `lit_status`, `lit_diff`, `lit_log`, `lit_show`, `lit_blame`, `lit_reflog`, `lit_add`, `lit_commit`, `lit_snapshot`, `lit_branch`, `lit_checkout`, `lit_merge`, `lit_resolve`, `lit_rebase`, `lit_cherry_pick`, `lit_revert`, `lit_reset`, `lit_stash`, `lit_tag`, `lit_push`, `lit_pull`, `lit_fetch`, `lit_clone`, `lit_search`, `lit_verify`, `lit_gc`, `lit_init`, `lit_config`, `lit_ontology`, `lit_schema`.

**Resources:** `lit://status`, `lit://branches`, `lit://log`, `lit://ontology`, `lit://schema`.

## Configuration

```toml
# .lit/config.toml (repo-local) or ~/.litconfig.toml (global)

[core]
default_branch = "main"

[agent]
default_output = "json"       # json | human | msgpack
auto_sign = true              # sign all commits with PQ signatures

[merge]
default_strategy = "recursive"
auto_resolve = true

[security]
encryption = "aes-256-gcm"
fips_mode = true
audit_log = true

[airgap]
enabled = false               # block all network protocols
strict_mode = false           # USB-only (no network shares)
```

Configuration priority: CLI flags > environment variables (`LIT_*`) > repo-local > user global > system > defaults.

## Cryptographic Security

| Layer          | Algorithm                                     | Standard        |
| -------------- | --------------------------------------------- | --------------- |
| **Hashing**    | SHA3-512 + BLAKE3 composite (192 hex chars)   | NIST FIPS 202   |
| **Signatures** | ML-DSA-87 (Dilithium5), Security Level 5      | NIST FIPS 204   |
| **Encryption** | AES-256-GCM                                   | NIST FIPS 197   |
| **KDF**        | PBKDF2-HMAC-SHA512, 600,000 iterations        | NIST SP 800-132 |
| **Audit logs** | HMAC-SHA256, tamper-evident                   | NIST FIPS 198-1 |
| **Compliance** | FIPS 140-3 Level 1, auto self-test at startup | NIST FIPS 140-3 |
| **PQ safety**  | Resistant to Shor's and Grover's algorithms   | —               |

FIPS Known Answer Tests (SHA-256, SHA-512, SHA3-512, HMAC-SHA-256, RNG health) execute automatically on every startup when `fips_mode = true`.

See [ENCRYPTION.md](docs/ENCRYPTION.md), [FIPS_140-3_COMPLIANCE.md](docs/FIPS_140-3_COMPLIANCE.md), and [CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md) for details.

## Transport Protocols

Lit auto-detects the transport from the URL and dispatches to the appropriate backend:

| Protocol    | URL Format                         | Use Case                              |
| ----------- | ---------------------------------- | ------------------------------------- |
| **HTTPS**   | `https://host/repo`                | Public and private remotes over TLS   |
| **SSH**     | `ssh://host/repo` or `host:repo`   | Authenticated access over SSH tunnels |
| **lit://**  | `lit://host:9418/repo`             | Custom binary TCP for LAN deployments |
| **file://** | `file:///path/to/repo`             | Local filesystem URL                  |
| **Local**   | `/path/to/repo` or `C:\repos\proj` | Bare path — no protocol prefix needed |
| **UNC**     | `\\server\share\repo`              | Windows network shares (SMB/CIFS)     |
| **USB**     | `E:\repos\proj` or `/media/usb/..` | Removable media (auto-detected)       |
| **stdio**   | `lit serve --stdio`                | JSON pipe for direct process control  |

## Operating Modes

Lit supports four operating modes that can be combined (e.g., sandbox + airgap):

### Standard Mode

Full distributed VCS with all network and local transports enabled. Push, pull, fetch, and clone work over HTTPS, SSH, `lit://`, and local paths.

```bash
lit clone https://github.com/org/repo       # HTTPS
lit clone ssh://server/repo                  # SSH
lit clone lit://192.168.1.10:9418/repo       # Custom TCP
lit clone /opt/repos/project                 # Local path
```

### Local-Only Mode

Operate without any remote configuration — purely local version control. All commands that don't involve a remote work identically. Useful for solo development, offline work, or repositories that never need to synchronize.

```bash
lit init myproject && cd myproject
lit add . && lit commit -m "initial commit"
lit branch feature && lit checkout feature
lit log --human
```

Local-only repos can add remotes later via `lit remote add` with any supported transport.

### Airgap Mode

Complete network isolation for classified and air-gapped environments. All TCP-based protocols (HTTPS, SSH, `lit://`, FTP) are blocked at the transport layer. Only physical and filesystem transports are allowed:

| Transport         | Airgap  | Airgap Strict |
| ----------------- | ------- | ------------- |
| Local filesystem  | Allowed | Allowed       |
| `file://` URL     | Allowed | Allowed       |
| USB / removable   | Allowed | Allowed       |
| Network shares    | Allowed | **Blocked**   |
| HTTPS / SSH / lit | Blocked | Blocked       |

```bash
lit --airgapped clone file:///media/usb/repo.lit
lit config set airgap.enabled true
lit config set airgap.strict_mode true   # USB-only, no network shares
```

Removable media is auto-detected via `GetDriveTypeW` on Windows and mount-point heuristics (`/media/`, `/mnt/`, `/Volumes/`) on Linux/macOS.

See [AIRGAP.md](docs/AIRGAP.md) for complete documentation.

### Sandbox Mode

Run untrusted code in process-isolated environments with filesystem, environment, and network fences:

```bash
lit sandbox init demo
lit sandbox run demo -- python3 -m pytest tests/
lit sandbox destroy demo
```

Every sandboxed process gets:

- **Cleared environment** — `env_clear()` strips all host variables
- **Redirected HOME/TEMP** — point into the sandbox directory
- **Restricted PATH** — system binaries only, no user-installed tools
- **Forced airgap** — `LIT_AIRGAPPED=1` blocks all network protocols
- **No credentials** — no cloud tokens, SSH keys, or API keys present
- **Symlink protection** — symlinks are skipped during sandbox copy to prevent escape

Sandboxes combine with airgap mode automatically. See [EXAMPLES.md § Sandboxed Execution](docs/EXAMPLES.md) and the cross-platform demo scripts in `examples/`.

## Use Cases

- **AI agent swarms** — multiple agents collaborating via structured API with file leasing
- **CI/CD pipelines** — machine-readable output eliminates fragile text parsing
- **MCP-enabled IDEs** — LLM agents operate on repos through tool calls (VS Code, Claude Desktop)
- **Sandboxed builds** — run untrusted code in isolated process environments
- **Air-gapped environments** — government, defense, critical infrastructure
- **Post-quantum security** — organizations preparing for quantum computing threats
- **Git migration** — import existing Git repos, export back when needed

## Testing

382 tests across unit, integration, and performance suites:

```shell
cargo test --lib -- --test-threads=1                             # 68 unit tests
cargo test --test command_tests -- --test-threads=1              # 239 command tests
cargo test --test feature_integration_test                       # 38 integration tests
cargo test --test performance_benchmarks --release               # 9 benchmarks
cargo test --test adversarial_test -- --test-threads=1           # 5 security tests
cargo test --test concurrency_test -- --test-threads=1           # 9 concurrency tests
cargo test --test network_integration_test -- --test-threads=1   # 14 network tests
```

## Documentation

- [QUICKSTART.md](docs/QUICKSTART.md) — Getting started guide
- [EXAMPLES.md](docs/EXAMPLES.md) — Usage examples (19 scenarios including sandbox demos)
- [DESIGN.md](docs/DESIGN.md) — Agentic-first architecture and design rationale
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) — System architecture deep-dive
- [ONTOLOGY.md](docs/ONTOLOGY.md) — Complete conceptual reference and type graph
- [ROADMAP.md](docs/ROADMAP.md) — Implementation roadmap and milestones
- [TESTING.md](docs/TESTING.md) — Testing guide
- [ENCRYPTION.md](docs/ENCRYPTION.md) — Encryption system documentation
- [ENCRYPTION_ENHANCEMENTS.md](docs/ENCRYPTION_ENHANCEMENTS.md) — Encryption hardening details
- [CRYPTOGRAPHY.md](docs/CRYPTOGRAPHY.md) — Cryptographic design documentation
- [KEY_DISTRIBUTION.md](docs/KEY_DISTRIBUTION.md) — Key distribution and management
- [FIPS_140-3_COMPLIANCE.md](docs/FIPS_140-3_COMPLIANCE.md) — FIPS 140-3 compliance documentation
- [SECURITY.md](docs/SECURITY.md) — Security model and threat mitigation
- [SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md) — DoD-standard security audit (14 findings, all remediated)
- [AIRGAP.md](docs/AIRGAP.md) — Airgap mode documentation
- [DEPLOYMENT.md](docs/DEPLOYMENT.md) — Deployment and operations guide
- [PROJECT_SUMMARY.md](docs/PROJECT_SUMMARY.md) — High-level project overview
- [CHANGELOG.md](CHANGELOG.md) — Release history
- [CONTRIBUTING.md](CONTRIBUTING.md) — Contribution guidelines

## License

This software is dual-licensed:

- **AGPL-3.0**: Free for open source use under the [GNU Affero General Public License v3.0](LICENSE)
- **Commercial**: Proprietary license available for organizations that cannot comply with AGPL requirements

For commercial licensing inquiries, contact: <licensing@nervosys.ai>

## Security

If you discover a security vulnerability, **do not** open a public issue. See [SECURITY.md](SECURITY.md) for responsible disclosure instructions.

For detailed security architecture: [docs/SECURITY.md](docs/SECURITY.md) | [docs/SECURITY_AUDIT.md](docs/SECURITY_AUDIT.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and submission guidelines.

This is a security-focused tool — all contributions must pass CI, include tests, and preserve structured output.
