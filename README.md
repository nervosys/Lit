# Lit — The Agentic-First Distributed Version Control System

**The world's first version control system designed for AI agents first and humans second.**

Lit is a complete Git replacement written in Rust, offering local and remote distributed version control with post-quantum cryptographic security, structured machine-readable output, and native API server and MCP integration for autonomous agent workflows.

## Why Lit?

Git was designed in 2005 for human developers using terminals. Every interface — output formatting, error messages, interactive prompts, conflict markers — assumes a human is reading and responding. AI agents must parse Git's freeform text output with fragile regex, work around interactive prompts, and translate `<<<<<<<` conflict markers into something actionable.

**Lit inverts this.** Every command emits structured JSON by default. Errors include machine-actionable codes and remediation hints. Merge conflicts are structured objects, not text markers. Batch operations accept JSONL on stdin. Nothing ever prompts for input. Humans get the same power through a `--human` flag.

|                         | Git                        | Lit                                     |
| ----------------------- | -------------------------- | --------------------------------------- |
| **Designed for**        | Human developers           | AI agents (humans supported)            |
| **Default output**      | Freeform text              | Structured JSON                         |
| **Error handling**      | Freeform strings           | Typed error codes + remediation         |
| **Batch operations**    | Shell scripting            | Native JSONL batch mode                 |
| **API access**          | Third-party wrappers       | Built-in HTTP + MCP server              |
| **Merge conflicts**     | `<<<<<<<` markers          | Structured conflict objects             |
| **Agent metadata**      | Commit message conventions | First-class metadata field              |
| **Cryptography**        | SHA-1 / SHA-256            | SHA3-512 + BLAKE3 (quantum-resistant)   |
| **Signatures**          | GPG / SSH                  | ML-DSA (post-quantum, NIST FIPS 204)    |
| **Encryption**          | None built-in              | AES-256-GCM at rest, TLS 1.3 in transit |
| **Interactive prompts** | Frequent                   | Never (zero-prompt design)              |

## Installation

```bash
cargo build --release
cargo install --path .
```

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

## Core Commands

### Version Control

```bash
lit init [--bare] [path]           # Initialize repository
lit add <files...>                 # Stage files
lit commit -m <message>            # Record changes
lit status                         # Show working tree status
lit log [--count N]                # Show commit history
lit diff [--word-diff] [--stat]    # Show changes (structured hunks)
lit show <object>                  # Inspect any object
lit branch [name] [--delete]       # Manage branches
lit checkout <target> [-b]         # Switch branches
lit merge <branch>                 # Merge branches (3-way)
lit resolve <file> --strategy=X    # Programmatic conflict resolution
lit tag <name> [--sign]            # Create tags
lit stash [push|pop|list]          # Stash work-in-progress
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
lit push <remote> <branch>         # Push to remote
lit pull <remote> <branch>         # Fetch and merge
lit clone <url> [directory]        # Clone repository
lit fetch <remote>                 # Download without merging
```

Supported transports: HTTPS, SSH, `lit://`, `file://`, USB/removable media, network shares.

### Agentic Commands

```bash
lit batch                          # Execute JSONL operations from stdin
lit serve --port 3000              # Launch HTTP API server
lit mcp-serve                      # Launch MCP tool server for LLM agents
lit tx <operations...>             # Atomic transaction mode
lit snapshot -m <msg>              # Add-all + commit in one step
lit search <query>                 # Full-text search across history
lit watch                          # Emit filesystem change events
lit swarm <join|status|sync>       # Multi-agent swarm coordination
lit ontology                       # Output command ontology for agent discovery
lit schema [--command <id>]        # Generate JSON Schema (draft 2020-12) from ontology
```

### Maintenance & Migration

```bash
lit verify                         # Full repository integrity check
lit gc                             # Garbage collection — pack loose objects
lit lfs track <pattern>            # Track large files via LFS
lit lfs migrate                    # Migrate existing large files to LFS
lit import-git <source>            # Import from Git repository
lit export-git <destination>       # Export to Git format
lit rotate-key                     # Rotate encryption passphrase
lit config [get|set|list]          # Manage configuration settings
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

## JSON Schema

Generate standard JSON Schema (draft 2020-12) from the ontology for agent SDK discovery and input validation:

```bash
# Full schema — all types and command interfaces
lit schema

# Single command schema
lit schema --command commit
# → {"$schema": "https://json-schema.org/draft/2020-12/schema",
#    "input": {"properties": {"message": {"type": "string"}, ...}}}
```

The schema is auto-generated from the ontology — types map to `$defs`, commands map to input/output schemas with metadata (idempotent, safe, side_effects, preconditions).

## API Server

```bash
# Launch for agent swarm access
lit serve --port 3000 --token $LIT_TOKEN
```

```bash
# Agents interact via REST
curl -X POST http://localhost:8384/api/v1/commit \
  -H "Authorization: Bearer $LIT_TOKEN" \
  -d '{"message":"implement feature X","author":"agent-7","sign":true}'
```

## MCP Server

Lit exposes itself as an MCP (Model Context Protocol) tool server, enabling LLM agents to interact with repositories directly through tool calls:

```bash
lit mcp-serve --stdio    # For stdio-based MCP clients
lit mcp-serve --port 8385  # For HTTP-based MCP clients
```

Tools (30): `lit_status`, `lit_diff`, `lit_log`, `lit_show`, `lit_blame`, `lit_reflog`, `lit_add`, `lit_commit`, `lit_snapshot`, `lit_branch`, `lit_checkout`, `lit_merge`, `lit_resolve`, `lit_rebase`, `lit_cherry_pick`, `lit_revert`, `lit_reset`, `lit_stash`, `lit_tag`, `lit_push`, `lit_pull`, `lit_fetch`, `lit_clone`, `lit_search`, `lit_verify`, `lit_gc`, `lit_init`, `lit_config`, `lit_ontology`, `lit_schema`.

Resources: `lit://status`, `lit://branches`, `lit://log`, `lit://ontology`, `lit://schema`.

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
```

Configuration priority: CLI flags > environment variables (`LIT_*`) > repo-local > user global > system > defaults.

## Cryptographic Security

- **Hashing**: SHA3-512 + BLAKE3 composite (192 hex chars, quantum-resistant)
- **Signatures**: ML-DSA-87 (Dilithium5) — NIST FIPS 204, Security Level 5
- **Encryption**: AES-256-GCM with PBKDF2-HMAC-SHA512 (600,000 iterations)
- **Audit**: HMAC-SHA256 signed tamper-evident logs
- **Compliance**: FIPS 140-3 Level 1, ISO/IEC 19790:2012
- **Post-quantum**: Resistant to Shor's and Grover's algorithms

See [ENCRYPTION.md](ENCRYPTION.md), [FIPS_140-3_COMPLIANCE.md](FIPS_140-3_COMPLIANCE.md), and [CRYPTOGRAPHY.md](CRYPTOGRAPHY.md) for details.

## Operating Modes

### Standard Mode
Full local and remote distributed VCS over HTTPS, SSH, or `lit://` protocol.

### Airgap Mode
Complete network isolation for classified and air-gapped environments. Physical transports only (USB, file shares).

```bash
lit --airgapped clone file:///media/usb/repo.lit
lit config set airgap.enabled true
```

See [AIRGAP.md](AIRGAP.md) for complete documentation.

## Use Cases

- **AI agent swarms**: Multiple agents collaborating on a codebase via structured API
- **CI/CD pipelines**: Machine-readable output eliminates fragile text parsing
- **MCP-enabled IDEs**: LLM agents operate on repos through tool calls
- **Air-gapped environments**: Government, defense, critical infrastructure
- **Post-quantum security**: Organizations preparing for quantum computing threats
- **Git migration**: Import existing Git repos, export back when needed

## Testing

428 tests across unit, integration, and performance suites:

```powershell
cargo test --lib -- --test-threads=1                   # 61 unit tests
cargo test --test command_tests -- --test-threads=1    # 239 command tests
cargo test --test feature_integration_test              # 30 integration tests
cargo test --test performance_benchmarks --release      # 9 benchmarks
cargo test --test adversarial_test -- --test-threads=1  # 5 security tests
cargo test --test concurrency_test -- --test-threads=1  # 9 concurrency tests
cargo test --test network_integration_test -- --test-threads=1 # 14 network tests
```

## Documentation

- [DESIGN.md](DESIGN.md) — Full agentic-first architecture and design rationale
- [ROADMAP.md](ROADMAP.md) — Implementation roadmap and milestones
- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture deep-dive
- [TESTING.md](TESTING.md) — Testing guide
- [QUICKSTART.md](QUICKSTART.md) — Getting started guide
- [EXAMPLES.md](EXAMPLES.md) — Usage examples
- [ENCRYPTION.md](ENCRYPTION.md) — Encryption system documentation
- [CRYPTOGRAPHY.md](CRYPTOGRAPHY.md) — Cryptographic design documentation
- [SECURITY.md](SECURITY.md) — Security model and threat mitigation
- [DEPLOYMENT.md](DEPLOYMENT.md) — Deployment and operations guide
- [AIRGAP.md](AIRGAP.md) — Airgap mode documentation
- [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) — High-level project overview

## License

MIT License

## Contributing

This is a security-focused tool. Contributions should:
- Maintain strict network restrictions
- Include security review considerations
- Add appropriate audit logging
- Follow Rust security best practices
- **Include comprehensive tests** (see testing documentation above)
