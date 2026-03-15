pub mod index;
pub mod query;
pub mod schema;
pub mod sync;

pub use query::{
    BacklinkRecord, FileRecord, ForwardLink, GraphData, GraphLink, GraphNode,
    NodeRecord, SearchResult, TagCount,
};
pub use sync::{SyncError, SyncResult};

use rusqlite::Connection;
use std::path::Path;

/// Open (or create) the database for a vault.
/// The database is stored as `.mycelium.db` inside the vault directory.
pub fn open_db(vault_path: &str) -> Result<Connection, SyncError> {
    let db_path = Path::new(vault_path).join(".mycelium.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| SyncError::Database(e.to_string()))?;

    schema::init_schema(&conn).map_err(|e| SyncError::Database(e.to_string()))?;
    schema::init_fts(&conn).map_err(|e| SyncError::Database(e.to_string()))?;

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
