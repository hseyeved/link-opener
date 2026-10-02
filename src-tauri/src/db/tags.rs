use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::{AppError, AppResult};

pub const MAX_TAG_LEN: usize = 50;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    /// Number of bookmarks with this tag.
    pub count: i64,
}

/// All tags in use, by name.
pub fn list_all(conn: &Connection) -> AppResult<Vec<Tag>> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.id, t.name, COUNT(bt.bookmark_id)
         FROM tags t JOIN bookmark_tags bt ON bt.tag_id = t.id
         GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
    )?;
    let tags = stmt
        .query_map([], |r| {
            Ok(Tag {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(tags)
}

/// Trims names, drops a leading `#` and empty names, and removes case-insensitive duplicates
/// (the first spelling wins).
pub fn normalize(names: &[String]) -> AppResult<Vec<String>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for name in names {
        let name = name.trim().trim_start_matches('#').trim();
        if name.is_empty() {
            continue;
        }
        if name.chars().count() > MAX_TAG_LEN {
            return Err(AppError::Invalid(format!(
                "tag \"{name}\" is longer than {MAX_TAG_LEN} characters"
            )));
        }
        if seen.insert(name.to_lowercase()) {
            out.push(name.to_string());
        }
    }
    Ok(out)
}

/// Replaces the bookmark's tags with `names` (already normalized). An existing tag keeps
/// its spelling: tagging with "rust" reuses "Rust".
pub fn set_for_bookmark(conn: &Connection, bookmark_id: i64, names: &[String]) -> AppResult<()> {
    conn.execute(
        "DELETE FROM bookmark_tags WHERE bookmark_id = ?1",
        [bookmark_id],
    )?;
    for name in names {
        conn.execute(
            "INSERT INTO tags (name) VALUES (?1) ON CONFLICT (name) DO NOTHING",
            [name],
        )?;
        conn.execute(
            "INSERT INTO bookmark_tags (bookmark_id, tag_id)
             SELECT ?1, id FROM tags WHERE name = ?2",
            params![bookmark_id, name],
        )?;
    }
    delete_unused(conn)
}

/// Removes tags no bookmark uses any more.
pub fn delete_unused(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM bookmark_tags)",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::bookmarks::{self, BookmarkPatch, NewBookmark};
    use crate::db::{folders, open_in_memory};

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn add(conn: &Connection, folder_id: Option<i64>, tags: &[&str]) -> bookmarks::Bookmark {
        let new = NewBookmark {
            folder_id,
            url: "x.com".into(),
            tags: strings(tags),
            ..Default::default()
        };
        bookmarks::create(conn, new).unwrap()
    }

    fn tag_names(conn: &Connection) -> Vec<(String, i64)> {
        list_all(conn)
            .unwrap()
            .into_iter()
            .map(|t| (t.name, t.count))
            .collect()
    }

    #[test]
    fn normalize_names() {
        let names = normalize(&strings(&[" Rust ", "#web", "rust", "", "  # ", "Web"])).unwrap();
        assert_eq!(names, ["Rust", "web"]);
        assert!(matches!(
            normalize(&["x".repeat(51)]),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn create_with_tags_sorted() {
        let conn = open_in_memory().unwrap();
        let b = add(&conn, None, &["zeta", "Alpha", "beta"]);
        assert_eq!(b.tags, ["Alpha", "beta", "zeta"]);
        assert_eq!(
            bookmarks::get(&conn, b.id).unwrap().tags,
            ["Alpha", "beta", "zeta"]
        );
        let untagged = add(&conn, None, &[]);
        assert!(untagged.tags.is_empty());
    }

    #[test]
    fn existing_spelling_is_reused() {
        let conn = open_in_memory().unwrap();
        add(&conn, None, &["Rust"]);
        let b = add(&conn, None, &["rust"]);
        assert_eq!(b.tags, ["Rust"]);
        assert_eq!(tag_names(&conn), [("Rust".to_string(), 2)]);
    }

    #[test]
    fn update_replaces_and_cleans_up() {
        let conn = open_in_memory().unwrap();
        let b = add(&conn, None, &["old", "keep"]);
        let patch = BookmarkPatch {
            tags: Some(strings(&["keep", "new"])),
            ..Default::default()
        };
        assert_eq!(
            bookmarks::update(&conn, b.id, patch).unwrap().tags,
            ["keep", "new"]
        );
        assert_eq!(
            tag_names(&conn),
            [("keep".to_string(), 1), ("new".to_string(), 1)]
        );

        // Patching other fields leaves tags alone.
        let patch = BookmarkPatch {
            title: Some("t".into()),
            ..Default::default()
        };
        assert_eq!(
            bookmarks::update(&conn, b.id, patch).unwrap().tags,
            ["keep", "new"]
        );
    }

    #[test]
    fn deleting_bookmarks_removes_unused_tags() {
        let conn = open_in_memory().unwrap();
        let a = add(&conn, None, &["shared", "only-a"]);
        add(&conn, None, &["shared"]);
        bookmarks::delete(&conn, a.id).unwrap();
        assert_eq!(tag_names(&conn), [("shared".to_string(), 1)]);

        let f = folders::create(&conn, None, "F").unwrap();
        add(&conn, Some(f.id), &["in-folder"]);
        folders::delete(&conn, f.id).unwrap();
        assert_eq!(tag_names(&conn), [("shared".to_string(), 1)]);
    }
}
