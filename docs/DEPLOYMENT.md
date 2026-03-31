# lit Deployment Guide

## Overview

This guide covers deploying Lit in a high-security computing environment.

## Prerequisites

### System Requirements
- **Operating System**: Windows, Linux, or macOS
- **Rust Toolchain**: 1.70 or later
- **Disk Space**: ~50 MB for installation
- **Memory**: Minimal (< 10 MB typical usage)
- **Network**: Intranet access (configured)

### Required Software
1. **Rust and Cargo**
   - Install from: https://rustup.rs/
   - Verify: `cargo --version`

2. **Build Tools** (platform-specific)
   - Windows: Visual Studio Build Tools
   - Linux: gcc, make
   - macOS: Xcode Command Line Tools

## Installation Methods

### Method 1: Automated Installation (Recommended)

#### Windows
```powershell
cd lit 
powershell -ExecutionPolicy Bypass -File install.ps1
```

#### Linux/macOS
```bash
cd lit 
chmod +x install.sh
./install.sh
```

### Method 2: Manual Installation

#### Step 1: Build
```bash
cd lit 
cargo build --release
```

#### Step 2: Install
```bash
cargo install --path .
```

#### Step 3: Configure
```bash
cp .litconfig.example ~/.litconfig
# Edit ~/.litconfig with your settings
```

### Method 3: Binary Distribution

For air-gapped environments:

1. Build on a connected machine:
   ```bash
   cargo build --release
   ```

2. Copy binary to target machine:
   - Windows: `target/release/lit.exe`
   - Linux/macOS: `target/release/lit`

3. Place in PATH:
   - Windows: `C:\Program Files\Lit\lit.exe`
   - Linux: `/usr/local/bin/lit`
   - macOS: `/usr/local/bin/lit`

## Configuration

### Global Configuration

Create `~/.litconfig`:

```toml
[network]
# Your organization's intranet IP ranges
allowed_networks = [
    "10.0.0.0/8",        # Adjust to your network
    "172.16.0.0/12",     # Adjust to your network
    "192.168.0.0/16",    # Adjust to your network
]

# Your organization's internal servers
allowed_hosts = [
    "git.internal.company.com",
    "repo.intranet.local",
    "192.168.1.100",
]

[security]
# Enable audit logging
audit_log = true

# Log file location
audit_log_path = "~/.lit/audit.log"
```

### Network Configuration Steps

1. **Identify Your Intranet Ranges**
   ```bash
   # Windows
   ipconfig
   
   # Linux/macOS
   ip addr show
   ifconfig
   ```

2. **Determine CIDR Notation**
   - Single IP: `192.168.1.100/32`
   - Subnet: `192.168.1.0/24`
   - Large range: `10.0.0.0/8`

3. **List Internal Servers**
   - DNS names of internal Git/lit servers
   - IP addresses if DNS not available

4. **Update Configuration**
   ```bash
   # Edit configuration
   notepad ~/.litconfig      # Windows
   nano ~/.litconfig         # Linux
   vim ~/.litconfig          # Advanced users
   ```

### Security Hardening

#### File Permissions (Linux/macOS)
```bash
chmod 600 ~/.litconfig
chmod 700 ~/.lit/
```

#### Audit Log Setup
```bash
# Create audit directory
mkdir -p ~/.lit

# Enable logging in config
# (already in .litconfig)

# Set up log rotation (Linux)
cat > /etc/logrotate.d/lit << EOF
/home/*/.lit/audit.log {
    daily
    rotate 30
    compress
    missingok
    notifempty
}
EOF
```

## Verification

### Installation Check
```bash
# Verify binary
which lit              # Linux/macOS
where.exe lit          # Windows

# Check version
lit --version

# Test basic command
lit --help
```

### Configuration Check
```bash
# View configuration
lit config show

# Test remote validation
lit remote add test lit://192.168.1.100/test.lit
lit remote remove test
```

### Functional Test
```bash
# Create test repository
mkdir test-lit-repo
cd test-lit-repo

# Initialize
lit init

# Create test file
echo "Test content" > test.txt

# Add and commit
lit add test.txt
lit commit -m "Test commit"

# Verify
lit log
lit status

# Cleanup
cd ..
rm -rf test-lit-repo
```

## Deployment Scenarios

### Scenario 1: Single User Workstation

1. Install lit using automated script
2. Configure for local network
3. Create repositories as needed
4. No server required

**Use Case**: Individual developer, local version control

### Scenario 2: Small Team (Shared Network)

1. Install lit on each workstation
2. Configure same intranet ranges
3. Set up shared network location (future)
4. Each user has local repositories

**Use Case**: Team with shared intranet access

### Scenario 3: Secure Facility

1. Build on external system
2. Transfer binary to air-gapped network
3. Install on each workstation
4. Configure facility's intranet ranges
5. Set up internal lit server (future)

**Use Case**: Classified/secure environment

### Scenario 4: Corporate Department

1. Package lit with corporate tools
2. Deploy via software distribution
3. Configure organization's networks
4. Integrate with existing infrastructure

**Use Case**: Enterprise deployment

## Integration

### With Existing Git

lit and Git can coexist:

```bash
# Public work - use Git
cd ~/projects/open-source
git init
git remote add origin https://github.com/user/repo.git

# Private work - use lit 
cd ~/projects/classified
lit init
lit remote add origin lit://git.internal.company.com/project.lit
```

### With IDEs

Add lit to IDE settings:

**VS Code**: Add to PATH
**IntelliJ**: Configure VCS tool path
**Sublime**: Update build system

### With CI/CD

For internal CI/CD systems:

```yaml
# Example: Jenkins
stages:
  - test
  - build

before_script:
  - lit clone lit://ci.internal/project.lit
  - cd project

test:
  script:
    - run-tests.sh

build:
  script:
    - build.sh
    - lit add build-artifacts/
    - lit commit -m "Build $CI_BUILD_ID"
```

## Maintenance

### Updates

```bash
# Pull latest code
cd lit 
git pull

# Rebuild
cargo build --release

# Reinstall
cargo install --path .
```

### Monitoring

#### Audit Log Review
```bash
# View recent activity
tail -n 50 ~/.lit/audit.log

# Search for specific operations
grep "NETWORK ACCESS" ~/.lit/audit.log

# Monitor in real-time
tail -f ~/.lit/audit.log
```

#### Repository Health
```bash
# Check repository
cd my-project
lit status

# View history
lit log --count 20

# List branches
lit branch --all
```

### Backup

#### User Repositories
```bash
# Backup entire project directory
tar -czf project-backup.tar.gz ~/project/.lit

# Or use rsync
rsync -av ~/project/.lit /backup/location/
```

#### Configuration
```bash
# Backup configuration
cp ~/.litconfig ~/backups/.litconfig.$(date +%Y%m%d)
```

## Troubleshooting

### Common Issues

#### "Cargo not found"
**Solution**: Install Rust toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### "Not in a lit repository"
**Solution**: Initialize repository
```bash
lit init
```

#### "Host not in allowed list"
**Solution**: Add host to configuration
```bash
# Edit ~/.litconfig
# Add host to [network] allowed_hosts
```

#### "Protocol not allowed"
**Solution**: Use lit:// protocol
```bash
# Wrong
lit remote add origin http://server/repo.git

# Correct
lit remote add origin lit://server/repo.lit
```

### Debug Mode

Enable verbose output:
```bash
# Set environment variable
export RUST_LOG=debug   # Linux/macOS
$env:RUST_LOG="debug"   # Windows PowerShell

# Run command
lit <command>
```

### Log Analysis

```bash
# Count network attempts
grep -c "NETWORK ACCESS" ~/.lit/audit.log

# Find failed attempts (future)
grep "ERROR" ~/.lit/audit.log

# Activity by date
grep "2025-10-23" ~/.lit/audit.log
```

## Security Checklist

- [ ] Configuration file permissions set correctly
- [ ] Intranet ranges configured accurately
- [ ] Audit logging enabled
- [ ] Log files secured
- [ ] Users trained on proper usage
- [ ] Access controls in place
- [ ] Backup procedures established
- [ ] Monitoring set up
- [ ] Incident response plan defined

## Support

### Documentation
- `README.md` - Overview
- `QUICKSTART.md` - Getting started
- `EXAMPLES.md` - Usage examples
- `ARCHITECTURE.md` - Technical details
- `TESTING.md` - Testing procedures

### Command Help
```bash
lit --help              # General help
lit <command> --help    # Command-specific help
```

### Logs
- Audit log: `~/.lit/audit.log`
- Build log: Check cargo output
- Application log: stderr output

## Best Practices

### For Administrators
1. Centralize configuration management
2. Regular audit log reviews
3. Periodic security assessments
4. User training and documentation
5. Backup and recovery procedures

### For Users
1. Commit frequently with clear messages
2. Use branches for features
3. Review status before committing
4. Keep configuration up to date
5. Report security concerns

### For Organizations
1. Define version control policies
2. Integrate with existing workflows
3. Provide user training
4. Monitor usage patterns
5. Plan for server deployment (future)

## Future Considerations

### Server Deployment (Planned)
- lit server software (future release)
- Repository hosting capabilities
- Push/pull protocol implementation
- User authentication/authorization

### Advanced Features (Implemented)
- Merge functionality with 3-way merge
- Tag support (lightweight and annotated with PQ signing)
- Word-level diff with `--word-diff`
- JSONL batch mode
- Atomic transactions
- REST API server (`lit serve`)
- MCP tool server (`lit mcp-serve`)
- Git import/export
- Large file storage (LFS)

## Appendix

### CIDR Reference
- `/32` - Single IP (255.255.255.255)
- `/24` - 256 addresses (255.255.255.0)
- `/16` - 65,536 addresses (255.255.0.0)
- `/8` - 16,777,216 addresses (255.0.0.0)

### Private IP Ranges
- `10.0.0.0/8` - 10.0.0.0 to 10.255.255.255
- `172.16.0.0/12` - 172.16.0.0 to 172.31.255.255
- `192.168.0.0/16` - 192.168.0.0 to 192.168.255.255

### File Locations
- **Binary**: `~/.cargo/bin/lit`
- **Config**: `~/.litconfig`
- **Audit Log**: `~/.lit/audit.log`
- **Repository**: `.lit/` in project root

---

**Document Version**: 2.0  
**Last Updated**: March 2026  
**For lit Version**: 1.0.0
