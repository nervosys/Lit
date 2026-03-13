# lit - Quantum-Resistant Version Control

## Quick Start Guide

### Installation

1. Build from source:
   ```bash
   cd lit 
   cargo build --release
   ```

2. Install the binary:
   ```bash
   cargo install --path .
   ```

3. Configure your intranet settings:
   ```bash
   cp .litconfig.example ~/.litconfig
   # Edit ~/.litconfig to add your intranet networks and hosts
   ```

### Basic Workflow

#### Initialize a Repository

```bash
# Create a new directory for your project
mkdir my-project
cd my-project

# Initialize lit repository
lit init
```

#### Make Changes and Commit

```bash
# Create or modify files
echo "Hello, Lit!" > README.txt

# Stage files
lit add README.txt
# Or stage all changes
lit add .

# Commit changes
lit commit -m "Initial commit"

# View status
lit status

# View history
lit log
```

#### Working with Branches

```bash
# Create a new branch
lit branch feature-x

# Switch to branch
lit checkout feature-x

# Or create and switch in one command
lit checkout -b feature-y

# List branches
lit branch

# Delete a branch
lit branch -d feature-x
```

#### Remote Operations (Intranet Only)

```bash
# Add an intranet remote
lit remote add origin lit://192.168.1.100/repo.lit

# List remotes
lit remote -v

# Push to remote (when server is available)
lit push origin main

# Pull from remote (when server is available)
lit pull origin main
```

## Security Features

### Encryption Workflow

lit provides military-grade AES-256-GCM encryption for all repository data:

```bash
# 1. Initialize repository
mkdir secure-project
cd secure-project
lit init

# 2. Enable encryption
cat > .lit/encryption.toml << EOF
enabled = true
key_file = "~/.lit/encryption.key"
fips_mode = true
cache_timeout_secs = 300
EOF

# 3. First encrypted commit (will prompt for passphrase)
echo "Classified data" > secret.txt
lit add secret.txt
lit commit -m "First encrypted commit"
# Enter passphrase: ********
# (creates ~/.lit/encryption.key with random salt)

# 4. Subsequent operations (within 5-minute cache window)
echo "More data" > secret2.txt
lit add secret2.txt
lit commit -m "Second commit"
# ✓ Using cached passphrase (no prompt)

# 5. After cache timeout
lit status
# Enter passphrase: ******** (prompted again)

# 6. View encrypted files (all data encrypted on disk)
cat .lit/objects/*/ab* 
# �����binary encrypted data�����

# 7. Rotate passphrase when needed
lit rotate-key
# Enter current passphrase: ********
# Enter new passphrase: ********
# Confirm new passphrase: ********
# ✅ All 156 objects re-encrypted
```

**What Gets Encrypted**:
- ✅ All objects (blobs, trees, commits)
- ✅ Index (staging area)
- ✅ All refs (branches, tags, remotes)
- ✅ HEAD file

**Passphrase Management**:
- **Strong Passphrase**: Minimum 16 characters recommended
- **Caching**: Default 5 minutes, configurable in `encryption.toml`
- **Rotation**: Use `lit rotate-key` every 90 days for compliance
- **Storage**: Passphrase NEVER stored on disk, only salt in key file

**FIPS 140-3 Compliance**:
- AES-256-GCM authenticated encryption (FIPS 197, NIST SP 800-38D)
- PBKDF2-HMAC-SHA512 key derivation (600,000 iterations, NIST SP 800-132)
- DRBG random generation (NIST SP 800-90A Rev. 1)
- See [ENCRYPTION.md](ENCRYPTION.md) for complete compliance documentation

### Network Restrictions

lit enforces strict network access controls:

1. **IP Whitelisting**: Only configured intranet IP ranges are allowed
2. **Protocol Enforcement**: Only the `lit://` protocol is permitted
3. **Audit Logging**: All network access attempts are logged

### Configuration

Edit `~/.litconfig`:

```toml
[network]
allowed_networks = [
    "10.0.0.0/8",
    "192.168.0.0/16"
]

allowed_hosts = [
    "git.internal.company.com"
]

[security]
audit_log = true
audit_log_path = "~/.lit/audit.log"
```

### Viewing Audit Logs

```bash
# View recent network access
tail -f ~/.lit/audit.log
```

## Architecture

### Storage Structure

```
.lit/
├── HEAD                    # Current branch reference
├── config                  # Repository configuration
├── description            # Repository description
├── index                  # Staging area
├── objects/              # Content-addressable object storage
│   ├── abcd/            # First 4 chars of hash
│   │   └── ef01...      # Remaining hash chars
│   └── ...
├── refs/
│   ├── heads/           # Branch references
│   │   └── main
│   ├── tags/            # Tag references
│   └── remotes/         # Remote-tracking branches
└── remotes              # Remote repository configurations
```

### Object Types

1. **Blob**: File content
2. **Tree**: Directory structure
3. **Commit**: Snapshot with metadata
4. **Tag**: Annotated reference with optional PQ signature

All objects are:
- Content-addressed (SHA3-512 + BLAKE3)
- Compressed (zlib)
- Immutable

## Differences from Git

| Feature   | Git                     | lit                |
| --------- | ----------------------- | ------------------ |
| Network   | Internet + Intranet     | Intranet only      |
| Protocols | git://, http://, ssh:// | lit:// only        |
| Security  | Standard                | Enforced whitelist |
| Audit     | Optional                | Built-in           |
| Features  | Full VCS                | Full VCS + agentic |

## Troubleshooting

### "Not in a lit repository"

Make sure you're in a directory initialized with `lit init`.

### "Host not in allowed list"

Add the host to `~/.litconfig` under `[network] allowed_hosts`.

### "IP address not in allowed network range"

Add the appropriate CIDR range to `~/.litconfig` under `[network] allowed_networks`.

### Check Configuration

```bash
lit config show
```

## Advanced Usage

### Viewing Objects

```bash
# Show a commit
lit show <commit-hash>

# Show a commit by branch
lit show main

# View detailed log
lit log --count 20
```

### Repository Information

```bash
# Current status
lit status

# Branch list
lit branch --all

# Remote list
lit remote -v
```

## Best Practices

1. **Regular Commits**: Commit frequently with descriptive messages
2. **Branch Strategy**: Use branches for features and experiments
3. **Security Review**: Regularly review audit logs
4. **Configuration**: Keep network whitelist up to date
5. **Documentation**: Document your branching and workflow conventions

## Advanced Features

### Tags

```bash
# Create a lightweight tag
lit tag v1.0

# Create an annotated tag with message
lit tag v2.0 -a -m "Release v2.0"

# Create a signed tag (post-quantum ML-DSA-87)
lit tag v3.0 -a -s -m "Signed release"

# List tags
lit tag --list

# Delete a tag
lit tag -d v1.0

# Verify a signed tag
lit tag --verify v3.0
```

### Stash

```bash
# Stash current changes
lit stash push -m "WIP: feature work"

# List stash entries
lit stash list

# Apply most recent stash
lit stash pop

# Apply specific stash without removing
lit stash apply --index 0

# Drop a stash entry
lit stash drop --index 0
```

### Snapshots

```bash
# One-command stage-all-and-commit
lit snapshot -m "Quick checkpoint"

# Snapshot with author
lit snapshot -m "Checkpoint" --author "Alice"

# Snapshot with metadata
lit snapshot -m "Build 42" --metadata '{"ticket":"PROJ-123"}'
```

### Search

```bash
# Search file contents
lit search "TODO"

# Search commit messages
lit search --messages "fix bug"

# Limit results
lit search "pattern" --max-results 5
```

### Diff

```bash
# Show changes between working tree and index
lit diff

# Word-level diff for prose/documentation
lit diff --word-diff
```

### Transactions

```bash
# Begin a transaction (WAL-based)
lit tx begin

# Perform operations...
lit add .
lit commit -m "Atomic update"

# Commit the transaction
lit tx commit

# Or rollback on failure
lit tx rollback
```

### Batch Operations

```bash
# Execute batch operations via JSONL stdin
echo '{"command":"status","args":{}}' | lit batch

# Dry run mode
echo '{"command":"status","args":{}}' | lit batch --dry-run

# Atomic mode (stop on first failure)
echo '{"command":"add","args":{"files":["a.txt"]}}' | lit batch --atomic
```

### Output Formats

```bash
# JSON output for scripting
lit status --output json

# Human-readable output (default)
lit log --output human
```

## License

MIT License - See LICENSE file for details
