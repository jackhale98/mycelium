use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

/// Application state shared across Tauri commands
pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    pub vault_path: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            db: Mutex::new(None),
            vault_path: Mutex::new(None),
        }
    }

    pub fn with_db<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Connection) -> Result<T, String>,
    {
        let guard = self.db.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(conn) => f(conn),
            None => Err("No vault is open. Please open a vault first.".to_string()),
        }
    }

    pub fn vault_path(&self) -> Result<PathBuf, String> {
        let guard = self.vault_path.lock().map_err(|e| e.to_string())?;
        guard
            .clone()
            .ok_or_else(|| "No vault is open.".to_string())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
