/// Centralized Error Types for Lit
/// Provides sanitized error messages that don't leak sensitive information
/// and machine-readable error codes for agentic consumption
use std::fmt;
use std::io;

/// Machine-readable error codes for structured output
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    RepoNotFound,
    RepoCorrupt,
    RefNotFound,
    RefConflict,
    MergeConflict,
    IndexLocked,
    AuthFailed,
    TransportDenied,
    CryptoError,
    ObjectNotFound,
    InvalidInput,
    NotImplemented,
    IoError,
    ConfigError,
    GeneralError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::RepoNotFound => "REPO_NOT_FOUND",
            ErrorCode::RepoCorrupt => "REPO_CORRUPT",
            ErrorCode::RefNotFound => "REF_NOT_FOUND",
            ErrorCode::RefConflict => "REF_CONFLICT",
            ErrorCode::MergeConflict => "MERGE_CONFLICT",
            ErrorCode::IndexLocked => "INDEX_LOCKED",
            ErrorCode::AuthFailed => "AUTH_FAILED",
            ErrorCode::TransportDenied => "TRANSPORT_DENIED",
            ErrorCode::CryptoError => "CRYPTO_ERROR",
            ErrorCode::ObjectNotFound => "OBJECT_NOT_FOUND",
            ErrorCode::InvalidInput => "INVALID_INPUT",
            ErrorCode::NotImplemented => "NOT_IMPLEMENTED",
            ErrorCode::IoError => "IO_ERROR",
            ErrorCode::ConfigError => "CONFIG_ERROR",
            ErrorCode::GeneralError => "GENERAL_ERROR",
        }
    }
}

/// Main error type for Lit operations
#[derive(Debug)]
pub enum LitError {
    /// Encryption-related errors (passphrase, key derivation, etc.)
    Encryption(String),
    /// I/O errors (file read/write)
    IO(String),
    /// Configuration errors
    Config(String),
    /// Network-related errors
    Network(String),
    /// Repository structure errors
    Repository(String),
    /// Git object errors
    Object(String),
    /// Index errors
    Index(String),
    /// General errors
    General(String),
}

impl LitError {
    /// Create an encryption error with detailed internal message
    pub fn encryption(internal_msg: impl Into<String>) -> Self {
        LitError::Encryption(internal_msg.into())
    }

    /// Create an I/O error with detailed internal message
    pub fn io(internal_msg: impl Into<String>) -> Self {
        LitError::IO(internal_msg.into())
    }

    /// Create a config error with detailed internal message
    pub fn config(internal_msg: impl Into<String>) -> Self {
        LitError::Config(internal_msg.into())
    }

    /// Create a network error with detailed internal message
    pub fn network(internal_msg: impl Into<String>) -> Self {
        LitError::Network(internal_msg.into())
    }

    /// Create a repository error with detailed internal message
    pub fn repository(internal_msg: impl Into<String>) -> Self {
        LitError::Repository(internal_msg.into())
    }

    /// Create an object error with detailed internal message
    pub fn object(internal_msg: impl Into<String>) -> Self {
        LitError::Object(internal_msg.into())
    }

    /// Create an index error with detailed internal message
    pub fn index(internal_msg: impl Into<String>) -> Self {
        LitError::Index(internal_msg.into())
    }

    /// Create a general error with detailed internal message
    pub fn general(internal_msg: impl Into<String>) -> Self {
        LitError::General(internal_msg.into())
    }

    /// Get the internal detailed error message (for logging only)
    pub fn internal_message(&self) -> &str {
        match self {
            LitError::Encryption(msg) => msg,
            LitError::IO(msg) => msg,
            LitError::Config(msg) => msg,
            LitError::Network(msg) => msg,
            LitError::Repository(msg) => msg,
            LitError::Object(msg) => msg,
            LitError::Index(msg) => msg,
            LitError::General(msg) => msg,
        }
    }

    /// Machine-readable error code for structured output
    pub fn error_code(&self) -> &'static str {
        match self {
            LitError::Encryption(_) => ErrorCode::CryptoError.as_str(),
            LitError::IO(_) => ErrorCode::IoError.as_str(),
            LitError::Config(_) => ErrorCode::ConfigError.as_str(),
            LitError::Network(_) => ErrorCode::TransportDenied.as_str(),
            LitError::Repository(msg) => {
                if msg.contains("not found")
                    || msg.contains("No .lit directory")
                    || msg.contains("find_repo_root")
                {
                    ErrorCode::RepoNotFound.as_str()
                } else {
                    ErrorCode::RepoCorrupt.as_str()
                }
            }
            LitError::Object(msg) => {
                if msg.contains("not found") || msg.contains("No such") {
                    ErrorCode::ObjectNotFound.as_str()
                } else {
                    ErrorCode::GeneralError.as_str()
                }
            }
            LitError::Index(_) => ErrorCode::GeneralError.as_str(),
            LitError::General(msg) => {
                if msg.contains("not yet implemented") || msg.contains("not yet fully implemented")
                {
                    ErrorCode::NotImplemented.as_str()
                } else if msg.contains("not found") {
                    ErrorCode::RefNotFound.as_str()
                } else {
                    ErrorCode::GeneralError.as_str()
                }
            }
        }
    }

    /// User-facing error message (safe to display — strips internal details)
    /// SECURITY: Returns category-based messages to prevent information disclosure (FINDING-003)
    pub fn user_message(&self) -> &str {
        match self {
            LitError::Encryption(_) => "Encryption operation failed",
            LitError::IO(_) => "I/O operation failed",
            LitError::Config(_) => "Configuration error",
            LitError::Network(_) => "Network operation failed",
            LitError::Repository(msg) => {
                if msg.contains("not found") || msg.contains("No .lit directory") {
                    "Not in a Lit repository"
                } else {
                    "Repository error"
                }
            }
            LitError::Object(msg) => {
                if msg.contains("not found") || msg.contains("No such") {
                    "Object not found"
                } else {
                    "Object error"
                }
            }
            LitError::Index(_) => "Index error",
            LitError::General(msg) => {
                if msg.contains("not yet implemented") || msg.contains("not yet fully implemented")
                {
                    "Feature not yet implemented"
                } else if msg.contains("not found") {
                    "Resource not found"
                } else {
                    "Operation failed"
                }
            }
        }
    }

    /// Actionable suggestions for agents to resolve the error
    pub fn suggestions(&self) -> Vec<&'static str> {
        match self {
            // The rendered message is deliberately sanitized, so these two
            // encryption cases would otherwise reach the user as a bare
            // "Operation failed" with nothing to act on.
            LitError::General(msg) | LitError::IO(msg)
                if msg.contains("no Lit encryption header") =>
            {
                vec![
                    "Encryption cannot be enabled for a repository that already has commits",
                    "Create a new repository with encryption enabled and import into it",
                ]
            }
            LitError::General(msg) | LitError::IO(msg)
                if msg.contains("Encryption not initialized") =>
            {
                vec![
                    "Set LIT_PASSPHRASE or LIT_PASSPHRASE_FILE to unlock the repository",
                    "Check encryption settings in .lit/encryption.toml",
                ]
            }
            LitError::Repository(msg)
                if msg.contains("not found") || msg.contains("No .lit directory") =>
            {
                vec![
                    "Run 'lit init' to create a repository",
                    "Check that you are in the correct directory",
                ]
            }
            LitError::Object(_) => {
                vec![
                    "Verify the object hash is correct",
                    "Run 'lit verify' to check repository integrity",
                ]
            }
            LitError::Network(_) => {
                vec![
                    "Check remote URL configuration with 'lit remote list'",
                    "Verify network/airgap settings with 'lit config show'",
                ]
            }
            // A refused agent handshake is not a wrong passphrase, and saying
            // so sends the user to check the one thing that is not the problem.
            // What actually happened is that something other than the agent is
            // on the recorded port, usually because the agent died without
            // clearing its endpoint file.
            LitError::Encryption(msg) if msg.contains("could not prove it is the agent") => {
                vec![
                    "Stop the stale agent with 'lit agent stop', then start a new one",
                    "Nothing was sent to the process on that port",
                ]
            }
            LitError::Encryption(_) => {
                vec![
                    "Verify passphrase is correct",
                    "Check encryption configuration",
                ]
            }
            LitError::General(msg) if msg.contains("not yet implemented") => {
                vec!["This feature is planned for a future release"]
            }
            _ => vec![],
        }
    }

    /// Log detailed error to secure log file (not stdout/stderr)
    pub fn log_detailed(&self) {
        // Only log if debug logging is enabled
        if std::env::var("LIT_DEBUG").is_ok() {
            let log_path = get_secure_log_path();
            if let Ok(path) = log_path {
                use std::fs::OpenOptions;
                use std::io::Write;

                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                let log_entry =
                    format!("[{}] {:?}: {}\n", timestamp, self, self.internal_message());

                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                    let _ = file.write_all(log_entry.as_bytes());
                }
            }
        }
    }
}

/// Display implementation shows sanitized error messages
/// SECURITY: Does not expose file paths, internal state, or detailed errors
impl fmt::Display for LitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LitError::Encryption(_) => write!(f, "Encryption operation failed"),
            LitError::IO(_) => write!(f, "I/O operation failed"),
            LitError::Config(_) => write!(f, "Configuration error"),
            LitError::Network(_) => write!(f, "Network operation failed"),
            LitError::Repository(_) => write!(f, "Repository operation failed"),
            LitError::Object(_) => write!(f, "Object operation failed"),
            LitError::Index(_) => write!(f, "Index operation failed"),
            LitError::General(_) => write!(f, "Operation failed"),
        }
    }
}

impl std::error::Error for LitError {}

/// Convert from io::Error
impl From<io::Error> for LitError {
    fn from(err: io::Error) -> Self {
        LitError::IO(err.to_string())
    }
}

/// Convert from String for backward compatibility
impl From<String> for LitError {
    fn from(msg: String) -> Self {
        LitError::General(msg)
    }
}

/// Convert from &str for convenience
impl From<&str> for LitError {
    fn from(msg: &str) -> Self {
        LitError::General(msg.to_string())
    }
}

/// Get secure log file path
fn get_secure_log_path() -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    let log_dir = home.join(".lit").join("logs");

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)
        .map_err(|e| format!("Failed to create log directory: {}", e))?;

    let log_file = log_dir.join("debug.log");

    // Ensure restrictive permissions on log file
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::PermissionsExt;

        // `create_new` rather than `create`: the only goal is to bring the file
        // into existence so the mode can be tightened below. Should it appear
        // between the check and the open, this fails harmlessly instead of
        // truncating a log that is already being written.
        if !log_file.exists() {
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&log_file)
                .ok();
        }

        if let Ok(metadata) = std::fs::metadata(&log_file) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o600); // Owner read/write only
            std::fs::set_permissions(&log_file, perms).ok();
        }
    }

    Ok(log_file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_sanitization() {
        let err = LitError::encryption("Detailed: failed to decrypt /home/user/secret/file.txt");
        assert_eq!(err.to_string(), "Encryption operation failed");
        assert!(!err.to_string().contains("/home"));
        assert!(!err.to_string().contains("secret"));
    }

    #[test]
    fn test_internal_message_access() {
        let err = LitError::io("Failed to read /etc/shadow");
        assert_eq!(err.internal_message(), "Failed to read /etc/shadow");
    }

    #[test]
    fn test_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let lit_err: LitError = io_err.into();
        assert_eq!(lit_err.to_string(), "I/O operation failed");
    }
}
