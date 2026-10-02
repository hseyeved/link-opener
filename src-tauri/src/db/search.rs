//! Bookmark search: every whitespace-separated term must appear (as a case-insensitive
//! substring) in the title, URL, notes or tags.

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};

use super::bookmarks::{Bookmark, COLUMNS};
use crate::error::AppResult;

pub const MAX_LIMIT: usize = 200;

/// Matching bookmarks, best first. An empty query returns recently opened bookmarks.
pub fn search(conn: &Connection, query: &str, limit: usize) -> AppResult<Vec<Bookmark>> {
    // The trigram tokenizer can't match terms shorter than 3 characters; those use LIKE.
    let (fts_terms, short_terms): (Vec<&str>, Vec<&str>) = query
        .split_whitespace()
        .partition(|t| t.chars().count() >= 3);

    let cols = COLUMNS;
    let mut params: Vec<Value> = Vec::new();

    let mut sql = if fts_terms.is_empty() {
        format!("SELECT {cols} FROM bookmarks b WHERE 1 = 1")
    } else {
        params.push(Value::Text(match_expr(&fts_terms)));
        format!(
            "SELECT {cols} FROM bookmarks_fts JOIN bookmarks b ON b.id = bookmarks_fts.rowid
             WHERE bookmarks_fts MATCH ?"
        )
    };

    for term in &short_terms {
        sql.push_str(
            " AND (b.title LIKE ? ESCAPE '\\' OR b.url LIKE ? ESCAPE '\\' OR b.notes LIKE ? ESCAPE '\\'
                   OR EXISTS (SELECT 1 FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
                              WHERE bt.bookmark_id = b.id AND t.name LIKE ? ESCAPE '\\'))",
        );
        let pattern = format!("%{}%", escape_like(term));
        params.extend(std::iter::repeat_n(Value::Text(pattern), 4));
    }

    // NULLs sort first in ascending order, so `DESC` puts never-opened bookmarks last.
    sql.push_str(if !fts_terms.is_empty() {
        // Column weights: title, url, notes, tags.
        " ORDER BY bm25(bookmarks_fts, 10.0, 4.0, 1.0, 6.0), b.open_count DESC, b.last_opened_at DESC"
    } else if short_terms.is_empty() {
        " ORDER BY b.last_opened_at DESC, b.open_count DESC, b.updated_at DESC"
    } else {
        " ORDER BY b.open_count DESC, b.last_opened_at DESC, b.title"
    });
    sql.push_str(" LIMIT ?");
    params.push(Value::Integer(limit.clamp(1, MAX_LIMIT) as i64));

    let mut stmt = conn.prepare(&sql)?;
    let results = stmt
        .query_map(params_from_iter(params), Bookmark::from_row)?
        .collect::<Result<_, _>>()?;
    Ok(results)
}

/// Each term as an FTS5 phrase (quotes doubled), implicitly ANDed.
fn match_expr(terms: &[&str]) -> String {
    terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::bookmarks::{self, BookmarkPatch, NewBookmark};
    use crate::db::{folders, open_in_memory, MIGRATIONS};

    fn add(conn: &Connection, title: &str, url: &str, notes: &str) -> Bookmark {
        add_in(conn, None, title, url, notes)
    }

    fn add_in(
        conn: &Connection,
        folder_id: Option<i64>,
        title: &str,
        url: &str,
        notes: &str,
    ) -> Bookmark {
        let new = NewBookmark {
            folder_id,
            title: title.into(),
            url: url.into(),
            notes: notes.into(),
            ..Default::default()
        };
        bookmarks::create(conn, new).unwrap()
    }

    fn titles(conn: &Connection, query: &str) -> Vec<String> {
        search(conn, query, 50)
            .unwrap()
            .into_iter()
            .map(|b| b.title)
            .collect()
    }

    fn tag(conn: &Connection, bookmark_id: i64, name: &str) -> i64 {
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", [name])
            .unwrap();
        let tag_id: i64 = conn
            .query_row("SELECT id FROM tags WHERE name = ?1", [name], |r| r.get(0))
            .unwrap();
        conn.execute(
            "INSERT INTO bookmark_tags (bookmark_id, tag_id) VALUES (?1, ?2)",
            [bookmark_id, tag_id],
        )
        .unwrap();
        tag_id
    }

    #[test]
    fn substring_match_in_any_column() {
        let conn = open_in_memory().unwrap();
        add(&conn, "Rust Language", "rust-lang.org", "");
        add(&conn, "Svelte", "svelte.dev/docs", "component framework");
        add(&conn, "Nothing", "example.com", "");

        assert_eq!(titles(&conn, "lang"), ["Rust Language"]); // inside a word
        assert_eq!(titles(&conn, "RUST"), ["Rust Language"]); // case-insensitive
        assert_eq!(titles(&conn, "/docs"), ["Svelte"]); // url
        assert_eq!(titles(&conn, "framew"), ["Svelte"]); // notes
        assert!(titles(&conn, "nomatch").is_empty());
    }

    #[test]
    fn all_terms_must_match() {
        let conn = open_in_memory().unwrap();
        add(&conn, "Rust book", "doc.rust-lang.org/book", "");
        add(&conn, "Rust std", "doc.rust-lang.org/std", "");
        assert_eq!(titles(&conn, "rust book"), ["Rust book"]);
        assert_eq!(titles(&conn, "book rust"), ["Rust book"]);
        assert_eq!(titles(&conn, "rust").len(), 2);
    }

    #[test]
    fn short_terms_use_like() {
        let conn = open_in_memory().unwrap();
        add(&conn, "Go by Example", "gobyexample.com", "");
        add(&conn, "Rust", "rust-lang.org", "");
        assert_eq!(titles(&conn, "go"), ["Go by Example"]);
        assert_eq!(titles(&conn, "go exam"), ["Go by Example"]);
        assert_eq!(titles(&conn, "r"), ["Rust"]);
    }

    #[test]
    fn special_characters_are_literal() {
        let conn = open_in_memory().unwrap();
        add(&conn, r#"100% "quoted" a_b"#, "x.com", "");
        add(&conn, "100 percent", "y.com", "");
        assert_eq!(titles(&conn, "%"), [r#"100% "quoted" a_b"#]);
        assert_eq!(titles(&conn, "_"), [r#"100% "quoted" a_b"#]);
        assert_eq!(titles(&conn, r#""quoted""#), [r#"100% "quoted" a_b"#]);
        assert_eq!(titles(&conn, "AND OR NOT").len(), 0); // FTS operators are just text
        assert_eq!(titles(&conn, "*").len(), 0);
    }

    #[test]
    fn title_match_ranks_above_notes_match() {
        let conn = open_in_memory().unwrap();
        add(&conn, "Other", "a.com", "mentions kubernetes once");
        add(&conn, "Kubernetes docs", "b.com", "");
        assert_eq!(titles(&conn, "kubernetes"), ["Kubernetes docs", "Other"]);
    }

    #[test]
    fn empty_query_lists_recently_opened_first() {
        let conn = open_in_memory().unwrap();
        let a = add(&conn, "A", "a.com", "");
        add(&conn, "B", "b.com", "");
        bookmarks::record_open(&conn, a.id).unwrap();
        assert_eq!(titles(&conn, "  ")[0], "A");
        assert_eq!(search(&conn, "", 1).unwrap().len(), 1);
    }

    #[test]
    fn index_follows_updates_and_deletes() {
        let conn = open_in_memory().unwrap();
        let b = add(&conn, "Before", "x.com", "");
        bookmarks::update(
            &conn,
            b.id,
            BookmarkPatch {
                title: Some("After".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(titles(&conn, "before").is_empty());
        assert_eq!(titles(&conn, "after"), ["After"]);

        bookmarks::delete(&conn, b.id).unwrap();
        assert!(titles(&conn, "after").is_empty());
        let fts_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM bookmarks_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts_rows, 0);
    }

    #[test]
    fn folder_delete_cascade_cleans_index() {
        let conn = open_in_memory().unwrap();
        let f = folders::create(&conn, None, "F").unwrap();
        add_in(&conn, Some(f.id), "Inside", "x.com", "");
        folders::delete(&conn, f.id).unwrap();
        assert!(titles(&conn, "inside").is_empty());
        let fts_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM bookmarks_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts_rows, 0);
    }

    #[test]
    fn tags_are_searchable() {
        let conn = open_in_memory().unwrap();
        let b = add(&conn, "Plain", "x.com", "");
        let tag_id = tag(&conn, b.id, "research");
        tag(&conn, b.id, "ml");
        assert_eq!(titles(&conn, "research"), ["Plain"]);
        assert_eq!(titles(&conn, "ml"), ["Plain"]); // short term: LIKE on tag names

        conn.execute("UPDATE tags SET name = 'reading' WHERE id = ?1", [tag_id])
            .unwrap();
        assert!(titles(&conn, "research").is_empty());
        assert_eq!(titles(&conn, "reading"), ["Plain"]);

        conn.execute("DELETE FROM bookmark_tags WHERE tag_id = ?1", [tag_id])
            .unwrap();
        assert!(titles(&conn, "reading").is_empty());
    }

    #[test]
    fn migration_backfills_existing_bookmarks() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        MIGRATIONS.to_version(&mut conn, 1).unwrap();
        conn.execute(
            "INSERT INTO bookmarks (title, url, position, created_at, updated_at)
             VALUES ('Old bookmark', 'https://old.example', 0, 0, 0)",
            [],
        )
        .unwrap();
        MIGRATIONS.to_latest(&mut conn).unwrap();
        assert_eq!(titles(&conn, "old"), ["Old bookmark"]);
    }
}
