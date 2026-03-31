# Lit — Agentic-First Distributed Version Control System

## Vision

**Lit is the world's first agentic-first distributed version control system.** It is a complete Git replacement designed for AI agents first and humans second, offering both local and remote distributed version control with post-quantum cryptographic security.

The name "Lit" stands on its own — it is not an acronym. Lit illuminates the path forward from Git: a VCS built for the era where autonomous AI agents are the primary authors, reviewers, and operators of code.

## Design Principles

### 1. Agents First, Humans Second

Every interface is designed with machine consumption as the default. Human-readable output is an optional presentation layer on top of structured data.

- **Structured I/O by default**: All commands emit JSON (or optionally MessagePack) to stdout. Human-readable formatting requires an explicit `--human` or `-H` flag, or a `LIT_OUTPUT=human` environment variable.
- **Deterministic output schemas**: Every command has a versioned JSON schema. Agents can parse output reliably without fragile regex.
- **Machine-parseable errors**: Errors are structured objects with error codes, categories, and machine-actionable remediation hints — not freeform strings.
- **Stdin-driven batch operations**: Agents can pipe structured input (JSONL) for batch `add`, `commit`, `branch`, and other operations.
- **Zero-prompt mode**: No operation ever blocks on interactive user input by default. Passphrases, confirmations, and choices are supplied via arguments, environment variables, or config.

### 2. Complete Git Replacement

Lit must be capable of replacing Git entirely in any workflow — local development, distributed collaboration, CI/CD pipelines, or agentic swarm operations.

- **Full VCS feature parity**: init, add, commit, status, log, branch, checkout, merge, rebase, diff, stash, tag, remote, push, pull, clone, fetch, reset, revert, cherry-pick, bisect, blame, reflog.
- **Git interop bridge**: `lit import-git` and `lit export-git` for migration. Lit can clone from Git URLs and push to Git remotes.
- **Standard protocols**: HTTPS, SSH, and Lit's native protocol for remote operations, in addition to the existing airgap transports (file://, USB, network shares).

### 3. Local or Remote Distributed

Lit works identically whether the repository is purely local, synced over LAN, replicated across data centers, or coordinated among a swarm of agents.

- **Local-first**: Every operation works offline with full history.
- **Peer-to-peer sync**: No central server required. Any Lit repo can push to or pull from any other.
- **Optional central server**: `lit serve` launches an HTTP/gRPC API server for team and agent coordination.
- **Swarm mode**: Multiple agents can operate on a shared repository concurrently with conflict-free replicated data types (CRDTs) for metadata and automatic merge strategies.

### 4. Post-Quantum Secure by Default

Carried forward from the existing architecture — all cryptographic primitives are NIST-approved, post-quantum resistant, and FIPS 140-3 compliant.

---

## Architecture

### System Layers

```
┌──────────────────────────────────────────────────────────────────────┐
│                        Agent / Human Interface                       │
│  ┌─────────┐  ┌──────────┐  ┌───────────┐  ┌─────────────────────┐ │
│  │ CLI     │  │ HTTP/gRPC│  │MCP Server │  │ Rust Library (crate)│ │
│  │(clap)   │  │ API      │  │(Model     │  │ (programmatic API)  │ │
│  │         │  │          │  │ Context   │  │                     │ │
│  │--json   │  │ (serve)  │  │ Protocol) │  │ use lit::Repo;      │ │
│  │--human  │  │          │  │           │  │                     │ │
│  └────┬────┘  └────┬─────┘  └─────┬─────┘  └─────────┬───────────┘ │
│       │            │              │                   │             │
│       └────────────┴──────┬───────┴───────────────────┘             │
│                           │                                         │
│                    ┌──────▼──────┐                                   │
│                    │ Command API │ ← Unified, typed, structured      │
│                    │  (Result<   │   return values (no println!)     │
│                    │   Response) │                                   │
│                    └──────┬──────┘                                   │
│                           │                                         │
│  ┌────────────────────────┴─────────────────────────────────┐       │
│  │                     Core Engine                           │       │
│  │  ┌──────────┐ ┌──────────┐ ┌────────┐ ┌───────────────┐ │       │
│  │  │ Objects  │ │ Refs     │ │ Index  │ │ Merge Engine  │ │       │
│  │  │ (DAG)    │ │ (branch, │ │(staging│ │ (3-way merge, │ │       │
│  │  │          │ │  tag,    │ │ area)  │ │  CRDT, auto-  │ │       │
│  │  │          │ │  HEAD)   │ │        │ │  resolve)     │ │       │
│  │  └──────────┘ └──────────┘ └────────┘ └───────────────┘ │       │
│  │  ┌──────────┐ ┌──────────┐ ┌────────────────────────┐   │       │
│  │  │ Diff     │ │ Hooks /  │ │ Transaction Manager    │   │       │
│  │  │ Engine   │ │ Events   │ │ (atomic multi-op)      │   │       │
│  │  └──────────┘ └──────────┘ └────────────────────────┘   │       │
│  └──────────────────────────────────────────────────────────┘       │
│                           │                                         │
│  ┌────────────────────────┴─────────────────────────────────┐       │
│  │                    Storage Layer                           │       │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │       │
│  │  │ Object Store │  │ Pack Files   │  │ Ref Store      │  │       │
│  │  │ (loose +     │  │ (deltified   │  │ (atomic ref    │  │       │
│  │  │  encrypted)  │  │  packed objs)│  │  transactions) │  │       │
│  │  └──────────────┘  └──────────────┘  └────────────────┘  │       │
│  └──────────────────────────────────────────────────────────┘       │
│                           │                                         │
│  ┌────────────────────────┴─────────────────────────────────┐       │
│  │                   Network / Sync Layer                    │       │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐  │       │
│  │  │ HTTPS    │ │ SSH      │ │ Lit://   │ │ Airgap     │  │       │
│  │  │ (fetch,  │ │ (fetch,  │ │ (native  │ │ (USB, file │  │       │
│  │  │  push)   │ │  push)   │ │  proto)  │ │  shares)   │  │       │
│  │  └──────────┘ └──────────┘ └──────────┘ └────────────┘  │       │
│  │  ┌──────────────────────────────────────────────────────┐│       │
│  │  │              Sync Engine (smart negotiation,         ││       │
│  │  │              pack transfer, CRDT metadata sync)      ││       │
│  │  └──────────────────────────────────────────────────────┘│       │
│  └──────────────────────────────────────────────────────────┘       │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │                    Security Layer                         │       │
│  │  AES-256-GCM │ SHA3+BLAKE3 │ ML-DSA │ FIPS 140-3 │ Audit│       │
│  └──────────────────────────────────────────────────────────┘       │
└──────────────────────────────────────────────────────────────────────┘
```

### Key Architectural Shifts from Current Codebase

| Aspect               | Current State                | Agentic-First Design                                                          |
| -------------------- | ---------------------------- | ----------------------------------------------------------------------------- |
| **Output**           | `println!()` in commands     | Commands return typed `Response` structs; rendering is separate               |
| **Error handling**   | `Result<(), String>`         | `Result<CommandResponse, LitError>` with structured error codes               |
| **API surface**      | CLI only                     | CLI + HTTP/gRPC + MCP + Rust library crate                                    |
| **Merge**            | Not implemented              | Full 3-way merge with auto-resolution strategies                              |
| **Diff**             | Not implemented              | Line-level, word-level, and AST-aware diff                                    |
| **Push/Pull/Clone**  | Stubs                        | Full implementation over HTTPS, SSH, Lit://, file://, USB                     |
| **Config**           | Home-dir only (`~/.lit/`)    | Repo-local `.lit/config.toml` + global `~/.litconfig.toml`, env var overrides |
| **Concurrency**      | Single-threaded command exec | File-level locking, lock-free reads, parallel object I/O                      |
| **Object storage**   | Loose objects only           | Loose + packfiles with delta compression                                      |
| **Batch operations** | One file/command at a time   | JSONL stdin batch mode for all mutating operations                            |
| **Events**           | None                         | Hook system (pre/post commit, push, merge, etc.) + event bus                  |

---

## Interface Design

### CLI — Structured Output by Default

Every command returns a JSON object to stdout. The object always contains a `"status"` field (`"ok"` or `"error"`) and a `"command"` field identifying the operation.

```bash
# Agent mode (default) — JSON output
$ lit status
{
  "status": "ok",
  "command": "status",
  "branch": "main",
  "head": "a1b2c3d4...",
  "staged": ["src/main.rs"],
  "modified": ["README.md"],
  "untracked": ["new_file.txt"],
  "clean": false
}

# Human mode — familiar format
$ lit status --human
On branch main
Changes to be committed:
  modified:   src/main.rs

Changes not staged for commit:
  modified:   README.md

Untracked files:
  new_file.txt

# Error handling — structured
$ lit checkout nonexistent
{
  "status": "error",
  "command": "checkout",
  "error": {
    "code": "REF_NOT_FOUND",
    "message": "Branch 'nonexistent' does not exist",
    "suggestions": ["main", "develop", "feature/auth"]
  }
}
```

### Output Format Control

```bash
# Environment variable (persistent for agent sessions)
export LIT_OUTPUT=json       # default
export LIT_OUTPUT=human      # human-readable
export LIT_OUTPUT=msgpack    # binary MessagePack (compact)

# Per-command flag (overrides env)
lit log --human
lit log --json
lit log --output=msgpack
```

### Batch Operations via JSONL

Agents can pipe batched operations through stdin:

```bash
echo '{"op":"add","files":["src/a.rs","src/b.rs"]}
{"op":"commit","message":"refactor auth module","author":"agent-1"}
{"op":"branch","name":"feature/new-api","create":true}' | lit batch

# Returns one JSON response per operation:
{"status":"ok","command":"add","files_added":2}
{"status":"ok","command":"commit","hash":"abc123...","tree":"def456..."}
{"status":"ok","command":"branch","name":"feature/new-api","created":true}
```

### HTTP/gRPC API Server

```bash
# Launch API server for agent swarm access
$ lit serve --port 8384 --auth-token $LIT_TOKEN

# Agents interact via REST
POST /api/v1/commit
Content-Type: application/json
Authorization: Bearer $LIT_TOKEN

{
  "message": "implement feature X",
  "author": "agent-7",
  "sign": true
}

# Response
{
  "status": "ok",
  "hash": "abc123...",
  "tree": "def456...",
  "parent": "789xyz...",
  "timestamp": 1719000000
}
```

### MCP (Model Context Protocol) Server

Lit exposes itself as an MCP tool server, allowing LLM agents (Claude, GPT, etc.) to interact with repositories directly through tool calls:

```bash
$ lit mcp-serve --stdio
# or
$ lit mcp-serve --port 8385
```

MCP tools exposed:
- `lit_status` — Get repository status
- `lit_diff` — Show changes between commits, branches, or working tree
- `lit_log` — Browse commit history
- `lit_commit` — Create a commit
- `lit_branch` — Create, list, delete branches
- `lit_checkout` — Switch branches or restore files
- `lit_merge` — Merge branches
- `lit_search` — Search commit messages, file contents, or blame history
- `lit_read_file` — Read file at any commit
- `lit_write_file` — Stage and commit a file in one operation

### Rust Library API

Lit is also a library crate for embedding in Rust applications:

```rust
use lit::{Repo, CommitOptions, StatusResponse, OutputFormat};

let repo = Repo::open(".")?;

// All operations return typed responses
let status: StatusResponse = repo.status()?;
for file in &status.modified {
    println!("Modified: {}", file);
}

repo.add(&["src/main.rs"])?;
let commit = repo.commit(CommitOptions {
    message: "automated fix".into(),
    author: Some("agent-1".into()),
    sign: true,
    ..Default::default()
})?;

println!("Committed: {}", commit.hash.short());
```

---

## Command Taxonomy

### Core VCS Commands (Existing — to be enhanced)

| Command    | Current Status | Required Changes                                   |
| ---------- | -------------- | -------------------------------------------------- |
| `init`     | Working        | Add `--json` output, return structured response    |
| `add`      | Working        | Add batch mode, glob patterns, `--dry-run`         |
| `commit`   | Working        | Add `--sign`, `--allow-empty`, structured response |
| `status`   | Working        | Return structured JSON instead of println          |
| `log`      | Working        | Add `--format=json`, filtering, pagination         |
| `branch`   | Working        | Add rename, upstream tracking                      |
| `checkout` | Working        | Add `--force`, path-based checkout                 |
| `show`     | Working        | Return typed object data                           |
| `remote`   | Working        | No major changes needed                            |
| `config`   | Working        | Add repo-local config, env var overrides           |

### Core VCS Commands (New — must implement)

| Command       | Priority | Description                                                                    |
| ------------- | -------- | ------------------------------------------------------------------------------ |
| `merge`       | P0       | 3-way merge with configurable strategies (recursive, ours, theirs, agent-auto) |
| `diff`        | P0       | Line-level diff with structured output (hunks as JSON arrays)                  |
| `push`        | P0       | Full implementation over HTTPS, SSH, file://, lit://                           |
| `pull`        | P0       | Fetch + merge, with auto-rebase option                                         |
| `clone`       | P0       | Full repository clone over all transports                                      |
| `fetch`       | P0       | Download objects and refs without merging                                      |
| `tag`         | P1       | Lightweight and annotated tags with PQ signatures                              |
| `stash`       | P1       | Save and restore work-in-progress                                              |
| `reset`       | P1       | Move HEAD, update index/working tree                                           |
| `revert`      | P1       | Create inverse commits                                                         |
| `cherry-pick` | P1       | Apply specific commits to current branch                                       |
| `rebase`      | P1       | Replay commits onto a new base                                                 |
| `blame`       | P2       | Line-by-line authorship tracking                                               |
| `bisect`      | P2       | Binary search for bug-introducing commits                                      |
| `reflog`      | P2       | Reference update history                                                       |

### Agentic Commands (New — Lit-only features)

| Command      | Description                                                         |
| ------------ | ------------------------------------------------------------------- |
| `batch`      | Execute multiple operations from JSONL stdin                        |
| `serve`      | Launch HTTP/gRPC API server                                         |
| `mcp-serve`  | Launch MCP tool server for LLM agents                               |
| `tx`         | Transaction mode: group operations atomically                       |
| `search`     | Full-text search of file contents, commit messages, and metadata    |
| `import-git` | Import a Git repository into Lit format                             |
| `export-git` | Export a Lit repository to Git format                               |
| `snapshot`   | Atomic add-and-commit in one step (common agent workflow)           |
| `resolve`    | Auto-resolve merge conflicts using configurable strategies          |
| `watch`      | Watch for filesystem changes and emit events (for agent reactivity) |
| `verify`     | Verify repository integrity (all hashes, signatures, refs)          |

---

## Data Model

### Object Types (Extended)

```
Blob        — File content (unchanged)
Tree        — Directory listing (unchanged)
Commit      — Snapshot with metadata (extended)
Tag         — Named reference to any object with optional signature (NEW)
PackIndex   — Index into packed object files (NEW)
```

### Commit Object (Extended)

```json
{
  "type": "commit",
  "tree": "<hash>",
  "parents": ["<hash>", ...],
  "author": {
    "name": "agent-7",
    "email": "agent-7@swarm.local",
    "timestamp": 1719000000
  },
  "committer": {
    "name": "lit-coordinator",
    "email": "coordinator@swarm.local",
    "timestamp": 1719000001
  },
  "message": "implement feature X",
  "metadata": {
    "agent_id": "agent-7",
    "agent_model": "claude-opus-4-20250514",
    "task_id": "TASK-1234",
    "confidence": 0.95,
    "intent": "feature_implementation",
    "parent_task": "EPIC-42",
    "tool_versions": {
      "lit": "0.2.0",
      "rustc": "1.82.0"
    }
  },
  "signature": {
    "algorithm": "ML-DSA-87",
    "value": "<base64>",
    "public_key": "<base64>"
  }
}
```

The `metadata` field is a free-form JSON object that agents can populate with context about **why** a change was made, **who** (which agent/model) made it, **what task** it relates to, and **how confident** the agent is in the change. This is first-class data, not an afterthought hidden in commit message conventions.

### Structured Diff Format

```json
{
  "status": "ok",
  "command": "diff",
  "files": [
    {
      "path": "src/main.rs",
      "status": "modified",
      "old_hash": "abc...",
      "new_hash": "def...",
      "hunks": [
        {
          "old_start": 10,
          "old_count": 3,
          "new_start": 10,
          "new_count": 5,
          "lines": [
            {"type": "context", "content": "fn main() {"},
            {"type": "delete",  "content": "    println!(\"old\");"},
            {"type": "add",     "content": "    println!(\"new\");"},
            {"type": "add",     "content": "    init_logging();"},
            {"type": "context", "content": "}"}
          ]
        }
      ]
    }
  ]
}
```

---

## Merge Engine

### Strategies

| Strategy     | Description                              | Agent Use Case                                       |
| ------------ | ---------------------------------------- | ---------------------------------------------------- |
| `recursive`  | Standard 3-way merge (default)           | General-purpose                                      |
| `ours`       | Keep current branch version on conflict  | Agent knows its changes are authoritative            |
| `theirs`     | Keep incoming branch version on conflict | Agent deferring to another agent's changes           |
| `agent-auto` | LLM-assisted conflict resolution         | Agent calls out to LLM to resolve semantic conflicts |
| `union`      | Keep both sides of conflicting lines     | Additive-only changes (e.g., log entries)            |

### Conflict Representation (Structured)

When conflicts occur, they are returned as structured data, not `<<<<<<<` markers:

```json
{
  "status": "conflict",
  "command": "merge",
  "conflicts": [
    {
      "path": "src/auth.rs",
      "hunks": [
        {
          "base": ["fn authenticate() {", "    check_token()"],
          "ours": ["fn authenticate() {", "    check_token()", "    log_access()"],
          "theirs": ["fn authenticate() {", "    validate_session()"],
          "auto_resolution": null
        }
      ]
    }
  ],
  "merged_files": ["src/main.rs", "README.md"],
  "summary": {
    "total_files": 3,
    "auto_merged": 2,
    "conflicted": 1
  }
}
```

Agents can then resolve conflicts programmatically:

```bash
echo '{"path":"src/auth.rs","resolution":"theirs"}' | lit resolve
# or
lit resolve src/auth.rs --strategy=theirs
```

---

## Sync Protocol

### Pack Transfer

Lit uses a pack-based transfer protocol similar to Git's smart protocol but with structured negotiation:

1. **Have/Want negotiation**: Client sends hashes it has; server responds with delta pack of missing objects.
2. **Delta compression**: Objects are deltified within packs to minimize transfer size.
3. **Streaming**: Large transfers are streamed, not buffered in memory.
4. **Resumable**: Interrupted transfers can resume from the last acknowledged pack segment.
5. **Encrypted**: All wire transfer is encrypted (TLS 1.3 for HTTPS, SSH for SSH, AES-256-GCM for lit://).

### Transport Matrix

| Transport     | Remote Operations        | Authentication         | Encryption         |
| ------------- | ------------------------ | ---------------------- | ------------------ |
| HTTPS         | push, pull, clone, fetch | Bearer token, mTLS     | TLS 1.3            |
| SSH           | push, pull, clone, fetch | SSH keys               | SSH                |
| `lit://`      | push, pull, clone, fetch | Pre-shared key         | AES-256-GCM        |
| `file://`     | push, pull, clone, fetch | Filesystem permissions | At-rest encryption |
| USB/removable | push, pull, clone        | Physical access        | At-rest encryption |
| Network share | push, pull, clone        | SMB/NFS auth           | At-rest encryption |

---

## Concurrency Model

### Repository-Level Locking

- **Read operations** (status, log, diff, show, blame): Lock-free. Multiple agents can read concurrently.
- **Write operations** (commit, merge, push): Acquire a `.lit/lock` file with an advisory lock. Only one write at a time per repository.
- **Index operations** (add, reset): Acquire `.lit/index.lock`. Can run concurrently with ref operations.

### Swarm Coordination

When multiple agents work on the same codebase (via `lit serve`):

- Each agent works on its own branch (automatically namespaced: `agents/<agent-id>/<branch>`).
- A coordinator agent (or human) merges agent branches into the integration branch.
- CRDTs track non-conflicting metadata (branch pointers, tags) for eventual consistency.
- The server provides a **lease** system: an agent can claim exclusive write access to specific files to prevent conflicts entirely.

---

## Configuration Hierarchy

```
1. Command-line arguments     (highest priority)
2. Environment variables      (LIT_*, e.g., LIT_OUTPUT=json)
3. Repo-local config          (.lit/config.toml)
4. User global config         (~/.litconfig.toml)
5. System config              (/etc/lit/config.toml)
6. Built-in defaults          (lowest priority)
```

### Key Configuration Options

```toml
# .lit/config.toml (repo-local)

[core]
default_branch = "main"
hash_algorithm = "composite-v1"    # SHA3-512 + BLAKE3

[agent]
default_output = "json"            # json | human | msgpack
batch_mode = true                  # accept JSONL from stdin
auto_sign = true                   # sign all commits with PQ signatures
metadata_schema = "strict"         # enforce metadata fields on commits

[server]
port = 8384
auth_method = "token"              # token | mtls | none
max_connections = 100

[merge]
default_strategy = "recursive"
auto_resolve = true                # attempt auto-resolution before reporting conflicts
conflict_format = "structured"     # structured (JSON) | markers (git-style <<<<<<)

[security]
encryption = "aes-256-gcm"
fips_mode = true
audit_log = true
```

---

## Event System

Lit emits structured events that agents can subscribe to:

### Hook System (Git-compatible + Extended)

```
pre-commit      — Before commit is created
post-commit     — After commit is created
pre-push        — Before objects are pushed
post-push       — After push completes
pre-merge       — Before merge begins
post-merge      — After merge completes
pre-receive     — Server-side: before accepting pushed refs
post-receive    — Server-side: after refs are updated
file-changed    — When `lit watch` detects filesystem changes
conflict        — When a merge conflict is detected
```

Hooks receive structured JSON on stdin with full context about the operation. Hook scripts can be any executable; they receive the event and can return JSON to modify behavior (e.g., a `pre-commit` hook can modify the commit metadata).

### Event Bus (for `lit serve`)

The API server provides a WebSocket endpoint at `/events` that streams repository events in real-time:

```json
{
  "event": "commit",
  "timestamp": 1719000000,
  "data": {
    "hash": "abc123...",
    "author": "agent-7",
    "branch": "agents/agent-7/feature-x",
    "message": "implement feature X"
  }
}
```

---

## Git Interoperability

### Import

```bash
# Import a Git repository into Lit
$ lit import-git /path/to/git-repo
# or from a Git remote URL
$ lit import-git https://github.com/user/repo.git

# Converts:
# - Git objects → Lit objects (rehashed with SHA3+BLAKE3)
# - Git refs → Lit refs
# - Git config → Lit config
# - .gitignore → .litignore (also reads .gitignore for compatibility)
```

### Export

```bash
# Export Lit repository to Git format
$ lit export-git /path/to/output

# Converts:
# - Lit objects → Git objects (SHA-1 hashed)
# - Lit refs → Git refs
# - Lit metadata → Git notes
```

### Coexistence

A repository can contain both `.git/` and `.lit/` directories. Lit ignores `.git/` and Git ignores `.lit/`. This enables gradual migration.

---

## Error Taxonomy

All errors have a structured code for programmatic handling:

| Code               | Category    | Example                                |
| ------------------ | ----------- | -------------------------------------- |
| `REPO_NOT_FOUND`   | Repository  | No `.lit/` directory found             |
| `REPO_CORRUPT`     | Repository  | Object hash mismatch                   |
| `REF_NOT_FOUND`    | Reference   | Branch or tag doesn't exist            |
| `REF_CONFLICT`     | Reference   | Non-fast-forward push                  |
| `MERGE_CONFLICT`   | Merge       | Conflicting changes in merge           |
| `INDEX_LOCKED`     | Concurrency | Another operation holds the index lock |
| `AUTH_FAILED`      | Security    | Invalid token or credentials           |
| `TRANSPORT_DENIED` | Network     | Airgap mode blocked the transport      |
| `CRYPTO_ERROR`     | Security    | Decryption or signature failure        |
| `OBJECT_NOT_FOUND` | Storage     | Referenced object missing              |
| `INVALID_INPUT`    | Validation  | Malformed command arguments            |

```json
{
  "status": "error",
  "command": "push",
  "error": {
    "code": "REF_CONFLICT",
    "message": "Non-fast-forward update to refs/heads/main",
    "details": {
      "local_head": "abc123...",
      "remote_head": "def456...",
      "common_ancestor": "789xyz..."
    },
    "suggestions": [
      "Run 'lit pull origin main' to integrate remote changes",
      "Run 'lit push origin main --force' to overwrite (destructive)"
    ]
  }
}
```

---

## Security Model (Unchanged — Carried Forward)

- **Hashing**: SHA3-512 + BLAKE3 composite (192 hex chars, quantum-resistant)
- **Encryption at rest**: AES-256-GCM with PBKDF2-HMAC-SHA512 key derivation (600,000 iterations)
- **Signatures**: ML-DSA (Dilithium5) — NIST FIPS 204, Security Level 5
- **Audit logging**: HMAC-SHA256 signed, tamper-evident audit trail
- **Airgap mode**: Complete network isolation with physical transport validation
- **FIPS 140-3**: Level 1 software module compliance
- **Wire encryption**: TLS 1.3 (HTTPS), SSH (SSH transport), AES-256-GCM (lit:// protocol)
- **Authentication**: Bearer tokens, mutual TLS, SSH keys, pre-shared keys
- **Zero-trust**: All received objects are verified (hash check + optional signature verification)

---

## Performance Targets

| Operation               | Target  | Notes                                |
| ----------------------- | ------- | ------------------------------------ |
| `status` (1000 files)   | < 50ms  | Parallel filesystem stat             |
| `add` (1000 files)      | < 200ms | Parallel hashing                     |
| `commit`                | < 20ms  | Tree construction + write            |
| `log` (100 commits)     | < 10ms  | Sequential read from pack            |
| `diff` (typical file)   | < 5ms   | Myers diff or patience diff          |
| `clone` (1GB repo, LAN) | < 30s   | Pack transfer with delta compression |
| `push` (10 commits)     | < 2s    | Delta pack + transfer                |
| Cold start (CLI)        | < 10ms  | No runtime, no GC, compiled binary   |

---

## Directory Structure (.lit/)

```
.lit/
├── HEAD                    # Current branch reference
├── config.toml             # Repository configuration
├── description             # Repository description
├── index                   # Staging area (binary format)
├── index.lock              # Advisory lock for index operations
├── lock                    # Advisory lock for write operations
├── remotes.toml            # Remote repository configuration
├── hooks/                  # Hook scripts
│   ├── pre-commit
│   ├── post-commit
│   └── ...
├── objects/                # Object store
│   ├── <4-char>/           # Sharded loose objects
│   │   └── <rest-of-hash>
│   ├── pack/               # Packed objects
│   │   ├── pack-<hash>.pack
│   │   └── pack-<hash>.idx
│   └── info/
├── refs/                   # References
│   ├── heads/              # Branch refs
│   ├── tags/               # Tag refs
│   ├── remotes/            # Remote tracking refs
│   │   └── origin/
│   │       ├── main
│   │       └── ...
│   └── stash               # Stash ref
├── logs/                   # Reflogs
│   ├── HEAD
│   └── refs/
│       └── heads/
├── metadata/               # Agent metadata store
│   └── tasks.json          # Task tracking for agent workflows
└── audit/                  # Audit logs
    └── audit.log
```

---

## Implementation Strategy

### Phase 0: Foundation Refactor (Internal — No new features)

Refactor all existing commands to return structured `CommandResponse` types instead of using `println!()`. This is the critical prerequisite that enables all other interfaces.

```rust
// Before (current)
pub fn execute(files: Vec<String>) -> Result<(), String> {
    // ...
    println!("Added {} file(s) to staging area", file_count);
    Ok(())
}

// After (agentic)
pub fn execute(files: Vec<String>) -> Result<AddResponse, LitError> {
    // ...
    Ok(AddResponse {
        files_added: added_files,
        files_skipped: skipped_files,
        bytes_total: total_bytes,
    })
}
```

### Phase 1: Git Feature Parity

Implement all missing Git-equivalent features: merge, diff, push, pull, clone, fetch, tag, stash, reset, revert, cherry-pick, rebase.

### Phase 2: Agentic Features

Add batch mode, transaction support, `lit serve`, `lit mcp-serve`, event system, agent metadata, swarm coordination.

### Phase 3: Git Interop

Implement `import-git`, `export-git`, and the ability to push to/pull from Git remotes.

### Phase 4: Performance & Scale

Pack files, delta compression, parallel I/O, memory-mapped object access, large file support.

---

## Why Lit, Not Git?

| Dimension               | Git                         | Lit                                     |
| ----------------------- | --------------------------- | --------------------------------------- |
| **Designed for**        | Human developers            | AI agents (humans supported)            |
| **Default output**      | Human text                  | Structured JSON                         |
| **Error handling**      | Freeform strings            | Typed error codes with remediation      |
| **Batch operations**    | Shell scripting hacks       | Native JSONL batch mode                 |
| **API access**          | Third-party wrappers        | Built-in HTTP/gRPC + MCP server         |
| **Merge conflicts**     | `<<<<<<<` text markers      | Structured conflict objects             |
| **Agent metadata**      | Commit message conventions  | First-class metadata field              |
| **Crypto**              | SHA-1 (deprecated), SHA-256 | SHA3-512 + BLAKE3 (quantum-resistant)   |
| **Signatures**          | GPG/SSH                     | ML-DSA (post-quantum)                   |
| **Encryption**          | None built-in               | AES-256-GCM at rest, TLS 1.3 in transit |
| **Interactive prompts** | Frequent                    | Never (zero-prompt by design)           |
| **Concurrency**         | File-level locks            | Structured locking + CRDT metadata      |
| **Cold start**          | ~5ms                        | < 10ms target (compiled Rust)           |

---

## Summary

Lit is not a simplified Git clone. It is a **ground-up rethinking** of version control for the agentic era. Every design decision — from structured output to batch operations to MCP integration — is made with the assumption that the primary user is an autonomous AI agent that needs deterministic, parseable, composable interfaces.

Humans are fully supported through the `--human` flag and familiar command names, but they are not the design center. The world has enough tools built for humans and awkwardly adapted for machines. Lit is built for machines and gracefully adapted for humans.
