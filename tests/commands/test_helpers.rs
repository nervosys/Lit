/// Thread-safe working directory guard for parallel test execution.
///
/// `set_current_dir` is process-global, so parallel tests that change
/// the working directory race with each other. This module provides a
/// mutex-backed RAII guard that serialises directory changes.
///
/// The guard is **reentrant**: if the current thread already holds the
/// CWD mutex (e.g. a test function acquired a `CwdGuard` and then
/// calls a helper like `create_commit` which also creates one), the
/// inner guard just changes the directory without re-locking.
use std::cell::Cell;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

static CWD_MUTEX: Mutex<()> = Mutex::new(());

thread_local! {
    static CWD_LOCKED: Cell<bool> = const { Cell::new(false) };
}

/// RAII guard that holds the CWD mutex, changes to `path`, and restores
/// the original directory on drop.
pub struct CwdGuard {
    original: std::path::PathBuf,
    /// `None` when this is a reentrant (inner) guard — the outer guard
    /// owns the real lock and will restore the directory.
    _lock: Option<MutexGuard<'static, ()>>,
}

impl CwdGuard {
    /// Acquire the global CWD lock and change to `path`.
    ///
    /// If the current thread already holds the lock, this is a no-lock
    /// reentrant acquisition that still changes the directory but will
    /// *not* restore it on drop (the outer guard does that).
    pub fn new(path: &Path) -> Self {
        let already_held = CWD_LOCKED.with(|c| c.get());
        if already_held {
            // Reentrant: just change dir, no lock, no restore on drop.
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
