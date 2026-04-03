# Lit — Implementation Roadmap

## Current State (v1.0.0)

All 6 phases complete. 50 CLI commands implemented with structured JSON/human/MsgPack output.
Full VCS feature parity with Git (diff, merge, push/pull/clone/fetch, stash, rebase, cherry-pick, blame, bisect, reflog).
Agentic features: batch mode, transactions, snapshots, agent metadata, search, watch, verify.
API server and MCP tool server for LLM agent integration. Swarm coordination with branch namespacing and file leases.
Git interop: import-git, export-git with SHA-1/SHA3+BLAKE3 conversion.
Performance: pack files, parallel I/O, LFS support, binary index format.

---

## Phase 0 — Foundation Refactor

**Goal**: Decouple command logic from output rendering. Every command returns a typed response struct; presentation is handled by a separate output layer.

### 0.1 Response Types & Output Layer
- [x] Define `CommandResponse` enum and per-command response structs (e.g., `StatusResponse`, `CommitResponse`, `AddResponse`, `LogResponse`)
- [x] Implement `OutputFormatter` trait with `JsonFormatter`, `HumanFormatter`, and `MsgpackFormatter`
- [x] Add global `--json`, `--human`, `--output=FORMAT` flags to CLI
- [x] Add `LIT_OUTPUT` environment variable support
- [x] Wire output formatting into `main.rs` dispatch

### 0.2 Structured Error Handling
- [x] Extend `LitError` with machine-readable error codes (e.g., `REF_NOT_FOUND`, `REPO_NOT_FOUND`, `MERGE_CONFLICT`)
- [x] Add `suggestions` field to errors (actionable remediation for agents)
- [x] Ensure all commands return `Result<T, LitError>` instead of `Result<(), String>`
- [x] Errors are rendered through the same `OutputFormatter` as responses

### 0.3 Refactor Existing Commands
- [x] `init` → returns `InitResponse { path, bare, created_dirs }`
- [x] `add` → returns `AddResponse { files_added, files_skipped, bytes }`
- [x] `commit` → returns `CommitResponse { hash, tree, parent, author, timestamp }`
- [x] `status` → returns `StatusResponse { branch, head, staged, modified, untracked, clean }`
- [x] `log` → returns `LogResponse { commits: Vec<CommitEntry> }`
- [x] `branch` → returns `BranchResponse { branches, current, created, deleted }`
- [x] `checkout` → returns `CheckoutResponse { target, previous, files_updated }`
- [x] `show` → returns `ShowResponse { object_type, hash, content }`
- [x] `remote` → returns `RemoteResponse { remotes, added, removed }`
- [x] `config` → returns `ConfigResponse { entries }`

### 0.4 Zero-Prompt Mode
- [x] Remove all interactive `rpassword` prompts from default path
- [x] Accept passphrases via `--passphrase`, `LIT_PASSPHRASE` env var, or `--passphrase-file`
- [x] Interactive prompts only when `--interactive` flag is explicitly set
- [x] All confirmation dialogs replaced with `--yes` / `--force` flags

### 0.5 Config Hierarchy
- [x] Support repo-local `.lit/config.toml` (currently only home-dir `~/.lit/`)
- [x] Implement priority chain: CLI args > env vars > repo-local > user global > system > defaults
- [x] Migrate airgap config from `~/.lit/airgap.toml` to support repo-local override
- [x] Migrate network config similarly

**Milestone**: All existing commands work identically but return structured data. `lit status --json` and `lit status --human` produce equivalent information in different formats.

---

## Phase 1 — Git Feature Parity

**Goal**: Implement all core VCS features needed to replace Git.

### 1.1 Diff Engine (P0)
- [x] Implement Myers diff algorithm for line-level diffing
- [x] Structured diff output as JSON hunks (see DESIGN.md)
- [x] `lit diff` — working tree vs index
- [x] `lit diff --staged` — index vs HEAD
- [x] `lit diff <commit1> <commit2>` — between commits
- [x] `lit diff <branch1> <branch2>` — between branches
- [x] Word-level diff mode (`--word-diff`)
- [x] Stat summary mode (`--stat`)

### 1.2 Merge Engine (P0)
- [x] Implement 3-way merge algorithm (common ancestor detection)
- [x] Merge strategies: `recursive` (default), `ours`, `theirs`
- [x] Structured conflict representation (JSON, not markers)
- [x] `lit merge <branch>` — merge branch into current
- [x] `lit resolve <file> --strategy=X` — programmatic conflict resolution
- [x] Fast-forward merge when possible
- [x] Merge commit creation with multiple parents

### 1.3 Push / Pull / Clone / Fetch (P0)
- [x] Object transfer protocol (direct object copy with dedup)
- [x] `file://` transport — copy objects between local directories
- [x] `lit push <remote> <branch>` — full implementation
- [x] `lit pull <remote> <branch>` — fetch + merge
- [x] `lit clone <url> [dir]` — full repository clone with working tree checkout
- [x] `lit fetch <remote>` — download objects and refs without merging
- [x] Remote-tracking branches (`refs/remotes/<name>/<branch>`)
- [x] Have/want negotiation for minimal transfer
- [x] HTTPS transport — full client/server implementation via `lit serve` + `ureq`
- [x] SSH transport (pipe-based via `lit serve --stdio` + system `ssh` command)
- [x] `lit://` native TCP transport (daemon mode via `lit serve --daemon`, client via `LitConnection`)

### 1.4 Tags (P1)
- [x] Lightweight tags (`lit tag <name>`)
- [x] Annotated tags (`lit tag -a <name> -m <message>`)
- [x] Signed tags (`lit tag --sign <name>`)
- [x] Tag listing, deletion
- [x] Tag push/fetch over remotes

### 1.5 Stash (P1)
- [x] `lit stash push` — save working tree and index state
- [x] `lit stash pop` — restore most recent stash
- [x] `lit stash list` — list stashed states
- [x] `lit stash drop` — discard a stash entry
- [x] `lit stash apply` — apply without removing from stash

### 1.6 Reset / Revert / Cherry-Pick (P1)
- [x] `lit reset --soft <commit>` — move HEAD only
- [x] `lit reset --mixed <commit>` — move HEAD + reset index (default)
- [x] `lit reset --hard <commit>` — move HEAD + reset index + working tree
- [x] `lit revert <commit>` — create inverse commit
- [x] `lit cherry-pick <commit>` — apply specific commit to current branch

### 1.7 Rebase (P1)
- [x] `lit rebase <base>` — replay commits onto new base
- [x] Interactive rebase (`lit rebase -i <base>`) — JSON-based todo list for agents
- [x] `--onto` support
- [x] Conflict handling during rebase (structured)

### 1.8 Blame / Bisect / Reflog (P2)
- [x] `lit blame <file>` — line-by-line authorship (structured JSON output)
- [x] `lit bisect start/good/bad/reset` — binary search for regressions
- [x] `lit reflog` — reference update history

**Milestone**: Lit can replace Git for all standard version control workflows. An agent or human can use Lit as their sole VCS.

---

## Phase 2 — Agentic Features

**Goal**: Build the features that make Lit uniquely suited for autonomous AI agents.

### 2.1 Batch Mode
- [x] `lit batch` command — reads JSONL from stdin, executes operations sequentially
- [x] One JSON response per operation on stdout
- [x] Transaction semantics: `--atomic` flag rolls back all operations if any fail
- [x] Validation mode: `--dry-run` checks all operations without executing

### 2.2 Transaction Support
- [x] `lit tx begin` / `lit tx commit` / `lit tx rollback` — group operations atomically
- [x] File-level write-ahead log for crash recovery
- [x] Lock coordination between concurrent transactions

### 2.3 Snapshot Command
- [x] `lit snapshot -m <message>` — atomic add-all + commit in one step
- [x] Common agent workflow: make changes → snapshot → push
- [x] Supports all commit options (sign, author, metadata)

### 2.4 Agent Metadata
- [x] First-class `metadata` field on commit objects (JSON)
- [x] Fields: `agent_id`, `agent_model`, `task_id`, `confidence`, `intent`, `tool_versions`
- [x] `--metadata` flag on commit and snapshot commands
- [x] `lit log --filter-metadata` for querying by metadata fields

### 2.5 Search
- [x] `lit search <query>` — full-text search across file contents at any commit
- [x] `lit search --messages <query>` — search commit messages
- [x] `lit search --metadata <key>=<value>` — search by agent metadata
- [x] Structured results with file paths, line numbers, commit references

### 2.6 Watch Mode
- [x] `lit watch` — monitor filesystem for changes, emit events as JSON to stdout
- [x] Integration with `lit serve` event bus
- [x] Debouncing and filtering options

### 2.7 Verify
- [x] `lit verify` — full repository integrity check
- [x] Verify all object hashes
- [x] Verify all signatures (if signed)
- [x] Verify ref consistency
- [x] Verify DAG connectivity (no dangling objects)
- [x] Structured report output

**Milestone**: Agents can perform complex multi-step workflows (batch operations, transactions, metadata-enriched commits) entirely through structured interfaces.

---

## Phase 3 — API Server & MCP

**Goal**: Expose Lit as a network service for agent swarms and MCP-enabled LLM agents.

### 3.1 HTTP/gRPC API Server
- [x] `lit serve` command — launch API server
- [x] REST API at `/api/v1/*` for all VCS operations
- [x] Authentication: bearer token, mutual TLS
- [x] Rate limiting and connection pooling
- [x] WebSocket `/events` endpoint for real-time event streaming
- [x] OpenAPI specification for API discovery

### 3.2 MCP Tool Server
- [x] `lit mcp-serve --stdio` — stdio-based MCP server
- [x] `lit mcp-serve --port N` — HTTP-based MCP server
- [x] MCP tools: `lit_status`, `lit_diff`, `lit_log`, `lit_commit`, `lit_branch`, `lit_checkout`, `lit_merge`, `lit_search`, `lit_read_file`, `lit_write_file`
- [x] MCP resource exposure: repository state, file contents, commit history

### 3.3 Swarm Coordination
- [x] Agent branch namespacing (`agents/<agent-id>/<branch>`)
- [x] File lease system (exclusive write access to specific files)
- [x] CRDT-based metadata sync for eventual consistency
- [x] Coordinator role for merging agent branches

**Milestone**: Multiple LLM agents (Claude, GPT, etc.) can collaborate on a shared codebase through MCP tool calls and the Lit API server, with structured conflict resolution.

---

## Phase 4 — Git Interop

**Goal**: Seamless migration between Git and Lit.

### 4.1 Import
- [x] `lit import-git <path-or-url>` — convert Git repo to Lit
- [x] Object conversion (rehash SHA-1 → SHA3+BLAKE3)
- [x] Ref conversion (branches, tags, HEAD)
- [x] Config migration
- [x] `.gitignore` → `.litignore` (with `.gitignore` fallback)

### 4.2 Export
- [x] `lit export-git <path>` — convert Lit repo to Git
- [x] Object conversion (SHA3+BLAKE3 → SHA-1)
- [x] Lit metadata → Git notes
- [x] Ref conversion (branches, tags, remotes, packed-refs)

### 4.3 Remote Git Interop
- [x] Push to Git remotes (convert on-the-fly)
- [x] Pull from Git remotes (convert on-the-fly)
- [x] Clone from Git URLs

**Milestone**: Organizations can adopt Lit incrementally — import existing Git repos, and export back if needed.

---

## Phase 5 — Performance & Scale

**Goal**: Handle large repositories and high-throughput agent workloads.

### 5.1 Pack Files
- [x] Delta compression for similar objects
- [x] Pack index for fast object lookup
- [x] `lit gc` — garbage collect and repack loose objects
- [x] Memory-mapped pack file access

### 5.2 Parallel I/O
- [x] Parallel object hashing (rayon)
- [x] Parallel filesystem stat for `lit status`
- [x] Concurrent object reads (lock-free)
- [x] Parallel pack file writing

### 5.3 Large File Support
- [x] LFS-style pointer files for large binary assets
- [x] Chunked storage for files > 100MB
- [x] Streaming blob read/write (avoid loading entire files into memory)

### 5.4 Index Format
- [x] Binary index format (currently JSON) for faster load/save
- [x] Index extensions for cached tree info
- [x] Filesystem monitor integration for faster status

**Milestone**: Lit handles repositories at the scale of Linux kernel, Chromium, or large monorepos with performance competitive to Git.

---

## Version Targets

| Version    | Phase         | Key Deliverable                                  |
| ---------- | ------------- | ------------------------------------------------ |
| **v0.2.0** | Phase 0       | Structured output, error codes, zero-prompt mode |
| **v0.3.0** | Phase 1.1-1.2 | Diff + merge engine                              |
| **v0.4.0** | Phase 1.3     | Full push/pull/clone/fetch                       |
| **v0.5.0** | Phase 1.4-1.8 | Tag, stash, reset, revert, rebase, blame         |
| **v0.6.0** | Phase 2       | Batch mode, transactions, metadata, search       |
| **v0.7.0** | Phase 3       | API server + MCP server                          |
| **v0.8.0** | Phase 4       | Git import/export                                |
| **v1.0.0** | Phase 5       | Performance, pack files, production-ready        |

---

## Non-Goals

- **GUI**: Lit is a CLI/API tool. GUI clients can be built on top of the API.
- **Hosting platform**: Lit is not GitHub/GitLab. It is the VCS engine. Hosting is a separate concern.
- **Backward compatibility with Git wire protocol**: Lit uses its own sync protocol. Git interop is via import/export conversion, not protocol compatibility.
- **Plugin system**: Hooks and the event bus cover extensibility. A plugin API adds complexity without clear agentic benefit.
