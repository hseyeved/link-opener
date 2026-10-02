pub mod bookmarks;
pub mod folders;
mod migrations;
pub mod search;
pub mod settings;
pub mod tags;
pub mod targets;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::error::AppResult;
pub use migrations::MIGRATIONS;

/// The app's single connection, held in Tauri state.
pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock doesn't leave SQLite in a bad state.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    init(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> AppResult<Connection> {
    let mut conn = Connection::open_in_memory()?;
    init(&mut conn)?;
    Ok(conn)
}

fn init(conn: &mut Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    MIGRATIONS.to_latest(conn)?;
    Ok(())
}

/// Sets `position` to each id's index in `ids`, skipping rows already in place.
/// `table` is one of our own table names, never user input.
pub(crate) fn renumber(conn: &Connection, table: &str, ids: &[i64]) -> AppResult<()> {
    let mut stmt = conn.prepare_cached(&format!(
        "UPDATE {table} SET position = ?2 WHERE id = ?1 AND position != ?2"
    ))?;
    for (position, id) in ids.iter().enumerate() {
        stmt.execute(params![id, position as i64])?;
    }
    Ok(())
}

/// Current time as unix milliseconds.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        MIGRATIONS.validate().unwrap();
    }

    #[test]
    fn foreign_keys_are_enabled() {
        let conn = open_in_memory().unwrap();
        let on: bool = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert!(on);
    }

    #[test]
    fn fk_rejects_missing_parent() {
        let conn = open_in_memory().unwrap();
        let err = conn
            .execute(
                "INSERT INTO folders (parent_id, name, position) VALUES (999, 'x', 0)",
                [],
            )
            .unwrap_err();
        assert!(err.to_string().contains("FOREIGN KEY"), "{err}");
    }
}
