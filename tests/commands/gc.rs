/// Command tests for `lit gc`
use std::fs;
use tempfile::TempDir;

#[test]
fn test_gc_empty_repo() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    let result = lit::commands::gc::execute();

    assert!(
        result.is_ok(),
        "GC should succeed on empty repo: {:?}",
        result
    );
    let response = result.unwrap();
    assert_eq!(
        response.objects_packed, 0,
        "Empty repo should have 0 objects"
    );
}

#[test]
fn test_gc_packs_loose_objects() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();

    lit::commands::init::execute(false, Some(repo_path.clone())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    // Create several files
    let mut file_names = Vec::new();
    for i in 0..5 {
        let name = format!("file{}.txt", i);
        fs::write(temp.path().join(&name), format!("content {}\n", i)).unwrap();
        file_names.push(name);
    }
    let add_result = lit::commands::add::execute(file_names);
    assert!(add_result.is_ok(), "Add should succeed: {:?}", add_result);

    let commit_result = lit::commands::commit::execute("add files".to_string(), None);
    assert!(
        commit_result.is_ok(),
        "Commit should succeed: {:?}",
        commit_result
    );

    let result = lit::commands::gc::execute();

    assert!(result.is_ok(), "GC should succeed: {:?}", result);
    let response = result.unwrap();
    assert!(
        response.objects_packed > 0,
        "Should pack at least some objects"
    );
    assert!(
        response.packs_created > 0,
        "Should create at least one pack"
    );
}

#[test]
fn test_gc_leaves_objects_readable() {
    let temp = TempDir::new().unwrap();
    lit::commands::init::execute(false, Some(temp.path().to_str().unwrap().to_string())).unwrap();
    let _cwd = super::test_helpers::CwdGuard::new(temp.path());

    fs::create_dir_all(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("main.rs"), "fn main() {}\n").unwrap();
    fs::write(temp.path().join("README.md"), "# readme\n").unwrap();
    lit::commands::add::execute(vec!["src/main.rs".to_string(), "README.md".to_string()]).unwrap();
    lit::commands::commit::execute("packed".to_string(), None).unwrap();

    let store = lit::storage::ObjectStore::new(temp.path());
    let before: Vec<String> = store
        .list()
        .unwrap()
        .iter()
        .map(|h| h.as_str().to_string())
        .collect();
    assert!(!before.is_empty(), "there should be objects to pack");

    let response = lit::commands::gc::execute().unwrap();
    assert!(response.objects_packed > 0, "gc should pack something");
    assert!(
        response.loose_removed > 0,
        "gc should remove the loose copies"
    );

    // gc deletes the loose objects once they are in a pack, so everything below
    // is served from the pack. Before the pack reader existed this left the
    // repository unreadable: `show`, `diff`, `checkout` and `export-git` all
    // failed and `list()` came back empty.
    let mut after: Vec<String> = store
        .list()
        .unwrap()
        .iter()
        .map(|h| h.as_str().to_string())
        .collect();
    let mut expected = before.clone();
    after.sort();
    expected.sort();
    assert_eq!(after, expected, "packed objects should still be listed");

    for hash in &expected {
        let hash = lit::core::ObjectHash::from_hex(hash.clone());
        assert!(store.exists(&hash), "{} should still exist", hash.short());
        assert!(
            store.read(&hash).is_ok(),
            "{} should still be readable from the pack",
            hash.short()
        );
    }
}
