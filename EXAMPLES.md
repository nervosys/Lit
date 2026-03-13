# lit Usage Examples

## Example 1: Simple Project

```bash
# Initialize a new project
$ mkdir my-secure-project
$ cd my-secure-project
$ lit init
Initialized empty lit repository in .lit

# Create some files
$ echo "# Secure Project" > README.md
$ echo "SECRET_KEY=xyz" > .env
$ echo "def calculate(): pass" > utils.py

# Stage files
$ lit add README.md utils.py
Added 2 file(s) to staging area

# Check status
$ lit status
On branch main

Changes to be committed:
  (use "lit checkout -- <file>..." to unstage)

        modified:   README.md
        modified:   utils.py

Untracked files:
  (use "lit add <file>..." to include in what will be committed)

        .env

# Commit
$ lit commit -m "Initial project setup"
[main a1b2c3d4] Initial project setup
2 file(s) changed

# View log
$ lit log
commit a1b2c3d4e5f6...
  (HEAD -> main)
Author: user
Date:   Wed Oct 23 14:30:00 2025 +0000

    Initial project setup
```

## Example 2: Feature Branch Workflow

```bash
# Start on main branch
$ lit branch
* main

# Create feature branch
$ lit checkout -b feature/authentication
Switched to a new branch 'feature/authentication'

# Work on feature
$ echo "class AuthSystem: pass" > auth.py
$ lit add auth.py
$ lit commit -m "Add authentication system"
[feature/authentication x9y8z7w6] Add authentication system
1 file(s) changed

# Add more commits
$ echo "def login(): pass" >> auth.py
$ lit add auth.py
$ lit commit -m "Implement login function"
[feature/authentication q5r6s7t8] Implement login function
1 file(s) changed

# View branch history
$ lit log --count 5
commit q5r6s7t8u9v0...
  (HEAD -> feature/authentication)
Author: user
Date:   Wed Oct 23 14:45:00 2025 +0000

    Implement login function

commit x9y8z7w6v5u4...
Author: user
Date:   Wed Oct 23 14:40:00 2025 +0000

    Add authentication system

commit a1b2c3d4e5f6...
Author: user
Date:   Wed Oct 23 14:30:00 2025 +0000

    Initial project setup

# Switch back to main
$ lit checkout main
Switched to branch 'main'

# Verify auth.py doesn't exist on main
$ ls auth.py
ls: cannot access 'auth.py': No such file or directory

# List all branches
$ lit branch --all
* main
  feature/authentication
```

## Example 3: Multiple Developers (Local Branches)

```bash
# Developer 1: UI work
$ lit checkout -b feature/ui
Switched to a new branch 'feature/ui'
$ echo "UI components" > ui.py
$ lit add ui.py
$ lit commit -m "Add UI components"

# Developer 2: Database work (on different machine)
$ lit checkout -b feature/database
Switched to a new branch 'feature/database'
$ echo "Database models" > models.py
$ lit add models.py
$ lit commit -m "Add database models"

# View all branches
$ lit branch --all
  main
  feature/ui
* feature/database
  feature/authentication
```

## Example 4: Examining Repository History

```bash
# Compact log
$ lit log --oneline
q5r6s7t8 Implement login function
x9y8z7w6 Add authentication system
a1b2c3d4 Initial project setup

# Show specific commit
$ lit show a1b2c3d4
commit a1b2c3d4e5f6a7b8c9d0...
Author: user
Date:   Wed Oct 23 14:30:00 2025 +0000

    Initial project setup

# Show current branch
$ lit log --count 1
commit q5r6s7t8u9v0...
  (HEAD -> feature/authentication)
Author: user
Date:   Wed Oct 23 14:45:00 2025 +0000

    Implement login function
```

## Example 5: Remote Configuration (Intranet)

```bash
# Configure intranet remote
$ lit remote add origin lit://192.168.1.100/secure-repo.lit
Added remote 'origin'

# List remotes
$ lit remote
origin

# Verbose remote list
$ lit remote -v
origin  lit://192.168.1.100/secure-repo.lit

# Try to add invalid remote (Internet)
$ lit remote add github http://github.com/user/repo.git
Error: Invalid protocol 'http'. Only 'lit://' protocol is allowed for intranet operations

# Add another intranet remote
$ lit remote add backup lit://10.0.1.50/backup-repo.lit
Added remote 'backup'

# Remove a remote
$ lit remote remove backup
Removed remote 'backup'
```

## Example 6: Configuration Management

```bash
# View current configuration
$ lit config show
lit Configuration
=================

[network]
Allowed Networks:
  - 10.0.0.0/8
  - 172.16.0.0/12
  - 192.168.0.0/16

Allowed Hosts:
  - git.internal.company.com

[security]
Audit Log: enabled
Audit Log Path: ~/.lit/audit.log

# Get specific config value
$ lit config get network.allowed_networks
10.0.0.0/8
172.16.0.0/12
192.168.0.0/16
```

## Example 7: Security Audit

```bash
# View audit log
$ cat ~/.lit/audit.log
2025-10-23T14:50:00Z | NETWORK ACCESS | lit://192.168.1.100/secure-repo.lit
2025-10-23T14:51:30Z | NETWORK ACCESS | lit://10.0.1.50/backup-repo.lit

# Monitor network access in real-time
$ tail -f ~/.lit/audit.log
```

## Example 8: Working with Detached HEAD

```bash
# Checkout specific commit
$ lit log --oneline
q5r6s7t8 Implement login function
x9y8z7w6 Add authentication system
a1b2c3d4 Initial project setup

$ lit checkout a1b2c3d4
HEAD is now at a1b2c3d4 (detached)

# View status in detached state
$ lit status
HEAD detached

# Create branch from detached HEAD
$ lit checkout -b hotfix/issue-123
Switched to a new branch 'hotfix/issue-123'
```

## Example 9: Staging Workflow

```bash
# Create multiple files
$ echo "config1" > config.json
$ echo "config2" > settings.json
$ echo "temp" > temp.txt

# Stage specific files
$ lit add config.json settings.json
Added 2 file(s) to staging area

# Check what's staged
$ lit status
On branch main

Changes to be committed:
  (use "lit checkout -- <file>..." to unstage)

        modified:   config.json
        modified:   settings.json

Untracked files:
  (use "lit add <file>..." to include in what will be committed)

        temp.txt

# Commit staged files
$ lit commit -m "Update configuration files"
[main m8n9o0p1] Update configuration files
2 file(s) changed

# temp.txt remains untracked
$ lit status
On branch main

Untracked files:
  (use "lit add <file>..." to include in what will be committed)

        temp.txt

nothing to commit, working tree clean (except untracked files)
```

## Example 10: Branch Management

```bash
# Create multiple branches
$ lit branch feature-1
Created branch 'feature-1'

$ lit branch feature-2
Created branch 'feature-2'

$ lit branch experimental
Created branch 'experimental'

# List all branches
$ lit branch
  experimental
  feature-1
  feature-2
* main

# Delete a branch
$ lit branch -d experimental
Deleted branch 'experimental'

# Try to delete current branch (error)
$ lit branch -d main
Error: Cannot delete the currently checked out branch

# Switch and delete
$ lit checkout feature-1
Switched to branch 'feature-1'

$ lit branch -d main
Deleted branch 'main'  # This works now
```

## Example 11: Complete Project Lifecycle

```bash
# 1. Initialize
$ mkdir classified-project
$ cd classified-project
$ lit init

# 2. Setup configuration
$ cp ~/.litconfig.example ~/.litconfig
# Edit to add your intranet servers

# 3. Create initial structure
$ mkdir -p src tests docs
$ echo "# Classified Project" > README.md
$ echo "Main code" > src/main.py
$ echo "Tests" > tests/test_main.py
$ echo "Documentation" > docs/guide.md

# 4. Initial commit
$ lit add .
$ lit commit -m "Initial project structure"

# 5. Development branch
$ lit checkout -b develop
$ echo "Development work" >> src/main.py
$ lit add src/main.py
$ lit commit -m "Add development features"

# 6. Feature branch from develop
$ lit checkout -b feature/new-capability
$ echo "New capability" > src/feature.py
$ lit add src/feature.py
$ lit commit -m "Implement new capability"

# 7. View project history
$ lit log --count 10

# 8. Configure remote
$ lit remote add origin lit://git.internal.company.com/classified.lit

# 9. View repository state
$ lit branch --all
$ lit remote -v
$ lit status
```

## Tips and Tricks

### Quick Status Check
```bash
alias gs='lit status'
alias gl='lit log --oneline'
alias gb='lit branch'
```

### Branch Navigation
```bash
# Quickly switch between branches
$ lit checkout -b temp-work
# ... do work ...
$ lit checkout main
```

### Finding Commits
```bash
# Show recent commits
$ lit log --count 5 --oneline

# Show specific commit details
$ lit show <hash>
```

### Configuration
```bash
# Always check your config before adding remotes
$ lit config show

# Verify allowed networks
$ lit config get network.allowed_networks
```

## Common Workflows

### Solo Developer
1. Initialize repository
2. Make changes
3. Stage and commit
4. Create branches for experiments
5. Switch between branches as needed

### Small Team (Same Network)
1. Each developer has local repository
2. Use branches for features
3. Configure shared intranet remote
4. (When server available) Push and pull changes
5. Review audit logs regularly

### High-Security Environment
1. Strict network configuration
2. Regular audit log reviews
3. Branch-per-feature workflow
4. Code review before merging
5. Separate public (git) and private (lit) repos

## Example 12: Working with Tags

```bash
# Create a lightweight tag at HEAD
$ lit tag v1.0
Created lightweight tag 'v1.0'

# Create an annotated tag
$ lit tag v2.0 -a -m "Production release v2.0"
Created annotated tag 'v2.0'

# Create a signed tag (post-quantum ML-DSA-87)
$ lit tag v3.0 -a -s -m "Signed release"
Created signed tag 'v3.0' (PQ: ML-DSA-87)

# List all tags
$ lit tag --list
v1.0
v2.0
v3.0

# Verify a signed tag
$ lit tag --verify v3.0
Good signature on tag 'v3.0' (PQ)

# Delete a tag
$ lit tag -d v1.0
Deleted tag 'v1.0'
```

## Example 13: Stash Workflow

```bash
# You're working on a feature but need to switch branches
$ echo "new feature code" >> feature.py
$ lit status
Modified files:
  feature.py

# Stash your changes
$ lit stash push -m "WIP: new feature"
Saved working directory to stash@{0}: WIP: new feature

# Working tree is now clean
$ lit status
On branch feature/auth
nothing to commit, working tree clean

# Switch branches, do other work...
$ lit checkout main

# Come back and restore your work
$ lit checkout feature/auth
$ lit stash pop
Restored stash@{0}: WIP: new feature

# View stash list
$ lit stash list
stash@{0}: WIP: feature work
stash@{1}: WIP: bugfix

# Apply without removing from stash
$ lit stash apply --index 1

# Drop a specific entry
$ lit stash drop --index 0
```

## Example 14: Snapshots (Quick Commits)

```bash
# Snapshot stages all files and commits in one step
$ lit snapshot -m "Quick checkpoint before refactor"
[a1b2c3d4] Snapshot: Quick checkpoint before refactor
  15 file(s) captured
  Author: user

# Snapshot with explicit author
$ lit snapshot -m "Pair programming session" --author "Alice & Bob"
[e5f6a7b8] Snapshot: Pair programming session
  15 file(s) captured
  Author: Alice & Bob

# Snapshot with metadata (for agentic workflows)
$ lit snapshot -m "Build 42" --metadata '{"ticket":"PROJ-123","ci":"passed"}'
[c9d0e1f2] Snapshot: Build 42
  15 file(s) captured
  Author: user
```

## Example 15: Search

```bash
# Search file contents for a pattern
$ lit search "TODO"
Search 'TODO': 3 result(s)
  src/main.py:42: # TODO: implement error handling
  src/utils.py:15: # TODO: add validation
  docs/guide.md:8: - TODO: write deployment section

# Search commit messages
$ lit search --messages "fix"
Search 'fix': 2 result(s)
  commit a1b2c3d4: Fix critical bug in parser
  commit e5f6a7b8: Fix another bug

# Limit results
$ lit search "import" --max-results 3
Search 'import': 3 result(s)
  src/main.py:1: import os
  src/main.py:2: import sys
  src/utils.py:1: import json
```

## Example 16: Diff and Word Diff

```bash
# View changes in working tree
$ lit diff
diff --lit a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1,3 +1,4 @@
 # My Project
+## Overview
 This is a project.

# Word-level diff (great for prose)
$ lit diff --word-diff
diff --lit a/README.md b/README.md
--- a/README.md
+++ b/README.md
# My {+Amazing+} Project
```

## Example 17: Transactions

```bash
# Begin an atomic transaction
$ lit tx begin
Transaction begin: Transaction started [a1b2c3d4]

# Perform multiple operations
$ echo "update 1" > file1.txt
$ lit add file1.txt
$ lit commit -m "Update file 1"
$ echo "update 2" > file2.txt
$ lit add file2.txt
$ lit commit -m "Update file 2"

# Commit the transaction (all-or-nothing)
$ lit tx commit
Transaction commit: Transaction committed [a1b2c3d4]

# Or rollback if something went wrong
$ lit tx rollback
Transaction rollback: Transaction rolled back [a1b2c3d4]
```

## Example 18: JSON Output for Scripting

```bash
# Get status as JSON (for CI/CD pipelines)
$ lit status --output json
{  "branch": "main",
  "staged": ["file1.txt"],
  "modified": [],
  "untracked": ["temp.txt"]
}

# Parse with jq
$ lit log --output json | jq '.hash'
```
