use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long a write made by the app itself suppresses the watcher event it triggers.
const OWN_WRITE_TTL: Duration = Duration::from_secs(10);

/// Handle to a running file watcher thread.
pub struct WatcherHandle {
    stop: Arc<AtomicBool>,
}

impl WatcherHandle {
    pub fn new(stop: Arc<AtomicBool>) -> Self {
        WatcherHandle { stop }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Application state shared across Tauri commands
pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub vault_path: Mutex<Option<PathBuf>>,
    /// How the open vault's files are reached.
    ///
    /// A real directory on desktop and iOS; on Android the Storage Access
    /// Framework bridge, since the folder the user grants has no path. Chosen
    /// once when a vault opens so nothing below has to ask which platform it is
    /// on. Defaults to the native one, which is also what an unopened vault
    /// wants — the alternative would be an Option every caller has to unwrap.
    vault_fs: Mutex<Arc<dyn db::VaultFs>>,
    own_writes: Mutex<HashMap<String, (String, Instant)>>,
    watcher: Mutex<Option<WatcherHandle>>,
}

/// Lock a mutex, recovering from poisoning instead of failing every later command.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            db: Mutex::new(None),
            vault_path: Mutex::new(None),
            vault_fs: Mutex::new(Arc::new(db::NativeFs)),
            own_writes: Mutex::new(HashMap::new()),
            watcher: Mutex::new(None),
        }
    }

    pub fn with_db<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        let guard = lock(&self.db);
        match guard.as_ref() {
            Some(conn) => f(conn),
            None => Err("No vault is open. Please open a vault first.".to_string()),
        }
    }

    pub fn vault_path(&self) -> Result<PathBuf, String> {
        lock(&self.vault_path)
            .clone()
            .ok_or_else(|| "No vault is open.".to_string())
    }

    pub fn set_db(&self, conn: Option<Connection>) {
        *lock(&self.db) = conn;
    }

    /// The open vault's file access. Cloned rather than borrowed so a caller
    /// can do file work without holding the lock across it.
    pub fn vault_fs(&self) -> Arc<dyn db::VaultFs> {
        Arc::clone(&lock(&self.vault_fs))
    }

    pub fn set_vault_fs(&self, fs: Arc<dyn db::VaultFs>) {
        *lock(&self.vault_fs) = fs;
    }

    pub fn set_vault_path(&self, path: Option<PathBuf>) {
        *lock(&self.vault_path) = path;
    }

    /// Record that the app itself just wrote `path` with the given content hash so
    /// the file watcher can ignore the event its own write produces.
    pub fn note_own_write(&self, path: &str, hash: &str) {
        let mut guard = lock(&self.own_writes);
        guard.retain(|_, (_, at)| at.elapsed() < OWN_WRITE_TTL);
        guard.insert(path.to_string(), (hash.to_string(), Instant::now()));
    }

    /// Returns true if `path` currently on disk with `hash` is a write this app just
    /// made. Consumes the record so a later external edit is not suppressed.
    pub fn take_own_write(&self, path: &str, hash: &str) -> bool {
        let mut guard = lock(&self.own_writes);
        guard.retain(|_, (_, at)| at.elapsed() < OWN_WRITE_TTL);
        match guard.get(path) {
            Some((known, _)) if known == hash => {
                guard.remove(path);
                true
            }
            _ => false,
        }
    }

    pub fn clear_own_writes(&self) {
        lock(&self.own_writes).clear();
    }

    /// Install a new watcher handle, stopping any watcher that was already running.
    pub fn set_watcher(&self, handle: WatcherHandle) {
        let mut guard = lock(&self.watcher);
        if let Some(previous) = guard.take() {
            previous.stop();
        }
        *guard = Some(handle);
    }

    /// Stop the running watcher, if any.
    pub fn stop_watcher(&self) {
        if let Some(previous) = lock(&self.watcher).take() {
            previous.stop();
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_from_a_poisoned_lock() {
        let state = Arc::new(AppState::new());
        let poisoner = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.db.lock().unwrap();
            panic!("poison the mutex");
        })
        .join();

        assert!(state.db.lock().is_err());
        let err = state.with_db(|_| Ok(())).unwrap_err();
        assert!(err.contains("No vault is open"));
        assert!(state.vault_path().is_err());
    }

    #[test]
    fn own_write_records_are_consumed_once() {
        let state = AppState::new();
        state.note_own_write("/vault/a.org", "hash-a");
        assert!(!state.take_own_write("/vault/a.org", "other"));
        assert!(state.take_own_write("/vault/a.org", "hash-a"));
        assert!(!state.take_own_write("/vault/a.org", "hash-a"));
    }

    #[test]
    fn stopping_watcher_sets_the_flag() {
        let state = AppState::new();
        let flag = Arc::new(AtomicBool::new(false));
        state.set_watcher(WatcherHandle::new(Arc::clone(&flag)));

        let second = Arc::new(AtomicBool::new(false));
        state.set_watcher(WatcherHandle::new(Arc::clone(&second)));
        assert!(flag.load(Ordering::SeqCst));
        assert!(!second.load(Ordering::SeqCst));

        state.stop_watcher();
        assert!(second.load(Ordering::SeqCst));
    }
}
