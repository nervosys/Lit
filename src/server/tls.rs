//! TLS material for the self-hosted server.
//!
//! NIST SP 800-171r3 requirements this supports:
//!
//! - `03.13.08` transmission confidentiality — CUI is not sent in the clear
//! - `03.13.10` cryptographic key management — the private key is checked for
//!   owner-only permissions before it is loaded
//!
//! ## What this does not give you
//!
//! Lit terminates TLS with `rustls`, whose default cryptographic provider is
//! **not** a FIPS 140-3 validated module. `03.13.11` requires FIPS-validated
//! cryptography to protect CUI. Lit's *at-rest* cryptography goes through the
//! FIPS module in `crypto::fips`; its TLS does not.
//!
//! So: native TLS here is the right default for an internal deployment and a
//! large improvement on plaintext, but a system that must demonstrate
//! FIPS-validated cryptography in transit has to terminate TLS in a validated
//! module in front of Lit — see docs/SELF_HOSTING.md for that pattern. This is
//! stated in the code as well as the docs because the difference is invisible
//! from the outside: both arrangements serve `https://`.

use crate::crypto::encryption::restrict_to_owner;
use std::fs;
use std::path::{Path, PathBuf};

/// Paths to the server's TLS material.
#[derive(Debug, Clone)]
pub struct TlsPaths {
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}

/// Loaded PEM material, ready to hand to the HTTP server.
pub struct TlsMaterial {
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl TlsMaterial {
    /// Read and sanity-check the certificate and key.
    ///
    /// The checks here are deliberately shallow — this is not a certificate
    /// validator. They catch the mistakes that otherwise surface as an opaque
    /// handshake failure hours later: swapped paths, a DER file named `.pem`,
    /// an empty file, a key readable by every local account.
    pub fn load(paths: &TlsPaths) -> Result<Self, String> {
        let certificate = read_pem(&paths.certificate, "certificate")?;
        if !contains_pem_label(&certificate, "CERTIFICATE") {
            return Err(format!(
                "{} does not contain a PEM CERTIFICATE block — is this the private key?",
                paths.certificate.display()
            ));
        }

        // Tighten the key before reading it, and fail if that cannot be done.
        // A private key readable by other local accounts is a finding in its
        // own right, not a warning to carry on past.
        restrict_to_owner(&paths.private_key).map_err(|e| {
            format!(
                "Could not restrict permissions on {}: {}",
                paths.private_key.display(),
                e
            )
        })?;
        let private_key = read_pem(&paths.private_key, "private key")?;
        if contains_pem_label(&private_key, "CERTIFICATE") {
            return Err(format!(
                "{} contains a CERTIFICATE block, not a private key — the paths may be swapped",
                paths.private_key.display()
            ));
        }
        if !contains_pem_label(&private_key, "PRIVATE KEY") {
            return Err(format!(
                "{} does not contain a PEM PRIVATE KEY block",
                paths.private_key.display()
            ));
        }
        if contains_pem_label(&private_key, "ENCRYPTED PRIVATE KEY") {
            return Err(format!(
                "{} is an encrypted private key; Lit cannot prompt for its passphrase. \
                 Decrypt it into a file with owner-only permissions, or terminate TLS in a proxy.",
                paths.private_key.display()
            ));
        }

        Ok(TlsMaterial {
            certificate,
            private_key,
        })
    }
}

fn read_pem(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("Failed to read TLS {} {}: {}", what, path.display(), e))?;
    if bytes.is_empty() {
        return Err(format!("TLS {} {} is empty", what, path.display()));
    }
    Ok(bytes)
}

/// Whether the PEM text contains a `-----BEGIN <label>-----` header.
fn contains_pem_label(bytes: &[u8], label: &str) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.strip_prefix("-----BEGIN ")
                .and_then(|rest| rest.strip_suffix("-----"))
        })
        .any(|found| found == label || found.ends_with(label))
}

/// Whether an address is a loopback address.
///
/// This decides whether plaintext is tolerable: a request that never leaves the
/// host is not a transmission for the purposes of `03.13.08`, while anything
/// bound to a routable address is.
pub fn is_loopback(addr: &str) -> bool {
    let host = addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(addr);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    match host.parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_loopback(),
        Err(_) => host.eq_ignore_ascii_case("localhost"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const CERT: &str = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n";
    const KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIIE\n-----END PRIVATE KEY-----\n";

    /// `TlsMaterial` deliberately does not implement `Debug` — it holds the
    /// private key — so tests extract the error rather than using `unwrap_err`.
    fn load_err(paths: &TlsPaths) -> String {
        match TlsMaterial::load(paths) {
            Ok(_) => panic!("expected the material to be refused"),
            Err(e) => e,
        }
    }

    fn write(dir: &TempDir, name: &str, body: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn well_formed_material_loads() {
        let dir = TempDir::new().unwrap();
        let paths = TlsPaths {
            certificate: write(&dir, "cert.pem", CERT),
            private_key: write(&dir, "key.pem", KEY),
        };
        let material = TlsMaterial::load(&paths).unwrap();
        assert!(!material.certificate.is_empty());
        assert!(!material.private_key.is_empty());
    }

    #[test]
    fn swapped_paths_are_named_as_such() {
        let dir = TempDir::new().unwrap();
        let paths = TlsPaths {
            certificate: write(&dir, "cert.pem", KEY),
            private_key: write(&dir, "key.pem", CERT),
        };
        let err = load_err(&paths);
        assert!(err.contains("private key"), "unhelpful error: {}", err);
    }

    #[test]
    fn an_encrypted_key_is_refused_with_a_way_forward() {
        let dir = TempDir::new().unwrap();
        let encrypted =
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nMIIE\n-----END ENCRYPTED PRIVATE KEY-----\n";
        let paths = TlsPaths {
            certificate: write(&dir, "cert.pem", CERT),
            private_key: write(&dir, "key.pem", encrypted),
        };
        let err = load_err(&paths);
        assert!(err.contains("encrypted private key"));
        assert!(err.contains("proxy"));
    }

    #[test]
    fn an_empty_file_is_refused() {
        let dir = TempDir::new().unwrap();
        let paths = TlsPaths {
            certificate: write(&dir, "cert.pem", ""),
            private_key: write(&dir, "key.pem", KEY),
        };
        assert!(load_err(&paths).contains("empty"));
    }

    #[test]
    fn an_rsa_private_key_block_is_accepted() {
        let dir = TempDir::new().unwrap();
        let rsa = "-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----\n";
        let paths = TlsPaths {
            certificate: write(&dir, "cert.pem", CERT),
            private_key: write(&dir, "key.pem", rsa),
        };
        assert!(TlsMaterial::load(&paths).is_ok());
    }

    #[test]
    fn loopback_detection_covers_the_forms_an_operator_writes() {
        assert!(is_loopback("127.0.0.1:8080"));
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("localhost:8080"));
        assert!(is_loopback("[::1]:8080"));
        assert!(!is_loopback("0.0.0.0:8080"));
        assert!(!is_loopback("192.168.1.10:8080"));
        assert!(!is_loopback("lit.internal.example.com:8080"));
    }
}
