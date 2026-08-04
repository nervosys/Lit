/// Command tests for `lit import-git`
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

/// Write a loose Git object into `git_dir` and return its SHA-1.
fn write_git_object(git_dir: &Path, obj_type: &str, body: &[u8]) -> String {
    let mut raw = Vec::new();
    raw.extend_from_slice(format!("{} {}\0", obj_type, body.len()).as_bytes());
    raw.extend_from_slice(body);

    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(&raw);
    let hash = hex::encode(hasher.finalize());

    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&raw).unwrap();
    let compressed = encoder.finish().unwrap();

    let dir = git_dir.join("objects").join(&hash[..2]);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(&hash[2..]), compressed).unwrap();
    hash
}

/// Build a Git tree body from `(mode, name, hash)` entries.
fn git_tree_body(entries: &[(&str, &str, &str)]) -> Vec<u8> {
    let mut body = Vec::new();
    for (mode, name, hash) in entries {
        body.extend_from_slice(mode.as_bytes());
        body.push(b' ');
        body.extend_from_slice(name.as_bytes());
        body.push(0);
        body.extend_from_slice(&hex::decode(hash).unwrap());
    }
    body
}

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
fn test_import_git_resolves_nested_references() {
    let git_temp = TempDir::new().unwrap();
    for dir in &["objects", "refs/heads", "refs/tags"] {
        fs::create_dir_all(git_temp.path().join(dir)).unwrap();
    }
    fs::write(git_temp.path().join("HEAD"), "ref: refs/heads/main\n").unwrap();

    // blob <- subtree <- root tree <- commit. A Lit tree names its children by
    // Lit hash, so each object can only be converted after everything it
    // points at. Loose objects are enumerated in filesystem order, which says
    // nothing about the graph, so the import has to sort this out itself.
    let blob = write_git_object(git_temp.path(), "blob", b"fn main() {}\n");
    let subtree = write_git_object(
        git_temp.path(),
        "tree",
        &git_tree_body(&[("100644", "main.rs", &blob)]),
    );
    let root = write_git_object(
        git_temp.path(),
        "tree",
        &git_tree_body(&[("40000", "src", &subtree)]),
    );
    let commit_body = format!(
        "tree {}\n\
         author Test <test@example.com> 1700000000 +0000\n\
         committer Test <test@example.com> 1700000000 +0000\n\
         \n\
         nested commit\n",
        root
    );
    let commit = write_git_object(git_temp.path(), "commit", commit_body.as_bytes());
    fs::write(
        git_temp.path().join("refs").join("heads").join("main"),
        format!("{}\n", commit),
    )
    .unwrap();

    let lit_temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(lit_temp.path().to_str().unwrap().to_string()))
        .unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(lit_temp.path());

    let response =
        lit::commands::import_git::execute(git_temp.path().to_str().unwrap().to_string()).unwrap();
    assert_eq!(
        response.objects_imported, 4,
        "blob, subtree, root tree and commit should all be imported"
    );

    // Every Lit hash the imported graph names must resolve in the store.
    let store = lit::storage::ObjectStore::new(lit_temp.path());
    let head = lit::core::read_ref(lit_temp.path(), "heads/main").unwrap();

    let mut stack = vec![lit::core::ObjectHash::from_hex(head)];
    let mut visited = 0;
    while let Some(hash) = stack.pop() {
        assert!(
            store.exists(&hash),
            "imported object {} is referenced but absent from the store",
            hash.short()
        );
        visited += 1;
        match store.read(&hash).unwrap() {
            lit::core::Object::Commit(commit) => {
                stack.push(commit.tree.clone());
                stack.extend(commit.parents.iter().cloned());
            }
            lit::core::Object::Tree(tree) => {
                stack.extend(tree.entries.iter().map(|e| e.hash.clone()));
            }
            _ => {}
        }
    }
    assert_eq!(visited, 4, "the whole chain should be reachable from HEAD");
}

#[test]
fn test_import_git_invalid_source() {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();

    let _cwd = super::test_helpers::CwdGuard::new(temp.path());
    let result = lit::commands::import_git::execute("/nonexistent/path".to_string());

    assert!(result.is_err(), "Should fail with invalid source path");
}

/// Run a Git command in `dir`, returning its stdout.
///
/// `None` means Git is unavailable or the command failed, which the
/// delta test below treats as "skip" rather than "fail" so the suite still
/// runs on machines without a Git CLI.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Every object Git holds in a repository, as a sorted list of SHA-1s.
fn git_object_hashes(dir: &Path) -> Vec<String> {
    let listing = git(
        dir,
        &[
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objectname)",
        ],
    )
    .expect("listing objects should succeed");
    let mut hashes: Vec<String> = listing
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    hashes.sort();
    hashes
}

/// Every loose object in an exported Git repository, as a sorted list.
fn exported_object_hashes(git_dir: &Path) -> Vec<String> {
    let mut hashes = Vec::new();
    for shard in fs::read_dir(git_dir.join("objects")).unwrap() {
        let shard = shard.unwrap();
        let prefix = shard.file_name().to_string_lossy().into_owned();
        if prefix.len() != 2 || !shard.file_type().unwrap().is_dir() {
            continue;
        }
        for object in fs::read_dir(shard.path()).unwrap() {
            let suffix = object.unwrap().file_name().to_string_lossy().into_owned();
            hashes.push(format!("{}{}", prefix, suffix));
        }
    }
    hashes.sort();
    hashes
}

#[test]
fn test_import_git_resolves_pack_deltas_end_to_end() {
    let source = TempDir::new().unwrap();
    if git(source.path(), &["--version"]).is_none() {
        eprintln!("skipping test_import_git_resolves_pack_deltas_end_to_end: no git CLI");
        return;
    }

    // Build a history whose revisions differ only slightly, so that repacking
    // stores most of them as deltas against one another.
    git(source.path(), &["init", "-q", "-b", "main", "."]).unwrap();
    git(source.path(), &["config", "user.email", "t@example.com"]).unwrap();
    git(source.path(), &["config", "user.name", "Test"]).unwrap();

    for rev in 1..=6 {
        let body: String = (0..1500)
            .map(|line| {
                format!(
                    "line {} of revision {} with padding to delta well\n",
                    line, rev
                )
            })
            .collect();
        fs::write(source.path().join("big.txt"), body).unwrap();
        fs::write(
            source.path().join("notes.md"),
            format!("up to rev {}\n", rev),
        )
        .unwrap();
        git(source.path(), &["add", "-A"]).unwrap();
        git(
            source.path(),
            &["commit", "-q", "-m", &format!("rev {}", rev)],
        )
        .unwrap();
    }
    git(
        source.path(),
        &["repack", "-a", "-d", "-f", "--depth=50", "--window=50"],
    )
    .unwrap();

    // A `verify-pack` line gains a depth and base-hash column when the entry
    // is a delta. If none are, the repack did not exercise what we are testing.
    let idx = fs::read_dir(source.path().join(".git").join("objects").join("pack"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("idx"))
        .expect("repack should produce a pack index");
    let verify = git(source.path(), &["verify-pack", "-v", idx.to_str().unwrap()]).unwrap();
    let deltas = verify
        .lines()
        .filter(|line| line.split_whitespace().count() == 7)
        .count();
    assert!(
        deltas > 0,
        "test setup produced no deltified objects, so it would not test delta resolution"
    );

    let expected = git_object_hashes(source.path());

    // Import into Lit, then export straight back out to Git.
    let lit_repo = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(lit_repo.path().to_str().unwrap().to_string()))
        .unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(lit_repo.path());

    let imported =
        lit::commands::import_git::execute(source.path().to_str().unwrap().to_string()).unwrap();
    assert_eq!(
        imported.objects_imported as usize,
        expected.len(),
        "every object in the source, deltas included, should be imported"
    );

    let round_tripped = lit_repo.path().join("round-trip.git");
    lit::commands::export_git::execute(round_tripped.to_str().unwrap().to_string()).unwrap();

    // Content-addressed storage makes this an exact-content assertion: the
    // hashes only match if every delta was reconstructed byte for byte.
    assert_eq!(
        exported_object_hashes(&round_tripped),
        expected,
        "Git -> Lit -> Git should reproduce every object under its original SHA-1"
    );
}

#[test]
fn test_import_git_round_trips_annotated_tags() {
    let source = TempDir::new().unwrap();
    if git(source.path(), &["--version"]).is_none() {
        eprintln!("skipping test_import_git_round_trips_annotated_tags: no git CLI");
        return;
    }

    git(source.path(), &["init", "-q", "-b", "main", "."]).unwrap();
    git(source.path(), &["config", "user.email", "t@example.com"]).unwrap();
    git(source.path(), &["config", "user.name", "Test"]).unwrap();

    for rev in 1..=3 {
        fs::write(source.path().join("f.txt"), format!("rev {}\n", rev)).unwrap();
        git(source.path(), &["add", "-A"]).unwrap();
        git(
            source.path(),
            &["commit", "-q", "-m", &format!("rev {}", rev)],
        )
        .unwrap();
    }

    // An annotated tag is a real object with its own hash; a lightweight tag
    // is just a ref. Both need to survive, and the multi-line message checks
    // that the body is carried across verbatim.
    git(
        source.path(),
        &[
            "tag",
            "-a",
            "v1.0",
            "-m",
            "First release\n\nWith a multi-line message.",
            "HEAD~1",
        ],
    )
    .unwrap();
    git(
        source.path(),
        &["tag", "-a", "v2.0", "-m", "Second release"],
    )
    .unwrap();
    git(source.path(), &["tag", "lightweight", "HEAD"]).unwrap();

    let types = git(
        source.path(),
        &[
            "cat-file",
            "--batch-all-objects",
            "--batch-check=%(objecttype)",
        ],
    )
    .unwrap();
    let tag_objects = types.lines().filter(|t| t.trim() == "tag").count();
    assert_eq!(
        tag_objects, 2,
        "setup should produce two annotated tag objects"
    );

    let expected = git_object_hashes(source.path());

    let lit_repo = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(lit_repo.path().to_str().unwrap().to_string()))
        .unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(lit_repo.path());

    let imported =
        lit::commands::import_git::execute(source.path().to_str().unwrap().to_string()).unwrap();
    assert_eq!(
        imported.objects_imported as usize,
        expected.len(),
        "annotated tag objects should be imported alongside everything else"
    );
    assert!(
        imported.refs_imported >= 4,
        "main plus three tag refs should be imported, saw {}",
        imported.refs_imported
    );

    let round_tripped = lit_repo.path().join("round-trip.git");
    lit::commands::export_git::execute(round_tripped.to_str().unwrap().to_string()).unwrap();

    assert_eq!(
        exported_object_hashes(&round_tripped),
        expected,
        "annotated tags should re-export under their original SHA-1"
    );
}
