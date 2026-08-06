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
use std::io::IsTerminal;
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

    /// Keys already derived in this process, so a command that opens several
    /// stores pays PBKDF2 once rather than once per store. Memory-only.
    static ref DERIVED_KEYS: Mutex<HashMap<String, std::sync::Arc<EncryptionKey>>> = Mutex::new(HashMap::new());
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

/// Keep a file readable only by its owner.
///
/// Mode 0600 on Unix; on Windows a DACL granting the current user alone, which
/// is what closes finding I-1 in docs/SECURITY_AUDIT.md. Both are real
/// restrictions on reading, not just writing.
pub(crate) fn restrict_to_owner(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("Failed to read permissions: {}", e))?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to restrict permissions: {}", e))?;
    }

    #[cfg(windows)]
    windows_restrict_to_owner(path)?;

    Ok(())
}

/// Replace a file's DACL with one granting only the current user.
///
/// This is what closes finding I-1 in docs/SECURITY_AUDIT.md. The read-only
/// attribute that stood here before stops writes and does nothing about reads,
/// so any local account could read the file; Windows needs an explicit ACL.
///
/// The new DACL is marked protected, which detaches it from the parent
/// directory's inherited entries — otherwise an inherited "Users: Read" would
/// survive and the restriction would be for nothing.
#[cfg(windows)]
fn windows_restrict_to_owner(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL};
    use windows::Win32::Security::Authorization::{
        SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, SET_ACCESS, SE_FILE_OBJECT,
        TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
    };
    use windows::Win32::Security::{
        GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
        PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    // Win32 wants a NUL-terminated wide string.
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    unsafe {
        // The SID of whoever is running this.
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
            .map_err(|e| format!("Failed to open process token: {}", e))?;

        let mut needed = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut needed);
        let mut buffer = vec![0u8; needed as usize];
        let info_result = GetTokenInformation(
            token,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut _),
            needed,
            &mut needed,
        );
        let _ = CloseHandle(token);
        info_result.map_err(|e| format!("Failed to read token user: {}", e))?;

        let user_sid: PSID = (*(buffer.as_ptr() as *const TOKEN_USER)).User.Sid;

        // One entry: this user, full control, not inherited by anything.
        let access = EXPLICIT_ACCESS_W {
            grfAccessPermissions: 0x001F_01FF, // FILE_ALL_ACCESS
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: std::ptr::null_mut(),
                MultipleTrusteeOperation: Default::default(),
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: PWSTR(user_sid.0 as *mut u16),
            },
        };

        let mut acl: *mut ACL = std::ptr::null_mut();
        let entries = [access];
        let status = SetEntriesInAclW(Some(&entries), None, &mut acl);
        if status.is_err() {
            return Err(format!("Failed to build ACL: {:?}", status));
        }

        // PROTECTED detaches the file from inherited entries; without it the
        // parent directory's grants would remain in force.
        let status = SetNamedSecurityInfoW(
            PWSTR(wide.as_mut_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            PSID::default(),
            PSID::default(),
            Some(acl),
            None,
        );

        if !acl.is_null() {
            let _ = LocalFree(HLOCAL(acl as *mut _));
        }

        if status.is_err() {
            return Err(format!("Failed to set file DACL: {:?}", status));
        }

        let _ = PSECURITY_DESCRIPTOR::default();
    }

    Ok(())
}

/// Let a file be replaced by a rename, undoing what `restrict_to_owner` set.
///
/// Only Windows needs this: it refuses to rename onto a read-only file, and
/// the restriction applied on the previous save is exactly that. A missing
/// file is fine — there is nothing to clear.
fn allow_replacement(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        if path.exists() {
            let mut perms = fs::metadata(path)
                .map_err(|e| format!("Failed to read permissions: {}", e))?
                .permissions();
            // Clippy warns because clearing read-only on Unix makes a file
            // world-writable. This block is Windows-only, where the attribute
            // is not a permission at all and clearing it is what allows the
            // replacing rename.
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            fs::set_permissions(path, perms)
                .map_err(|e| format!("Failed to clear read-only attribute: {}", e))?;
        }
    }

    #[cfg(not(windows))]
    let _ = path;

    Ok(())
}

/// Identify a derived key by the file it came from and the passphrase that
/// unlocked it, without keeping the passphrase around.
///
/// Both parts matter: the file alone would hand back the wrong key after a
/// `rotate-key` within one process.
fn derived_key_id(key_file: &str, passphrase: &str) -> String {
    use sha3::{Digest, Sha3_256};
    let mut hasher = Sha3_256::new();
    hasher.update(key_file.as_bytes());
    hasher.update([0u8]); // keep the two fields from running together
    hasher.update(passphrase.as_bytes());
    hex::encode(hasher.finalize())
}

/// A key already derived in this process, if there is one.
fn cached_derived_key(id: &str) -> Option<std::sync::Arc<EncryptionKey>> {
    DERIVED_KEYS.lock().ok()?.get(id).cloned()
}

/// Remember a successfully derived key for the life of the process.
fn remember_derived_key(id: String, key: std::sync::Arc<EncryptionKey>) {
    if let Ok(mut keys) = DERIVED_KEYS.lock() {
        keys.insert(id, key);
    }
}

/// Where the failed-attempt count for a key file is kept between runs.
fn throttle_state_path(repo_path: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}.throttle", repo_path))
}

/// The throttle state as it is written to disk. Times are Unix seconds; a
/// `SystemTime` has no stable serialized form worth depending on here.
#[derive(Serialize, Deserialize, Default)]
struct PersistedThrottle {
    count: u32,
    last_attempt_secs: u64,
    lockout_until_secs: Option<u64>,
}

fn to_unix(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn from_unix(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

/// Read the stored attempt count, treating anything unreadable as a clean
/// slate. A corrupt or missing file must not lock a legitimate user out of
/// their own repository — the throttle exists to slow guessing, not to become
/// a way of denying access.
fn load_throttle(repo_path: &str) -> FailedAttemptTracker {
    let stored: Option<PersistedThrottle> = fs::read(throttle_state_path(repo_path))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok());

    match stored {
        Some(s) => FailedAttemptTracker {
            count: s.count,
            last_attempt: from_unix(s.last_attempt_secs),
            lockout_until: s.lockout_until_secs.map(from_unix),
        },
        None => FailedAttemptTracker {
            count: 0,
            last_attempt: SystemTime::now(),
            lockout_until: None,
        },
    }
}

/// Persist the attempt count so the next process sees it.
///
/// Best-effort: a repository on read-only media should still be usable, and
/// failing the operation because the throttle could not be written would turn
/// a hardening measure into an outage.
fn store_throttle(repo_path: &str, tracker: &FailedAttemptTracker) {
    let state = PersistedThrottle {
        count: tracker.count,
        last_attempt_secs: to_unix(tracker.last_attempt),
        lockout_until_secs: tracker.lockout_until.map(to_unix),
    };

    let path = throttle_state_path(repo_path);
    if let Ok(raw) = serde_json::to_vec(&state) {
        if fs::write(&path, raw).is_ok() {
            // The file says how many times someone has recently failed to open
            // this key, which is worth no more exposure than the key itself.
            let _ = restrict_to_owner(&path);
        }
    }
}

/// Check rate limit for passphrase attempts
/// Returns Ok(()) if attempt is allowed, Err with message if rate limited
///
/// The count lives on disk rather than in this process. Every `lit` command is
/// a new process, so an in-memory counter starts at zero for each one: a script
/// that reruns the binary was never slowed by the backoff and never reached the
/// five-attempt lockout at all, which is the case the throttle exists for.
///
/// This raises the cost of guessing; it does not stop an attacker who can
/// delete the state file. That is the same directory as the key file, so such
/// an attacker is already inside the boundary the throttle assumes — PBKDF2 at
/// 600k iterations remains the defence that does not depend on that assumption.
fn check_rate_limit(repo_path: &str) -> Result<(), String> {
    let _serialize = FAILED_ATTEMPTS
        .lock()
        .map_err(|_| "Internal error: rate-limit lock poisoned".to_string())?;
    let mut tracker = load_throttle(repo_path);

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
        store_throttle(repo_path, &tracker);
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
    let Ok(_serialize) = FAILED_ATTEMPTS.lock() else {
        return;
    };
    let mut tracker = load_throttle(repo_path);

    tracker.count += 1;
    tracker.last_attempt = SystemTime::now();

    // Lock out for 5 minutes after 5 failed attempts
    if tracker.count >= 5 {
        tracker.lockout_until = Some(SystemTime::now() + Duration::from_secs(300));
        eprintln!("Warning: Account locked due to multiple failed attempts. Locked for 5 minutes.");
    }

    store_throttle(repo_path, &tracker);
}

/// Clear failed attempt counter (called on successful authentication)
fn clear_failed_attempts(repo_path: &str) {
    if let Ok(_serialize) = FAILED_ATTEMPTS.lock() {
        // A correct passphrase clears the record, so an ordinary typo costs a
        // few seconds and nothing more once the user gets it right.
        let _ = fs::remove_file(throttle_state_path(repo_path));
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
        // SECURITY: Test bypass only available in test builds (FINDING-001)
        #[cfg(not(test))]
        validate_passphrase_strength(passphrase)?;
        #[cfg(test)]
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
        // SECURITY: Rate limit check — test bypass only in test builds (FINDING-001)
        let key_file_str = key_file.to_string_lossy().to_string();
        #[cfg(not(test))]
        check_rate_limit(&key_file_str)?;
        #[cfg(test)]
        if !passphrase.starts_with("test-") {
            check_rate_limit(&key_file_str)?;
        }

        if !key_file.exists() {
            return Err(
                "Encryption key file not found. Initialize repository with encryption first."
                    .to_string(),
            );
        }

        // Re-apply on load, not only at creation. The salt in this file is what
        // an offline brute force of the passphrase needs, and a key file written
        // before the restriction existed keeps its inherited ACL for the life of
        // the installation otherwise — the one on this machine dates to March.
        restrict_to_owner(key_file)?;

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
            // SECURITY: Record failed attempt — test bypass only in test builds (FINDING-001)
            #[cfg(not(test))]
            record_failed_attempt(&key_file_str);
            #[cfg(test)]
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
    /// SECURITY: Uses atomic write (temp file + rename) to prevent corruption
    /// on crash or power loss. Includes verification hash for passphrase validation.
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

        // Atomic write: write to temp file then rename to prevent corruption
        let temp_file = key_file.with_extension("tmp");
        fs::write(&temp_file, &data)
            .map_err(|e| format!("Failed to write temp key file: {}", e))?;

        // Restrict before the file takes its real name, so it is never briefly
        // readable under the path an attacker would watch.
        //
        // No key material is stored here — the key is derived from the
        // passphrase and this salt — but the verification hash lets anyone
        // holding the file test passphrase guesses offline, without needing the
        // repository at all. That is worth keeping to the owner.
        restrict_to_owner(&temp_file)?;

        // Windows refuses to rename onto a read-only file, and the file being
        // replaced is one this function marked read-only last time. Clearing
        // the attribute first is what lets `rotate-key` save a second time; a
        // test covers it, because the failure only appears on the second save.
        allow_replacement(key_file)?;

        fs::rename(&temp_file, key_file)
            .map_err(|e| format!("Failed to rename key file: {}", e))?;

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
        // Every AES-GCM operation in the crate goes through an engine, so this
        // is the one place that can promise the self-tests ran first no matter
        // which binary is driving. Runs once per process.
        crate::crypto::fips::ensure_self_tests()?;

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
        // Invocation limit for a random nonce (NIST SP 800-38D §8.3).
        //
        // The counter is per engine, so this bounds one process rather than the
        // lifetime of the key; a durable count would need state that survives
        // the command. It is a backstop, not the guarantee — `rotate-key`
        // remains the real control.
        let count = self.nonce_counter.fetch_add(1, Ordering::SeqCst);
        if count >= MAX_ENCRYPTIONS_PER_KEY {
            return Err(format!(
                "Encryption limit exceeded ({} operations). Key rotation required for security.",
                MAX_ENCRYPTIONS_PER_KEY
            ));
        }

        // Nonce: 96 random bits, the RBG-based construction of NIST SP 800-38D
        // §8.2.2, which is why the invocation limit above is 2^32.
        //
        // This was previously a counter in the top 8 bytes with 4 random bytes
        // after it, described as guaranteeing uniqueness. It did not: the
        // counter lives in the engine and restarts at zero for every engine —
        // every process, and every store or index opened within one — so the
        // first encryption after each start always reused counter 0 and only
        // those 4 random bytes stood between two nonces. Colliding 32 bits is
        // a birthday problem over roughly 65,000 encryptions, and a repeated
        // nonce under one AES-GCM key does not merely leak the XOR of the two
        // plaintexts, it exposes the GHASH key and with it forgery.
        //
        // 96 random bits put the same collision out of reach, and the nonce is
        // stored alongside the ciphertext, so data written under the old scheme
        // still decrypts.
        use aes_gcm::aead::rand_core::RngCore;
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
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
pub fn get_cached_passphrase(repo_path: &str) -> Option<Zeroizing<String>> {
    if let Ok(mut cache) = PASSPHRASE_CACHE.lock() {
        if let Some(entry) = cache.get(repo_path) {
            if entry.is_valid() {
                return Some(entry.passphrase.clone());
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
/// SECURITY: Returns Zeroizing<String> to ensure passphrase is cleared from memory.
fn get_passphrase_non_interactive(
    repo_path: &str,
    config: &EncryptionConfig,
) -> Option<Zeroizing<String>> {
    // 1. Check LIT_PASSPHRASE env var
    if let Ok(pass) = std::env::var("LIT_PASSPHRASE") {
        if !pass.is_empty() {
            return Some(Zeroizing::new(pass));
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
                return Some(Zeroizing::new(pass));
            }
        }
    }

    // 3. Check cache
    if config.cache_timeout_secs > 0 {
        if let Some(cached) = get_cached_passphrase(repo_path) {
            return Some(cached);
        }
    }

    // 4. Ask the agent, if one is running. This is the only source that spans
    //    commands — the cache above cannot, since each command is a new
    //    process. Deliberately last: an explicitly supplied passphrase should
    //    win over a stored one, so that overriding it does not require stopping
    //    the agent first.
    if let Some(from_agent) = crate::crypto::agent::get(repo_path) {
        return Some(from_agent);
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
) -> Result<Zeroizing<String>, String> {
    // Try non-interactive sources first
    if let Some(pass) = get_passphrase_non_interactive(repo_path, config) {
        return Ok(pass);
    }

    // Agent safety: never block on an interactive prompt when there is no TTY
    // (the default for agents, pipes, and CI). Fail fast with remediation.
    if !std::io::stdin().is_terminal() {
        return Err(
            "no passphrase available and no interactive terminal; set LIT_PASSPHRASE or \
             LIT_PASSPHRASE_FILE"
                .to_string(),
        );
    }

    // Fall back to interactive prompt
    rpassword::prompt_password(prompt_text)
        .map(Zeroizing::new)
        .map_err(|e| format!("Failed to read passphrase: {}", e))
}

/// Minimum passphrase length (NIST SP 800-63B recommendation for high security)
const MIN_PASSPHRASE_LENGTH: usize = 16;

/// Validate passphrase strength
///
/// Requirements:
/// - Minimum 16 characters (NIST SP 800-63B)
/// - At least 3 of: uppercase, lowercase, digits, special characters
fn validate_passphrase_strength(passphrase: &str) -> Result<(), String> {
    // SECURITY: Test bypass only available in test builds (FINDING-001)
    #[cfg(test)]
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
pub fn prompt_for_passphrase_confirmation(prompt_text: &str) -> Result<Zeroizing<String>, String> {
    // Check LIT_PASSPHRASE env var
    if let Ok(pass) = std::env::var("LIT_PASSPHRASE") {
        if !pass.is_empty() {
            validate_passphrase_strength(&pass)?;
            return Ok(Zeroizing::new(pass));
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
                return Ok(Zeroizing::new(pass));
            }
        }
    }

    // Agent safety: never block on an interactive prompt when there is no TTY.
    if !std::io::stdin().is_terminal() {
        return Err(
            "no passphrase available and no interactive terminal; set LIT_PASSPHRASE or \
             LIT_PASSPHRASE_FILE"
                .to_string(),
        );
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

    Ok(Zeroizing::new(pass1))
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

    /// Build a manager, initializing it from a non-interactive passphrase
    /// source when encryption is enabled and one is available.
    ///
    /// Every command builds its object store through `ObjectStore::new`, which
    /// returns `Self` rather than a `Result` and must not prompt — Lit is
    /// zero-prompt by design. So the passphrase comes from `LIT_PASSPHRASE`,
    /// `LIT_PASSPHRASE_FILE` or the cache, and nothing else.
    ///
    /// With encryption enabled and no source available the manager stays
    /// uninitialized on purpose: the first encrypt or decrypt then reports
    /// that plainly, which is a better failure than a constructor that cannot
    /// explain itself.
    pub fn new_auto(config: EncryptionConfig, repo_path: &Path) -> Self {
        let mut manager = EncryptionManager::new(config);
        if !manager.config.enabled {
            return manager;
        }

        let repo = repo_path.to_string_lossy().to_string();
        let Some(passphrase) = get_passphrase_non_interactive(&repo, &manager.config) else {
            return manager;
        };

        manager.repo_path = Some(repo.clone());
        if let Err(e) = manager.initialize(&passphrase) {
            eprintln!("Warning: encryption is enabled but could not be unlocked: {e}");
            return manager;
        }

        if manager.config.cache_timeout_secs > 0 {
            let timeout = Duration::from_secs(manager.config.cache_timeout_secs);
            cache_passphrase(&repo, (*passphrase).clone(), Some(timeout));
        }

        manager
    }

    /// Whether `data` carries our encryption header.
    ///
    /// Lets a reader tell ciphertext from content written before encryption
    /// was switched on, so a repository part-way through migration stays
    /// readable. Nothing we write in the clear begins with this byte: refs hold
    /// hex or `ref: `, the index holds JSON.
    pub fn is_encrypted_payload(data: &[u8]) -> bool {
        data.first() == Some(&ENCRYPTION_VERSION)
    }

    /// Initialize encryption with passphrase
    pub fn initialize(&mut self, passphrase: &str) -> Result<(), String> {
        if !self.config.enabled {
            return Ok(());
        }

        let expanded = shellexpand::tilde(&self.config.key_file);
        let key_file = Path::new(expanded.as_ref());

        // A command opens several stores — the object store, the index, and the
        // pack reader behind them — and each one lands here. Deriving the key
        // every time means paying PBKDF2's 600,000 iterations several times
        // over for a single `lit status`. Reuse a key already derived in this
        // process for the same file and passphrase.
        //
        // The cache is memory-only and dies with the process, so it widens no
        // window that holding the key for the length of one command already
        // opens. Only successful derivations are stored, so a wrong passphrase
        // still goes the long way round and still meets the rate limiter.
        let cache_id = derived_key_id(expanded.as_ref(), passphrase);
        if let Some(key) = cached_derived_key(&cache_id) {
            self.engine = Some(EncryptionEngine::new(&key)?);
            return Ok(());
        }

        // Load or create encryption key
        let key = if key_file.exists() {
            EncryptionKey::load(key_file, passphrase)?
        } else {
            let key = EncryptionKey::from_passphrase(passphrase, &EncryptionKey::generate_salt())?;
            key.save(&self.config.key_file, passphrase)?;
            key
        };

        let key = std::sync::Arc::new(key);
        remember_derived_key(cache_id, std::sync::Arc::clone(&key));

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
        let actual_passphrase: Zeroizing<String> = if let Some(pass) = passphrase {
            Zeroizing::new(pass.to_string())
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
            cache_passphrase(repo_path, (*actual_passphrase).clone(), Some(timeout));
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
            Some(engine) => {
                // Data written before encryption was switched on carries no
                // header of ours, so it fails here with a version number taken
                // from whatever byte happened to be first — 123 for the `{` of
                // the plaintext index, which explains nothing. Encryption
                // cannot be turned on for a repository that already has
                // content, and this is where a user finds that out.
                if encrypted
                    .first()
                    .is_some_and(|version| *version != ENCRYPTION_VERSION)
                {
                    return Err(
                        "This data has no Lit encryption header. Encryption cannot be \
                         enabled for a repository that already contains unencrypted \
                         commits — start a new encrypted repository and import into it."
                            .to_string(),
                    );
                }
                engine.decrypt(encrypted)
            }
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

    /// Serializes tests that mutate the process-global passphrase cache.
    ///
    /// Several tests call [`clear_passphrase_cache`], which wipes every entry;
    /// running them in parallel lets one test clear another's freshly-cached
    /// entry, producing spurious failures. Holding this lock makes those tests
    /// mutually exclusive. Poisoning is recovered from since a panic in one
    /// test must not cascade into the others.
    static CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn cache_test_guard() -> std::sync::MutexGuard<'static, ()> {
        CACHE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A key-file path belonging to a single test.
    ///
    /// Tests that exercise `EncryptionKey::save`/`load` write a real file, and
    /// the rate-limiter keys its failed-attempt tracker off that path. Pointing
    /// them at `~/.lit/encryption.key` therefore made them collide with one
    /// another — and, since they delete the file to start clean, destroyed the
    /// operator's real key on any run that included them. A per-test path in
    /// the temp directory isolates the file and the tracker together.
    fn test_key_path(label: &str) -> std::path::PathBuf {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "lit_enc_test_{}_{}_{}.key",
            std::process::id(),
            label,
            n
        ));
        let _ = fs::remove_file(&path);
        path
    }

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
        let _guard = cache_test_guard();
        let repo_path = "/tmp/test-repo";
        let passphrase = "cache-test-passphrase".to_string();

        // Clear cache first
        clear_passphrase_cache();

        // Should return None when not cached
        assert!(get_cached_passphrase(repo_path).is_none());

        // Cache passphrase with 5 second timeout
        cache_passphrase(repo_path, passphrase.clone(), Some(Duration::from_secs(5)));

        // Should retrieve cached passphrase
        assert_eq!(&*get_cached_passphrase(repo_path).unwrap(), &passphrase);

        // Clear specific entry
        clear_cached_passphrase(repo_path);
        assert!(get_cached_passphrase(repo_path).is_none());
    }

    #[test]
    fn test_passphrase_cache_expiration() {
        let _guard = cache_test_guard();
        let repo_path = "/tmp/test-repo-expire";
        let passphrase = "expire-test".to_string();

        clear_passphrase_cache();

        // Cache with a short timeout, then wait well past it and assert the
        // entry was evicted. This test deliberately avoids asserting immediate
        // availability — that behavior is covered by `test_passphrase_caching`,
        // and a tight "available right now" check would race the timeout under
        // heavy parallel CPU load. Asserting only expiration is robust: more
        // load can only make the entry *more* expired, never less.
        cache_passphrase(
            repo_path,
            passphrase.clone(),
            Some(Duration::from_millis(200)),
        );

        std::thread::sleep(Duration::from_millis(600));

        // Should be expired and removed
        assert!(get_cached_passphrase(repo_path).is_none());
    }

    #[test]
    fn test_passphrase_cache_multiple_repos() {
        let _guard = cache_test_guard();
        let repo1 = "/tmp/multi-cache-repo1";
        let repo2 = "/tmp/multi-cache-repo2";
        let pass1 = "password1".to_string();
        let pass2 = "password2".to_string();

        clear_passphrase_cache();

        // Cache different passphrases for different repos
        cache_passphrase(repo1, pass1.clone(), Some(Duration::from_secs(60)));
        cache_passphrase(repo2, pass2.clone(), Some(Duration::from_secs(60)));

        // Should retrieve correct passphrase for each repo
        assert_eq!(&*get_cached_passphrase(repo1).unwrap(), &pass1);
        assert_eq!(&*get_cached_passphrase(repo2).unwrap(), &pass2);
    }

    #[test]
    fn test_encryption_manager_with_cache() {
        use std::env;

        let _guard = cache_test_guard();

        let key_file = test_key_path("manager_cache");

        let temp_dir = env::temp_dir();
        let repo_path = temp_dir.join("test-cache-manager");
        let repo_str = repo_path.to_str().unwrap();

        clear_passphrase_cache();

        let config = EncryptionConfig {
            enabled: true,
            key_file: key_file.to_string_lossy().into_owned(),
            cache_timeout_secs: 300, // 5 minutes
            ..Default::default()
        };

        let mut manager = EncryptionManager::new(config);
        let passphrase = "test-cache-manager-pass";

        // Initialize with cache
        manager
            .initialize_with_cache(repo_str, Some(passphrase))
            .unwrap();

        // Passphrase should be cached
        assert_eq!(&*get_cached_passphrase(repo_str).unwrap(), passphrase);

        // Should be able to initialize again without providing passphrase
        let mut manager2 = EncryptionManager::new(manager.config.clone());
        manager2.initialize_with_cache(repo_str, None).unwrap();

        // Clear cache for cleanup
        clear_passphrase_cache();
        let _ = fs::remove_file(&key_file);
    }

    /// The throttle has to outlive the process that recorded the attempts.
    ///
    /// Every `lit` command is a new process. While the counter lived in a
    /// `static`, each one started at zero: the exponential backoff never grew
    /// past its first step and the five-attempt lockout could not be reached at
    /// all by a script that reran the binary, which is the case it exists for.
    /// Reading the state back from disk is what a second process does, so that
    /// is what this asserts.
    #[test]
    fn test_throttle_state_outlives_the_process() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("outlives.key");
        let key = key_path.to_string_lossy().into_owned();

        for _ in 0..5 {
            record_failed_attempt(&key);
        }

        // What a freshly started process would see.
        let seen = load_throttle(&key);
        assert_eq!(seen.count, 5, "the count should have survived on disk");
        assert!(
            seen.lockout_until.is_some(),
            "five failures should have produced a lockout a new process can see"
        );

        // And it should actually refuse, rather than merely recording a number.
        assert!(
            check_rate_limit(&key).is_err(),
            "a locked-out key should be refused"
        );

        // A correct passphrase clears it, so an ordinary typo is not sticky.
        clear_failed_attempts(&key);
        assert_eq!(load_throttle(&key).count, 0);
        assert!(check_rate_limit(&key).is_ok());
    }

    /// Corrupt or unreadable state must not lock the owner out of their own
    /// repository — the throttle slows guessing, it is not an access control.
    #[test]
    fn test_unreadable_throttle_state_is_treated_as_a_clean_slate() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("corrupt.key");
        let key = key_path.to_string_lossy().into_owned();

        fs::write(throttle_state_path(&key), b"this is not json").unwrap();

        assert_eq!(load_throttle(&key).count, 0);
        assert!(check_rate_limit(&key).is_ok());
    }

    /// Exercises the brute-force throttle on `EncryptionKey::load`.
    ///
    /// Ignored for runtime, not correctness: each attempt that gets as far as
    /// verification runs PBKDF2 at 600k iterations, which costs seconds in an
    /// unoptimized build. Run it with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn test_rate_limiting() {
        let key_file = test_key_path("rate_limiting");
        let key_file_str = key_file.to_string_lossy().into_owned();

        // A passphrase that does NOT start with "test-", so the test-only
        // bypass in `load` leaves the rate-limit check in play.
        let passphrase = "correct-passphrase-1234567890";
        let salt = EncryptionKey::generate_salt();
        let key = EncryptionKey::from_passphrase(passphrase, &salt).unwrap();
        key.save(&key_file_str, passphrase).unwrap();

        // A wrong passphrase is rejected on its merits, and counted.
        assert!(EncryptionKey::load(&key_file, "wrong-password-111111111111").is_err());

        // The next attempt falls inside the backoff window, so the throttle
        // turns it away before any verification happens. The throttle refuses
        // rather than sleeping, so the caller is told how long to wait instead
        // of having a thread parked on its behalf.
        // `unwrap_err` is avoided throughout: it would require `EncryptionKey`
        // to be `Debug`, and that type holds live key material.
        let start = std::time::Instant::now();
        let throttled = EncryptionKey::load(&key_file, "wrong-password-222222222222")
            .err()
            .expect("an attempt inside the backoff window must be refused");
        assert!(
            throttled.contains("wait"),
            "expected a rate-limit refusal, got: {}",
            throttled
        );
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "the throttle should refuse immediately rather than block the caller"
        );

        // Once the 2^1-second window passes, attempts are evaluated again — the
        // failure that comes back is about the passphrase, not the throttle.
        std::thread::sleep(Duration::from_millis(2_100));
        let correct = EncryptionKey::load(&key_file, passphrase);
        assert!(
            correct.is_ok(),
            "the correct passphrase should be accepted once the window passes: {:?}",
            correct.as_ref().err()
        );

        // Success clears the counter, so the next wrong attempt is judged on
        // its merits rather than being thrown out by the throttle.
        let after_reset = EncryptionKey::load(&key_file, "wrong-again-444444444444")
            .err()
            .expect("a wrong passphrase must still fail");
        assert!(
            !after_reset.contains("wait"),
            "a successful load should reset the counter, got: {}",
            after_reset
        );

        let _ = fs::remove_file(&key_file);
    }

    /// Nonces must not repeat across freshly created engines.
    ///
    /// The old construction put an engine-local counter in the top 8 bytes of
    /// the nonce, so every new engine — every process, every store opened —
    /// started again at zero and the first encryption always carried the same
    /// 8 leading bytes. Only 4 random bytes separated two such nonces, and a
    /// repeated nonce under one AES-GCM key is catastrophic. Simulate a run of
    /// separate processes and require the nonces to be distinct.
    #[test]
    fn test_nonces_do_not_repeat_across_engines() {
        let key = EncryptionKey::from_passphrase("NonceProbe!12345", &[7u8; SALT_SIZE]).unwrap();

        let mut nonces = std::collections::HashSet::new();
        let mut leading_zero_runs = 0;

        for _ in 0..64 {
            // A fresh engine each time, as a new process would build.
            let engine = EncryptionEngine::new(&key).unwrap();
            let blob = engine.encrypt(b"same plaintext every time").unwrap();
            let nonce = blob[1..1 + NONCE_SIZE].to_vec();

            if nonce[..8] == [0u8; 8] {
                leading_zero_runs += 1;
            }
            assert!(
                nonces.insert(nonce),
                "a nonce repeated across engines, which breaks AES-GCM"
            );
        }

        // Under the old scheme every one of these would have started 0x00 * 8.
        assert!(
            leading_zero_runs <= 1,
            "{} of 64 nonces began with eight zero bytes, which means the \
             counter is resetting rather than the nonce being random",
            leading_zero_runs
        );
    }
}

#[cfg(test)]
mod key_file_permission_tests {
    use super::*;

    /// Saving over an existing key file must keep working.
    ///
    /// The key file is restricted to its owner before being renamed into
    /// place. On Windows that restriction is the read-only attribute, and
    /// `fs::rename` onto a read-only destination is exactly what `rotate-key`
    /// does — so if it failed, rotation would break on the second save.
    #[test]
    fn test_key_file_can_be_saved_over() {
        let path = std::env::temp_dir().join(format!("lit_keyperm_{}.key", std::process::id()));
        let _ = fs::remove_file(&path);
        let path_str = path.to_string_lossy().to_string();

        let first =
            EncryptionKey::from_passphrase("FirstPassphrase!123", &[1u8; SALT_SIZE]).unwrap();
        first
            .save(&path_str, "FirstPassphrase!123")
            .expect("first save should succeed");

        let second =
            EncryptionKey::from_passphrase("SecondPassphrase!234", &[2u8; SALT_SIZE]).unwrap();
        second
            .save(&path_str, "SecondPassphrase!234")
            .expect("saving over an existing key file should succeed, as rotate-key does");

        // The second key's salt should be what is on disk now.
        let stored = fs::read(&path).unwrap();
        assert_eq!(
            &stored[..SALT_SIZE],
            &[2u8; SALT_SIZE],
            "the rewrite should have taken effect"
        );

        let _ = fs::remove_file(&path);
    }

    /// The restriction has to tighten a file that already exists, not only one
    /// this version created. Keys written before the restriction existed sit on
    /// disk with whatever the umask gave them, and only a call on the load path
    /// ever corrects them.
    #[test]
    fn test_restrict_to_owner_tightens_an_existing_permissive_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("preexisting.key");
        fs::write(&path, b"secret").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&path, perms).unwrap();

            restrict_to_owner(&path).unwrap();

            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "group and other should have lost all access");
        }

        #[cfg(windows)]
        restrict_to_owner(&path).unwrap();

        // Whatever the platform, the owner must still be able to read it back —
        // a restriction that locks out the process that applied it is a bug.
        assert_eq!(fs::read(&path).unwrap(), b"secret");
    }
}
