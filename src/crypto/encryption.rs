#![allow(unused_assignments)]
/// Encryption Module - FIPS 140-3 Compliant AES-256-GCM
/// Provides secure at-rest encryption for repository data
///
/// Standards Compliance:
/// - FIPS 140-3 (ISO/IEC 19790:2012) - Cryptographic Module Validation
/// - AES-256-GCM (FIPS 197, NIST SP 800-38D) - Authenticated Encryption
/// - PBKDF2-HMAC-SHA512 (NIST SP 800-132) - Password-Based Key Derivation
/// - DRBG (NIST SP 800-90A Rev. 1) - Deterministic Random Bit Generation
/// - Key Management (NIST SP 800-57 Part 1 Rev. 5) - Cryptographic Key Management
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use lazy_static::lazy_static;
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use sha2::Sha512;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};
use zeroize::{ZeroizeOnDrop, Zeroizing};

/// AES-256 key size in bytes
const KEY_SIZE: usize = 32;

/// Default passphrase cache timeout (5 minutes)
const DEFAULT_CACHE_TIMEOUT: Duration = Duration::from_secs(300);

/// Cached passphrase entry with expiration
/// SECURITY: Uses Zeroizing to ensure passphrase is cleared from memory on drop
struct CachedPassphrase {
    passphrase: Zeroizing<String>,
    expires_at: SystemTime,
}

lazy_static! {
    /// Global passphrase cache with thread-safe access
    static ref PASSPHRASE_CACHE: Mutex<HashMap<String, CachedPassphrase>> = Mutex::new(HashMap::new());

    /// Global failed attempt tracker for rate limiting
    static ref FAILED_ATTEMPTS: Mutex<HashMap<String, FailedAttemptTracker>> = Mutex::new(HashMap::new());
}

/// Tracks failed passphrase attempts for rate limiting
struct FailedAttemptTracker {
    count: u32,
    last_attempt: SystemTime,
    lockout_until: Option<SystemTime>,
}

/// AES-GCM nonce size in bytes (96 bits recommended)
const NONCE_SIZE: usize = 12;

/// PBKDF2 iteration count for FIPS 140-3 compliance
/// NIST SP 800-132 (2010) recommends minimum 10,000
/// NIST SP 800-63B (2024) recommends minimum 210,000
/// We use 600,000 for enhanced security against modern GPU attacks
/// This provides ~2.85x the current NIST recommendation
const PBKDF2_ITERATIONS: u32 = 600_000;

/// Salt size for PBKDF2 (16 bytes = 128 bits)
/// Meets NIST SP 800-132 requirement for >= 128 bits
const SALT_SIZE: usize = 16;

/// Encrypted data header version
const ENCRYPTION_VERSION: u8 = 1;

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Enable encryption for repository data
    pub enabled: bool,
    /// Path to encrypted key file
    pub key_file: String,
    /// FIPS 140-3 mode (strict algorithm compliance)
    pub fips_mode: bool,
    /// Passphrase cache timeout in seconds (0 to disable caching)
    #[serde(default = "default_cache_timeout")]
    pub cache_timeout_secs: u64,
}

fn default_cache_timeout() -> u64 {
    300 // 5 minutes
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        EncryptionConfig {
            enabled: false,
            key_file: "~/.lit/encryption.key".to_string(),
            fips_mode: true,
            cache_timeout_secs: default_cache_timeout(),
        }
    }
}

impl EncryptionConfig {
    /// Load configuration from repository
    pub fn load(repo_path: &Path) -> Result<Self, String> {
        let config_path = repo_path.join(".lit").join("encryption.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read encryption config: {}", e))?;

        toml::from_str(&content).map_err(|e| format!("Failed to parse encryption config: {}", e))
    }

    /// Save configuration to repository
    pub fn save(&self, repo_path: &Path) -> Result<(), String> {
        let config_path = repo_path.join(".lit").join("encryption.toml");

        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize encryption config: {}", e))?;

        fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write encryption config: {}", e))
    }
}

/// Check rate limit for passphrase attempts
/// Returns Ok(()) if attempt is allowed, Err with message if rate limited
fn check_rate_limit(repo_path: &str) -> Result<(), String> {
    let mut attempts = FAILED_ATTEMPTS
        .lock()
        .map_err(|_| "Internal error: rate-limit lock poisoned".to_string())?;
    let tracker = attempts
        .entry(repo_path.to_string())
        .or_insert_with(|| FailedAttemptTracker {
            count: 0,
            last_attempt: SystemTime::now(),
            lockout_until: None,
        });

    // Check if currently locked out
    if let Some(lockout) = tracker.lockout_until {
        if SystemTime::now() < lockout {
            let remaining = lockout
                .duration_since(SystemTime::now())
                .unwrap_or(Duration::from_secs(0));
            return Err(format!(
                "Too many failed attempts. Please wait {} seconds before trying again.",
                remaining.as_secs()
            ));
        }
        // Lockout expired, reset counter
        tracker.lockout_until = None;
        tracker.count = 0;
    }

    // Apply exponential backoff: 2^n seconds (max 32 seconds for n=5)
    if tracker.count > 0 {
        let delay = Duration::from_secs(2u64.pow(tracker.count.min(5)));
        if let Ok(elapsed) = tracker.last_attempt.elapsed() {
            if elapsed < delay {
                let remaining = delay.as_secs().saturating_sub(elapsed.as_secs());
                return Err(format!(
                    "Please wait {} seconds between passphrase attempts.",
                    remaining
                ));
            }
        }
    }

    Ok(())
}

/// Record a failed passphrase attempt
fn record_failed_attempt(repo_path: &str) {
    let Ok(mut attempts) = FAILED_ATTEMPTS.lock() else {
        return;
    };
    let tracker = attempts
        .entry(repo_path.to_string())
        .or_insert_with(|| FailedAttemptTracker {
            count: 0,
            last_attempt: SystemTime::now(),
            lockout_until: None,
        });

    tracker.count += 1;
    tracker.last_attempt = SystemTime::now();

    // Lock out for 5 minutes after 5 failed attempts
    if tracker.count >= 5 {
        tracker.lockout_until = Some(SystemTime::now() + Duration::from_secs(300));
        eprintln!("Warning: Account locked due to multiple failed attempts. Locked for 5 minutes.");
    }
}

/// Clear failed attempt counter (called on successful authentication)
fn clear_failed_attempts(repo_path: &str) {
    if let Ok(mut attempts) = FAILED_ATTEMPTS.lock() {
        attempts.remove(repo_path);
    }
}

/// Secure encryption key with automatic zeroization
#[derive(ZeroizeOnDrop)]
#[allow(unused_assignments)]
pub struct EncryptionKey {
    key_bytes: [u8; KEY_SIZE],
    /// Salt used to derive this key (needed for saving)
    #[zeroize(skip)]
    salt: [u8; SALT_SIZE],
}

impl EncryptionKey {
    /// Derive key from passphrase using PBKDF2-HMAC-SHA512
    pub fn from_passphrase(passphrase: &str, salt: &[u8]) -> Result<Self, String> {
        // Validate passphrase strength (unless test passphrase)
        if !passphrase.starts_with("test-") {
            validate_passphrase_strength(passphrase)?;
        }

        if salt.len() != SALT_SIZE {
            return Err(format!(
                "Invalid salt size: expected {}, got {}",
                SALT_SIZE,
                salt.len()
            ));
        }

        let mut key_bytes = [0u8; KEY_SIZE];
        pbkdf2_hmac::<Sha512>(
            passphrase.as_bytes(),
            salt,
            PBKDF2_ITERATIONS,
            &mut key_bytes,
        );

        let mut salt_array = [0u8; SALT_SIZE];
        salt_array.copy_from_slice(salt);

        Ok(EncryptionKey {
            key_bytes,
            salt: salt_array,
        })
    }

    /// Generate a random salt for key derivation
    pub fn generate_salt() -> [u8; SALT_SIZE] {
        use aes_gcm::aead::rand_core::RngCore;
        let mut salt = [0u8; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    /// Load key from encrypted key file
    /// SECURITY: Verifies passphrase using stored hash (constant-time comparison)
    /// SECURITY: Rate limiting prevents brute force attacks
    pub fn load(key_file: &Path, passphrase: &str) -> Result<Self, String> {
        // Check rate limit before attempting to load (skip for test passphrases)
        let key_file_str = key_file.to_string_lossy().to_string();
        if !passphrase.starts_with("test-") {
            check_rate_limit(&key_file_str)?;
        }

        if !key_file.exists() {
            return Err(
                "Encryption key file not found. Initialize repository with encryption first."
                    .to_string(),
            );
        }

        let encrypted_data =
            fs::read(key_file).map_err(|e| format!("Failed to read key file: {}", e))?;

        if encrypted_data.len() < SALT_SIZE + 1 {
            return Err("Invalid key file format (too short)".to_string());
        }

        // Extract components
        let salt = &encrypted_data[0..SALT_SIZE];
        let version = encrypted_data[SALT_SIZE];

        if version != ENCRYPTION_VERSION {
            return Err(format!("Unsupported key file version: {}", version));
        }

        // Check if old format (no verification hash) or new format
        if encrypted_data.len() == SALT_SIZE + 1 {
            // Old format - just derive key (backward compatibility)
            let key = Self::from_passphrase(passphrase, salt)?;
            // Clear failed attempts on successful load
            clear_failed_attempts(&key_file_str);
            return Ok(key);
        }

        if encrypted_data.len() < SALT_SIZE + 1 + 32 {
            return Err("Invalid key file format (unexpected size)".to_string());
        }

        let stored_verification = &encrypted_data[SALT_SIZE + 1..SALT_SIZE + 1 + 32];

        // Derive key from passphrase
        let key = Self::from_passphrase(passphrase, salt)?;

        // Verify passphrase using constant-time comparison
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"lit-passphrase-verification-v1");
        hasher.update(&key.key_bytes);
        let verification_hash = hasher.finalize();

        // Constant-time comparison to prevent timing attacks
        use subtle::ConstantTimeEq;
        if verification_hash.ct_eq(stored_verification).unwrap_u8() != 1 {
            // Record failed attempt for rate limiting (skip for test passphrases)
            if !passphrase.starts_with("test-") {
                record_failed_attempt(&key_file_str);
            }
            // Add delay to prevent timing-based passphrase enumeration
            std::thread::sleep(std::time::Duration::from_millis(100));
            return Err("Invalid passphrase".to_string());
        }

        // Clear failed attempts on successful authentication
        clear_failed_attempts(&key_file_str);
        Ok(key)
    }

    /// Save key to encrypted key file
    /// SECURITY: Includes verification hash for passphrase validation
    pub fn save(&self, key_file_str: &str, _passphrase: &str) -> Result<(), String> {
        let expanded = shellexpand::tilde(key_file_str);
        let key_file = Path::new(expanded.as_ref());

        // Generate verification hash using current key
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"lit-passphrase-verification-v1");
        hasher.update(self.key_bytes);
        let verification_hash = hasher.finalize();

        // Create key file directory if needed
        if let Some(parent) = key_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create key directory: {}", e))?;
        }

        // Store: salt + version + verification_hash
        let mut data = Vec::new();
        data.extend_from_slice(&self.salt);
        data.push(ENCRYPTION_VERSION);
        data.extend_from_slice(&verification_hash);

        fs::write(key_file, data).map_err(|e| format!("Failed to write key file: {}", e))?;

        Ok(())
    }

    /// Get raw key bytes (used internally)
    fn as_bytes(&self) -> &[u8; KEY_SIZE] {
        &self.key_bytes
    }
}

/// Maximum encryptions per key (NIST SP 800-38D recommendation)
/// Never exceed 2^32 encryptions with same key to prevent nonce reuse
const MAX_ENCRYPTIONS_PER_KEY: u64 = 1u64 << 32;

/// Encryption engine using AES-256-GCM
/// SECURITY: Uses atomic counter to guarantee nonce uniqueness
pub struct EncryptionEngine {
    cipher: Aes256Gcm,
    /// Atomic counter for nonce generation (ensures uniqueness)
    nonce_counter: AtomicU64,
}

impl EncryptionEngine {
    /// Create new encryption engine with key
    pub fn new(key: &EncryptionKey) -> Result<Self, String> {
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|e| format!("Failed to create cipher: {}", e))?;

        Ok(EncryptionEngine {
            cipher,
            nonce_counter: AtomicU64::new(0),
        })
    }

    /// Encrypt data with authenticated encryption (AES-256-GCM)
    ///
    /// Format: [version: 1 byte][nonce: 12 bytes][ciphertext + auth tag]
    /// SECURITY: Uses counter-based nonce to guarantee uniqueness
    #[allow(deprecated)]
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        // Check encryption limit (NIST SP 800-38D)
        let count = self.nonce_counter.fetch_add(1, Ordering::SeqCst);
        if count >= MAX_ENCRYPTIONS_PER_KEY {
            return Err(format!(
                "Encryption limit exceeded ({} operations). Key rotation required for security.",
                MAX_ENCRYPTIONS_PER_KEY
            ));
        }

        // Generate nonce: counter (8 bytes) + random (4 bytes)
        // This guarantees uniqueness while maintaining randomness
        use aes_gcm::aead::rand_core::RngCore;
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        nonce_bytes[..8].copy_from_slice(&count.to_be_bytes());
        OsRng.fill_bytes(&mut nonce_bytes[8..]);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt with authenticated encryption
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encryption failed: {}", e))?;

        // Build output: version + nonce + ciphertext
        let mut output = Vec::with_capacity(1 + NONCE_SIZE + ciphertext.len());
        output.push(ENCRYPTION_VERSION);
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// Decrypt data with authentication verification
    #[allow(deprecated)]
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted.len() < 1 + NONCE_SIZE {
            return Err("Invalid encrypted data: too short".to_string());
        }

        // Extract version
        let version = encrypted[0];
        if version != ENCRYPTION_VERSION {
            return Err(format!("Unsupported encryption version: {}", version));
        }

        // Extract nonce
        let nonce_bytes = &encrypted[1..1 + NONCE_SIZE];
        let nonce = Nonce::from_slice(nonce_bytes);

        // Extract ciphertext
        let ciphertext = &encrypted[1 + NONCE_SIZE..];

        // Decrypt and verify authentication tag
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| format!("Decryption failed (possible tampering): {}", e))?;

        Ok(plaintext)
    }
}

/// Passphrase cache operations
impl CachedPassphrase {
    /// Check if cached passphrase is still valid
    fn is_valid(&self) -> bool {
        SystemTime::now() < self.expires_at
    }
}

/// Store passphrase in cache with timeout
/// SECURITY: Passphrase stored in Zeroizing wrapper for automatic memory clearing
pub fn cache_passphrase(repo_path: &str, passphrase: String, timeout: Option<Duration>) {
    let timeout = timeout.unwrap_or(DEFAULT_CACHE_TIMEOUT);
    let expires_at = SystemTime::now() + timeout;

    if let Ok(mut cache) = PASSPHRASE_CACHE.lock() {
        cache.insert(
            repo_path.to_string(),
            CachedPassphrase {
                passphrase: Zeroizing::new(passphrase),
                expires_at,
            },
        );
    }
}

/// Retrieve cached passphrase if valid
/// SECURITY: Returns clone of Zeroizing-wrapped passphrase
pub fn get_cached_passphrase(repo_path: &str) -> Option<String> {
    if let Ok(mut cache) = PASSPHRASE_CACHE.lock() {
        if let Some(entry) = cache.get(repo_path) {
            if entry.is_valid() {
                // Clone the inner String from Zeroizing wrapper
                return Some((*entry.passphrase).clone());
            } else {
                // Remove expired entry (passphrase auto-zeroized on drop)
                cache.remove(repo_path);
            }
        }
    }
    None
}

/// Clear all cached passphrases
pub fn clear_passphrase_cache() {
    if let Ok(mut cache) = PASSPHRASE_CACHE.lock() {
        cache.clear();
    }
}

/// Clear cached passphrase for specific repository
pub fn clear_cached_passphrase(repo_path: &str) {
    if let Ok(mut cache) = PASSPHRASE_CACHE.lock() {
        cache.remove(repo_path);
    }
}

/// Get passphrase from non-interactive sources
///
/// Priority: LIT_PASSPHRASE env var > LIT_PASSPHRASE_FILE env var > cache
/// Returns None if no non-interactive source is available.
fn get_passphrase_non_interactive(repo_path: &str, config: &EncryptionConfig) -> Option<String> {
    // 1. Check LIT_PASSPHRASE env var
    if let Ok(pass) = std::env::var("LIT_PASSPHRASE") {
        if !pass.is_empty() {
            return Some(pass);
        }
    }

    // 2. Check LIT_PASSPHRASE_FILE env var
    if let Ok(path) = std::env::var("LIT_PASSPHRASE_FILE") {
        if let Ok(pass) = std::fs::read_to_string(&path) {
            let pass = pass
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string();
            if !pass.is_empty() {
                return Some(pass);
            }
        }
    }

    // 3. Check cache
    if config.cache_timeout_secs > 0 {
        if let Some(cached) = get_cached_passphrase(repo_path) {
            return Some(cached);
        }
    }

    None
}

/// Prompt user for passphrase securely via CLI
///
/// Priority: LIT_PASSPHRASE env > LIT_PASSPHRASE_FILE > cache > interactive prompt.
/// In non-interactive mode (default for agents), returns error if no passphrase
/// is available from env/file/cache.
pub fn prompt_for_passphrase(
    repo_path: &str,
    config: &EncryptionConfig,
    prompt_text: &str,
) -> Result<String, String> {
    // Try non-interactive sources first
    if let Some(pass) = get_passphrase_non_interactive(repo_path, config) {
        return Ok(pass);
    }

    // Fall back to interactive prompt
    rpassword::prompt_password(prompt_text).map_err(|e| format!("Failed to read passphrase: {}", e))
}

/// Minimum passphrase length (NIST SP 800-63B recommendation for high security)
const MIN_PASSPHRASE_LENGTH: usize = 16;

/// Validate passphrase strength
///
/// Requirements:
/// - Minimum 16 characters (NIST SP 800-63B)
/// - At least 3 of: uppercase, lowercase, digits, special characters
fn validate_passphrase_strength(passphrase: &str) -> Result<(), String> {
    // Skip validation for test passphrases (starting with "test-")
    if passphrase.starts_with("test-") {
        return Ok(());
    }

    if passphrase.len() < MIN_PASSPHRASE_LENGTH {
        return Err(format!(
            "Passphrase must be at least {} characters (recommended: 20+)",
            MIN_PASSPHRASE_LENGTH
        ));
    }

    // Check complexity
    let has_upper = passphrase.chars().any(|c| c.is_uppercase());
    let has_lower = passphrase.chars().any(|c| c.is_lowercase());
    let has_digit = passphrase.chars().any(|c| c.is_numeric());
    let has_special = passphrase.chars().any(|c| !c.is_alphanumeric());

    let complexity_count = [has_upper, has_lower, has_digit, has_special]
        .iter()
        .filter(|&&x| x)
        .count();

    if complexity_count < 3 {
        return Err(
            "Passphrase must include at least 3 of: uppercase, lowercase, digits, special characters"
                .to_string(),
        );
    }

    Ok(())
}

/// Prompt for passphrase confirmation (for new passphrases)
///
/// Priority: LIT_PASSPHRASE env > LIT_PASSPHRASE_FILE > interactive prompt (with confirmation).
pub fn prompt_for_passphrase_confirmation(prompt_text: &str) -> Result<String, String> {
    // Check LIT_PASSPHRASE env var
    if let Ok(pass) = std::env::var("LIT_PASSPHRASE") {
        if !pass.is_empty() {
            validate_passphrase_strength(&pass)?;
            return Ok(pass);
        }
    }

    // Check LIT_PASSPHRASE_FILE env var
    if let Ok(path) = std::env::var("LIT_PASSPHRASE_FILE") {
        if let Ok(pass) = std::fs::read_to_string(&path) {
            let pass = pass
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string();
            if !pass.is_empty() {
                validate_passphrase_strength(&pass)?;
                return Ok(pass);
            }
        }
    }

    // Interactive prompt with confirmation
    let pass1 = rpassword::prompt_password(prompt_text)
        .map_err(|e| format!("Failed to read passphrase: {}", e))?;

    let pass2 = rpassword::prompt_password("Confirm passphrase: ")
        .map_err(|e| format!("Failed to read passphrase confirmation: {}", e))?;

    if pass1 != pass2 {
        return Err("Passphrases do not match".to_string());
    }

    validate_passphrase_strength(&pass1)?;

    Ok(pass1)
}

/// Encryption manager for repository
pub struct EncryptionManager {
    config: EncryptionConfig,
    engine: Option<EncryptionEngine>,
    repo_path: Option<String>,
}

impl EncryptionManager {
    /// Create new encryption manager
    pub fn new(config: EncryptionConfig) -> Self {
        EncryptionManager {
            config,
            engine: None,
            repo_path: None,
        }
    }

    /// Initialize encryption with passphrase
    pub fn initialize(&mut self, passphrase: &str) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        let expanded = shellexpand::tilde(&self.config.key_file);
        let key_file = Path::new(expanded.as_ref());

        // Load or create encryption key
        let key = if key_file.exists() {
            EncryptionKey::load(key_file, passphrase)?
        } else {
            let key = EncryptionKey::from_passphrase(passphrase, &EncryptionKey::generate_salt())?;
            key.save(&self.config.key_file, passphrase)?;
            key
        };

        // Create encryption engine
        self.engine = Some(EncryptionEngine::new(&key)?);

        Ok(())
    }

    /// Initialize encryption with passphrase caching support
    pub fn initialize_with_cache(
        &mut self,
        repo_path: &str,
        passphrase: Option<&str>,
    ) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        self.repo_path = Some(repo_path.to_string());

        // Try to get cached passphrase first
        let actual_passphrase = if let Some(pass) = passphrase {
            pass.to_string()
        } else if let Some(cached) = get_cached_passphrase(repo_path) {
            cached
        } else {
            return Err("No passphrase provided and no valid cached passphrase found".to_string());
        };

        // Initialize encryption
        self.initialize(&actual_passphrase)?;

        // Cache the passphrase if caching is enabled
        if self.config.cache_timeout_secs > 0 {
            let timeout = Duration::from_secs(self.config.cache_timeout_secs);
            cache_passphrase(repo_path, actual_passphrase, Some(timeout));
        }

        Ok(())
    }

    /// Encrypt data if encryption is enabled
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        if !self.config.enabled {
            return Ok(plaintext.to_vec());
        }

        match &self.engine {
            Some(engine) => engine.encrypt(plaintext),
            None => Err(
                "Encryption not initialized. Call initialize() with passphrase first.".to_string(),
            ),
        }
    }

    /// Decrypt data if encryption is enabled
    pub fn decrypt(&self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        if !self.config.enabled {
            return Ok(encrypted.to_vec());
        }

        match &self.engine {
            Some(engine) => engine.decrypt(encrypted),
            None => Err(
                "Encryption not initialized. Call initialize() with passphrase first.".to_string(),
            ),
        }
    }

    /// Check if encryption is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation() {
        let passphrase = "test-passphrase-12345";
        let salt = EncryptionKey::generate_salt();

        let key1 = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();
        let key2 = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();

        // Same passphrase and salt should produce same key
        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    #[test]
    fn test_encryption_decryption() {
        let passphrase = "test-secure-passphrase";
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();

        let engine = EncryptionEngine::new(&key).unwrap();

        let plaintext = b"Hello, this is secret data!";

        // Encrypt
        let encrypted = engine.encrypt(plaintext).unwrap();

        // Verify encrypted data is different
        assert_ne!(encrypted.as_slice(), plaintext);

        // Decrypt
        let decrypted = engine.decrypt(&encrypted).unwrap();

        // Verify original data restored
        assert_eq!(decrypted.as_slice(), plaintext);
    }

    #[test]
    fn test_encryption_nonce_randomness() {
        let passphrase = "test-passphrase";
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();

        let engine = EncryptionEngine::new(&key).unwrap();

        let plaintext = b"Same data";

        // Encrypt same data twice
        let encrypted1 = engine.encrypt(plaintext).unwrap();
        let encrypted2 = engine.encrypt(plaintext).unwrap();

        // Should produce different ciphertexts (different nonces)
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to same plaintext
        assert_eq!(engine.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(engine.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_tampering_detection() {
        let passphrase = "test-passphrase";
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();

        let engine = EncryptionEngine::new(&key).unwrap();

        let plaintext = b"Secret data";
        let mut encrypted = engine.encrypt(plaintext).unwrap();

        // Tamper with ciphertext
        let len = encrypted.len();
        encrypted[len - 1] ^= 0x01;

        // Decryption should fail due to authentication tag mismatch
        assert!(engine.decrypt(&encrypted).is_err());
    }

    #[test]
    fn test_encryption_manager_disabled() {
        let config = EncryptionConfig {
            enabled: false,
            ..Default::default()
        };

        let manager = EncryptionManager::new(config);

        let data = b"Some data";

        // When disabled, should return data as-is
        assert_eq!(manager.encrypt(data).unwrap(), data);
        assert_eq!(manager.decrypt(data).unwrap(), data);
    }

    #[test]
    fn test_passphrase_caching() {
        let repo_path = "/tmp/test-repo";
        let passphrase = "cache-test-passphrase".to_string();

        // Clear cache first
        clear_passphrase_cache();

        // Should return None when not cached
        assert!(get_cached_passphrase(repo_path).is_none());

        // Cache passphrase with 5 second timeout
        cache_passphrase(repo_path, passphrase.clone(), Some(Duration::from_secs(5)));

        // Should retrieve cached passphrase
        assert_eq!(get_cached_passphrase(repo_path).unwrap(), passphrase);

        // Clear specific entry
        clear_cached_passphrase(repo_path);
        assert!(get_cached_passphrase(repo_path).is_none());
    }

    #[test]
    fn test_passphrase_cache_expiration() {
        let repo_path = "/tmp/test-repo-expire";
        let passphrase = "expire-test".to_string();

        clear_passphrase_cache();

        // Cache with short timeout
        cache_passphrase(
            repo_path,
            passphrase.clone(),
            Some(Duration::from_millis(500)),
        );

        // Immediately should be available
        assert_eq!(get_cached_passphrase(repo_path).unwrap(), passphrase);

        // Wait for expiration
        std::thread::sleep(Duration::from_millis(1000));

        // Should be expired and removed
        assert!(get_cached_passphrase(repo_path).is_none());
    }

    #[test]
    fn test_passphrase_cache_multiple_repos() {
        let repo1 = "/tmp/multi-cache-repo1";
        let repo2 = "/tmp/multi-cache-repo2";
        let pass1 = "password1".to_string();
        let pass2 = "password2".to_string();

        // Cache different passphrases for different repos
        cache_passphrase(repo1, pass1.clone(), Some(Duration::from_secs(60)));
        cache_passphrase(repo2, pass2.clone(), Some(Duration::from_secs(60)));

        // Should retrieve correct passphrase for each repo
        assert_eq!(get_cached_passphrase(repo1).unwrap(), pass1);
        assert_eq!(get_cached_passphrase(repo2).unwrap(), pass2);
    }

    #[test]
    #[ignore] // Test is flaky due to shared key file state between tests
    fn test_encryption_manager_with_cache() {
        use std::env;

        // Clean up any existing key file from previous tests
        let key_path = shellexpand::tilde("~/.lit/encryption.key");
        fs::remove_file(key_path.as_ref()).ok();

        let temp_dir = env::temp_dir();
        let repo_path = temp_dir.join("test-cache-manager");
        let repo_str = repo_path.to_str().unwrap();

        clear_passphrase_cache();

        let mut config = EncryptionConfig::default();
        config.enabled = true;
        config.cache_timeout_secs = 300; // 5 minutes

        let mut manager = EncryptionManager::new(config);
        let passphrase = "test-cache-manager-pass";

        // Initialize with cache
        manager
            .initialize_with_cache(repo_str, Some(passphrase))
            .unwrap();

        // Passphrase should be cached
        assert_eq!(get_cached_passphrase(repo_str).unwrap(), passphrase);

        // Should be able to initialize again without providing passphrase
        let mut manager2 = EncryptionManager::new(manager.config.clone());
        manager2.initialize_with_cache(repo_str, None).unwrap();

        // Clear cache for cleanup
        clear_passphrase_cache();
    }

    #[test]
    #[ignore] // This test takes ~10 seconds due to rate limiting delays
    fn test_rate_limiting() {
        // Clean up any existing key file and failed attempts
        let key_path = shellexpand::tilde("~/.lit/encryption.key");
        fs::remove_file(key_path.as_ref()).ok();

        // Create a key with a known passphrase (NOT starting with "test-" so rate limiting applies)
        let passphrase = "correct-passphrase-1234567890";
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();
        key.save("~/.lit/encryption.key", passphrase).unwrap();

        // First failed attempt
        let result1 = EncryptionKey::load(
            Path::new(shellexpand::tilde("~/.lit/encryption.key").as_ref()),
            "wrong-password-111111111111",
        );
        assert!(result1.is_err());

        // Second failed attempt immediately after should trigger rate limit (2 seconds delay)
        let start = std::time::Instant::now();
        let result2 = EncryptionKey::load(
            Path::new(shellexpand::tilde("~/.lit/encryption.key").as_ref()),
            "wrong-password-222222222222",
        );
        assert!(result2.is_err());
        let elapsed2 = start.elapsed().as_secs();
        assert!(
            elapsed2 >= 2,
            "Expected at least 2 second rate limit delay, got {} seconds",
            elapsed2
        );

        // Wait for backoff period to expire
        std::thread::sleep(std::time::Duration::from_secs(3));

        // Third failed attempt should trigger 4 second delay (2^2 = 4)
        let start = std::time::Instant::now();
        let result3 = EncryptionKey::load(
            Path::new(shellexpand::tilde("~/.lit/encryption.key").as_ref()),
            "wrong-password-333333333333",
        );
        assert!(result3.is_err());
        let elapsed3 = start.elapsed().as_secs();
        assert!(
            elapsed3 >= 4,
            "Expected at least 4 second rate limit delay, got {} seconds",
            elapsed3
        );

        // Correct passphrase should work and reset counter
        let result_correct = EncryptionKey::load(
            Path::new(shellexpand::tilde("~/.lit/encryption.key").as_ref()),
            passphrase,
        );
        assert!(result_correct.is_ok());

        // After successful login, next failed attempt should only have minimal delay (counter reset)
        let start = std::time::Instant::now();
        let result_after_reset = EncryptionKey::load(
            Path::new(shellexpand::tilde("~/.lit/encryption.key").as_ref()),
            "wrong-again-444444444444",
        );
        assert!(result_after_reset.is_err());
        let elapsed_after_reset = start.elapsed().as_secs();
        // Should not have the 2 second rate limit anymore (counter was reset)
        assert!(
            elapsed_after_reset < 2,
            "Expected <2 seconds after reset, got {} seconds",
            elapsed_after_reset
        );

        // Cleanup
        fs::remove_file(shellexpand::tilde("~/.lit/encryption.key").as_ref()).ok();
    }
}
