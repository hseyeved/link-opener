use rusqlite::{params, Connection, OptionalExtension};

use crate::error::AppResult;

/// Last browser/profile/private choice, as `LaunchTarget` JSON.
pub const LAST_TARGET: &str = "last_target";
/// Global shortcut text; unset = the default, "" = none.
pub const GLOBAL_SHORTCUT: &str = "global_shortcut";
/// `BrowserPrefs` JSON: browser order, hidden browsers/profiles, labels, custom browsers.
pub const BROWSER_PREFS: &str = "browser_prefs";
/// "false" = closing the window quits; anything else hides it to the tray.
pub const CLOSE_TO_TRAY: &str = "close_to_tray";

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn get_set_overwrite() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get(&conn, "k").unwrap(), None);
        set(&conn, "k", "1").unwrap();
        set(&conn, "k", "2").unwrap();
        assert_eq!(get(&conn, "k").unwrap().as_deref(), Some("2"));
    }
}
