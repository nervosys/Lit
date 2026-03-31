# Airgap Mode - Isolated Network Operations

## Overview

Lit's **Airgap Mode** is a security feature designed for environments that require complete network isolation. It blocks all network protocols (HTTP, HTTPS, SSH, FTP, and even the custom `lit://` protocol) and restricts operations to **physical transports only**:

- **USB drives and removable media**
- **Network file shares (SMB/CIFS)** - when not in strict mode
- **Direct filesystem paths**
- **`file://` protocol**

This is ideal for:
- **Air-gapped networks** (physically isolated from the internet)
- **Classified environments** (military, government, intelligence)
- **High-security facilities** (nuclear plants, critical infrastructure)
- **Compliance requirements** (ITAR, EAR, CUI handling)

---

## Quick Start

### Enable Airgap Mode (Command Line)

```bash
# Enable for a single command
lit --airgapped clone file:///mnt/usb/repo.lit

# Enable for all operations
lit config set airgap.enabled true

# Enable strict mode (USB/removable media only, blocks network shares)
lit config set airgap.strict_mode true
```

### Enable Airgap Mode (Configuration File)

Create or edit `~/.lit/airgap.toml`:

```toml
enabled = true
strict_mode = false  # Set to true to block network shares
audit_log = true
audit_log_path = "~/.lit/airgap_audit.log"

# Allowed transport types
allowed_transports = [
    "LocalFilesystem",
    "RemovableMedia",
    "NetworkShare",
    "FileProtocol"
]

# Optional: Restrict to specific removable media paths
# allowed_media = [
#     "/mnt/approved_usb",
#     "E:\\",  # Windows drive letter
# ]

# Optional: Restrict to specific network shares
# allowed_shares = [
#     "\\\\approved-server\\lit-repos",
#     "//10.0.0.5/shared/repos"
# ]
```

---

## Transport Types

### ✅ Allowed in Airgap Mode

#### 1. **Local Filesystem**
Direct paths on the local machine (always allowed).

```bash
lit clone /path/to/repo
lit remote add origin /opt/repos/project.lit
```

#### 2. **Removable Media (USB, External Drives)**
Automatically detected on:
- **Windows**: Removable drive types (E:, F:, etc.)
- **Linux/macOS**: Common mount points (`/media/`, `/mnt/`, `/Volumes/`)

```bash
# Linux/macOS
lit clone /media/usb/repo.lit
lit clone /mnt/external/project.lit

# macOS
lit clone /Volumes/USB_DRIVE/repo.lit

# Windows
lit clone E:\repos\project.lit
lit clone F:\backup\repo.lit
```

#### 3. **Network File Shares**
SMB/CIFS network shares (blocked in strict mode).

```bash
# Windows UNC paths
lit clone \\server\share\repo.lit
lit remote add backup \\10.0.0.5\lit-repos\project.lit

# Unix-style
lit clone //server/share/repo.lit
```

#### 4. **File Protocol**
Standard `file://` URLs.

```bash
lit clone file:///path/to/repo
lit remote add origin file:///opt/repos/project.lit

# Windows
lit clone file:///E:/repos/project.lit
```

---

### 🚫 Blocked in Airgap Mode

All network protocols are automatically blocked:

| Protocol            | Example                        | Blocked |
| ------------------- | ------------------------------ | ------- |
| HTTP/HTTPS          | `https://github.com/user/repo` | ✅ Yes   |
| SSH/SCP             | `ssh://server/repo`            | ✅ Yes   |
| Custom Lit Protocol | `lit://192.168.1.100/repo`     | ✅ Yes   |
| FTP/FTPS            | `ftp://server/repo`            | ✅ Yes   |
| Any other network   | `git://`, `rsync://`, etc.     | ✅ Yes   |

When you attempt a blocked operation:

```bash
$ lit --airgapped clone https://github.com/user/repo
Error: 🚫 AIRGAP MODE: Transport type Http is blocked.
Only physical transports allowed (USB, network shares, local filesystem).
Use --airgapped=false to disable airgap mode.
```

---

## Operating Modes

### Standard Airgap Mode

Allows USB drives, local filesystem, network shares, and `file://` protocol.

```bash
lit config set airgap.enabled true
lit config set airgap.strict_mode false
```

**Use case**: Organizations with isolated networks but internal file servers.

### Strict Airgap Mode

Allows **only** USB drives and local filesystem. Blocks network shares.

```bash
lit config set airgap.enabled true
lit config set airgap.strict_mode true
```

**Use case**: Completely air-gapped facilities with no network connectivity.

---

## Security Features

### 1. Transport Validation

Every `clone`, `push`, `pull`, and `remote add` operation validates the transport:

```bash
$ lit --airgapped clone file:///mnt/usb/repo.lit
🔒 AIRGAP MODE ENABLED - Network protocols blocked
🔍 Transport: FileProtocol
📍 Path: /mnt/usb/repo.lit
```

### 2. Audit Logging

All transport access attempts are logged (enabled by default).

**Log location**: `~/.lit/airgap_audit.log`

```
2025-10-24T10:30:45Z | AIRGAP TRANSPORT | FileProtocol | /mnt/usb/repo.lit
2025-10-24T10:31:12Z | AIRGAP TRANSPORT | RemovableMedia | E:\repos\project.lit
2025-10-24T10:32:05Z | AIRGAP TRANSPORT | NetworkShare | \\server\share\repo.lit
```

Disable logging (not recommended for security):

```bash
# Edit ~/.lit/airgap.toml
audit_log = false
```

### 3. Whitelist Configuration

Restrict access to specific devices or shares:

```toml
# Only allow specific USB drives
allowed_media = [
    "/media/approved_usb_001",
    "E:\\"  # Specific Windows drive
]

# Only allow specific network shares
allowed_shares = [
    "\\\\secure-server\\lit-repos",
    "//10.0.1.50/approved/repos"
]
```

Attempts to access non-whitelisted paths will be blocked:

```bash
$ lit clone /media/unknown_usb/repo
Error: 🚫 AIRGAP MODE: Removable media path '/media/unknown_usb/repo' 
is not in the allowed list. Configure allowed media in ~/.lit/airgap.toml
```

---

## Workflow Examples

### Scenario 1: USB-Based Code Transfer

**Goal**: Transfer commits between two air-gapped machines using a USB drive.

```bash
# Machine A (source)
cd /home/user/project
lit commit -m "Feature complete"

# Create a bundle on USB
lit push usb master --force  # After configuring remote

# Machine B (destination)
cd /opt/projects/project
lit pull usb master

# Configure the USB remote (one-time setup)
lit remote add usb file:///media/usb/project.lit
```

### Scenario 2: Network Share Repository

**Goal**: Central repository on an isolated network file server.

```bash
# Create bare repository on network share
mkdir \\server\lit-repos\project.lit
lit init --bare \\server\lit-repos\project.lit

# Developers configure the remote
lit remote add origin \\server\lit-repos\project.lit

# Normal workflow (all in airgap mode)
lit --airgapped push origin master
lit --airgapped pull origin master
```

### Scenario 3: Strict Airgap (USB Only)

**Goal**: Maximum isolation for classified work.

```bash
# Enable strict mode
lit config set airgap.strict_mode true

# Only USB and local paths work
lit clone /media/classified_usb/project.lit  # ✅ Works
lit clone \\server\share\repo.lit            # ❌ Blocked

Error: 🚫 AIRGAP STRICT MODE: Network shares are blocked in strict mode.
Use USB/removable media only.
```

---

## Configuration Reference

### Airgap Configuration File

**Location**: `~/.lit/airgap.toml`

```toml
# Enable airgap mode globally
enabled = true

# Enable strict mode (USB/local only, blocks network shares)
strict_mode = false

# Allowed transport types (do not modify unless you know what you're doing)
allowed_transports = [
    "LocalFilesystem",
    "RemovableMedia",
    "NetworkShare",
    "FileProtocol"
]

# Whitelist specific removable media (empty = allow all)
allowed_media = []

# Whitelist specific network shares (empty = allow all)
allowed_shares = []

# Enable audit logging
audit_log = true

# Audit log path
audit_log_path = "~/.lit/airgap_audit.log"
```

### View Current Configuration

```bash
lit config show
```

Output:
```
Lit Configuration
==================

[airgap]
Airgap Mode: ✅ ENABLED
Strict Mode: disabled
Allowed Transports:
  - LocalFilesystem
  - RemovableMedia
  - NetworkShare
  - FileProtocol

[network]
Allowed Networks:
  - 10.0.0.0/8
  - 172.16.0.0/12
  - 192.168.0.0/16

[security]
Network Audit Log: enabled
Network Audit Log Path: ~/.lit/audit.log
Airgap Audit Log: enabled
Airgap Audit Log Path: ~/.lit/airgap_audit.log
```

---

## Command Reference

### Global Flag

```bash
# Enable airgap mode for a single command
lit --airgapped <command>

# Examples
lit --airgapped clone file:///mnt/usb/repo.lit
lit --airgapped push origin master
lit --airgapped remote add backup \\server\share\repo.lit
```

### Configuration Commands

```bash
# Show all configuration
lit config show

# Get airgap settings
lit config get airgap.enabled
lit config get airgap.strict_mode

# Enable airgap mode persistently
lit config set airgap.enabled true

# Enable strict mode
lit config set airgap.strict_mode true

# Disable airgap mode
lit config set airgap.enabled false
```

---

## Security Best Practices

### 1. **Always Enable Audit Logging**

Keep `audit_log = true` in your configuration. Logs provide:
- Compliance evidence
- Incident investigation trails
- Detection of unauthorized access attempts

### 2. **Use Strict Mode for Classified Work**

If your environment prohibits any network connectivity:

```bash
lit config set airgap.strict_mode true
```

This blocks even internal network shares, allowing only USB/removable media.

### 3. **Whitelist Approved Devices**

For maximum control, specify exact USB drive paths or network shares:

```toml
allowed_media = ["/media/approved_usb_001"]
allowed_shares = ["\\\\secure-server\\classified-repos"]
```

### 4. **Combine with FIPS 140-3 Mode**

For federal/military compliance:

```bash
# Enable both airgap and FIPS cryptography
lit config set airgap.enabled true
lit config set fips.enabled true  # (when FIPS CLI is implemented)

# Use quantum-resistant signatures for commits
lit commit -m "Classified feature" --sign
```

### 5. **Physical Security**

- **USB drives**: Use encrypted, FIPS-validated USB drives
- **Storage**: Keep USB media in approved secure containers
- **Destruction**: Follow data destruction protocols (DoD 5220.22-M)

### 6. **Regular Configuration Audits**

```bash
# Periodically review settings
lit config show

# Check audit logs
tail -f ~/.lit/airgap_audit.log
```

---

## Troubleshooting

### "Transport type is blocked"

**Problem**: Attempting to use a network protocol in airgap mode.

```
Error: 🚫 AIRGAP MODE: Transport type Http is blocked.
```

**Solution**: Use a physical transport:
```bash
# Change from HTTP to file://
lit clone file:///mnt/usb/repo.lit

# Or disable airgap mode temporarily
lit clone <url>  # (without --airgapped flag)
```

### "Not in the allowed list"

**Problem**: Path not whitelisted in configuration.

```
Error: 🚫 AIRGAP MODE: Removable media path '/media/usb2' 
is not in the allowed list.
```

**Solution**: Add to `~/.lit/airgap.toml`:
```toml
allowed_media = ["/media/usb1", "/media/usb2"]
```

### "Network shares blocked in strict mode"

**Problem**: Trying to use network shares with `strict_mode = true`.

```
Error: 🚫 AIRGAP STRICT MODE: Network shares are blocked.
```

**Solution**: Either disable strict mode or use USB only:
```bash
lit config set airgap.strict_mode false
```

---

## Comparison: Airgap vs Network Mode

| Feature               | Airgap Mode        | Network Mode                    |
| --------------------- | ------------------ | ------------------------------- |
| HTTP/HTTPS            | ❌ Blocked          | ✅ Allowed (with LAN validation) |
| SSH/SCP               | ❌ Blocked          | ✅ Allowed (with LAN validation) |
| Custom `lit://`       | ❌ Blocked          | ✅ Allowed (LAN only)            |
| USB/Removable         | ✅ Allowed          | ✅ Allowed                       |
| Network Shares        | ✅ Allowed*         | ✅ Allowed                       |
| Local Filesystem      | ✅ Allowed          | ✅ Allowed                       |
| Strict Mode Available | ✅ Yes              | ❌ No                            |
| Audit Logging         | ✅ Yes (transports) | ✅ Yes (network access)          |

\* *Blocked in strict mode*

---

## Implementation Details

### Transport Detection Algorithm

1. **Protocol-based detection** (e.g., `http://`, `ssh://`, `file://`)
2. **Path prefix detection** (e.g., `\\`, `//` for network shares)
3. **Windows API** (`GetDriveTypeW`) for removable drive detection
4. **Mount point analysis** (Linux/macOS: `/media/`, `/mnt/`, `/Volumes/`)

### Removable Media Detection

**Windows**:
- Uses `GetDriveTypeW` API to check drive type
- Detects `DRIVE_REMOVABLE` flag
- Works with USB drives, SD cards, external drives

**Linux/macOS**:
- Checks common mount points: `/media/`, `/mnt/`, `/Volumes/`
- Future: Can integrate with `udisks2` or `diskutil` for enhanced detection

---

## Compliance & Standards

### NIST SP 800-53 Controls

Airgap mode helps satisfy:

- **SC-7 (Boundary Protection)**: Physical isolation from untrusted networks
- **AC-4 (Information Flow Enforcement)**: Control data flow to approved transports
- **AU-2 (Audit Events)**: Log all transport access attempts

### ITAR/EAR Compliance

For controlled technical data:
- Enable **strict mode** for USB-only transfers
- Configure **whitelist** for approved devices
- Maintain **audit logs** for compliance evidence

### DoD Environments

- Compatible with **SIPRNET** air-gapped networks
- Supports **Classified Information Handling**
- Integrates with **FIPS 140-3 cryptography**

---

## Roadmap

### Planned Features

- [ ] **Smart removable media detection** (enhanced device fingerprinting)
- [ ] **USB device whitelisting** by serial number/vendor ID
- [ ] **Automatic repository bundling** for USB transfer
- [ ] **Integrity verification** for USB-transferred data
- [ ] **Tamper detection** for audit logs
- [ ] **Integration with hardware security modules (HSMs)**

---

## See Also

- [CRYPTOGRAPHY.md](CRYPTOGRAPHY.md) - Quantum-resistant cryptography
- [FIPS_140-3_COMPLIANCE.md](FIPS_140-3_COMPLIANCE.md) - Federal cryptographic compliance
- [DEPLOYMENT.md](DEPLOYMENT.md) - Deployment strategies
- [EXAMPLES.md](EXAMPLES.md) - Usage examples

---

## Support

For issues or questions about airgap mode:

1. Check audit logs: `~/.lit/airgap_audit.log`
2. Review configuration: `lit config show`
3. Test transport detection: `lit --airgapped clone <test-path>`

**Security Note**: Never disable airgap mode in environments that require physical isolation. This is a critical security control.
