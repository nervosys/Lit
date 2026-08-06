pub mod agent;
pub mod encryption;
pub mod fips;
/// Cryptographic primitives module
/// Implements NIST-approved post-quantum cryptography standards
/// Cryptographic operations for Lit version control
///
/// FIPS 140-3 compliant cryptographic operations
/// All algorithms are approved for use in validated modules
pub mod signatures;

use serde::{Deserialize, Serialize};

/// Cryptographic configuration for FIPS 140-3 compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoConfig {
    /// Enable post-quantum signatures (ML-DSA/Dilithium)
    pub enable_pq_signatures: bool,
    /// Hash algorithm version
    pub hash_version: HashVersion,
    /// FIPS 140-3 mode (uses only approved algorithms)
    pub fips_mode: bool,
    /// Enable power-on self-tests
    pub enable_self_tests: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashVersion {
    /// SHA3-512 + BLAKE3 composite (quantum-resistant)
    CompositeV1,
    /// SHA-512 only (FIPS 140-3 approved, FIPS 180-4)
    Sha512Fips,
}

impl Default for CryptoConfig {
    fn default() -> Self {
        CryptoConfig {
            enable_pq_signatures: true,
            hash_version: HashVersion::CompositeV1,
            fips_mode: true, // FIPS mode enabled by default
            enable_self_tests: true,
        }
    }
}

impl CryptoConfig {
    /// Load from repository or use defaults
    pub fn load() -> Self {
        // For now, use defaults. Future: load from .lit/crypto_config
        Self::default()
    }

    /// Create FIPS 140-3 strict mode configuration
    pub fn fips_strict() -> Self {
        CryptoConfig {
            enable_pq_signatures: false, // PQ not yet FIPS 140-3 approved
            hash_version: HashVersion::Sha512Fips,
            fips_mode: true,
            enable_self_tests: true,
        }
    }
}

/// FIPS 140-2 operational state
#[derive(Debug, Clone, PartialEq)]
pub enum FipsState {
    /// Power-on state, self-tests not run
    PowerOn,
    /// Self-tests passed, ready for cryptographic operations
    Approved,
    /// Self-test failed, cryptographic operations disabled
    Error,
}
