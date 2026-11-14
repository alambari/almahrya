//! Operational state in SQLite: received events, read state, agent run history.
//!
//! Configuration does not live here; it stays in versioned config files.

mod events;
mod migrate;
mod sessions;

use rusqlite::Connection;
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("stored payload is invalid: {0}")]
    Payload(#[from] serde_json::Error),
    #[error("stored value is invalid: {0}")]
    Corrupt(String),
    #[error("store lock poisoned")]
    Poisoned,
}

/// SQLite-backed store. Cheap to share behind an `Arc`; calls are serialized.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Opens (creating if needed) the database at `path`, enables WAL and runs migrations.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate::run(&mut conn)?;
        Ok(Store {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn.lock().map_err(|_| StoreError::Poisoned)
    }
}
