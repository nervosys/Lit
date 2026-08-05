/// Command tests for at-rest encryption
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1`.
use std::fs;
use std::sync::atomic::{AtomicU32, Ordering};
use tempfile::TempDir;

static COUNTER: AtomicU32 = AtomicU32::new(0);

const PASSPHRASE: &str = "CorrectHorseBattery!99";
const SECRET: &[u8] = b"TOP-SECRET-PAYLOAD-4242";

/// A repository with at-rest encryption switched on from the start.
///
/// Returns the repo and the key-file path, which is per-test so concurrent
/// tests neither share a key nor a rate-limit counter.
fn encrypted_repo() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();

    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let key = std::env::temp_dir().join(format!("lit_enc_test_{}_{}.key", std::process::id(), n));
    let _ = fs::remove_file(&key);

    fs::write(
        temp.path().join(".lit").join("encryption.toml"),
        format!(
            "enabled = true\nkey_file = \"{}\"\nfips_mode = false\ncache_timeout_secs = 0\n",
            key.to_string_lossy().replace(char::from(92u8), "/")
        ),
    )
    .unwrap();

    // `init` wrote a plaintext index before encryption was configured. Drop it
    // so the repository starts encrypted rather than half-converted.
    let _ = fs::remove_file(temp.path().join(".lit").join("index"));

    (temp, key)
}

/// Every file under `.lit` that contains the secret in the clear.
fn plaintext_leaks(repo: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(repo.join(".lit"))
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            if let Ok(bytes) = fs::read(entry.path()) {
                if bytes.windows(SECRET.len()).any(|w| w == SECRET) {
                    found.push(entry.path().to_path_buf());
                }
            }
        }
    }
    found
}

#[test]
fn test_encrypted_repository_never_writes_plaintext() {
    let (temp, key) = encrypted_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());
    std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);

    fs::write(temp.path().join("s.txt"), SECRET).unwrap();
    lit::commands::add::execute(vec!["s.txt".to_string()]).unwrap();
    lit::commands::commit::execute("encrypted".to_string(), None).unwrap();

    assert!(
        plaintext_leaks(temp.path()).is_empty(),
        "loose objects leaked plaintext: {:?}",
        plaintext_leaks(temp.path())
    );

    // Packing must not undo the encryption. write_pack goes through the same
    // manager the loose objects did, so the pack is ciphertext too.
    lit::commands::gc::execute().unwrap();
    assert!(
        plaintext_leaks(temp.path()).is_empty(),
        "gc leaked plaintext into the pack: {:?}",
        plaintext_leaks(temp.path())
    );

    // And it is all still readable through the pack.
    assert!(lit::commands::status::execute().is_ok(), "status after gc");
    assert!(
        lit::commands::show::execute("HEAD".to_string()).is_ok(),
        "show HEAD after gc"
    );

    // Encryption is only worth anything if the data is unreachable without the
    // passphrase. With no source available `new_auto` leaves the manager locked.
    std::env::remove_var("LIT_PASSPHRASE");
    assert!(
        lit::commands::status::execute().is_err(),
        "an encrypted repository must not be readable without the passphrase"
    );

    let _ = fs::remove_file(&key);
}

#[test]
fn test_enabling_encryption_on_an_existing_repository_explains_itself() {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Commit first, unencrypted.
    fs::write(temp.path().join("f.txt"), b"plain").unwrap();
    lit::commands::add::execute(vec!["f.txt".to_string()]).unwrap();
    lit::commands::commit::execute("plain".to_string(), None).unwrap();

    // Then switch encryption on, which cannot work: the existing index and
    // objects carry no header of ours.
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let key = std::env::temp_dir().join(format!("lit_mig_{}_{}.key", std::process::id(), n));
    let _ = fs::remove_file(&key);
    fs::write(
        temp.path().join(".lit").join("encryption.toml"),
        format!(
            "enabled = true\nkey_file = \"{}\"\nfips_mode = false\ncache_timeout_secs = 0\n",
            key.to_string_lossy().replace(char::from(92u8), "/")
        ),
    )
    .unwrap();
    std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);

    let err = lit::commands::status::execute().unwrap_err();

    // The rendered message is sanitized, so the suggestions are the only thing
    // the user sees. Previously this surfaced as "Unsupported encryption
    // version: 123" internally and a bare "Operation failed" outside.
    let hints = err.suggestions().join(" ");
    assert!(
        hints.contains("already has commits"),
        "the failure should explain that encryption cannot be switched on later, got: {}",
        hints
    );

    std::env::remove_var("LIT_PASSPHRASE");
    let _ = fs::remove_file(&key);
}
