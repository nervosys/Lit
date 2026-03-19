/// Command tests for `lit import-git`
use tempfile::TempDir;

/// Helper: create a minimal bare Git repository with one blob
fn create_test_git_repo(path: &std::path::Path) {
    use std::fs;
    use std::io::Write;

    for dir in &["objects", "refs/heads", "refs/tags"] {
        fs::create_dir_all(path.join(dir)).unwrap();
    }
    fs::write(path.join("HEAD"), "ref: refs/heads/main\n").unwrap();

    let content = b"hello\n";
    let header = format!("blob {}\0", content.len());
    let mut raw = Vec::new();
    raw.extend_from_slice(header.as_bytes());
    raw.extend_from_slice(content);

    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(&raw);
    let hash = hex::encode(hasher.finalize());

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).unwrap();
    let compressed = encoder.finish().unwrap();

    let obj_dir = path.join("objects").join(&hash[..2]);
    fs::create_dir_all(&obj_dir).unwrap();
    fs::write(obj_dir.join(&hash[2..]), &compressed).unwrap();
}

#[test]
fn test_import_git_creates_lit_repo() {
    let git_temp = TempDir::new().unwrap();
    create_test_git_repo(git_temp.path());

    let lit_temp = TempDir::new().unwrap();
    // Init a lit repo at the explicit path
    lit::commands::init::execute(false, Some(lit_temp.path().to_str().unwrap().to_string()))
        .unwrap();

    let _cwd = super::test_helpers::CwdGuard::new(lit_temp.path());
    let result = lit::commands::import_git::execute(git_temp.path().to_str().unwrap().to_string());

    assert!(result.is_ok(), "import-git should succeed: {:?}", result);
    let response = result.unwrap();
    assert!(
        response.objects_imported > 0,
        "Should import at least one object"
    );
}

#[test]
fn test_import_git_invalid_source() {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());
    let result = lit::commands::import_git::execute("/nonexistent/path".to_string());

    assert!(result.is_err(), "Should fail with invalid source path");
}
