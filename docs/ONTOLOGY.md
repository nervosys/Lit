# Lit Ontology — Complete Conceptual Reference

This document is the authoritative human-readable ontology for the Lit version control system.
It defines every concept, type, relationship, and classification in the system, organized as a
formal knowledge graph. For the machine-readable (JSON) equivalent, run `lit ontology`.

## Namespace Prefixes

| Prefix    | URI                                          | Purpose                    |
| --------- | -------------------------------------------- | -------------------------- |
| `lit:`    | `https://lit-vcs.dev/ontology/v1#`           | Lit-specific concepts      |
| `vcs:`    | `https://lit-vcs.dev/ontology/vcs#`          | Version control primitives |
| `schema:` | `https://schema.org/`                        | General-purpose types      |
| `mcp:`    | `https://modelcontextprotocol.io/schema/v1#` | Model Context Protocol     |

---

## 1. System Identity

| Property       | Value                                                          |
| -------------- | -------------------------------------------------------------- |
| **Name**       | Lit                                                            |
| **Type**       | `schema:SoftwareApplication` / `vcs:VersionControlSystem`      |
| **Version**    | 1.0.0                                                          |
| **Language**   | Rust (edition 2021)                                            |
| **License**    | MIT                                                            |
| **Repository** | `github.com/nervosys/Lit`                                      |
| **Design**     | Agentic-first — all interfaces emit structured JSON by default |

---

## 2. Capability Taxonomy

Capabilities are top-level functional axes. Each maps to one or more command categories.

| ID                       | Name                        | Description                                                                             |
| ------------------------ | --------------------------- | --------------------------------------------------------------------------------------- |
| `version-control`        | Distributed Version Control | Full DAG-based version control with branches, merges, commits, and tags                 |
| `post-quantum-crypto`    | Post-Quantum Cryptography   | ML-DSA-87 signatures and quantum-resistant hashing (SHA3-512 + BLAKE3)                  |
| `fips-compliance`        | FIPS 140-3 Compliance       | SHA3-512, AES-256-GCM, HMAC-SHA-256, PBKDF2 key derivation with secure zeroization      |
| `structured-io`          | Structured I/O              | All commands produce structured JSON output by default, with human-readable alternative |
| `batch-mode`             | Batch Operations            | Execute multiple operations from JSONL stdin with atomic and dry-run modes              |
| `transactions`           | Transaction Support         | Begin/commit/rollback with write-ahead log for crash recovery                           |
| `agent-metadata`         | Agent Metadata              | First-class metadata field on commits for agent_id, task_id, confidence, intent         |
| `search`                 | Full-Text Search            | Search file contents, commit messages, and agent metadata                               |
| `snapshot`               | Atomic Snapshots            | Single-command add-all + commit for agent workflows                                     |
| `integrity-verification` | Repository Verification     | Full integrity check of objects, refs, DAG connectivity, and index                      |
| `mcp-server`             | MCP Tool Server             | Model Context Protocol server for LLM agent integration (stdio and HTTP)                |
| `rest-api`               | REST API Server             | HTTP API for remote repository operations with bearer token authentication              |
| `swarm-coordination`     | Multi-Agent Swarm           | Agent registration, branch namespacing, and file lease system                           |
| `airgap-mode`            | Air-Gap Mode                | Blocks all network protocols, allows only physical/local transports                     |
| `encryption-at-rest`     | Encryption at Rest          | AES-256-GCM encryption for all repository objects and refs                              |

---

## 3. Object Type Hierarchy

All persistent data is stored as content-addressed objects in `.lit/objects/`.

### 3.1 Core Objects

```
vcs:Object (abstract)
├── vcs:Blob        — file content (compressed bytes)
├── vcs:Tree        — directory listing (entries → Blob | Tree)
├── vcs:Commit      — snapshot (tree + parents + author + timestamp + message + metadata? + signature?)
└── vcs:Tag         — named reference to a commit (name + target + tagger + message + signature?)
```

### 3.2 Object Properties

#### `vcs:Blob`
| Property | Type    | Required | Description             |
| -------- | ------- | -------- | ----------------------- |
| `data`   | `bytes` | ✅        | Compressed file content |

#### `vcs:Tree`
| Property  | Type          | Required | Description              |
| --------- | ------------- | -------- | ------------------------ |
| `entries` | `TreeEntry[]` | ✅        | Files and subdirectories |

#### `vcs:TreeEntry`
| Property      | Type         | Required | Description                                                    |
| ------------- | ------------ | -------- | -------------------------------------------------------------- |
| `mode`        | `string`     | ✅        | `100644` (normal), `100755` (executable), `040000` (directory) |
| `name`        | `string`     | ✅        | File or directory name                                         |
| `hash`        | `ObjectHash` | ✅        | Hash of the referenced blob or tree                            |
| `object_type` | `string`     | ✅        | `"blob"` or `"tree"`                                           |

#### `vcs:Commit`
| Property    | Type                  | Required | Description                                     |
| ----------- | --------------------- | -------- | ----------------------------------------------- |
| `tree`      | `ObjectHash`          | ✅        | Hash of the root tree object                    |
| `parents`   | `ObjectHash[]`        | ✅        | Parent commit hashes (empty for initial commit) |
| `author`    | `string`              | ✅        | Author identity                                 |
| `timestamp` | `integer`             | ✅        | Unix timestamp (seconds since epoch)            |
| `message`   | `string`              | ✅        | Commit message                                  |
| `metadata`  | `AgentMetadata\|null` | ❌        | Optional agent metadata                         |
| `signature` | `PQSignature\|null`   | ❌        | Optional ML-DSA-87 signature                    |

#### `vcs:Tag`
| Property    | Type                | Required | Description                  |
| ----------- | ------------------- | -------- | ---------------------------- |
| `name`      | `string`            | ✅        | Tag name                     |
| `target`    | `ObjectHash`        | ✅        | The tagged commit hash       |
| `tagger`    | `string`            | ✅        | Author of the tag            |
| `message`   | `string`            | ✅        | Tag message                  |
| `signature` | `PQSignature\|null` | ❌        | Optional ML-DSA-87 signature |

### 3.3 Reference Types

```
vcs:Reference (abstract)
├── vcs:Branch      — mutable pointer to a commit (.lit/refs/heads/)
├── vcs:RemoteRef   — remote-tracking ref (.lit/refs/remotes/)
└── vcs:TagRef      — immutable pointer (.lit/refs/tags/)
```

#### `vcs:Branch`
| Property | Type         | Required | Description                      |
| -------- | ------------ | -------- | -------------------------------- |
| `name`   | `string`     | ✅        | Branch name                      |
| `target` | `ObjectHash` | ✅        | The commit this branch points to |

#### `vcs:RemoteConfig`
| Property | Type     | Required | Description                  |
| -------- | -------- | -------- | ---------------------------- |
| `name`   | `string` | ✅        | Remote name (e.g., `origin`) |
| `url`    | `string` | ✅        | Remote repository URL        |

### 3.4 Derived / Supporting Types

#### `lit:ObjectHash`
192-character hex string: SHA3-512 (128 chars) + BLAKE3 (64 chars) concatenated.

| Segment      | Algorithm | Length  | Standard      |
| ------------ | --------- | ------- | ------------- |
| `[0..128)`   | SHA3-512  | 128 hex | NIST FIPS 202 |
| `[128..192)` | BLAKE3    | 64 hex  | —             |

#### `lit:AgentMetadata`
| Property        | Type     | Required | Description                                           |
| --------------- | -------- | -------- | ----------------------------------------------------- |
| `agent_id`      | `string` | ❌        | Unique identifier of the agent                        |
| `agent_model`   | `string` | ❌        | Model name/version (e.g., `claude-opus-4-20250514`)   |
| `task_id`       | `string` | ❌        | Task or work item identifier                          |
| `confidence`    | `number` | ❌        | Self-assessed confidence (0.0–1.0)                    |
| `intent`        | `string` | ❌        | Human-readable intent description                     |
| `tool_versions` | `object` | ❌        | Versions of tools used (compiler, linter, etc.)       |
| `parent_task`   | `string` | ❌        | Reference to a parent task for hierarchical workflows |
| `session_id`    | `string` | ❌        | Conversation or session identifier                    |

#### `lit:PQSignature`
| Property    | Type     | Required | Description                        |
| ----------- | -------- | -------- | ---------------------------------- |
| `algorithm` | `string` | ✅        | `"ML-DSA-87"`                      |
| `signature` | `bytes`  | ✅        | Raw signature bytes (~4,627 bytes) |

#### `lit:PQKeyPair`
| Property     | Type    | Required | Description                         |
| ------------ | ------- | -------- | ----------------------------------- |
| `public_key` | `bytes` | ✅        | ML-DSA-87 public key (~2,592 bytes) |
| `secret_key` | `bytes` | ✅        | ML-DSA-87 secret key (~4,880 bytes) |

#### `lit:IndexEntry`
| Property | Type     | Required | Description                      |
| -------- | -------- | -------- | -------------------------------- |
| `path`   | `string` | ✅        | Relative file path               |
| `hash`   | `string` | ✅        | Object hash of staged content    |
| `mode`   | `string` | ✅        | File mode (100644, 100755, etc.) |

#### `lit:DiffHunk`
| Property    | Type         | Required | Description                      |
| ----------- | ------------ | -------- | -------------------------------- |
| `old_start` | `integer`    | ✅        | Starting line number in original |
| `old_count` | `integer`    | ✅        | Number of lines from original    |
| `new_start` | `integer`    | ✅        | Starting line number in modified |
| `new_count` | `integer`    | ✅        | Number of lines in modified      |
| `lines`     | `DiffLine[]` | ✅        | Individual line changes          |

#### `lit:DiffLine`
| Property  | Type     | Required | Description                         |
| --------- | -------- | -------- | ----------------------------------- |
| `kind`    | `string` | ✅        | `"context"`, `"add"`, or `"remove"` |
| `content` | `string` | ✅        | Line content                        |

#### `lit:FileLease`
| Property      | Type      | Required | Description                  |
| ------------- | --------- | -------- | ---------------------------- |
| `agent_id`    | `string`  | ✅        | The agent holding the lease  |
| `path`        | `string`  | ✅        | File path the lease covers   |
| `acquired_at` | `integer` | ✅        | Unix timestamp when acquired |
| `expires_at`  | `integer` | ✅        | Unix timestamp when expires  |

#### `lit:EncryptionConfig`
| Property         | Type      | Required | Description                      |
| ---------------- | --------- | -------- | -------------------------------- |
| `enabled`        | `boolean` | ✅        | Whether encryption is active     |
| `algorithm`      | `string`  | ✅        | `"AES-256-GCM"`                  |
| `salt`           | `bytes`   | ✅        | Random salt for key derivation   |
| `kdf_iterations` | `integer` | ✅        | PBKDF2 iteration count (600,000) |

#### `lit:LfsPointer`
| Property  | Type      | Required | Description                            |
| --------- | --------- | -------- | -------------------------------------- |
| `version` | `string`  | ✅        | LFS pointer format version             |
| `oid`     | `string`  | ✅        | Object identifier (`sha3-blake3:hash`) |
| `size`    | `integer` | ✅        | Original file size in bytes            |

#### `lit:PackFile`
| Property       | Type      | Required | Description                   |
| -------------- | --------- | -------- | ----------------------------- |
| `magic`        | `string`  | ✅        | File magic bytes: `LITP`      |
| `version`      | `integer` | ✅        | Pack format version           |
| `object_count` | `integer` | ✅        | Number of objects in the pack |

#### `lit:TransactionState`
| Property     | Type       | Required | Description                       |
| ------------ | ---------- | -------- | --------------------------------- |
| `tx_id`      | `string`   | ✅        | Unique transaction identifier     |
| `started_at` | `integer`  | ✅        | Unix timestamp when began         |
| `operations` | `object[]` | ✅        | Operations within the transaction |

#### `lit:ReflogEntry`
| Property    | Type         | Required | Description                   |
| ----------- | ------------ | -------- | ----------------------------- |
| `index`     | `integer`    | ✅        | Entry index (0 = most recent) |
| `old_hash`  | `ObjectHash` | ✅        | Previous ref target           |
| `new_hash`  | `ObjectHash` | ✅        | New ref target                |
| `action`    | `string`     | ✅        | Action that caused the change |
| `message`   | `string`     | ✅        | Description of the change     |
| `timestamp` | `integer`    | ✅        | Unix timestamp                |

---

## 4. Command Taxonomy (42 Commands)

Commands are classified by category with explicit preconditions, side effects, idempotency, and
sequencing relationships (preceded_by / follows).

### 4.1 Core Version Control (7 commands)

| Command  | Description                                         | Safe | Idempotent |
| -------- | --------------------------------------------------- | ---- | ---------- |
| `init`   | Create a new Lit repository                         | ✅    | ✅          |
| `add`    | Stage files to the index                            | ❌    | ✅          |
| `commit` | Record staged changes as a new commit               | ❌    | ❌          |
| `status` | Show working tree status (branch, staged, modified) | ✅    | ✅          |
| `log`    | Display commit history from HEAD                    | ✅    | ✅          |
| `diff`   | Show differences (working tree, index, commits)     | ✅    | ✅          |
| `show`   | Display contents of a commit, tree, or blob         | ✅    | ✅          |

### 4.2 Branching & Merging (4 commands)

| Command    | Description                                           | Safe | Idempotent |
| ---------- | ----------------------------------------------------- | ---- | ---------- |
| `branch`   | List, create, or delete branches                      | ✅    | ✅          |
| `checkout` | Switch branch or restore working tree files           | ❌    | ❌          |
| `merge`    | Merge another branch into current                     | ❌    | ❌          |
| `resolve`  | Resolve merge conflicts (strategy or manual finalize) | ❌    | ❌          |

### 4.3 Collaboration (5 commands)

| Command  | Description                                    | Safe | Idempotent |
| -------- | ---------------------------------------------- | ---- | ---------- |
| `remote` | Add, remove, or list remote URLs (LAN only)    | ✅    | ✅          |
| `clone`  | Clone a remote repository into a new directory | ❌    | ❌          |
| `fetch`  | Download objects and refs without merging      | ❌    | ✅          |
| `push`   | Upload commits to a remote                     | ❌    | ❌          |
| `pull`   | Fetch and merge from a remote                  | ❌    | ❌          |

### 4.4 History Manipulation (7 commands)

| Command       | Description                                           | Safe | Idempotent |
| ------------- | ----------------------------------------------------- | ---- | ---------- |
| `tag`         | Create, list, delete, sign, or verify tags            | ✅    | ✅          |
| `stash`       | Save, restore, list, or drop stashed changes          | ❌    | ❌          |
| `reset`       | Reset HEAD (soft/mixed/hard)                          | ❌    | ❌          |
| `revert`      | Create inverse commit undoing a specific commit       | ❌    | ❌          |
| `cherry-pick` | Apply changes from a specific commit                  | ❌    | ❌          |
| `rebase`      | Reapply commits onto a new base (interactive support) | ❌    | ❌          |
| `reflog`      | Show reference change history                         | ✅    | ✅          |

### 4.5 Analysis (2 commands)

| Command  | Description                              | Safe | Idempotent |
| -------- | ---------------------------------------- | ---- | ---------- |
| `blame`  | Show revision/author per line            | ✅    | ✅          |
| `bisect` | Binary search for bug-introducing commit | ❌    | ❌          |

### 4.6 Agent-Optimized (6 commands)

| Command    | Description                                              | Safe | Idempotent |
| ---------- | -------------------------------------------------------- | ---- | ---------- |
| `snapshot` | Atomic stage-all + commit                                | ❌    | ❌          |
| `batch`    | Execute JSONL operations (atomic/dry-run modes)          | ❌    | ❌          |
| `search`   | Full-text search (files, messages, metadata)             | ✅    | ✅          |
| `verify`   | Full repository integrity check                          | ✅    | ✅          |
| `watch`    | Monitor filesystem for changes (continuous JSONL stream) | ✅    | ✅          |
| `ontology` | Output the machine-readable ontology as JSON             | ✅    | ✅          |

### 4.7 Swarm Coordination (5 commands)

| Command               | Description                             | Safe | Idempotent |
| --------------------- | --------------------------------------- | ---- | ---------- |
| `swarm register`      | Register an agent for multi-agent work  | ✅    | ❌          |
| `swarm list`          | List registered agents                  | ✅    | ✅          |
| `swarm lease-acquire` | Acquire exclusive write lease on a file | ❌    | ❌          |
| `swarm lease-release` | Release a file lease                    | ✅    | ❌          |
| `swarm lease-list`    | List all active file leases             | ✅    | ✅          |

### 4.8 Server & API (2 commands)

| Command     | Description                                         | Safe | Idempotent |
| ----------- | --------------------------------------------------- | ---- | ---------- |
| `serve`     | REST API + lit:// daemon + stdio server             | ❌    | ❌          |
| `mcp-serve` | MCP JSON-RPC 2.0 server (stdio and HTTP transports) | ❌    | ❌          |

### 4.9 Configuration (1 command)

| Command  | Description                                    | Safe | Idempotent |
| -------- | ---------------------------------------------- | ---- | ---------- |
| `config` | Show, get, or set repository and global config | ✅    | ✅          |

### 4.10 Git Interop (2 commands)

| Command      | Description                                          | Safe | Idempotent |
| ------------ | ---------------------------------------------------- | ---- | ---------- |
| `import-git` | Import Git repository → Lit (SHA-1 → composite hash) | ❌    | ❌          |
| `export-git` | Export Lit repository → Git (composite hash → SHA-1) | ❌    | ❌          |

### 4.11 Performance (3 commands)

| Command       | Description                                  | Safe | Idempotent |
| ------------- | -------------------------------------------- | ---- | ---------- |
| `gc`          | Pack loose objects into LITP pack files      | ❌    | ❌          |
| `lfs track`   | Track file patterns for Large File Storage   | ✅    | ✅          |
| `lfs migrate` | Migrate existing large files to LFS pointers | ❌    | ❌          |

### 4.12 Security (1 command)

| Command      | Description                                       | Safe | Idempotent |
| ------------ | ------------------------------------------------- | ---- | ---------- |
| `rotate-key` | Re-encrypt all objects/refs with a new passphrase | ❌    | ❌          |

### 4.13 Transaction (3 commands)

| Command       | Description                                     | Safe | Idempotent |
| ------------- | ----------------------------------------------- | ---- | ---------- |
| `tx begin`    | Start transaction with write-ahead log          | ❌    | ❌          |
| `tx commit`   | Finalize transaction                            | ❌    | ❌          |
| `tx rollback` | Undo transaction, restore pre-transaction state | ❌    | ❌          |

---

## 5. MCP Tool Mapping (30 Tools)

All MCP tools are prefixed `lit_` and exposed via JSON-RPC 2.0 over stdio or HTTP.

| MCP Tool          | CLI Equivalent    | Category      |
| ----------------- | ----------------- | ------------- |
| `lit_init`        | `lit init`        | Core          |
| `lit_add`         | `lit add`         | Core          |
| `lit_commit`      | `lit commit`      | Core          |
| `lit_status`      | `lit status`      | Core          |
| `lit_log`         | `lit log`         | Core          |
| `lit_diff`        | `lit diff`        | Core          |
| `lit_show`        | `lit show`        | Core          |
| `lit_branch`      | `lit branch`      | Branching     |
| `lit_checkout`    | `lit checkout`    | Branching     |
| `lit_merge`       | `lit merge`       | Branching     |
| `lit_resolve`     | `lit resolve`     | Branching     |
| `lit_tag`         | `lit tag`         | Tag           |
| `lit_stash`       | `lit stash`       | History       |
| `lit_reset`       | `lit reset`       | History       |
| `lit_revert`      | `lit revert`      | History       |
| `lit_rebase`      | `lit rebase`      | History       |
| `lit_cherry_pick` | `lit cherry-pick` | History       |
| `lit_blame`       | `lit blame`       | Analysis      |
| `lit_reflog`      | `lit reflog`      | History       |
| `lit_push`        | `lit push`        | Collaboration |
| `lit_pull`        | `lit pull`        | Collaboration |
| `lit_fetch`       | `lit fetch`       | Collaboration |
| `lit_clone`       | `lit clone`       | Collaboration |
| `lit_search`      | `lit search`      | Agent         |
| `lit_verify`      | `lit verify`      | Agent         |
| `lit_gc`          | `lit gc`          | Performance   |
| `lit_snapshot`    | `lit snapshot`    | Agent         |
| `lit_config`      | `lit config`      | Configuration |
| `lit_ontology`    | `lit ontology`    | Discovery     |
| `lit_schema`      | `lit schema`      | Discovery     |

### MCP Resources

| URI              | Description                    |
| ---------------- | ------------------------------ |
| `lit://status`   | Current repository status      |
| `lit://branches` | All branches and their targets |
| `lit://log`      | Recent commit history          |
| `lit://ontology` | Full ontology (JSON)           |
| `lit://schema`   | JSON Schema for all types      |

---

## 6. Protocol Taxonomy

### 6.1 CLI Protocol

| Property           | Value                                |
| ------------------ | ------------------------------------ |
| **Binary**         | `lit`                                |
| **Default output** | JSON                                 |
| **Human mode**     | `--human`                            |
| **Encryption**     | `--passphrase` / `--passphrase-file` |
| **Airgap**         | `--airgapped`                        |

### 6.2 REST API Protocol

| Property           | Value                                       |
| ------------------ | ------------------------------------------- |
| **Base path**      | `/api/v1`                                   |
| **Content type**   | `application/json`                          |
| **Authentication** | Bearer token (`--token` or `LIT_API_TOKEN`) |
| **Rate limiting**  | 100 requests/60 seconds per IP              |
| **Binding**        | `127.0.0.1` (localhost only)                |

### 6.3 MCP Protocol

| Property             | Value                         |
| -------------------- | ----------------------------- |
| **Protocol version** | `2024-11-05`                  |
| **RPC format**       | JSON-RPC 2.0                  |
| **Tool prefix**      | `lit_`                        |
| **Stdio transport**  | `lit mcp-serve --stdio`       |
| **HTTP transport**   | `lit mcp-serve --port <PORT>` |

### 6.4 Batch Protocol

| Property         | Value                                    |
| ---------------- | ---------------------------------------- |
| **Format**       | JSONL (one JSON object per line)         |
| **Input**        | stdin                                    |
| **Atomic mode**  | `--atomic` (stop on first failure)       |
| **Dry-run mode** | `--dry-run` (validate without executing) |

### 6.5 Transport Protocols

| Protocol      | Layer    | Encryption            | Authentication   | Scope             |
| ------------- | -------- | --------------------- | ---------------- | ----------------- |
| HTTPS         | Network  | TLS 1.3               | Bearer token     | LAN               |
| SSH           | Network  | SSH transport         | SSH keys         | LAN               |
| `lit://`      | Network  | None (plain TCP)      | None (localhost) | Localhost         |
| `file://`     | Local    | At-rest (AES-256-GCM) | FS permissions   | Local             |
| USB/removable | Physical | At-rest (AES-256-GCM) | Physical access  | Airgap            |
| SMB/NFS       | Physical | At-rest (AES-256-GCM) | Share auth       | Non-strict airgap |

---

## 7. Cryptographic Ontology

### 7.1 Algorithm Classification

| Category           | Algorithm          | Standard                  | Quantum-Resistant | Use in Lit                    |
| ------------------ | ------------------ | ------------------------- | ----------------- | ----------------------------- |
| Primary hashing    | SHA3-512           | NIST FIPS 202             | ✅ (256-bit)       | First 128 hex of object hash  |
| Secondary hashing  | BLAKE3             | —                         | ✅ (128-bit)       | Last 64 hex of object hash    |
| Digital signatures | ML-DSA-87          | NIST FIPS 204             | ✅ (256-bit)       | Commit/tag signing            |
| Encryption         | AES-256-GCM        | NIST FIPS 197, SP 800-38D | ✅ (128-bit)       | At-rest object/ref encryption |
| Key derivation     | PBKDF2-HMAC-SHA512 | NIST SP 800-132           | ✅                 | Passphrase → 256-bit key      |
| Audit integrity    | HMAC-SHA-256       | NIST FIPS 198-1           | ✅                 | Tamper-evident audit log      |
| Legacy (interop)   | SHA-1              | Deprecated                | ❌                 | Git import/export only        |

### 7.2 FIPS 140-3 Self-Tests (Power-On)

| KAT Test         | Algorithm    | What It Verifies                                |
| ---------------- | ------------ | ----------------------------------------------- |
| SHA-256 KAT      | SHA-256      | Known-answer test against NIST vector           |
| SHA-512 KAT      | SHA-512      | Known-answer test against NIST vector           |
| SHA3-512 KAT     | SHA3-512     | Known-answer test against NIST vector           |
| HMAC-SHA-256 KAT | HMAC-SHA-256 | Known-answer test against NIST vector           |
| RNG health test  | OsRng        | Repetition count test, stuck-at-fault detection |

### 7.3 Key Sizes

| Component     | Size               |
| ------------- | ------------------ |
| Object hash   | 768 bits (192 hex) |
| AES key       | 256 bits           |
| PBKDF2 salt   | 256 bits           |
| PBKDF2 iters  | 600,000            |
| ML-DSA-87 pk  | ~2,592 bytes       |
| ML-DSA-87 sk  | ~4,880 bytes       |
| ML-DSA-87 sig | ~4,627 bytes       |
| GCM nonce     | 96 bits            |
| HMAC key      | 256 bits           |

---

## 8. Workflow Ontology

Predefined agent workflows connecting commands in recommended sequences.

### 8.1 Basic Agent Workflow
```
status → snapshot → push
```

### 8.2 Agent Branch Workflow
```
checkout -b → snapshot → checkout → merge → push
```

### 8.3 Batch Operation Workflow
```
batch --dry-run → batch --atomic
```

### 8.4 Transaction Workflow
```
tx begin → [add, commit, ...] → tx commit | tx rollback
```

### 8.5 Multi-Agent Collaboration
```
swarm register → swarm lease-acquire → checkout -b → snapshot → swarm lease-release → push
```

### 8.6 Verify and Fix
```
verify → search (if failures) → snapshot (if fixes)
```

### 8.7 Git Migration
```
import-git → verify → log → branch --all
```

### 8.8 Agent Code Review
```
log → diff → blame → search → snapshot (review metadata)
```

### 8.9 Automated Bug Bisection
```
bisect start → bisect bad → bisect good → verify (loop) → bisect reset
```

### 8.10 Encrypted Repository
```
init → config set (encryption) → snapshot → rotate-key → verify
```

---

## 9. Error Ontology

All errors follow a structured JSON envelope:

```json
{
  "status": "error",
  "command": "<command_name>",
  "error": {
    "code": "<ERROR_CODE>",
    "message": "<human_readable_message>",
    "suggestions": ["<recovery_hint>"]
  }
}
```

### Error Categories

| Code               | Description                          | Recoverable | Suggested Action                                 |
| ------------------ | ------------------------------------ | ----------- | ------------------------------------------------ |
| `REPO_NOT_FOUND`   | Not a Lit repository                 | ✅           | Run `lit init`                                   |
| `REPO_CORRUPT`     | Repository data corrupt/inconsistent | ❌           | Run `lit verify`; may require re-clone           |
| `NO_COMMITS`       | No commits in repository             | ✅           | Create files and run `lit snapshot -m "initial"` |
| `MERGE_CONFLICT`   | Merge conflict detected              | ✅           | Use `lit resolve --all --strategy ours`          |
| `NOTHING_STAGED`   | No files staged for commit           | ✅           | Run `lit add <files>` or use `lit snapshot`      |
| `REF_NOT_FOUND`    | Branch/tag/ref does not exist        | ✅           | Run `lit branch --all` or `lit tag --list`       |
| `REF_CONFLICT`     | Reference already exists             | ✅           | Use a different name or delete existing ref      |
| `OBJECT_NOT_FOUND` | Object hash not found in store       | ❌           | Run `lit verify`                                 |
| `INDEX_LOCKED`     | Index locked by another operation    | ✅           | Wait or remove `.lit/index.lock`                 |
| `TX_IN_PROGRESS`   | Another transaction is active        | ✅           | Run `lit tx rollback`                            |
| `LEASE_HELD`       | File lease held by another agent     | ✅           | Wait for expiration or coordinate                |
| `TRANSPORT_DENIED` | Network transport blocked            | ✅           | Disable `--airgapped` or use `file://`           |
| `AUTH_FAILED`      | Authentication failed                | ✅           | Check `--token` or `LIT_API_TOKEN`               |
| `CRYPTO_ERROR`     | Encryption/decryption failed         | ✅           | Verify passphrase                                |
| `INVALID_INPUT`    | Invalid argument or parameter        | ✅           | Check `lit <command> --help`                     |
| `IO_ERROR`         | File system read/write error         | ❌           | Check permissions and disk space                 |
| `CONFIG_ERROR`     | Config missing or malformed          | ✅           | Run `lit config show` or `lit init`              |
| `NOT_IMPLEMENTED`  | Feature not yet implemented          | ❌           | Planned for a future release                     |

---

## 10. Architecture Layers

Lit follows a 7-layer architecture from CLI entry point to disk I/O:

```
┌──────────────────────────────────────────────────────┐
│ Layer 7: CLI / API / MCP Entry Points                │
│   main.rs, response.rs, formatter.rs                 │
├──────────────────────────────────────────────────────┤
│ Layer 6: Command Dispatch                            │
│   commands/*.rs (42 modules)                         │
├──────────────────────────────────────────────────────┤
│ Layer 5: Domain Logic                                │
│   core/objects.rs, core/refs.rs, core/diff.rs,       │
│   core/merge.rs                                      │
├──────────────────────────────────────────────────────┤
│ Layer 4: Network & Transport                         │
│   network/validator.rs, network/transport.rs,        │
│   network/https.rs, network/ssh.rs,                  │
│   network/lit_protocol.rs, network/airgap.rs,        │
│   network/audit.rs                                   │
├──────────────────────────────────────────────────────┤
│ Layer 3: Cryptography                                │
│   crypto/encryption.rs (AES-256-GCM, PBKDF2)        │
│   crypto/signatures.rs (ML-DSA-87)                   │
│   crypto/fips.rs (self-tests, KATs, RNG health)      │
├──────────────────────────────────────────────────────┤
│ Layer 2: Storage Engine                              │
│   storage/objects.rs (content-addressable store)     │
│   storage/index.rs (staging area)                    │
│   storage/binary_index.rs (pack file indexes)        │
├──────────────────────────────────────────────────────┤
│ Layer 1: File System / OS                            │
│   std::fs, dirs, walkdir, shellexpand                │
└──────────────────────────────────────────────────────┘
```

---

## 11. Repository Layout

```
.lit/
├── HEAD                          # Current branch reference
├── config.toml                   # Repository configuration
├── encryption.key                # Encrypted key file (if encryption enabled)
├── index                         # Staging area (binary)
├── audit.log                     # HMAC-signed audit log
├── audit.key                     # Audit log HMAC key (read-only)
├── objects/                      # Content-addressable object store
│   └── <first 4 hex>/           # Sharded by first 4 chars of hash
│       └── <remaining 188 hex>  # Compressed object data
├── refs/
│   ├── heads/                    # Branch references
│   │   └── main                  # → commit hash
│   ├── tags/                     # Tag references
│   │   └── v1.0                  # → commit or tag object hash
│   └── remotes/
│       └── origin/               # Remote-tracking refs
│           └── main
├── packs/                        # LITP pack files (after gc)
│   ├── <hash>.pack               # Packed objects
│   └── <hash>.idx                # Pack index
├── stash/                        # Stashed changes
├── lfs/                          # Large file storage objects
├── remotes                       # Remote URL configuration
├── reflog/                       # Reference update history
├── bisect.json                   # Active bisect state
├── transaction.json              # Active transaction WAL
├── transaction.lock              # Transaction lock file
└── swarm/                        # Multi-agent coordination
    ├── agents/                   # Registered agent files
    └── leases/                   # Active file leases
```

---

## 12. Configuration Hierarchy

Configuration follows a 6-level precedence (highest wins):

```
CLI flags > Environment variables > Repo config > Global config > System config > Defaults
```

### Environment Variables

| Variable              | Purpose                                  |
| --------------------- | ---------------------------------------- |
| `LIT_PASSPHRASE`      | Encryption passphrase                    |
| `LIT_PASSPHRASE_FILE` | Path to file containing passphrase       |
| `LIT_API_TOKEN`       | Bearer token for REST API authentication |

### Configuration Sections

```toml
[core]
default_branch = "main"
default_output = "json"          # json | human | msgpack
auto_sign = true                 # ML-DSA-87 auto-sign commits

[agent]
default_output = "json"
auto_sign = true
metadata = {}                    # Default agent metadata

[merge]
default_strategy = "recursive"   # recursive | ours | theirs | agent-auto | union
auto_resolve = true

[security]
encryption = "aes-256-gcm"
fips_mode = true
audit_log = true
audit_log_path = "~/.lit/audit.log"

[network]
allowed_networks = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]
allowed_hosts = ["git.internal.company.com"]

[airgap]
enabled = false
strict_mode = false              # true = USB only; false = USB + network shares
allowed_transports = ["LocalFilesystem", "RemovableMedia", "NetworkShare", "FileProtocol"]
allowed_media = []               # Empty = all USB

[encryption]
enabled = true
key_file = "~/.lit/encryption.key"
fips_mode = true
cache_timeout_secs = 300         # Passphrase cache TTL (0 = disabled)
```

---

## 13. Compliance Mapping

| Standard            | Level / Scope     | Status            |
| ------------------- | ----------------- | ----------------- |
| **NIST FIPS 140-3** | Level 1           | ✅ 95% complete    |
| **NIST FIPS 202**   | SHA-3             | ✅ Approved        |
| **NIST FIPS 204**   | ML-DSA-87         | ✅ Approved        |
| **NIST FIPS 197**   | AES-256           | ✅ Approved        |
| **NIST FIPS 198-1** | HMAC              | ✅ Approved        |
| **NIST SP 800-132** | PBKDF2            | ✅ 600K iterations |
| **NIST SP 800-38D** | AES-GCM           | ✅ 96-bit nonce    |
| **NSA CNSA 2.0**    | Quantum-resistant | ✅ Compatible      |
| **CMMC 2.0**        | —                 | ✅ Aligned         |

---

## 14. Dependency Graph

### Direct Dependencies (Core)

| Crate             | Version | Purpose                                 |
| ----------------- | ------- | --------------------------------------- |
| `clap`            | 4.5     | CLI argument parsing (derive macros)    |
| `sha3`            | 0.10    | SHA3-512 hashing (NIST FIPS 202)        |
| `blake3`          | 1.5     | BLAKE3 hashing                          |
| `aes-gcm`         | 0.10    | AES-256-GCM encryption (NIST FIPS 197)  |
| `pbkdf2`          | 0.12    | PBKDF2 key derivation (NIST SP 800-132) |
| `hmac`            | 0.12    | HMAC-SHA-256 (NIST FIPS 198-1)          |
| `sha2`            | 0.10    | SHA-256/SHA-512 (for HMAC, PBKDF2)      |
| `pqcrypto-mldsa`  | 0.1     | ML-DSA-87 post-quantum signatures       |
| `pqcrypto-traits` | 0.3     | Trait abstractions for pqcrypto         |
| `serde`           | 1.x     | Serialization framework                 |
| `serde_json`      | 1.x     | JSON serialization                      |
| `rmp-serde`       | 1.x     | MsgPack serialization                   |
| `flate2`          | 1.x     | zlib compression                        |
| `chrono`          | 0.4     | Timestamp handling                      |
| `tiny_http`       | 0.12    | Minimal HTTP server                     |
| `ureq`            | 2.x     | HTTP client (pure Rust TLS)             |
| `rayon`           | 1.x     | Parallel I/O                            |
| `zeroize`         | 1.8     | Secure memory zeroization               |
| `subtle`          | 2.6     | Constant-time comparison                |
| `walkdir`         | 2.x     | Directory traversal                     |
| `regex`           | 1.x     | URL parsing, pattern matching           |

---

## 15. Relationship Map

### Object → Reference Relationships
```
Commit ──tree──→ Tree ──entries──→ Blob
  │                              └──→ Tree (recursive)
  └──parents──→ Commit (DAG)
  
Branch  ──target──→ Commit
Tag     ──target──→ Commit
HEAD    ──ref──→ Branch ──target──→ Commit
```

### Command → Object Side Effects
```
init      → creates .lit/ structure
add       → modifies Index (stages files → Blob)
commit    → creates Commit + Tree objects, updates Branch
tag       → creates Tag object, writes TagRef
checkout  → updates HEAD, restores working tree from Tree/Blob
merge     → creates merge Commit with 2+ parents
gc        → creates PackFile from loose objects
```

### Command Sequencing Graph
```
init ──→ add ──→ commit ──→ push
  │        │        │         ↑
  │        ↓        ↓         │
  │     status    log ──→ show │
  │        ↑        ↑         │
  │        └─ diff ─┘         │
  │                           │
  └──→ config                 │
  └──→ clone ──→ pull ──→ merge ──→ push
  └──→ branch ──→ checkout ──→ merge
```

---

## 16. Glossary

| Term               | Definition                                                                                                                            |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Agentic-first**  | Designed for AI agents as primary users; humans supported via `--human` flag                                                          |
| **Composite hash** | SHA3-512 + BLAKE3 concatenated (192 hex chars / 768 bits)                                                                             |
| **DAG**            | Directed acyclic graph of commit objects (same structure as Git)                                                                      |
| **FIPS mode**      | Restricts to NIST-approved algorithms only (no BLAKE3, no non-FIPS features)                                                          |
| **Index**          | Staging area mapping file paths to object hashes (`.lit/index`)                                                                       |
| **LITP**           | Lit Pack format — binary container for multiple compressed objects with CRC32                                                         |
| **ML-DSA-87**      | Module-Lattice Digital Signature Algorithm, parameter set 87 (NIST FIPS 204, Security Level 5). Formerly known as CRYSTALS-Dilithium5 |
| **MCP**            | Model Context Protocol — JSON-RPC 2.0 interface for LLM tool integration                                                              |
| **Object hash**    | Content address: 192-hex-char fingerprint used to identify any Lit object                                                             |
| **Pack file**      | Compressed collection of objects created by `lit gc` for storage efficiency                                                           |
| **PQ**             | Post-quantum — cryptographic algorithm resistant to quantum computer attacks                                                          |
| **Swarm**          | Multi-agent coordination system with registration, branch namespacing, and file leases                                                |
| **WAL**            | Write-ahead log used by transactions for crash recovery and rollback                                                                  |
