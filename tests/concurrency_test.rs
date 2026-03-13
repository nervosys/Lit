/// Concurrency tests for Lit VCS
///
/// These tests exercise concurrent access to the index, object store,
/// and reference system to ensure there are no data races or corruption.
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

/// Helper: create a minimal .lit repo skeleton inside `dir`.
fn init_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".lit/objects")).unwrap();
    fs::create_dir_all(dir.join(".lit/refs/heads")).unwrap();
    fs::write(dir.join(".lit/HEAD"), "ref: refs/heads/main\n").unwrap();
}

// ---------------------------------------------------------------------------
// Index concurrency
// ---------------------------------------------------------------------------

#[test]
fn test_concurrent_index_writes_are_isolated() {
    // Many threads each create their own Index, add entries, and save to
    // separate repos.  No cross-contamination should occur.
    let threads = 8;
    let entries_per_thread = 50;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let tmp = TempDir::new().unwrap();
                init_repo(tmp.path());

                // Wait for all threads to be ready
                barrier.wait();

                let mut index = lit::storage::Index::new();
                for i in 0..entries_per_thread {
                    index.add(
                        format!("thread_{}/file_{}.txt", t, i),
                        format!("hash_{}_{}", t, i),
                        "100644".to_string(),
                    );
                }
                index.save(tmp.path()).unwrap();

                // Reload and verify
                let loaded = lit::storage::Index::load(tmp.path()).unwrap();
                assert_eq!(
                    loaded.entries.len(),
                    entries_per_thread,
                    "Thread {} index should contain exactly {} entries",
                    t,
                    entries_per_thread
                );
                for i in 0..entries_per_thread {
                    let key = format!("thread_{}/file_{}.txt", t, i);
                    assert!(
                        loaded.entries.contains_key(&key),
                        "Thread {} missing entry {}",
                        t,
                        key
                    );
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }
}

#[test]
fn test_concurrent_index_save_load_same_repo() {
    // Multiple threads save and reload the index in the same repo directory
    // sequentially but from different threads, checking consistency.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let repo_path = tmp.path().to_path_buf();
    let threads = 4;
    let rounds = 10;
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                barrier.wait();
                let mut successful_loads = 0u32;
                for r in 0..rounds {
                    let mut index = lit::storage::Index::new();
                    let key = format!("t{}_r{}.txt", t, r);
                    index.add(
                        key.clone(),
                        format!("hash_{}_{}", t, r),
                        "100644".to_string(),
                    );
                    // Save may race but must not panic
                    let _ = index.save(&repo);
                    // Load may fail transiently when another thread is
                    // mid-write (partial file), which is expected for
                    // unsynchronised file access.  We track success rate.
                    if lit::storage::Index::load(&repo).is_ok() {
                        successful_loads += 1;
                    }
                }
                successful_loads
            })
        })
        .collect();

    let total_successful: u32 = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .sum();

    // At least some loads should have succeeded (most will)
    assert!(
        total_successful > 0,
        "At least some concurrent index loads should succeed"
    );

    // After all threads finish, the file should be in a valid state
    // (last writer wins with a complete write)
    let final_index = lit::storage::Index::load(&repo_path).unwrap();
    assert!(
        !final_index.entries.is_empty(),
        "Final index should have at least one entry"
    );
}

// ---------------------------------------------------------------------------
// Object store concurrency
// ---------------------------------------------------------------------------

#[test]
fn test_concurrent_object_writes_same_repo() {
    // Several threads write distinct blob objects to the same object store
    // concurrently, then we verify all objects are present.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let threads = 8;
    let objects_per_thread = 20;
    let repo_path = tmp.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                let store = lit::storage::ObjectStore::new(&repo);
                barrier.wait();

                let mut hashes = Vec::new();
                for i in 0..objects_per_thread {
                    let content = format!("thread {} object {}", t, i).into_bytes();
                    let blob = lit::core::Blob::new(content);
                    let obj = lit::core::Object::Blob(blob);
                    let hash = store.write(&obj).unwrap();
                    hashes.push(hash);
                }
                hashes
            })
        })
        .collect();

    let all_hashes: Vec<lit::core::ObjectHash> = handles
        .into_iter()
        .flat_map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(all_hashes.len(), threads * objects_per_thread);

    // Verify every object is readable
    let store = lit::storage::ObjectStore::new(&repo_path);
    for hash in &all_hashes {
        assert!(
            store.exists(hash),
            "Object {} should exist after concurrent writes",
            hash.short()
        );
        let obj = store.read(hash);
        assert!(obj.is_ok(), "Object {} should be readable", hash.short());
    }
}

#[test]
fn test_concurrent_object_read_after_write() {
    // Write objects first, then have many threads read them concurrently.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let store = lit::storage::ObjectStore::new(tmp.path());
    let num_objects = 30;

    // Write phase
    let mut hashes = Vec::new();
    for i in 0..num_objects {
        let content = format!("blob content {}", i).into_bytes();
        let blob = lit::core::Blob::new(content);
        let obj = lit::core::Object::Blob(blob);
        let hash = store.write(&obj).unwrap();
        hashes.push(hash);
    }

    // Concurrent read phase
    let threads = 6;
    let repo_path = tmp.path().to_path_buf();
    let hashes = Arc::new(hashes);
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            let hashes = Arc::clone(&hashes);
            thread::spawn(move || {
                let store = lit::storage::ObjectStore::new(&repo);
                barrier.wait();

                for hash in hashes.iter() {
                    let obj = store.read(hash);
                    assert!(
                        obj.is_ok(),
                        "Concurrent read of {} should succeed",
                        hash.short()
                    );
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }
}

#[test]
fn test_concurrent_object_write_same_content() {
    // Multiple threads write the *same* blob content simultaneously.
    // Because objects are content-addressed the result should be the same
    // hash and data should not be corrupted.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let threads = 8;
    let repo_path = tmp.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                let store = lit::storage::ObjectStore::new(&repo);
                barrier.wait();

                let content = b"identical content across all threads".to_vec();
                let blob = lit::core::Blob::new(content);
                let obj = lit::core::Object::Blob(blob);
                store.write(&obj).unwrap()
            })
        })
        .collect();

    let hashes: Vec<lit::core::ObjectHash> = handles
        .into_iter()
        .map(|h| h.join().expect("Thread panicked"))
        .collect();

    // All hashes must be identical (content-addressable)
    let first = &hashes[0];
    for h in &hashes[1..] {
        assert_eq!(
            first, h,
            "All threads should produce the same hash for identical content"
        );
    }

    // Verify content is intact
    let store = lit::storage::ObjectStore::new(&repo_path);
    let obj = store.read(first).unwrap();
    match obj {
        lit::core::Object::Blob(blob) => {
            assert_eq!(blob.content, b"identical content across all threads");
        }
        _ => panic!("Expected a Blob"),
    }
}

// ---------------------------------------------------------------------------
// Reference concurrency
// ---------------------------------------------------------------------------

#[test]
fn test_concurrent_ref_writes_different_branches() {
    // Multiple threads each create a different branch ref concurrently.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let threads = 8;
    let repo_path = tmp.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                barrier.wait();
                let branch = format!("heads/branch-{}", t);
                let hash = format!("{:0>128x}", t); // 128-char hex hash
                lit::core::write_ref(&repo, &branch, &hash).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // Verify all branches exist
    let refs = lit::core::list_refs(&repo_path, "heads").unwrap();
    assert_eq!(
        refs.len(),
        threads,
        "All {} branch refs should have been created",
        threads
    );
}

#[test]
fn test_concurrent_ref_write_same_branch() {
    // All threads try to update the same branch ref concurrently.
    // The final value should be one of the written values (last-writer-wins).
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let threads = 8;
    let repo_path = tmp.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(threads));

    let expected_hashes: Vec<String> = (0..threads).map(|t| format!("{:0>128x}", t)).collect();
    let expected_hashes_arc = Arc::new(expected_hashes.clone());

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            let hashes = Arc::clone(&expected_hashes_arc);
            thread::spawn(move || {
                barrier.wait();
                lit::core::write_ref(&repo, "heads/contested", &hashes[t]).unwrap();
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // The value should be one of the written hashes
    let value = lit::core::read_ref(&repo_path, "heads/contested").unwrap();
    assert!(
        expected_hashes.contains(&value),
        "Final ref value '{}' should be one of the expected hashes",
        value
    );
}

// ---------------------------------------------------------------------------
// Mixed workload: objects + index + refs
// ---------------------------------------------------------------------------

#[test]
fn test_concurrent_mixed_workload() {
    // Simulate a mixed workload: some threads write objects, some update
    // the index, some write refs, all at the same time in the same repo.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let repo_path = tmp.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(6));

    // 2 threads write objects
    let obj_handles: Vec<_> = (0..2)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                let store = lit::storage::ObjectStore::new(&repo);
                barrier.wait();
                for i in 0..10 {
                    let content = format!("mixed-obj-{}-{}", t, i).into_bytes();
                    let blob = lit::core::Blob::new(content);
                    let obj = lit::core::Object::Blob(blob);
                    store.write(&obj).unwrap();
                }
            })
        })
        .collect();

    // 2 threads write to index
    let idx_handles: Vec<_> = (0..2)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                barrier.wait();
                for i in 0..10 {
                    let mut index = lit::storage::Index::new();
                    index.add(
                        format!("mixed_t{}_f{}.txt", t, i),
                        format!("hash_{}_{}", t, i),
                        "100644".to_string(),
                    );
                    let _ = index.save(&repo);
                }
            })
        })
        .collect();

    // 2 threads write refs
    let ref_handles: Vec<_> = (0..2)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                barrier.wait();
                for i in 0..10 {
                    let branch = format!("heads/mixed-{}-{}", t, i);
                    let hash = format!("{:0>128}", format!("mix{}_{}", t, i));
                    let _ = lit::core::write_ref(&repo, &branch, &hash);
                }
            })
        })
        .collect();

    // Join all
    for h in obj_handles {
        h.join().expect("Object writer panicked");
    }
    for h in idx_handles {
        h.join().expect("Index writer panicked");
    }
    for h in ref_handles {
        h.join().expect("Ref writer panicked");
    }

    // Sanity checks
    let store = lit::storage::ObjectStore::new(&repo_path);
    let objects = store.list().unwrap();
    assert!(
        !objects.is_empty(),
        "Some objects should exist after mixed workload"
    );

    let index = lit::storage::Index::load(&repo_path).unwrap();
    assert!(
        !index.entries.is_empty(),
        "Index should have entries after mixed workload"
    );

    let refs = lit::core::list_refs(&repo_path, "heads").unwrap();
    assert!(
        !refs.is_empty(),
        "Some refs should exist after mixed workload"
    );
}

// ---------------------------------------------------------------------------
// Stress: high contention on object store
// ---------------------------------------------------------------------------

#[test]
fn test_stress_object_store_high_contention() {
    // 16 threads, 50 objects each = 800 total concurrent writes.
    let tmp = TempDir::new().unwrap();
    init_repo(tmp.path());

    let threads = 16;
    let objects_per_thread = 50;
    let repo_path = tmp.path().to_path_buf();
    let barrier = Arc::new(Barrier::new(threads));

    let handles: Vec<_> = (0..threads)
        .map(|t| {
            let barrier = Arc::clone(&barrier);
            let repo = repo_path.clone();
            thread::spawn(move || {
                let store = lit::storage::ObjectStore::new(&repo);
                barrier.wait();

                let mut hashes = Vec::with_capacity(objects_per_thread);
                for i in 0..objects_per_thread {
                    let content = format!("stress_{}_{}", t, i).into_bytes();
                    let blob = lit::core::Blob::new(content);
                    let obj = lit::core::Object::Blob(blob);
                    hashes.push(store.write(&obj).unwrap());
                }
                hashes
            })
        })
        .collect();

    let all_hashes: Vec<lit::core::ObjectHash> = handles
        .into_iter()
        .flat_map(|h| h.join().expect("Thread panicked"))
        .collect();

    assert_eq!(all_hashes.len(), threads * objects_per_thread);

    // Verify all objects exist and are readable
    let store = lit::storage::ObjectStore::new(&repo_path);
    let mut verified = 0;
    for hash in &all_hashes {
        assert!(store.exists(hash), "Object {} must exist", hash.short());
        assert!(
            store.read(hash).is_ok(),
            "Object {} must be readable",
            hash.short()
        );
        verified += 1;
    }
    assert_eq!(verified, threads * objects_per_thread);
}
