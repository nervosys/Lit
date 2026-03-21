/// Post-quantum digital signatures using NIST ML-DSA
/// NIST FIPS 204 - Module-Lattice-Based Digital Signature Standard
use pqcrypto_mldsa::mldsa87;
use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};
use serde::{Deserialize, Serialize};

/// Post-quantum signature using ML-DSA-87 (FIPS 204)
/// ML-DSA-87 provides NIST security level 5 (highest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQSignature {
    /// The signature bytes
    pub signature: Vec<u8>,
    /// Public key for verification
    pub public_key: Vec<u8>,
    /// Algorithm identifier
    pub algorithm: String,
}

/// Post-quantum keypair
pub struct PQKeyPair {
    pub public_key: mldsa87::PublicKey,
    pub secret_key: mldsa87::SecretKey,
}

impl PQKeyPair {
    /// Generate a new quantum-resistant keypair
    pub fn generate() -> Self {
        let (pk, sk) = mldsa87::keypair();
        PQKeyPair {
            public_key: pk,
            secret_key: sk,
        }
    }

    /// Sign a message with post-quantum signature
    pub fn sign(&self, message: &[u8]) -> PQSignature {
        let sig = mldsa87::detached_sign(message, &self.secret_key);

        PQSignature {
            signature: sig.as_bytes().to_vec(),
            public_key: self.public_key.as_bytes().to_vec(),
            algorithm: "ML-DSA-87".to_string(),
        }
    }
}

impl PQSignature {
    /// Verify a post-quantum signature
    pub fn verify(&self, message: &[u8]) -> Result<(), String> {
        // Reconstruct public key
        let pk =
            mldsa87::PublicKey::from_bytes(&self.public_key).map_err(|_| "Invalid public key")?;

        // Reconstruct signature
        let sig = mldsa87::DetachedSignature::from_bytes(&self.signature)
            .map_err(|_| "Invalid signature")?;

        // Verify
        mldsa87::verify_detached_signature(&sig, message, &pk)
            .map_err(|_| "Signature verification failed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pq_signature() {
        let keypair = PQKeyPair::generate();
        let message = b"Test commit message";

        let signature = keypair.sign(message);
        assert!(signature.verify(message).is_ok());

        // Verify wrong message fails
        let wrong_message = b"Wrong message";
        assert!(signature.verify(wrong_message).is_err());
    }
}
