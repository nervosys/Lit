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
