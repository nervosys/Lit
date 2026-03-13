# lit Testing Guide

## Automated Test Suite

Lit has a comprehensive automated test suite with **387 tests** covering all features:

- **198 CLI command tests** (coverage across 41 command modules)
- **63 unit tests** (core objects, crypto, network, storage)
- **Integration tests** (core functionality, network transport)
- **Performance benchmarks** (baseline metrics)
- **Adversarial/security tests** (airgap validation)

### Quick Start

```powershell
# Run all command tests (REQUIRES --test-threads=1)
cargo test --test command_tests -- --test-threads=1

# Run integration tests
cargo test --test feature_integration_test

# Run performance benchmarks
cargo test --test performance_benchmarks --release -- --nocapture
```

### Documentation

- 📋 **[TESTING_QUICK_REFERENCE.md](TESTING_QUICK_REFERENCE.md)** - Command cheat sheet
- 📊 **[TEST_SUITE_SUMMARY.md](TEST_SUITE_SUMMARY.md)** - Comprehensive test summary
- 📈 **[TEST_COVERAGE_PLAN.md](TEST_COVERAGE_PLAN.md)** - Coverage tracking
- 📁 **[tests/commands/README.md](tests/commands/README.md)** - Command test details

---

## Manual Testing

### Basic Workflow Test

1. **Initialize a repository:**
   ```bash
   mkdir test-repo
   cd test-repo
   lit init
   ```

   Expected output: `Initialized empty lit repository in .lit`

2. **Create and add files:**
   ```bash
   echo "Hello, Lit!" > README.txt
   echo "print('test')" > test.py
   lit add README.txt test.py
   ```

   Expected output: `Added 2 file(s) to staging area`

3. **Check status:**
   ```bash
   lit status
   ```

   Expected output: Shows files in "Changes to be committed"

4. **Commit changes:**
   ```bash
   lit commit -m "Initial commit"
   ```

   Expected output: `[main xxxxxxxx] Initial commit`

5. **View history:**
   ```bash
   lit log
   ```

   Expected output: Shows commit with message "Initial commit"

6. **Create a branch:**
   ```bash
   lit branch feature-test
   lit branch
   ```

   Expected output: Lists both `main` (with *) and `feature-test`

7. **Switch branches:**
   ```bash
   lit checkout feature-test
   lit branch
   ```

   Expected output: Now `feature-test` is marked with *

8. **Make changes on branch:**
   ```bash
   echo "Feature work" > feature.txt
   lit add feature.txt
   lit commit -m "Add feature"
   ```

9. **Switch back to main:**
   ```bash
   lit checkout main
   ls
   ```

   Expected: `feature.txt` should not be present

10. **View log on different branches:**
    ```bash
    lit log
    lit checkout feature-test
    lit log
    ```

### Network Security Test

1. **View configuration:**
   ```bash
   lit config show
   ```

   Expected: Shows network and security configuration

2. **Try to add invalid remote:**
   ```bash
   lit remote add bad http://github.com/repo.git
   ```

   Expected: Error - only `lit://` protocol allowed

3. **Add valid intranet remote:**
   ```bash
   lit remote add origin lit://192.168.1.100/repo.lit
   lit remote -v
   ```

   Expected: Shows the configured remote

4. **Check audit log (if enabled):**
   ```bash
   cat ~/.lit/audit.log
   ```

   Expected: Shows logged network operations

### Object Inspection Test

1. **Show commit:**
   ```bash
   lit log --oneline
   # Copy a commit hash
   lit show <hash>
   ```

   Expected: Shows commit details

## Automated Testing

Run the test suite:

```bash
cargo test
```

### Test Coverage

The current test suite covers:
- Object hash generation (SHA3-512 + BLAKE3)
- Blob, Tree, Commit, and Tag creation
- Index operations
- Object storage and retrieval
- CIDR network matching
- URL parsing
- Transport detection (HTTPS/SSH/lit://)
- Diff engine with word-diff and stat modes
- Encryption and passphrase caching
- Airgap validation and audit logging

## Integration Testing

### Test with Real Repository

```bash
# Create a test project
mkdir my-test-project
cd my-test-project
lit init

# Create multiple files
echo "# Project" > README.md
echo "TODO items" > TODO.txt
mkdir src
echo "fn main() {}" > src/main.rs

# Add and commit
lit add .
lit commit -m "Initial project structure"

# Create branches
lit checkout -b dev
echo "Development work" > dev.txt
lit add dev.txt
lit commit -m "Dev work"

# Switch back and verify isolation
lit checkout main
# dev.txt should not exist here

# View history
lit log --count 20
```

## Performance Testing

### Large Repository Test

```bash
# Create many files
for i in {1..100}; do
    echo "File $i" > "file_$i.txt"
done

# Add all
lit add .

# Commit
lit commit -m "100 files"

# Check status
lit status
```

## Error Handling Tests

1. **Not in a repository:**
   ```bash
   cd /tmp
   lit status
   ```
   Expected: Error - "Not in a lit repository"

2. **Empty commit:**
   ```bash
   lit commit -m "Empty"
   ```
   Expected: Error - "Nothing to commit"

3. **Delete current branch:**
   ```bash
   lit branch -d main
   ```
   Expected: Error - "Cannot delete the currently checked out branch"

## Cleanup

```bash
# Remove test repositories
rm -rf test-repo my-test-project
```

## Known Limitations

The following features are not yet implemented:
- [ ] Network operations (push, pull, clone) - requires server
- [ ] Merge functionality
- [ ] Interactive staging
- [ ] Diff viewing
- [ ] Deep directory nesting in trees
- [ ] Binary file detection
- [ ] Large file handling
- [ ] Submodules
- [ ] Tags
- [ ] Stash

## Reporting Issues

When reporting issues, include:
1. lit version: `lit --version`
2. Steps to reproduce
3. Expected behavior
4. Actual behavior
5. Operating system
6. Error messages
