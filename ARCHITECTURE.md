# lit Architecture Documentation

## Overview

Lit is an agentic-first distributed version control system written in Rust, designed for AI agents first and humans second, with post-quantum cryptographic security for high-security environments. It implements core Git principles while enforcing strict network restrictions.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                         lit CLI                             │
│                      (main.rs)                              │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   Commands   │    │     Core     │    │   Storage    │
│              │    │              │    │              │
│ • init       │    │ • objects    │    │ • index      │
│ • add        │    │ • refs       │    │ • objects    │
│ • commit     │    │ • diff       │    │              │
│ • status     │    │              │    │              │
│ • log        │    │              │    │              │
│ • branch     │    │              │    │              │
│ • checkout   │    │              │    │              │
│ • remote     │    │              │    │              │
│ • merge      │    │              │    │              │
│ • diff       │    │              │    │              │
│ • tag, stash │    │              │    │              │
│ • batch, tx  │    │              │    │              │
│ • serve, mcp │    │              │    │              │
│ • +28 more   │    │              │    │              │
└──────────────┘    └──────────────┘    └──────────────┘
                            │
                            ▼
                    ┌──────────────┐
                    │   Network    │
                    │              │
                    │ • transport  │
                    │ • validator  │
                    │ • airgap     │
                    │ • audit      │
                    │ • https/ssh  │
                    │ • lit_proto  │
                    └──────────────┘
```

## Module Structure

### 1. Core Module (`src/core/`)

#### Objects (`objects.rs`)
Implements the fundamental data structures:

- **ObjectHash**: SHA3-512 + BLAKE3 composite hash
  - Content-addressable identifier
  - 128-character hexadecimal string (64 bytes)
  - Short form (8 chars) for display
  - Quantum-resistant (NIST FIPS 202)

- **Object**: Enum of all object types
  - `Blob`: File content
  - `Tree`: Directory structure
  - `Commit`: Snapshot with metadata
  - `Tag`: Annotated tag with optional PQ signature

- **Blob**: Stores raw file content
  ```rust
  struct Blob {
      content: Vec<u8>
  }
  ```

- **Tree**: Directory representation
  ```rust
  struct Tree {
      entries: Vec<TreeEntry>
  }
  
  struct TreeEntry {
      mode: String,      // File permissions (e.g., "100644")
      name: String,      // Filename or directory name
      hash: ObjectHash,  // Points to blob or subtree
      object_type: String // "blob" or "tree"
  }
  ```

- **Commit**: Snapshot in history
  ```rust
  struct Commit {
      tree: ObjectHash,           // Root tree
      parents: Vec<ObjectHash>,   // Parent commits
      author: String,             // Author name
      committer: String,          // Committer name
      timestamp: i64,             // Unix timestamp
      message: String             // Commit message
  }
  ```

#### References (`refs.rs`)
Manages references to commits:

- **Reference Types**:
  - Branches: `refs/heads/*`
  - Tags: `refs/tags/*`
  - Remote tracking: `refs/remotes/*`
  - HEAD: Special reference to current position

- **Functions**:
  - `read_ref()`: Read a reference value
  - `write_ref()`: Update a reference
  - `delete_ref()`: Remove a reference
  - `list_refs()`: List all references in a prefix
  - `read_head()`: Get current HEAD commit
  - `get_current_branch()`: Get branch name
  - `update_head()`: Switch branches
  - `set_head_detached()`: Enter detached HEAD state

### 2. Storage Module (`src/storage/`)

#### Object Store (`objects.rs`)
Handles persistent object storage:

- **Directory Structure**:
  ```
  .lit/objects/
  ├── ab/
  │   └── cdef123456...  (compressed object)
  └── ...
  ```

- **Features**:
  - Content-addressable storage
  - Zlib compression
  - Immutable objects
  - Hash-based deduplication

- **Operations**:
  - `write()`: Store an object
  - `read()`: Retrieve an object
  - `exists()`: Check object presence
  - `list()`: List all objects

#### Index (`index.rs`)
Manages the staging area:

- **Structure**:
  ```rust
  struct Index {
      entries: HashMap<String, IndexEntry>
  }
  
  struct IndexEntry {
      path: String,    // Relative path
      hash: String,    // Object hash
      mode: String     // File mode
  }
  ```

- **Operations**:
  - `load()`: Load from disk
  - `save()`: Persist to disk
  - `add()`: Stage a file
  - `remove()`: Unstage a file
  - `sorted_entries()`: Get entries by path order

### 3. Network Module (`src/network/`)

#### Validator (`validator.rs`)
Enforces intranet-only access:

- **NetworkConfig**:
  ```rust
  struct NetworkConfig {
      allowed_networks: Vec<String>,  // CIDR ranges
      allowed_hosts: Vec<String>,     // Hostname whitelist
      audit_log: bool,                // Enable logging
      audit_log_path: Option<String>  // Log file path
  }
  ```

- **NetworkValidator**:
  - URL validation
  - Protocol enforcement (lit:// only)
  - IP address checking
  - CIDR range matching
  - Audit logging

- **Security Features**:
  - Default private network ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
  - Protocol whitelist
  - Host/IP whitelist
  - All access attempts logged

### 4. Commands Module (`src/commands/`)

Each command is a separate module:

#### Local Operations
- **init**: Initialize repository
  - Create `.lit` directory structure
  - Set up initial HEAD reference
  - Create empty index

- **add**: Stage files
  - Read file content
  - Create blob objects
  - Update index

- **commit**: Create commits
  - Build tree from index
  - Create commit object
  - Update branch reference

- **status**: Show working tree state
  - Compare working directory to index
  - Show staged changes
  - Show untracked files

- **log**: Display commit history
  - Walk parent chain
  - Format output
  - Support --oneline and --count

- **branch**: Branch management
  - List branches
  - Create new branches
  - Delete branches

- **checkout**: Switch branches
  - Update HEAD
  - Update working directory
  - Support detached HEAD

- **show**: Display objects
  - Show commit details
  - Display tree contents
  - Show blob content

#### Network Operations
- **remote**: Remote management (add/remove/list, URL validation)
- **push**: Send objects and refs to remote repository
- **pull**: Fetch and merge from remote
- **clone**: Full repository clone with working tree checkout
- **fetch**: Download objects and refs without merging

#### Advanced Operations
- **merge**: 3-way merge with recursive/ours/theirs strategies
- **diff**: Structured diff with `--word-diff` and `--stat` modes
- **tag**: Lightweight and annotated tags with optional PQ signing
- **stash**: Save and restore working tree state
- **search**: Full-text search across files and commit messages
- **blame**: Line-level attribution
- **bisect**: Binary search for regressions

#### Agentic Operations
- **batch**: JSONL batch command execution with atomic mode
- **transaction**: WAL-based atomic multi-operation transactions
- **snapshot**: Atomic stage-all and commit
- **serve**: REST API server for programmatic access
- **mcp_serve**: Model Context Protocol tool server
- **watch**: Filesystem monitoring with auto-commit

#### Interop & Maintenance
- **import-git**: Convert Git repositories to Lit
- **export-git**: Convert Lit repositories to Git
- **gc**: Garbage collection and pack file optimization
- **lfs**: Large file storage with pointer files
- **rotate-key**: Encryption key rotation

## Data Flow

### Adding and Committing Files

```
Working Directory
      │
      │ lit add
      ▼
   Index (staging area)
      │
      │ lit commit
      ▼
  Blob Objects ──┐
                 │
                 ▼
             Tree Object
                 │
                 ▼
            Commit Object
                 │
                 ▼
          Branch Reference
```

### Checkout Flow

```
  Branch Reference
      │
      ▼
  Commit Object
      │
      ▼
  Tree Object
      │
      ├──> Blob Objects
      │         │
      │         ▼
      │    Working Directory
      │
      └──> Subtree Objects
                │
                ▼
           (recursive)
```

## File System Layout

### Repository Structure

```
my-project/
├── .lit/
│   ├── HEAD                    # Current branch or commit
│   ├── config                  # Repository configuration
│   ├── description             # Repository description
│   ├── index                   # Staging area (JSON)
│   ├── remotes                 # Remote configurations (JSON)
│   ├── objects/
│   │   ├── ab/
│   │   │   └── cdef...         # Compressed object
│   │   └── ...
│   └── refs/
│       ├── heads/
│       │   ├── main            # Main branch
│       │   └── feature-x       # Feature branch
│       ├── tags/               # Tags (future)
│       └── remotes/            # Remote tracking (future)
└── (working directory files)
```

### Global Configuration

```
~/.litconfig                     # Global configuration (TOML)
~/.lit/
    └── audit.log               # Network access log
```

## Object Storage Format

### Object File Format

1. **Serialization**: Objects are serialized to JSON
2. **Compression**: Compressed with zlib
3. **Storage**: Written to `.lit/objects/XX/YYYYYY...`
- XX = first 4 characters of hash
   - YYYYYY... = remaining characters

### Example Object

```json
{
  "Commit": {
    "tree": "abc123...",
    "parents": ["def456..."],
    "author": "user",
    "committer": "user",
    "timestamp": 1234567890,
    "message": "Initial commit"
  }
}
```

## Security Model

### Network Access Control

1. **Protocol Validation**
   - Only `lit://` protocol allowed
   - Reject `http://`, `https://`, `git://`, etc.

2. **IP Validation**
   - Parse IP from URL
   - Check against CIDR whitelist
   - Reject if not in allowed ranges

3. **Hostname Validation**
   - Check against allowed hosts list
   - Reject if not whitelisted

4. **Audit Logging**
   - Log all network access attempts
   - Include timestamp and URL
   - Store in `~/.lit/audit.log`

### Access Control Flow

```
Network Request
      │
      ▼
 Parse URL
      │
      ▼
 Check Protocol ─── reject ──> Error
      │ (lit:// only)
      ▼
 Extract Host
      │
      ▼
 Check Whitelist ── reject ──> Error
      │ (IP/hostname)
      ▼
   Log Access
      │
      ▼
   Allow Request
```

## Design Decisions

### Quantum-Resistant Cryptography

#### Why SHA3-512 + BLAKE3?
- **SHA3-512**: NIST FIPS 202 approved, quantum-resistant hash function
- **BLAKE3**: Modern, high-performance cryptographic hash with quantum resistance
- **Composite Approach**: Defense-in-depth - if one algorithm is compromised, the other provides security
- **Future-Proof**: Resistant to both classical and quantum computer attacks
- **Hash Length**: 192 hex chars (96 bytes) provides 2^256 collision resistance even against quantum computers

#### Why ML-DSA (Dilithium5)?
- **NIST Standard**: FIPS 204 - officially approved post-quantum signature scheme
- **Security Level 5**: Highest NIST security level, exceeds requirements
- **Lattice-Based**: Resistant to Shor's algorithm (which breaks RSA/ECC)
- **Well-Studied**: Extensive cryptanalysis by global research community
- **Performance**: Acceptable signature generation and verification times

#### Post-Quantum vs Classical
| Feature       | Classical (Git)    | Lit (Post-Quantum)  |
| ------------- | ------------------ | ------------------- |
| Hash Function | SHA-1 (broken)     | SHA3-512 + BLAKE3   |
| Hash Length   | 40 hex chars       | 192 hex chars       |
| Signatures    | GPG (RSA/ECC)      | ML-DSA (Dilithium5) |
| Quantum Safe  | ❌ No               | ✅ Yes               |
| NIST Approved | ❌ SHA-1 deprecated | ✅ FIPS 202, 204     |

### Why SHA-256 Removed?
- Git uses SHA-1 (deprecated due to collisions)
- SHA-256 provides better security than SHA-1 but not quantum-resistant
- SHA-3 (Keccak) uses different construction than SHA-2, provides quantum resistance
- NIST recommends SHA-3 for new systems requiring long-term security

### Why JSON for Objects?
- Human-readable for debugging
- Easier to implement than Git's custom format
- Acceptable performance for simplified version
- Future: Could optimize with binary format

### Why zlib Compression?
- Same as Git
- Good compression ratio
- Fast decompression
- Standard library support

### Why Custom Protocol (lit://)?
- Clear distinction from Git protocols
- Easy to validate and restrict
- Signals intranet-only intent
- Prevents accidental Internet access

### Simplified Features
Some Git features are simplified or use different approaches:
- Merge uses structured JSON conflict representation (not markers)
- Pack files use a simpler format than Git
- No Git hooks (use `lit watch` or `lit batch` instead)
- No worktrees or partial clones

## Extension Points

### Future Enhancements

1. **Server Implementation**
   - lit protocol server
   - Authentication/authorization
   - Repository hosting
   - Push/pull operations

2. **Merge Support**
   - Three-way merge
   - Conflict detection
   - Interactive resolution

3. **Performance**
   - Pack files for efficient storage
   - Delta compression
   - Index caching

4. **Features**
   - Tags
   - Stash
   - Interactive rebase
   - Diff viewer
   - Blame/annotate

## Testing Strategy

### Unit Tests
- Object creation and hashing
- Index operations
- Reference management
- Network validation
- CIDR matching

### Integration Tests
- Full workflow tests
- Branch operations
- Multi-commit scenarios

### Security Tests
- Network access validation
- Protocol enforcement
- Audit log verification

## Dependencies

### Core Dependencies
- `clap`: CLI argument parsing with derive macros
- `sha3`: SHA3-512 hashing (NIST FIPS 202)
- `blake3`: High-performance cryptographic hash
- `flate2`: zlib compression
- `serde`/`serde_json`: Serialization
- `chrono`: Timestamp handling
- `aes-gcm`: AES-256-GCM encryption
- `pqcrypto-dilithium`: ML-DSA post-quantum signatures
- `rmp-serde`: MsgPack serialization
- `rayon`: Parallel I/O operations

### Utility Dependencies
- `walkdir`: Directory traversal
- `regex`: URL parsing
- `dirs`: Home directory detection
- `shellexpand`: Path expansion

## Performance Characteristics

### Time Complexity
- Object lookup: O(1) - hash-based
- Index operations: O(n) - number of files
- Log traversal: O(k) - commits to display
- Tree checkout: O(n) - files in tree

### Space Complexity
- Objects: Compressed, deduplicated
- Index: O(n) - tracked files
- References: O(1) - single file per ref

### Scalability Limits
- Small to medium repositories (< 10,000 files)
- Not optimized for large monorepos
- Linear history traversal
- No pack file optimization

## Comparison with Git

| Feature           | Git                | lit                                   |
| ----------------- | ------------------ | ------------------------------------- |
| Hash Algorithm    | SHA-1 (broken)     | SHA3-512 + BLAKE3 (quantum-resistant) |
| Hash Length       | 40 hex chars       | 192 hex chars                         |
| Object Storage    | Pack files + loose | Loose objects only                    |
| Signatures        | GPG (RSA/ECC)      | ML-DSA (Dilithium5) optional          |
| Quantum Resistant | ❌ No               | ✅ Yes (NIST standards)                |
| Network Protocols | Many               | lit:// only                           |
| Network Scope     | Internet           | Intranet only                         |
| Merge             | Advanced           | Not implemented                       |
| Complexity        | High               | Simplified                            |
| Security Focus    | General            | High-security + quantum-safe          |
| Audit Logging     | Optional           | Built-in                              |
| NIST Compliance   | ❌ SHA-1 deprecated | ✅ FIPS 202, 204                       |

## License

MIT License - See LICENSE file
