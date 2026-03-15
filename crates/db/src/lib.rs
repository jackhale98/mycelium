pub mod index;
pub mod query;
pub mod schema;
pub mod sync;

pub use query::{
    BacklinkRecord, FileRecord, ForwardLink, GraphData, GraphLink, GraphNode,
    HeadlineRecord, NodeRecord, SearchResult, TagCount,
};
pub use sync::{SyncError, SyncResult};

use rusqlite::Connection;
use std::path::{Path, PathBuf};

/// Get the app data directory for storing databases.
/// Uses platform-specific conventions:
///   Linux: ~/.local/share/mycelium/
///   macOS: ~/Library/Application Support/mycelium/
///   Windows: %APPDATA%/mycelium/
pub fn app_data_dir() -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("~"))
    } else if cfg!(target_os = "windows") {
        dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("~/.local/share"))
    };
    base.join("mycelium")
}

/// Generate a stable database filename from a vault path.
/// Uses a hash of the absolute vault path so each vault gets its own DB.
fn db_filename_for_vault(vault_path: &str) -> String {
    use sha2::{Digest, Sha256};
    let canonical = Path::new(vault_path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(vault_path));
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    format!("vault-{}.db", &hash[..12])
}

/// Open (or create) the database for a vault.
/// Stored in the app data directory, NOT in the vault itself.
pub fn open_db(vault_path: &str) -> Result<Connection, SyncError> {
    let data_dir = app_data_dir();
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| SyncError::Io(format!("Failed to create data dir: {e}")))?;

    let db_path = data_dir.join(db_filename_for_vault(vault_path));
    let conn = Connection::open(&db_path)
        .map_err(|e| SyncError::Database(e.to_string()))?;

    schema::init_schema(&conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(&conn).map_err(|e| SyncError::Database(e.to_string()))?;

    // Store the vault path in the DB so we can resolve relative paths
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
         INSERT OR REPLACE INTO meta (key, value) VALUES ('vault_path', '{}');
         INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', '1');",
        vault_path.replace('\'', "''")
    )).map_err(|e| SyncError::Database(e.to_string()))?;

    Ok(conn)
}

/// Open an in-memory database (for testing)
pub fn open_memory_db() -> Result<Connection, SyncError> {
    let conn = Connection::open_in_memory()
        .map_err(|e| SyncError::Database(e.to_string()))?;

    schema::init_schema(&conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(&conn).map_err(|e| SyncError::Database(e.to_string()))?;

    Ok(conn)
}
