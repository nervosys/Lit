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

/// A repository with encryption on and no `key_file` named, so it takes the
/// per-repository default.
fn encrypted_repo_with_default_key() -> TempDir {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();

    fs::write(
        temp.path().join(".lit").join("encryption.toml"),
        "enabled = true\nfips_mode = false\ncache_timeout_secs = 0\n",
    )
    .unwrap();
    let _ = fs::remove_file(temp.path().join(".lit").join("index"));

    temp
}

/// Where a repository's default key file actually lands on disk.
fn resolved_key_file(repo: &std::path::Path) -> std::path::PathBuf {
    let configured = lit::crypto::encryption::default_key_file(repo);
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .expect("no home directory");
    std::path::PathBuf::from(configured.replacen('~', &home, 1))
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
fn test_two_repositories_do_not_share_one_key_file() {
    // The documented `key_file = "~/.lit/encryption.key"` was one file for
    // every repository on the machine, so the second repository to be
    // initialised had to reuse the first one's passphrase or fail. With no
    // key_file configured each repository resolves its own.
    let first = encrypted_repo_with_default_key();
    let second = encrypted_repo_with_default_key();

    let key_first = resolved_key_file(first.path());
    let key_second = resolved_key_file(second.path());
    assert_ne!(
        key_first, key_second,
        "two repositories resolved to the same key file"
    );
    let _ = fs::remove_file(&key_first);
    let _ = fs::remove_file(&key_second);

    let other_passphrase = "DifferentHorse!77";

    {
        let _cwd = super::test_helpers::CwdGuard::new(first.path());
        std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);
        fs::write(first.path().join("a.txt"), SECRET).unwrap();
        lit::commands::add::execute(vec!["a.txt".to_string()]).unwrap();
        lit::commands::commit::execute("first".to_string(), None).unwrap();
    }

    // A different passphrase in the second repository. Against a shared key
    // file this failed with "Invalid passphrase".
    {
        let _cwd = super::test_helpers::CwdGuard::new(second.path());
        std::env::set_var("LIT_PASSPHRASE", other_passphrase);
        fs::write(second.path().join("b.txt"), SECRET).unwrap();
        lit::commands::add::execute(vec!["b.txt".to_string()]).unwrap();
        lit::commands::commit::execute("second".to_string(), None).unwrap();
        assert!(
            lit::commands::status::execute().is_ok(),
            "second repository"
        );
    }

    assert!(key_first.exists(), "the first key file was not created");
    assert!(key_second.exists(), "the second key file was not created");

    // Each opens with its own passphrase and not with the other's.
    {
        let _cwd = super::test_helpers::CwdGuard::new(first.path());
        std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);
        assert!(lit::commands::status::execute().is_ok(), "first reopened");
        std::env::set_var("LIT_PASSPHRASE", other_passphrase);
        assert!(
            lit::commands::status::execute().is_err(),
            "the other repository's passphrase must not open this one"
        );
    }

    std::env::remove_var("LIT_PASSPHRASE");
    let _ = fs::remove_file(&key_first);
    let _ = fs::remove_file(&key_second);
    let _ = fs::remove_file(key_first.with_extension("key.throttle"));
    let _ = fs::remove_file(key_second.with_extension("key.throttle"));
}

#[test]
fn test_the_resolved_key_file_is_recorded_so_a_move_does_not_lose_it() {
    // The default path is derived from the repository's location, so it has to
    // be written down the first time it is resolved — otherwise moving the
    // repository silently points it at a key that does not exist.
    let repo = encrypted_repo_with_default_key();
    let key = resolved_key_file(repo.path());
    let _ = fs::remove_file(&key);

    let config = lit::crypto::encryption::EncryptionConfig::load(repo.path()).unwrap();
    assert!(!config.key_file.is_empty(), "key_file was left unresolved");

    let written = fs::read_to_string(repo.path().join(".lit").join("encryption.toml")).unwrap();
    assert!(
        written.contains(&config.key_file),
        "the resolved key file was not recorded in encryption.toml, got: {}",
        written
    );

    let _ = fs::remove_file(&key);
}

#[test]
fn test_a_missing_key_file_is_not_quietly_replaced() {
    // Deleting the key file used to make the next command create a new one,
    // which then failed to decrypt anything with a message about headers.
    let repo = encrypted_repo_with_default_key();
    let key = resolved_key_file(repo.path());
    let _ = fs::remove_file(&key);

    let _cwd = super::test_helpers::CwdGuard::new(repo.path());
    std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);

    fs::write(repo.path().join("f.txt"), SECRET).unwrap();
    lit::commands::add::execute(vec!["f.txt".to_string()]).unwrap();
    lit::commands::commit::execute("encrypted".to_string(), None).unwrap();
    assert!(key.exists(), "the key file should have been created");

    fs::remove_file(&key).unwrap();

    // A real second command would be a new process. In-process the key is
    // already derived and cached, and the cache is consulted before the file.
    lit::crypto::encryption::clear_derived_key_cache();

    let err = lit::commands::status::execute().unwrap_err();
    let rendered = format!("{} {}", err, err.suggestions().join(" "));
    assert!(
        rendered.contains("key file is gone"),
        "a lost key file should say so, got: {}",
        rendered
    );
    assert!(
        !key.exists(),
        "a new key file was created over an encrypted repository"
    );

    std::env::remove_var("LIT_PASSPHRASE");
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
fn test_refs_expose_neither_hash_nor_name_on_disk() {
    let (temp, key) = encrypted_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());
    std::env::set_var("LIT_PASSPHRASE", PASSPHRASE);

    fs::write(temp.path().join("f.txt"), SECRET).unwrap();
    lit::commands::add::execute(vec!["f.txt".to_string()]).unwrap();
    lit::commands::commit::execute("encrypted".to_string(), None).unwrap();
    lit::commands::branch::execute(Some("feature-x".to_string()), false, false).unwrap();

    // Neither the hash a ref points at nor the ref's own name may be readable
    // off disk. Refs were once written in the clear, exposing the whole commit
    // graph; encrypting the contents then still left every branch and tag name
    // visible, because a name is a filename.
    let head = lit::core::read_ref(temp.path(), "heads/main").unwrap();
    let lit_dir = temp.path().join(".lit");

    assert!(
        lit_dir.join("refs.enc").exists(),
        "an encrypted repository should keep its refs in one encrypted index"
    );

    let mut loose = Vec::new();
    let mut exposes_name = Vec::new();
    let mut exposes_hash = Vec::new();
    for entry in walkdir::WalkDir::new(&lit_dir).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().starts_with(lit_dir.join("refs")) {
            loose.push(entry.path().to_path_buf());
        }
        if entry.path().to_string_lossy().contains("feature-x") {
            exposes_name.push(entry.path().to_path_buf());
        }
        if let Ok(bytes) = fs::read(entry.path()) {
            if bytes.windows(9).any(|w| w == b"feature-x") {
                exposes_name.push(entry.path().to_path_buf());
            }
            if bytes.windows(head.len()).any(|w| w == head.as_bytes()) {
                exposes_hash.push(entry.path().to_path_buf());
            }
        }
    }

    assert!(loose.is_empty(), "refs left loose on disk: {:?}", loose);
    assert!(
        exposes_name.is_empty(),
        "the branch name is readable on disk: {:?}",
        exposes_name
    );
    assert!(
        exposes_hash.is_empty(),
        "the commit hash is readable on disk: {:?}",
        exposes_hash
    );

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
    assert!(
        lit::core::list_refs(temp.path(), "heads")
            .unwrap()
            .iter()
            .any(|r| r.name == "feature-x"),
        "the branch must still be listed even though its name is not on disk"
    );

    std::env::remove_var("LIT_PASSPHRASE");
    let _ = fs::remove_file(&key);
}
