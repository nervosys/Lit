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

/// Turn encryption on for a repository that already has plaintext commits.
fn enable_encryption_late(repo: &std::path::Path) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let key = std::env::temp_dir().join(format!("lit_mig_{}_{}.key", std::process::id(), n));
    let _ = fs::remove_file(&key);
    fs::write(
        repo.join(".lit").join("encryption.toml"),
        format!(
            "enabled = true\nkey_file = \"{}\"\nfips_mode = false\ncache_timeout_secs = 0\n",
            key.to_string_lossy().replace(char::from(92u8), "/")
        ),
    )
    .unwrap();
    key
}

#[test]
fn test_migrate_encryption_rescues_a_plaintext_repository() {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Commit in the clear, then pack, so migration has to handle both a pack
    // and the loose objects `gc` leaves behind.
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("a.txt"), SECRET).unwrap();
    fs::write(temp.path().join("b.txt"), b"plain").unwrap();
    lit::commands::add::execute(vec!["src/a.txt".to_string(), "b.txt".to_string()]).unwrap();
    lit::commands::commit::execute("plaintext".to_string(), None).unwrap();
    lit::commands::gc::execute().unwrap();

    let key = enable_encryption_late(temp.path());
    std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);

    // Before migrating, the repository is unusable: its content carries no
    // encryption header and every command that reads the index fails.
    assert!(
        lit::commands::status::execute().is_err(),
        "the repository should be unreadable before migration"
    );

    let response = lit::commands::migrate_encryption::execute().unwrap();
    assert!(
        response.objects_unpacked > 0,
        "the pack should have been expanded, got {:?}",
        response
    );

    // Now it works, and nothing is left in the clear.
    assert!(
        lit::commands::status::execute().is_ok(),
        "status after migration"
    );
    assert!(
        lit::commands::show::execute("HEAD".to_string()).is_ok(),
        "show HEAD after migration"
    );
    assert!(
        plaintext_leaks(temp.path()).is_empty(),
        "migration left plaintext behind: {:?}",
        plaintext_leaks(temp.path())
    );

    // Running it again must be a no-op rather than double-encrypting.
    let again = lit::commands::migrate_encryption::execute().unwrap();
    assert_eq!(
        again.objects_encrypted, 0,
        "second run should encrypt nothing"
    );
    assert_eq!(again.objects_unpacked, 0, "second run should find no packs");
    assert!(
        again.already_encrypted > 0,
        "second run should find work done"
    );
    assert!(
        lit::commands::status::execute().is_ok(),
        "the repository must survive a second migration"
    );

    std::env::remove_var("LIT_PASSPHRASE");
    let _ = fs::remove_file(&key);
}

#[test]
fn test_ref_contents_are_encrypted_but_names_are_not() {
    let (temp, key) = encrypted_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());
    std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);

    fs::write(temp.path().join("f.txt"), SECRET).unwrap();
    lit::commands::add::execute(vec!["f.txt".to_string()]).unwrap();
    lit::commands::commit::execute("encrypted".to_string(), None).unwrap();
    lit::commands::branch::execute(Some("feature-x".to_string()), false, false).unwrap();

    // The commit hash a ref points at must not be readable off disk. Refs used
    // to be written in the clear, so the whole commit graph was visible even
    // when every object was encrypted.
    let head = lit::core::read_ref(temp.path(), "heads/main").unwrap();
    for name in ["main", "feature-x"] {
        let raw = fs::read(
            temp.path()
                .join(".lit")
                .join("refs")
                .join("heads")
                .join(name),
        )
        .unwrap();
        assert!(
            lit::crypto::encryption::EncryptionManager::is_encrypted_payload(&raw),
            "refs/heads/{} should be ciphertext",
            name
        );
        assert!(
            !raw.windows(head.len()).any(|w| w == head.as_bytes()),
            "refs/heads/{} still exposes the commit hash",
            name
        );
    }

    // And it all still reads back.
    assert!(lit::commands::status::execute().is_ok(), "status");
    assert!(
        lit::commands::branch::execute(None, false, false).is_ok(),
        "branch"
    );
    assert!(
        lit::commands::show::execute("HEAD".to_string()).is_ok(),
        "show HEAD"
    );

    // What this does not hide: a branch name is a filename, so it stays
    // visible whatever the contents are encrypted with. Asserted so the
    // limitation is recorded rather than assumed away.
    assert!(
        temp.path()
            .join(".lit")
            .join("refs")
            .join("heads")
            .join("feature-x")
            .exists(),
        "branch names remain visible as directory entries"
    );

    std::env::remove_var("LIT_PASSPHRASE");
    let _ = fs::remove_file(&key);
}
