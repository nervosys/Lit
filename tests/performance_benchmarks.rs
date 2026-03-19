/// Performance benchmarks for Lit VCS
///
/// These benchmarks test performance with large objects, many commits, and index stress.
/// Run with: cargo test --test performance_benchmarks --release -- --nocapture --test-threads=1
use std::fs;
use std::time::Instant;
use tempfile::TempDir;

mod cwd_guard {
    use std::cell::Cell;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard};

    static CWD_MUTEX: Mutex<()> = Mutex::new(());
    thread_local! { static CWD_LOCKED: Cell<bool> = const { Cell::new(false) }; }

    pub struct CwdGuard {
        original: std::path::PathBuf,
        _lock: Option<MutexGuard<'static, ()>>,
    }
    impl CwdGuard {
        pub fn new(path: &Path) -> Self {
            let already_held = CWD_LOCKED.with(|c| c.get());
            if already_held {
                std::env::set_current_dir(path).unwrap();
                return CwdGuard {
                    original: std::path::PathBuf::new(),
                    _lock: None,
                };
            }
            let lock = CWD_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            CWD_LOCKED.with(|c| c.set(true));
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            CwdGuard {
                original,
                _lock: Some(lock),
            }
        }
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            if self._lock.is_some() {
                let _ = std::env::set_current_dir(&self.original);
                CWD_LOCKED.with(|c| c.set(false));
            }
        }
    }
}

// Helper to initialize a test repository
fn init_test_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().to_str().unwrap().to_string();
    lit::commands::init::execute(false, Some(repo_path)).unwrap();
    temp
}

// Helper to create a file with specific size
fn create_file_with_size(dir: &std::path::Path, name: &str, size_bytes: usize) {
    let content = vec![b'A'; size_bytes];
    fs::write(dir.join(name), content).unwrap();
}

// Helper to create a commit
fn create_commit(repo_path: &std::path::Path, files: Vec<String>, message: &str) {
    let _cwd = cwd_guard::CwdGuard::new(repo_path);

    lit::commands::add::execute(files).unwrap();
    lit::commands::commit::execute(message.to_string(), None).unwrap();
}

#[test]
fn bench_large_file_add() {
    let temp = init_test_repo();

    // Create a 10MB file
    let size_mb = 10;
    let size_bytes = size_mb * 1024 * 1024;
    create_file_with_size(temp.path(), "large_file.bin", size_bytes);

    let _cwd = cwd_guard::CwdGuard::new(temp.path());

    let start = Instant::now();
    lit::commands::add::execute(vec!["large_file.bin".to_string()]).unwrap();
    let duration = start.elapsed();

    println!("✓ Add {}MB file: {:?}", size_mb, duration);
    let threshold = if cfg!(debug_assertions) { 30 } else { 5 };
    assert!(
        duration.as_secs() < threshold,
        "Should add {}MB file in under {} seconds (took {:?})",
        size_mb,
        threshold,
        duration,
    );
}

#[test]
fn bench_many_small_files() {
    let temp = init_test_repo();

    let num_files = 100;
    let mut files = Vec::new();

    // Create 100 small files
    for i in 0..num_files {
        let filename = format!("file_{}.txt", i);
        fs::write(temp.path().join(&filename), format!("Content {}", i)).unwrap();
        files.push(filename);
    }

    let _cwd = cwd_guard::CwdGuard::new(temp.path());

    let start = Instant::now();
    lit::commands::add::execute(files).unwrap();
    let duration = start.elapsed();

    println!("✓ Add {} small files: {:?}", num_files, duration);
    assert!(
        duration.as_secs() < 3,
        "Should add {} files in under 3 seconds",
        num_files
    );
}

#[test]
fn bench_many_commits() {
    let temp = init_test_repo();

    let num_commits = 50;

    let start = Instant::now();
    for i in 0..num_commits {
        let filename = format!("file_{}.txt", i);
        fs::write(temp.path().join(&filename), format!("Content {}", i)).unwrap();
        create_commit(temp.path(), vec![filename], &format!("Commit {}", i));
    }
    let duration = start.elapsed();

    let avg_time = duration.as_millis() as f64 / num_commits as f64;
    println!(
        "✓ {} commits: {:?} (avg: {:.2}ms/commit)",
        num_commits, duration, avg_time
    );
    assert!(
        duration.as_secs() < 30,
        "Should create {} commits in under 30 seconds",
        num_commits
    );

    let temp_path = temp.path().to_path_buf();
    let _cwd = cwd_guard::CwdGuard::new(&temp_path);

    // Benchmark log performance
    let log_start = Instant::now();
    lit::commands::log::execute(num_commits, false).unwrap();
    let log_duration = log_start.elapsed();

    println!("✓ Log {} commits: {:?}", num_commits, log_duration);
    assert!(
        log_duration.as_secs() < 2,
        "Should log {} commits in under 2 seconds",
        num_commits
    );
}

#[test]
fn bench_commit_large_file() {
    let temp = init_test_repo();

    // Create a 5MB file
    let size_mb = 5;
    let size_bytes = size_mb * 1024 * 1024;
    create_file_with_size(temp.path(), "large.bin", size_bytes);

    let _cwd = cwd_guard::CwdGuard::new(temp.path());

    lit::commands::add::execute(vec!["large.bin".to_string()]).unwrap();

    let start = Instant::now();
    lit::commands::commit::execute("Large file commit".to_string(), None).unwrap();
    let duration = start.elapsed();

    println!("✓ Commit {}MB file: {:?}", size_mb, duration);
    assert!(
        duration.as_secs() < 3,
        "Should commit {}MB file in under 3 seconds",
        size_mb
    );
}

#[test]
fn bench_index_operations() {
    let temp = init_test_repo();

    let num_entries = 500;

    // Add many files to index
    let start = Instant::now();
    let mut index = lit::storage::Index::new();
    for i in 0..num_entries {
        index.add(
            format!("file_{}.txt", i),
            format!("{:064x}", i),
            "100644".to_string(),
        );
    }
    index.save(temp.path()).unwrap();
    let add_duration = start.elapsed();

    println!("✓ Add {} entries to index: {:?}", num_entries, add_duration);

    // Benchmark index load
    let load_start = Instant::now();
    let loaded_index = lit::storage::Index::load(temp.path()).unwrap();
    let load_duration = load_start.elapsed();

    println!(
        "✓ Load index with {} entries: {:?}",
        num_entries, load_duration
    );
    assert_eq!(loaded_index.entries.len(), num_entries);
    assert!(
        load_duration.as_millis() < 100,
        "Should load index in under 100ms"
    );
}

#[test]
fn bench_branch_operations() {
    let temp = init_test_repo();

    // Create initial commit
    fs::write(temp.path().join("test.txt"), "content").unwrap();
    create_commit(temp.path(), vec!["test.txt".to_string()], "Initial commit");

    let _cwd = cwd_guard::CwdGuard::new(temp.path());

    let num_branches = 20;

    // Benchmark branch creation
    let start = Instant::now();
    for i in 0..num_branches {
        lit::commands::branch::execute(Some(format!("branch_{}", i)), false, false).unwrap();
    }
    let create_duration = start.elapsed();

    println!("✓ Create {} branches: {:?}", num_branches, create_duration);

    // Benchmark branch listing
    let list_start = Instant::now();
    lit::commands::branch::execute(None, false, false).unwrap();
    let list_duration = list_start.elapsed();

    println!("✓ List {} branches: {:?}", num_branches, list_duration);
    assert!(
        list_duration.as_millis() < 100,
        "Should list branches in under 100ms"
    );
}

#[test]
fn bench_status_with_many_files() {
    let temp = init_test_repo();

    let num_files = 100;

    // Create many files
    for i in 0..num_files {
        fs::write(
            temp.path().join(format!("file_{}.txt", i)),
            format!("Content {}", i),
        )
        .unwrap();
    }

    let _cwd = cwd_guard::CwdGuard::new(temp.path());

    let start = Instant::now();
    lit::commands::status::execute().unwrap();
    let duration = start.elapsed();

    println!(
        "✓ Status with {} untracked files: {:?}",
        num_files, duration
    );
    assert!(
        duration.as_secs() < 2,
        "Should run status in under 2 seconds"
    );
}

#[test]
fn bench_checkout_performance() {
    let temp = init_test_repo();

    // Create initial commit with multiple files
    for i in 0..20 {
        fs::write(
            temp.path().join(format!("file_{}.txt", i)),
            format!("Content {}", i),
        )
        .unwrap();
    }

    let _cwd = cwd_guard::CwdGuard::new(temp.path());

    let files: Vec<String> = (0..20).map(|i| format!("file_{}.txt", i)).collect();
    lit::commands::add::execute(files).unwrap();
    lit::commands::commit::execute("Initial commit".to_string(), None).unwrap();

    // Create and checkout a branch
    let start = Instant::now();
    lit::commands::checkout::execute("new-branch".to_string(), true).unwrap();
    let duration = start.elapsed();

    println!("✓ Checkout branch with 20 files: {:?}", duration);
    assert!(duration.as_millis() < 500, "Should checkout in under 500ms");
}

#[test]
fn bench_object_store_performance() {
    let temp = init_test_repo();

    let store = lit::storage::ObjectStore::new(temp.path());
    let num_objects = 100;

    // Benchmark object writes
    let start = Instant::now();
    let mut hashes = Vec::new();
    for i in 0..num_objects {
        let content = format!("Object content {}", i).into_bytes();
        let blob = lit::core::Blob::new(content);
        let object = lit::core::Object::Blob(blob);
        let hash = store.write(&object).unwrap();
        hashes.push(hash);
    }
    let write_duration = start.elapsed();

    println!("✓ Write {} objects: {:?}", num_objects, write_duration);

    // Benchmark object reads
    let read_start = Instant::now();
    for hash in &hashes {
        store.read(hash).unwrap();
    }
    let read_duration = read_start.elapsed();

    println!("✓ Read {} objects: {:?}", num_objects, read_duration);
    assert!(
        read_duration.as_millis() < 500,
        "Should read objects quickly"
    );
}
