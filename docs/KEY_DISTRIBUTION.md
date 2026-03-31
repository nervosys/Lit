# Secure Key File Distribution Guide

**Document Version**: 1.0  
**Last Updated**: October 25, 2025  
**Security Classification**: UNCLASSIFIED // FOR OFFICIAL USE ONLY

---

## Overview

This guide provides best practices for securely distributing the Lit encryption key file (`~/.lit/encryption.key`) between systems. The key file contains cryptographic material that **must be protected** during transfer.

⚠️ **WARNING**: Improper key distribution can compromise the security of all encrypted repositories.

---

## Key File Contents

The encryption key file contains:
- **Salt** (16 bytes): Random value for key derivation
- **Version** (1 byte): Key file format version
- **Verification Hash** (32 bytes): SHA256 hash for passphrase validation

**Total Size**: 49 bytes (current format)

While the key file itself does not contain the encryption key (which is derived from your passphrase), it is still sensitive because:
1. It enables decryption when combined with the passphrase
2. Loss of the key file makes encrypted data unrecoverable
3. An attacker with the key file can perform offline passphrase attacks

---

## ❌ DO NOT

### Never Use These Methods

1. **Email** - Emails are typically unencrypted and stored on multiple servers
   ```bash
   ❌ WRONG: echo "~/.lit/encryption.key" | mail -a ~/.lit/encryption.key user@example.com
   ```

2. **Version Control** - Git repositories may be public or leaked
   ```bash
   ❌ WRONG: git add ~/.lit/encryption.key
   ```

3. **Unencrypted Network Transfer** - Plain FTP, HTTP, or unencrypted network shares
   ```bash
   ❌ WRONG: ftp upload encryption.key
   ❌ WRONG: curl -T encryption.key http://server/
   ```

4. **Cloud Storage (Without Encryption)** - Dropbox, Google Drive, etc. without encryption
   ```bash
   ❌ WRONG: cp ~/.lit/encryption.key ~/Dropbox/
   ```

5. **Messaging Apps** - WhatsApp, Slack, Discord, Teams (even if "encrypted")
   ```bash
   ❌ WRONG: Send as file attachment in messaging app
   ```

6. **Shared Drives** - Network drives accessible by multiple users
   ```bash
   ❌ WRONG: cp ~/.lit/encryption.key /mnt/shared_drive/
   ```

---

## ✅ DO

### Recommended Secure Transfer Methods

### Method 1: GPG Encryption + Secure Copy (Recommended)

**Best for**: Remote transfer over network

```bash
# On source system
gpg --encrypt --recipient user@example.com ~/.lit/encryption.key
scp ~/.lit/encryption.key.gpg user@destination:/tmp/

# On destination system
gpg --decrypt /tmp/encryption.key.gpg > ~/.lit/encryption.key
chmod 600 ~/.lit/encryption.key
shred -u /tmp/encryption.key.gpg  # Securely delete encrypted file
```

**Verification**:
```bash
# Compare checksums to verify integrity
# On source:
sha256sum ~/.lit/encryption.key

# On destination:
sha256sum ~/.lit/encryption.key
# Should match exactly
```

---

### Method 2: Encrypted USB Drive (Airgap Transfer)

**Best for**: Physical transfer, airgapped systems

**Windows (BitLocker)**:
1. Insert USB drive
2. Right-click drive → "Turn on BitLocker"
3. Choose strong password (16+ characters)
4. Copy `encryption.key` to encrypted drive
5. Safely eject drive

**Linux (LUKS)**:
```bash
# Create encrypted USB drive
cryptsetup luksFormat /dev/sdX  # Replace sdX with USB device
cryptsetup open /dev/sdX secure_usb
mkfs.ext4 /dev/mapper/secure_usb
mount /dev/mapper/secure_usb /mnt/usb

# Copy key file
cp ~/.lit/encryption.key /mnt/usb/
sync

# Unmount and close
umount /mnt/usb
cryptsetup close secure_usb

# On destination system
cryptsetup open /dev/sdX secure_usb
mount /dev/mapper/secure_usb /mnt/usb
cp /mnt/usb/encryption.key ~/.lit/
chmod 600 ~/.lit/encryption.key
umount /mnt/usb
cryptsetup close secure_usb
```

**macOS (Disk Utility)**:
1. Open Disk Utility
2. Select USB drive → Erase
3. Format: "Mac OS Extended (Journaled, Encrypted)"
4. Set strong password
5. Copy `encryption.key` to encrypted volume

---

### Method 3: Password Manager with End-to-End Encryption

**Best for**: Personal use, small teams

**Supported Password Managers**:
- 1Password (encrypted vaults)
- Bitwarden (self-hosted or cloud)
- KeePassXC (local database)

**Steps**:
```bash
# Convert key file to base64 for storage
base64 ~/.lit/encryption.key > key.txt

# Store key.txt content in password manager secure note
# On destination:
# Retrieve from password manager, save to file
base64 -d > ~/.lit/encryption.key
chmod 600 ~/.lit/encryption.key
```

---

### Method 4: SSH Secure Copy with Host Verification

**Best for**: Trusted network, known hosts

```bash
# Ensure SSH host key is verified (prevents MITM)
ssh-keyscan -H destination.example.com >> ~/.ssh/known_hosts

# Use SCP with strict host checking
scp -o StrictHostKeyChecking=yes ~/.lit/encryption.key user@destination:~/.lit/

# On destination, verify permissions
chmod 600 ~/.lit/encryption.key
```

---

### Method 5: QR Code (For Airgap Transfer)

**Best for**: Extremely secure environments, airgapped systems

```bash
# On source system
qrencode -o key_qr.png < ~/.lit/encryption.key

# Display QR code on screen (or print on paper)
display key_qr.png  # Linux
open key_qr.png     # macOS

# On destination system (scan with camera)
zbarcam > ~/.lit/encryption.key  # Linux with webcam
# Or use mobile app to scan and type manually

chmod 600 ~/.lit/encryption.key

# Destroy QR code image
shred -u key_qr.png
```

---

### Method 6: Shamir's Secret Sharing (For High Security)

**Best for**: Multiple trustees, disaster recovery

```bash
# Split key into 5 shares, require 3 to reconstruct
ssss-split -t 3 -n 5 < ~/.lit/encryption.key

# Distribute shares to different trustees
# Each share is useless alone, need 3 to reconstruct

# To reconstruct:
ssss-combine -t 3 > ~/.lit/encryption.key
chmod 600 ~/.lit/encryption.key
```

---

## Security Checklist

Before transferring the key file, verify:

- [ ] Transfer method is encrypted end-to-end
- [ ] Recipient identity is verified (no MITM attack)
- [ ] Transfer channel integrity is verified (checksums)
- [ ] Original key file permissions are restrictive (`chmod 600`)
- [ ] Temporary files are securely deleted after transfer
- [ ] Transfer logs are cleared (if applicable)
- [ ] Key file is tested on destination before deleting source

---

## Key File Backup Best Practices

### Backup Storage

**Recommended**:
1. **Encrypted USB drive** stored in physical safe
2. **Hardware security module (HSM)** for enterprise
3. **Encrypted cloud backup** (encrypted locally first with GPG)
4. **Split key shares** using Shamir's Secret Sharing

**Storage Duration**:
- Keep backups for at least 5 years
- Test backup restoration annually
- Rotate backups every 2 years

### Backup Encryption Example

```bash
# Create encrypted backup
tar czf - ~/.lit/encryption.key | \
    gpg --symmetric --cipher-algo AES256 > \
    encryption_key_backup_$(date +%Y%m%d).tar.gz.gpg

# Store in secure location
# To restore:
gpg --decrypt encryption_key_backup_YYYYMMDD.tar.gz.gpg | \
    tar xzf - -C ~/
```

---

## Enterprise Distribution

### For Organizations

1. **Use Configuration Management**:
   ```yaml
   # Ansible example
   - name: Deploy encryption key
     copy:
       src: "{{ vault_encryption_key }}"  # Encrypted in Ansible Vault
       dest: ~/.lit/encryption.key
       mode: '0600'
       owner: "{{ user }}"
   ```

2. **Hardware Security Modules (HSM)**:
   - Store master key in HSM
   - Derive per-user keys from master
   - Centralized key rotation

3. **Certificate-Based Distribution**:
   - Use X.509 certificates for authentication
   - Encrypt key file with recipient's public key
   - Require certificate verification before decryption

---

## Incident Response

### If Key File Is Compromised

1. **Immediately**:
   ```bash
   # Rotate to new passphrase
   lit rotate-key
   ```

2. **Verify**:
   ```bash
   # Check for unauthorized decryption attempts
   lit log --encrypted
   ```

3. **Distribute New Key**:
   - Use secure method from this guide
   - Notify all authorized users
   - Revoke old key file

### If Key File Is Lost

⚠️ **Critical**: Without the key file, encrypted data is **permanently unrecoverable**

**Prevention**:
- Maintain multiple secure backups
- Test backup restoration quarterly
- Document backup locations securely

---

## Compliance Considerations

### Regulatory Requirements

| Standard           | Requirement                     | Implementation               |
| ------------------ | ------------------------------- | ---------------------------- |
| **NIST SP 800-57** | Key transport encryption        | GPG/AES-256 encryption       |
| **FIPS 140-3**     | Key zeroization                 | Use `shred` after transfer   |
| **GDPR**           | Data protection during transfer | End-to-end encryption        |
| **HIPAA**          | Encryption in transit           | TLS 1.3 for network transfer |

---

## Quick Reference

### Transfer Method Comparison

| Method           | Security | Convenience | Use Case          |
| ---------------- | -------- | ----------- | ----------------- |
| GPG + SCP        | ★★★★★    | ★★★☆☆       | Remote transfer   |
| Encrypted USB    | ★★★★★    | ★★★★☆       | Physical transfer |
| Password Manager | ★★★★☆    | ★★★★★       | Personal use      |
| SSH/SCP          | ★★★★☆    | ★★★★☆       | Trusted network   |
| QR Code          | ★★★☆☆    | ★★☆☆☆       | Airgap transfer   |
| Shamir Sharing   | ★★★★★    | ★☆☆☆☆       | Disaster recovery |

---

## Additional Resources

- [NIST SP 800-57: Key Management](https://csrc.nist.gov/publications/detail/sp/800-57-part-1/rev-5/final)
- [GPG Documentation](https://gnupg.org/documentation/)
- [Shamir's Secret Sharing](https://en.wikipedia.org/wiki/Shamir%27s_Secret_Sharing)
- [LUKS Encryption Guide](https://gitlab.com/cryptsetup/cryptsetup)

---

## Support

For questions about secure key distribution:
- Email: security@lit-vcs.example.com
- Documentation: https://github.com/nervosys/lit/docs
- Security Advisories: SECURITY.md

---

**Document Classification**: UNCLASSIFIED // FOR OFFICIAL USE ONLY  
**Revision History**:
- v1.0 (2025-10-25): Initial release
