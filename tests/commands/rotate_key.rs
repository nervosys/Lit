/// Command tests for `lit rotate-key`
///
/// NOTE: These tests modify the current working directory and must be run with
/// `cargo test --test command_tests -- --test-threads=1` to avoid test interference.
use std::fs;
use tempfile::TempDir;

const OLD_PASSPHRASE: &str = "CorrectHorseBattery!99";
const NEW_PASSPHRASE: &str = "AnotherHorseEntirely!42";
const SECRET: &[u8] = b"ROTATION-TEST-PAYLOAD-909";

fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

#[test]
fn test_rotate_key_without_encryption() {
    let temp = init_test_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::rotate_key::rotate_key();
    assert!(
        result.is_err(),
        "Rotate key should fail when encryption is not enabled"
    );
}

/// An encrypted repository holding one commit, packed.
///
/// Packing matters: rotation used to ignore `.lit/packs` entirely, so packed
/// objects stayed encrypted under a key that no longer existed anywhere.
fn packed_encrypted_repo() -> (TempDir, std::path::PathBuf) {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();

    fs::write(
        temp.path().join(".lit").join("encryption.toml"),
        "enabled = true\nfips_mode = false\ncache_timeout_secs = 0\n",
    )
    .unwrap();
    let _ = fs::remove_file(temp.path().join(".lit").join("index"));

    let configured = lit::crypto::encryption::default_key_file(temp.path());
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .expect("no home directory");
    let key = std::path::PathBuf::from(configured.replacen('~', &home, 1));
    let _ = fs::remove_file(&key);

    (temp, key)
}

#[test]
fn test_rotate_key_re_encrypts_the_whole_repository() {
    let (temp, key) = packed_encrypted_repo();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());
    std::env::set_var("LIT_PASSPHRASE", OLD_PASSPHRASE);

    fs::write(temp.path().join("s.txt"), SECRET).unwrap();
    lit::commands::add::execute(vec!["s.txt".to_string()]).unwrap();
    lit::commands::commit::execute("before rotation".to_string(), None).unwrap();
    lit::commands::branch::execute(Some("feature-x".to_string()), false, false).unwrap();
    lit::commands::gc::execute().unwrap();

    let head_before = lit::core::read_ref(temp.path(), "heads/main").unwrap();

    // The rotation itself. Every part of this had never been executed: the one
    // existing test covered only the encryption-disabled early return.
    let response =
        lit::commands::rotate_key::rotate_with_passphrases(OLD_PASSPHRASE, NEW_PASSPHRASE)
            .expect("rotation failed");
    assert!(
        response.objects_rotated > 0,
        "nothing was rotated: {:?}",
        response
    );

    // The repository opens with the new passphrase, all of it — including the
    // refs, which live in the encrypted ref index, and the objects that were
    // inside the pack.
    std::env::set_var("LIT_PASSPHRASE", NEW_PASSPHRASE);
    lit::crypto::encryption::clear_derived_key_cache();

    assert!(
        lit::commands::status::execute().is_ok(),
        "status after rotation"
    );
    assert_eq!(
        lit::core::read_ref(temp.path(), "heads/main").unwrap(),
        head_before,
        "the ref index did not survive rotation"
    );
    assert!(
        lit::core::list_refs(temp.path(), "heads")
            .unwrap()
            .iter()
            .any(|r| r.name == "feature-x"),
        "branches did not survive rotation"
    );
    assert!(
        lit::commands::show::execute("HEAD".to_string()).is_ok(),
        "the commit did not survive rotation"
    );

    // And the old passphrase no longer opens it.
    std::env::set_var("LIT_PASSPHRASE", OLD_PASSPHRASE);
    lit::crypto::encryption::clear_derived_key_cache();
    assert!(
        lit::commands::status::execute().is_err(),
        "the old passphrase still opens the repository"
    );

    std::env::remove_var("LIT_PASSPHRASE");
    let _ = fs::remove_file(&key);
    let _ = fs::remove_file(key.with_extension("key.throttle"));
}
