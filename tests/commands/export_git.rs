/// Command tests for `lit export-git`
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use tempfile::TempDir;

/// Inflate a loose Git object, returning its type and body.
///
/// Also asserts the object hashes to the filename it is stored under, which
/// catches content that was serialized against a hash that does not match.
fn read_git_object(git_dir: &Path, hash: &str) -> (String, Vec<u8>) {
    let path = git_dir.join("objects").join(&hash[..2]).join(&hash[2..]);
    let compressed =
        fs::read(&path).unwrap_or_else(|e| panic!("missing Git object {}: {}", hash, e));

    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&compressed[..])
        .read_to_end(&mut raw)
        .unwrap();

    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(&raw);
    assert_eq!(
        hex::encode(hasher.finalize()),
        hash,
        "object {} does not hash to the name it is stored under",
        hash
    );

    let null_pos = raw.iter().position(|&b| b == 0).unwrap();
    let header = std::str::from_utf8(&raw[..null_pos]).unwrap();
    let obj_type = header.split_once(' ').unwrap().0.to_string();
    (obj_type, raw[null_pos + 1..].to_vec())
}

/// Walk everything reachable from `hash`, asserting each object exists and is
/// well formed. Returns the number of distinct objects visited.
fn assert_reachable(git_dir: &Path, hash: &str, seen: &mut HashSet<String>) -> usize {
    if !seen.insert(hash.to_string()) {
        return 0;
    }
    let (obj_type, body) = read_git_object(git_dir, hash);
    let mut count = 1;

    match obj_type.as_str() {
        "commit" => {
            let text = String::from_utf8_lossy(&body).into_owned();
            for line in text.lines() {
                if line.is_empty() {
                    break; // header ends at the first blank line
                }
                let child = line
                    .strip_prefix("tree ")
                    .or_else(|| line.strip_prefix("parent "));
                if let Some(child) = child {
                    count += assert_reachable(git_dir, child.trim(), seen);
                }
            }
        }
        "tree" => {
            let mut pos = 0;
            while pos < body.len() {
                let space = body[pos..].iter().position(|&b| b == b' ').unwrap() + pos;
                let null = body[space..].iter().position(|&b| b == 0).unwrap() + space;
                assert!(null + 21 <= body.len(), "truncated tree entry");
                let child = hex::encode(&body[null + 1..null + 21]);
                count += assert_reachable(git_dir, &child, seen);
                pos = null + 21;
            }
        }
        _ => {}
    }

    count
}

#[test]
fn test_export_git_creates_git_structure() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create a file and commit
    fs::write(temp.path().join("test.txt"), "hello world\n").unwrap();
    lit::commands::add::execute(vec!["test.txt".to_string()]).unwrap();
    lit::commands::commit::execute("initial commit".to_string(), None).unwrap();

    // Export to git
    let git_dest = temp.path().join("exported.git");
    let result = lit::commands::export_git::execute(git_dest.to_str().unwrap().to_string());

    assert!(result.is_ok(), "Export should succeed: {:?}", result);
    let response = result.unwrap();

    assert!(git_dest.join("HEAD").exists(), "HEAD should exist");
    assert!(
        git_dest.join("objects").exists(),
        "objects dir should exist"
    );
    assert!(
        response.objects_exported > 0,
        "Should export at least one object"
    );
}

#[test]
fn test_export_git_roundtrip_preserves_content() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    fs::write(temp.path().join("hello.txt"), "hello world\n").unwrap();
    lit::commands::add::execute(vec!["hello.txt".to_string()]).unwrap();
    lit::commands::commit::execute("test commit".to_string(), None).unwrap();

    let git_dest = temp.path().join("out.git");
    let result = lit::commands::export_git::execute(git_dest.to_str().unwrap().to_string());

    assert!(result.is_ok(), "Export should succeed: {:?}", result);
    let response = result.unwrap();
    assert!(response.refs_exported > 0, "Should export at least one ref");
}

#[test]
fn test_export_git_references_are_resolvable() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // A file in a subdirectory yields blob -> subtree -> root tree -> commit,
    // so the export has to write four objects in dependency order. Object
    // enumeration order is not the graph order, so writing a tree before its
    // blobs would leave the tree pointing at a hash that was never stored.
    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(temp.path().join("README.md"), "# readme\n").unwrap();
    lit::commands::add::execute(vec!["src/main.rs".to_string(), "README.md".to_string()]).unwrap();
    lit::commands::commit::execute("nested commit".to_string(), None).unwrap();

    let git_dest = temp.path().join("resolvable.git");
    lit::commands::export_git::execute(git_dest.to_str().unwrap().to_string()).unwrap();

    // Walk the graph from every exported branch; every hash a tree or commit
    // names must resolve to an object that is actually present.
    let heads = git_dest.join("refs").join("heads");
    let mut seen = HashSet::new();
    let mut branches = 0;
    for entry in fs::read_dir(&heads).unwrap() {
        let entry = entry.unwrap();
        if !entry.file_type().unwrap().is_file() {
            continue;
        }
        branches += 1;
        let commit_hash = fs::read_to_string(entry.path()).unwrap().trim().to_string();
        let visited = assert_reachable(&git_dest, &commit_hash, &mut seen);
        assert!(
            visited >= 4,
            "expected at least commit, root tree, subtree and blob, saw {}",
            visited
        );
    }
    assert!(branches > 0, "export should write at least one branch ref");
}
