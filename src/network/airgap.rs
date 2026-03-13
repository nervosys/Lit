use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Global airgap mode flag
static AIRGAP_MODE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Airgap configuration for isolated network environments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirgapConfig {
    /// Enable airgap mode (blocks all network protocols)
    pub enabled: bool,

    /// Allowed transport types
    pub allowed_transports: Vec<TransportType>,

    /// Allowed removable media paths (USB drives, etc.)
    pub allowed_media: Vec<String>,

    /// Allowed network shares (SMB/CIFS paths)
    pub allowed_shares: Vec<String>,

    /// Enable strict mode (blocks even LAN protocols)
    pub strict_mode: bool,

    /// Audit logging for transport access
    pub audit_log: bool,

    /// Audit log path
    pub audit_log_path: Option<String>,
}

/// Transport types for airgapped environments
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportType {
    /// Local filesystem (always allowed)
    LocalFilesystem,

    /// USB/removable media drives
    RemovableMedia,

    /// Network file shares (SMB/CIFS)
    NetworkShare,

    /// Direct file:// protocol
    FileProtocol,

    /// Blocked: HTTP/HTTPS
    Http,

    /// Blocked: SSH/SCP
    Ssh,

    /// Blocked: Custom lit:// network protocol
    LitProtocol,

    /// Blocked: FTP/FTPS
    Ftp,

    /// Blocked: Any other network protocol
    Other,
}

impl Default for AirgapConfig {
    fn default() -> Self {
        AirgapConfig {
            enabled: false,
            allowed_transports: vec![
                TransportType::LocalFilesystem,
                TransportType::RemovableMedia,
                TransportType::NetworkShare,
                TransportType::FileProtocol,
            ],
            allowed_media: vec![],
            allowed_shares: vec![],
            strict_mode: false,
            audit_log: true,
            audit_log_path: Some("~/.lit/airgap_audit.log".to_string()),
        }
    }
}

impl AirgapConfig {
    /// Load configuration from file
    pub fn load() -> Result<Self, String> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(AirgapConfig::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read airgap config: {}", e))?;

        toml::from_str(&content).map_err(|e| format!("Failed to parse airgap config: {}", e))
    }

    /// Get the config file path
    fn config_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".lit").join("airgap.toml"))
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), String> {
        let config_path = Self::config_path()?;

        // Create parent directory if needed
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&config_path, content).map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Enable airgap mode globally
    pub fn enable_airgap_mode() {
        AIRGAP_MODE_ENABLED.store(true, Ordering::SeqCst);
    }

    /// Disable airgap mode globally
    pub fn disable_airgap_mode() {
        AIRGAP_MODE_ENABLED.store(false, Ordering::SeqCst);
    }

    /// Check if airgap mode is enabled
    pub fn is_airgap_mode() -> bool {
        AIRGAP_MODE_ENABLED.load(Ordering::SeqCst)
    }
}

/// Airgap validator for transport restrictions
pub struct AirgapValidator {
    config: AirgapConfig,
}

impl AirgapValidator {
    /// Create a new validator
    pub fn new() -> Result<Self, String> {
        let config = AirgapConfig::load()?;

        // Apply global airgap mode if configured
        if config.enabled {
            AirgapConfig::enable_airgap_mode();
        }

        Ok(AirgapValidator { config })
    }

    /// Validate a path/URL for airgapped access
    pub fn validate_transport(&self, path: &str) -> Result<TransportInfo, String> {
        // If airgap mode is not enabled, allow everything
        if !self.config.enabled && !AirgapConfig::is_airgap_mode() {
            return Ok(TransportInfo {
                transport_type: self.detect_transport_type(path)?,
                normalized_path: path.to_string(),
                is_allowed: true,
            });
        }

        // Detect transport type
        let transport_type = self.detect_transport_type(path)?;

        // Check if transport type is allowed
        if !self.config.allowed_transports.contains(&transport_type) {
            return Err(format!(
                "🚫 AIRGAP MODE: Transport type {:?} is blocked. \
                 Only physical transports allowed (USB, network shares, local filesystem). \
                 Use --airgapped=false to disable airgap mode.",
                transport_type
            ));
        }

        // Additional validation based on transport type
        match &transport_type {
            TransportType::RemovableMedia => {
                self.validate_removable_media(path)?;
            }
            TransportType::NetworkShare => {
                if self.config.strict_mode {
                    return Err(
                        "🚫 AIRGAP STRICT MODE: Network shares are blocked in strict mode. \
                         Use USB/removable media only."
                            .to_string(),
                    );
                }
                self.validate_network_share(path)?;
            }
            TransportType::Http
            | TransportType::Ssh
            | TransportType::LitProtocol
            | TransportType::Ftp
            | TransportType::Other => {
                return Err(format!(
                    "🚫 AIRGAP MODE: Network protocol {:?} is blocked. \
                     Use file://, USB drives, or network shares only.",
                    transport_type
                ));
            }
            _ => {}
        }

        // Log if enabled
        if self.config.audit_log {
            self.log_transport_access(path, &transport_type)?;
        }

        Ok(TransportInfo {
            transport_type: transport_type.clone(),
            normalized_path: self.normalize_path(path)?,
            is_allowed: true,
        })
    }

    /// Detect the transport type from a path/URL
    fn detect_transport_type(&self, path: &str) -> Result<TransportType, String> {
        // Protocol-based detection
        if path.starts_with("http://") || path.starts_with("https://") {
            return Ok(TransportType::Http);
        }
        if path.starts_with("ssh://") || path.starts_with("scp://") {
            return Ok(TransportType::Ssh);
        }
        if path.starts_with("lit://") {
            return Ok(TransportType::LitProtocol);
        }
        if path.starts_with("ftp://") || path.starts_with("ftps://") {
            return Ok(TransportType::Ftp);
        }
        if path.starts_with("file://") {
            return Ok(TransportType::FileProtocol);
        }

        // Path-based detection
        let path_obj = Path::new(path);

        // Windows network share detection (\\server\share or //server/share)
        if path.starts_with(r"\\") || path.starts_with("//") {
            return Ok(TransportType::NetworkShare);
        }

        // Windows drive letter detection
        #[cfg(target_os = "windows")]
        {
            if let Some(first_component) = path_obj.components().next() {
                use std::path::Component;
                if let Component::Prefix(prefix) = first_component {
                    use std::path::Prefix;
                    match prefix.kind() {
                        Prefix::Disk(_) | Prefix::VerbatimDisk(_) => {
                            // Check if it's a removable drive
                            if self.is_removable_drive(path)? {
                                return Ok(TransportType::RemovableMedia);
                            }
                            return Ok(TransportType::LocalFilesystem);
                        }
                        Prefix::UNC(_, _) | Prefix::VerbatimUNC(_, _) => {
                            return Ok(TransportType::NetworkShare);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Unix absolute path
        if path.starts_with('/') {
            // Check if it's a mount point for removable media
            if self.is_removable_mount(path)? {
                return Ok(TransportType::RemovableMedia);
            }
            return Ok(TransportType::LocalFilesystem);
        }

        // Relative path - treat as local filesystem
        Ok(TransportType::LocalFilesystem)
    }

    /// Check if a Windows drive is removable (USB, etc.)
    #[cfg(target_os = "windows")]
    fn is_removable_drive(&self, path: &str) -> Result<bool, String> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        // Extract drive letter
        let path_obj = Path::new(path);
        let drive = if let Some(first_component) = path_obj.components().next() {
            first_component.as_os_str().to_string_lossy().to_string()
        } else {
            return Ok(false);
        };

        // Add backslash if not present
        let drive_root = if drive.ends_with('\\') {
            drive
        } else {
            format!("{}\\", drive)
        };

        // Convert to wide string for Windows API
        let wide: Vec<u16> = OsStr::new(&drive_root)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: `wide` is a valid null-terminated UTF-16 string from OsStr conversion.
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::core::PCWSTR;
            use windows::Win32::Storage::FileSystem::GetDriveTypeW;

            let drive_type = GetDriveTypeW(PCWSTR::from_raw(wide.as_ptr()));
            // DRIVE_REMOVABLE = 2
            Ok(drive_type == 2)
        }

        #[cfg(not(target_os = "windows"))]
        Ok(false)
    }

    #[cfg(not(target_os = "windows"))]
    fn is_removable_drive(&self, _path: &str) -> Result<bool, String> {
        Ok(false)
    }

    /// Check if a Unix path is a mount point for removable media
    fn is_removable_mount(&self, path: &str) -> Result<bool, String> {
        // Common removable media mount points
        let removable_paths = vec![
            "/media/",
            "/mnt/",
            "/Volumes/", // macOS
            "/run/media/",
        ];

        for mount_prefix in removable_paths {
            if path.starts_with(mount_prefix) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Validate removable media access
    fn validate_removable_media(&self, path: &str) -> Result<(), String> {
        // If no specific media paths are configured, allow all removable media
        if self.config.allowed_media.is_empty() {
            return Ok(());
        }

        // Check if path starts with any allowed media path
        for allowed in &self.config.allowed_media {
            if path.starts_with(allowed) {
                return Ok(());
            }
        }

        Err(format!(
            "🚫 AIRGAP MODE: Removable media path '{}' is not in the allowed list. \
             Configure allowed media in ~/.lit/airgap.toml",
            path
        ))
    }

    /// Validate network share access
    fn validate_network_share(&self, path: &str) -> Result<(), String> {
        // If no specific shares are configured, allow all network shares
        if self.config.allowed_shares.is_empty() {
            return Ok(());
        }

        // Check if path starts with any allowed share path
        for allowed in &self.config.allowed_shares {
            if path.starts_with(allowed) {
                return Ok(());
            }
        }

        Err(format!(
            "🚫 AIRGAP MODE: Network share '{}' is not in the allowed list. \
             Configure allowed shares in ~/.lit/airgap.toml",
            path
        ))
    }

    /// Normalize a path for consistent handling
    fn normalize_path(&self, path: &str) -> Result<String, String> {
        // Remove file:// prefix if present
        let path = if let Some(stripped) = path.strip_prefix("file://") {
            stripped
        } else {
            path
        };

        // Expand environment variables and tildes
        let expanded =
            shellexpand::full(path).map_err(|e| format!("Failed to expand path: {}", e))?;

        Ok(expanded.to_string())
    }

    /// Log a transport access attempt
    fn log_transport_access(
        &self,
        path: &str,
        transport_type: &TransportType,
    ) -> Result<(), String> {
        if let Some(log_path) = &self.config.audit_log_path {
            let expanded_path = shellexpand::tilde(log_path);
            let log_path = PathBuf::from(expanded_path.as_ref());

            // Create parent directory if needed
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create log directory: {}", e))?;
            }

            let timestamp = chrono::Utc::now().to_rfc3339();
            let log_entry = format!(
                "{} | AIRGAP TRANSPORT | {:?} | {}\n",
                timestamp, transport_type, path
            );

            use std::io::Write;
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .map_err(|e| format!("Failed to open log file: {}", e))?;

            file.write_all(log_entry.as_bytes())
                .map_err(|e| format!("Failed to write to log: {}", e))?;
        }

        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &AirgapConfig {
        &self.config
    }
}

/// Information about a validated transport
#[derive(Debug, Clone)]
pub struct TransportInfo {
    /// The detected transport type
    pub transport_type: TransportType,

    /// Normalized path (expanded variables, etc.)
    pub normalized_path: String,

    /// Whether this transport is allowed
    pub is_allowed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_detection_http() {
        let validator = AirgapValidator {
            config: AirgapConfig::default(),
        };

        assert_eq!(
            validator
                .detect_transport_type("http://example.com")
                .unwrap(),
            TransportType::Http
        );
        assert_eq!(
            validator
                .detect_transport_type("https://example.com")
                .unwrap(),
            TransportType::Http
        );
    }

    #[test]
    fn test_transport_detection_ssh() {
        let validator = AirgapValidator {
            config: AirgapConfig::default(),
        };

        assert_eq!(
            validator
                .detect_transport_type("ssh://server/repo")
                .unwrap(),
            TransportType::Ssh
        );
        assert_eq!(
            validator
                .detect_transport_type("scp://server/repo")
                .unwrap(),
            TransportType::Ssh
        );
    }

    #[test]
    fn test_transport_detection_lit() {
        let validator = AirgapValidator {
            config: AirgapConfig::default(),
        };

        assert_eq!(
            validator
                .detect_transport_type("lit://192.168.1.100/repo")
                .unwrap(),
            TransportType::LitProtocol
        );
    }

    #[test]
    fn test_transport_detection_network_share() {
        let validator = AirgapValidator {
            config: AirgapConfig::default(),
        };

        assert_eq!(
            validator
                .detect_transport_type(r"\\server\share\repo")
                .unwrap(),
            TransportType::NetworkShare
        );
        assert_eq!(
            validator
                .detect_transport_type("//server/share/repo")
                .unwrap(),
            TransportType::NetworkShare
        );
    }

    #[test]
    fn test_transport_detection_file_protocol() {
        let validator = AirgapValidator {
            config: AirgapConfig::default(),
        };

        assert_eq!(
            validator
                .detect_transport_type("file:///path/to/repo")
                .unwrap(),
            TransportType::FileProtocol
        );
    }

    #[test]
    fn test_airgap_blocks_network_protocols() {
        let mut config = AirgapConfig::default();
        config.enabled = true;
        let validator = AirgapValidator { config };

        // Should block HTTP
        assert!(validator.validate_transport("http://example.com").is_err());

        // Should block SSH
        assert!(validator.validate_transport("ssh://server/repo").is_err());

        // Should block lit:// protocol
        assert!(validator
            .validate_transport("lit://192.168.1.100/repo")
            .is_err());
    }

    #[test]
    fn test_airgap_allows_local_filesystem() {
        let mut config = AirgapConfig::default();
        config.enabled = true;
        let validator = AirgapValidator { config };

        // Should allow local paths
        assert!(validator.validate_transport("/path/to/repo").is_ok());
        assert!(validator.validate_transport("./relative/path").is_ok());
        assert!(validator.validate_transport("file:///path/to/repo").is_ok());
    }

    #[test]
    fn test_airgap_strict_mode_blocks_shares() {
        let mut config = AirgapConfig::default();
        config.enabled = true;
        config.strict_mode = true;
        let validator = AirgapValidator { config };

        // Should block network shares in strict mode
        assert!(validator.validate_transport(r"\\server\share").is_err());
    }

    #[test]
    fn test_path_normalization() {
        let validator = AirgapValidator {
            config: AirgapConfig::default(),
        };

        assert_eq!(
            validator.normalize_path("file:///tmp/test").unwrap(),
            "/tmp/test"
        );
    }
}
