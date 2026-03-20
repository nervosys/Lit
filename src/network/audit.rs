/// Audit Log with HMAC Integrity Protection
/// Provides tamper-evident logging for security events
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use zeroize::Zeroizing;

type HmacSha256 = Hmac<Sha256>;

/// Audit log manager with HMAC signing
pub struct AuditLog {
    log_path: PathBuf,
    signing_key: Zeroizing<Vec<u8>>,
}

impl AuditLog {
    /// Create or load an audit log with HMAC signing
    pub fn new(log_path: Option<&str>) -> Result<Self, String> {
        let log_path = if let Some(path) = log_path {
            PathBuf::from(shellexpand::tilde(path).as_ref())
        } else {
            Self::default_log_path()?
        };

        // Create log directory if needed
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create log directory: {}", e))?;
        }

        // Load or generate signing key
        let signing_key = Self::get_or_create_signing_key()?;

        // Set restrictive permissions on log file
        #[cfg(unix)]
        if log_path.exists() {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&log_path)
                .map_err(|e| format!("Failed to get log file metadata: {}", e))?
                .permissions();
            perms.set_mode(0o600); // Owner read/write only
            fs::set_permissions(&log_path, perms)
                .map_err(|e| format!("Failed to set log file permissions: {}", e))?;
        }

        Ok(AuditLog {
            log_path,
            signing_key,
        })
    }

    /// Append a signed entry to the audit log
    pub fn log(&self, event_type: &str, message: &str) -> Result<(), String> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let log_message = format!("{} | {} | {}", timestamp, event_type, message);

        // Create HMAC signature
        let signature = self.sign_message(&log_message)?;

        // Format: timestamp | event_type | message | signature
        let signed_entry = format!("{} | {}\n", log_message, hex::encode(signature));

        // Append to log file
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        file.write_all(signed_entry.as_bytes())
            .map_err(|e| format!("Failed to write to log: {}", e))?;

        Ok(())
    }

    /// Verify the integrity of all log entries
    pub fn verify(&self) -> Result<VerificationResult, String> {
        if !self.log_path.exists() {
            return Ok(VerificationResult {
                total_entries: 0,
                valid_entries: 0,
                invalid_entries: vec![],
            });
        }

        let content = fs::read_to_string(&self.log_path)
            .map_err(|e| format!("Failed to read log file: {}", e))?;

        let mut total = 0;
        let mut valid = 0;
        let mut invalid = vec![];

        for (line_num, line) in content.lines().enumerate() {
            total += 1;

            // Parse line: timestamp | event_type | message | signature
            let parts: Vec<&str> = line.rsplitn(2, " | ").collect();
            if parts.len() != 2 {
                invalid.push((line_num + 1, "Invalid format".to_string()));
                continue;
            }

            let signature_hex = parts[0];
            let message = parts[1];

            // Decode signature
            let stored_signature = match hex::decode(signature_hex) {
                Ok(sig) => sig,
                Err(_) => {
                    invalid.push((line_num + 1, "Invalid signature encoding".to_string()));
                    continue;
                }
            };

            // Verify signature
            match self.verify_signature(message, &stored_signature) {
                Ok(true) => valid += 1,
                Ok(false) => invalid.push((line_num + 1, "Invalid signature".to_string())),
                Err(e) => invalid.push((line_num + 1, format!("Verification error: {}", e))),
            }
        }

        Ok(VerificationResult {
            total_entries: total,
            valid_entries: valid,
            invalid_entries: invalid,
        })
    }

    /// Sign a message with HMAC-SHA256
    fn sign_message(&self, message: &str) -> Result<Vec<u8>, String> {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|e| format!("HMAC initialization failed: {}", e))?;

        mac.update(message.as_bytes());
        Ok(mac.finalize().into_bytes().to_vec())
    }

    /// Verify a message signature
    fn verify_signature(&self, message: &str, signature: &[u8]) -> Result<bool, String> {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|e| format!("HMAC initialization failed: {}", e))?;

        mac.update(message.as_bytes());

        match mac.verify_slice(signature) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get or create the HMAC signing key
    fn get_or_create_signing_key() -> Result<Zeroizing<Vec<u8>>, String> {
        let key_path = Self::signing_key_path()?;

        if key_path.exists() {
            // Load existing key
            let key_data =
                fs::read(&key_path).map_err(|e| format!("Failed to read signing key: {}", e))?;
            Ok(Zeroizing::new(key_data))
        } else {
            // Generate new key
            use aes_gcm::aead::rand_core::RngCore;
            use aes_gcm::aead::OsRng;

            let mut key = vec![0u8; 32]; // 256-bit key
            OsRng.fill_bytes(&mut key);

            // Save key with restrictive permissions
            fs::write(&key_path, &key)
                .map_err(|e| format!("Failed to write signing key: {}", e))?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&key_path)
                    .map_err(|e| format!("Failed to get key file metadata: {}", e))?
                    .permissions();
                perms.set_mode(0o600); // Owner read/write only
                fs::set_permissions(&key_path, perms)
                    .map_err(|e| format!("Failed to set key file permissions: {}", e))?;
            }

            #[cfg(windows)]
            {
                // SECURITY: On Windows, mark file as read-only for the owner.
                // Full DACL restriction would require winapi; read-only is a
                // reasonable baseline to prevent accidental writes.
                let mut perms = fs::metadata(&key_path)
                    .map_err(|e| format!("Failed to get key file metadata: {}", e))?
                    .permissions();
                perms.set_readonly(true);
                fs::set_permissions(&key_path, perms)
                    .map_err(|e| format!("Failed to set key file permissions: {}", e))?;
            }

            Ok(Zeroizing::new(key))
        }
    }

    /// Get default log path
    fn default_log_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Could not determine home directory")?;
        Ok(home.join(".lit").join("audit.log"))
    }

    /// Get signing key path
    fn signing_key_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Could not determine home directory")?;
        Ok(home.join(".lit").join("audit.key"))
    }
}

/// Result of audit log verification
#[derive(Debug)]
pub struct VerificationResult {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub invalid_entries: Vec<(usize, String)>, // (line_number, error)
}

impl VerificationResult {
    /// Check if all entries are valid
    pub fn is_valid(&self) -> bool {
        self.invalid_entries.is_empty()
    }

    /// Get verification summary
    pub fn summary(&self) -> String {
        if self.is_valid() {
            format!(
                "✓ All {} audit log entries verified successfully",
                self.total_entries
            )
        } else {
            format!(
                "✗ Verification failed: {}/{} entries invalid",
                self.invalid_entries.len(),
                self.total_entries
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_audit_log_signing() {
        let temp_log = NamedTempFile::new().unwrap();
        let log_path = temp_log.path().to_str().unwrap();

        let audit = AuditLog::new(Some(log_path)).unwrap();

        // Log some events
        audit.log("TEST", "First test event").unwrap();
        audit.log("TEST", "Second test event").unwrap();

        // Verify
        let result = audit.verify().unwrap();
        assert_eq!(result.total_entries, 2);
        assert_eq!(result.valid_entries, 2);
        assert!(result.is_valid());
    }

    #[test]
    fn test_tamper_detection() {
        let temp_log = NamedTempFile::new().unwrap();
        let log_path = temp_log.path().to_str().unwrap();

        let audit = AuditLog::new(Some(log_path)).unwrap();

        // Log an event
        audit.log("TEST", "Original message").unwrap();

        // Tamper with the log file
        let mut content = fs::read_to_string(&temp_log).unwrap();
        content = content.replace("Original message", "Tampered message");
        fs::write(&temp_log, content).unwrap();

        // Verify should fail
        let result = audit.verify().unwrap();
        assert!(!result.is_valid());
        assert_eq!(result.invalid_entries.len(), 1);
    }

    #[test]
    fn test_signing_key_persistence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_path = temp_dir.path().join("test.log");

        // Create first instance
        let audit1 = AuditLog::new(Some(log_path.to_str().unwrap())).unwrap();
        audit1.log("TEST", "Test event").unwrap();

        // Create second instance (should load same key)
        let audit2 = AuditLog::new(Some(log_path.to_str().unwrap())).unwrap();
        let result = audit2.verify().unwrap();

        assert!(result.is_valid());
    }
}
