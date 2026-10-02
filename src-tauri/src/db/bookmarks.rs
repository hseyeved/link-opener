use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Deserializer, Serialize};

use super::{folders, now_ms, tags};
use crate::browsers::LaunchTarget;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub notes: String,
    pub favicon: Option<String>,
    pub position: i64,
    /// Browser to open with instead of asking; overrides the folders' defaults.
    pub default_target: Option<LaunchTarget>,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_opened_at: Option<i64>,
    pub open_count: i64,
    /// Tag names, sorted case-insensitively.
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NewBookmark {
    pub folder_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub notes: String,
    pub tags: Vec<String>,
    pub default_target: Option<LaunchTarget>,
    /// File name in the favicons dir, from `fetch_metadata`.
    pub favicon: Option<String>,
}

/// Fields left out are unchanged. `folderId: null` moves the bookmark to Unfiled;
/// `defaultTarget: null` clears the default browser.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookmarkPatch {
    pub title: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    #[serde(deserialize_with = "present")]
    pub folder_id: Option<Option<i64>>,
    pub tags: Option<Vec<String>>,
    #[serde(deserialize_with = "present")]
    pub default_target: Option<Option<LaunchTarget>>,
    #[serde(deserialize_with = "present")]
    pub favicon: Option<Option<String>>,
}

/// Distinguishes a field set to `null` (`Some(None)`) from a missing one (`None`).
pub(crate) fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Selects from `bookmarks b`. Tags come back as one string separated by U+001F.
pub(crate) const COLUMNS: &str =
    "b.id, b.folder_id, b.title, b.url, b.notes, b.favicon, b.position, \
    b.default_target, b.created_at, b.updated_at, b.last_opened_at, b.open_count, \
    (SELECT group_concat(name, char(31)) FROM (
        SELECT t.name FROM bookmark_tags bt JOIN tags t ON t.id = bt.tag_id
        WHERE bt.bookmark_id = b.id ORDER BY t.name COLLATE NOCASE))";

impl Bookmark {
    pub(crate) fn from_row(row: &Row) -> rusqlite::Result<Self> {
        let tags: Option<String> = row.get(12)?;
        Ok(Self {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            title: row.get(2)?,
            url: row.get(3)?,
            notes: row.get(4)?,
            favicon: row.get(5)?,
            position: row.get(6)?,
            default_target: parse_target(row.get(7)?),
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
            last_opened_at: row.get(10)?,
            open_count: row.get(11)?,
            tags: tags
                .map(|t| t.split('\u{1f}').map(str::to_string).collect())
                .unwrap_or_default(),
        })
    }
}

/// A stored target that no longer parses is treated as unset.
pub(crate) fn parse_target(json: Option<String>) -> Option<LaunchTarget> {
    serde_json::from_str(&json?).ok()
}

pub(crate) fn target_json(target: Option<&LaunchTarget>) -> AppResult<Option<String>> {
    target
        .map(|t| serde_json::to_string(t).map_err(|e| AppError::Invalid(e.to_string())))
        .transpose()
}

/// Bookmarks directly in `folder_id` (`None` = Unfiled), in position order.
pub fn list(conn: &Connection, folder_id: Option<i64>) -> AppResult<Vec<Bookmark>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM bookmarks b WHERE b.folder_id IS ?1 ORDER BY b.position, b.id"
    ))?;
    let bookmarks = stmt
        .query_map([folder_id], Bookmark::from_row)?
        .collect::<Result<_, _>>()?;
    Ok(bookmarks)
}

/// Every bookmark, by folder then position.
pub fn list_all(conn: &Connection) -> AppResult<Vec<Bookmark>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM bookmarks b ORDER BY b.folder_id, b.position, b.id"
    ))?;
    let bookmarks = stmt
        .query_map([], Bookmark::from_row)?
        .collect::<Result<_, _>>()?;
    Ok(bookmarks)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Bookmark> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM bookmarks b WHERE b.id = ?1"),
        [id],
        Bookmark::from_row,
    )
    .optional()?
    .ok_or(AppError::NotFound("bookmark"))
}

pub fn create(conn: &Connection, new: NewBookmark) -> AppResult<Bookmark> {
    let url = normalize_url(&new.url)?;
    let tag_names = tags::normalize(&new.tags)?;
    validate_favicon(new.favicon.as_deref())?;
    ensure_folder(conn, new.folder_id)?;
    let tx = conn.unchecked_transaction()?;
    let position = next_position(&tx, new.folder_id)?;
    let now = now_ms();
    tx.execute(
        "INSERT INTO bookmarks (folder_id, title, url, notes, position, default_target, favicon, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            new.folder_id,
            new.title.trim(),
            url,
            new.notes,
            position,
            target_json(new.default_target.as_ref())?,
            new.favicon,
            now
        ],
    )?;
    let id = tx.last_insert_rowid();
    tags::set_for_bookmark(&tx, id, &tag_names)?;
    tx.commit()?;
    get(conn, id)
}

pub fn update(conn: &Connection, id: i64, patch: BookmarkPatch) -> AppResult<Bookmark> {
    let mut b = get(conn, id)?;
    if let Some(title) = patch.title {
        b.title = title.trim().to_string();
    }
    if let Some(url) = patch.url {
        b.url = normalize_url(&url)?;
    }
    if let Some(notes) = patch.notes {
        b.notes = notes;
    }
    if let Some(target) = patch.default_target {
        b.default_target = target;
    }
    if let Some(favicon) = patch.favicon {
        validate_favicon(favicon.as_deref())?;
        b.favicon = favicon;
    }
    let tag_names = patch.tags.as_deref().map(tags::normalize).transpose()?;

    let tx = conn.unchecked_transaction()?;
    if let Some(folder_id) = patch.folder_id {
        if folder_id != b.folder_id {
            ensure_folder(&tx, folder_id)?;
            b.folder_id = folder_id;
            b.position = next_position(&tx, folder_id)?;
        }
    }
    tx.execute(
        "UPDATE bookmarks
         SET folder_id = ?2, title = ?3, url = ?4, notes = ?5, position = ?6,
             default_target = ?7, favicon = ?8, updated_at = ?9
         WHERE id = ?1",
        params![
            id,
            b.folder_id,
            b.title,
            b.url,
            b.notes,
            b.position,
            target_json(b.default_target.as_ref())?,
            b.favicon,
            now_ms()
        ],
    )?;
    if let Some(names) = tag_names {
        tags::set_for_bookmark(&tx, id, &names)?;
    }
    tx.commit()?;
    get(conn, id)
}

/// Moves the bookmark into `folder_id` at `index` among that folder's other bookmarks
/// (`None` or past the end = last). Also reorders within the same folder.
pub fn move_to(
    conn: &Connection,
    id: i64,
    folder_id: Option<i64>,
    index: Option<usize>,
) -> AppResult<Bookmark> {
    let b = get(conn, id)?;
    ensure_folder(conn, folder_id)?;
    let tx = conn.unchecked_transaction()?;
    let mut ids: Vec<i64> = tx
        .prepare_cached(
            "SELECT id FROM bookmarks WHERE folder_id IS ?1 AND id != ?2 ORDER BY position, id",
        )?
        .query_map(params![folder_id, id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    ids.insert(index.unwrap_or(ids.len()).min(ids.len()), id);
    if folder_id != b.folder_id {
        tx.execute(
            "UPDATE bookmarks SET folder_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, folder_id, now_ms()],
        )?;
    }
    super::renumber(&tx, "bookmarks", &ids)?;
    tx.commit()?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM bookmarks WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound("bookmark"));
    }
    tags::delete_unused(conn)
}

/// Stores fetched metadata: sets the favicon (when one was found) and fills the title only
/// if it is still empty, so a title the user typed is never overwritten.
pub fn apply_metadata(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    favicon: Option<&str>,
) -> AppResult<Bookmark> {
    validate_favicon(favicon)?;
    get(conn, id)?;
    if let Some(favicon) = favicon {
        conn.execute(
            "UPDATE bookmarks SET favicon = ?2 WHERE id = ?1",
            params![id, favicon],
        )?;
    }
    if let Some(title) = title {
        conn.execute(
            "UPDATE bookmarks SET title = ?2 WHERE id = ?1 AND title = ''",
            params![id, title],
        )?;
    }
    get(conn, id)
}

/// Web bookmarks without a favicon, as (id, url).
pub fn missing_favicons(conn: &Connection) -> AppResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, url FROM bookmarks
         WHERE favicon IS NULL AND (url LIKE 'http://%' OR url LIKE 'https://%')
         ORDER BY id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(rows)
}

/// Favicon names that bookmarks use, for cleaning up the favicons dir.
pub fn favicons_in_use(conn: &Connection) -> AppResult<std::collections::HashSet<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT favicon FROM bookmarks WHERE favicon IS NOT NULL")?;
    let names = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}

/// The frontend builds a file path from the name, so only accept names we generate.
fn validate_favicon(name: Option<&str>) -> AppResult<()> {
    match name {
        Some(name) if !crate::metadata::is_favicon_name(name) => {
            Err(AppError::Invalid(format!("invalid favicon name {name:?}")))
        }
        _ => Ok(()),
    }
}

/// Bumps `open_count` and sets `last_opened_at`. Doesn't touch `updated_at`.
pub fn record_open(conn: &Connection, id: i64) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE bookmarks SET open_count = open_count + 1, last_opened_at = ?2 WHERE id = ?1",
        params![id, now_ms()],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("bookmark"));
    }
    Ok(())
}

fn ensure_folder(conn: &Connection, folder_id: Option<i64>) -> AppResult<()> {
    match folder_id {
        Some(id) if !folders::exists(conn, id)? => Err(AppError::NotFound("folder")),
        _ => Ok(()),
    }
}

pub(crate) fn next_position(conn: &Connection, folder_id: Option<i64>) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM bookmarks WHERE folder_id IS ?1",
        [folder_id],
        |r| r.get(0),
    )?)
}

/// Trims the URL and adds `https://` when it has no scheme.
pub fn normalize_url(url: &str) -> AppResult<String> {
    let url = url.trim();
    if url.is_empty() {
        return Err(AppError::Invalid("URL must not be empty".into()));
    }
    if has_scheme(url) {
        Ok(url.to_string())
    } else {
        Ok(format!("https://{url}"))
    }
}

fn has_scheme(url: &str) -> bool {
    if url.contains("://") {
        return true;
    }
    // Schemes without `//`. A bare `host:port` must not count as a scheme.
    let lower = url.to_ascii_lowercase();
    ["mailto:", "about:", "data:", "tel:"]
        .iter()
        .any(|s| lower.starts_with(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn new(folder_id: Option<i64>, title: &str, url: &str) -> NewBookmark {
        NewBookmark {
            folder_id,
            title: title.into(),
            url: url.into(),
            ..Default::default()
        }
    }

    #[test]
    fn create_get_list() {
        let conn = open_in_memory().unwrap();
        let b = create(&conn, new(None, " Rust ", "rust-lang.org")).unwrap();
        assert_eq!(b.title, "Rust");
        assert_eq!(b.url, "https://rust-lang.org");
        assert_eq!(b.folder_id, None);
        assert_eq!(b.open_count, 0);
        assert_eq!(b.created_at, b.updated_at);
        assert_eq!(get(&conn, b.id).unwrap(), b);
        assert_eq!(list(&conn, None).unwrap(), vec![b]);
    }

    #[test]
    fn list_is_scoped_to_folder() {
        let conn = open_in_memory().unwrap();
        let f = folders::create(&conn, None, "F").unwrap();
        create(&conn, new(None, "a", "a.com")).unwrap();
        create(&conn, new(Some(f.id), "b", "b.com")).unwrap();
        assert_eq!(list(&conn, None).unwrap()[0].title, "a");
        assert_eq!(list(&conn, Some(f.id)).unwrap()[0].title, "b");
        assert_eq!(list(&conn, None).unwrap().len(), 1);
    }

    #[test]
    fn positions_append() {
        let conn = open_in_memory().unwrap();
        let f = folders::create(&conn, None, "F").unwrap();
        let a = create(&conn, new(Some(f.id), "a", "a.com")).unwrap();
        let b = create(&conn, new(Some(f.id), "b", "b.com")).unwrap();
        let u = create(&conn, new(None, "u", "u.com")).unwrap();
        assert_eq!((a.position, b.position, u.position), (0, 1, 0));
        let titles: Vec<_> = list(&conn, Some(f.id))
            .unwrap()
            .into_iter()
            .map(|b| b.title)
            .collect();
        assert_eq!(titles, ["a", "b"]);
    }

    #[test]
    fn create_rejects_missing_folder_and_empty_url() {
        let conn = open_in_memory().unwrap();
        assert!(matches!(
            create(&conn, new(Some(7), "x", "x.com")),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            create(&conn, new(None, "x", "  ")),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn update_fields() {
        let conn = open_in_memory().unwrap();
        let b = create(&conn, new(None, "Old", "old.com")).unwrap();
        let patch = BookmarkPatch {
            title: Some("New".into()),
            notes: Some("some notes".into()),
            ..Default::default()
        };
        let u = update(&conn, b.id, patch).unwrap();
        assert_eq!(u.title, "New");
        assert_eq!(u.notes, "some notes");
        assert_eq!(u.url, "https://old.com");
        assert!(u.updated_at >= b.updated_at);

        let u = update(
            &conn,
            b.id,
            BookmarkPatch {
                url: Some("http://new.com".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(u.url, "http://new.com");
        assert!(matches!(
            update(
                &conn,
                b.id,
                BookmarkPatch {
                    url: Some("".into()),
                    ..Default::default()
                }
            ),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            update(&conn, 999, BookmarkPatch::default()),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn folder_change_appends_to_new_folder() {
        let conn = open_in_memory().unwrap();
        let f = folders::create(&conn, None, "F").unwrap();
        create(&conn, new(Some(f.id), "a", "a.com")).unwrap();
        create(&conn, new(Some(f.id), "b", "b.com")).unwrap();
        let u = create(&conn, new(None, "u", "u.com")).unwrap();
        assert_eq!(u.position, 0);

        let moved = update(
            &conn,
            u.id,
            BookmarkPatch {
                folder_id: Some(Some(f.id)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(moved.folder_id, Some(f.id));
        assert_eq!(moved.position, 2);

        // Same folder: position unchanged.
        let same = update(
            &conn,
            u.id,
            BookmarkPatch {
                folder_id: Some(Some(f.id)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(same.position, 2);

        let back = update(
            &conn,
            u.id,
            BookmarkPatch {
                folder_id: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(back.folder_id, None);
        assert_eq!(back.position, 0);

        assert!(matches!(
            update(
                &conn,
                u.id,
                BookmarkPatch {
                    folder_id: Some(Some(999)),
                    ..Default::default()
                }
            ),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn move_reorders_and_changes_folder() {
        let conn = open_in_memory().unwrap();
        let f = folders::create(&conn, None, "F").unwrap();
        let a = create(&conn, new(None, "a", "a.com")).unwrap();
        let b = create(&conn, new(None, "b", "b.com")).unwrap();
        let c = create(&conn, new(None, "c", "c.com")).unwrap();
        let titles = |folder| -> Vec<String> {
            list(&conn, folder)
                .unwrap()
                .into_iter()
                .map(|b| b.title)
                .collect()
        };

        move_to(&conn, c.id, None, Some(0)).unwrap();
        assert_eq!(titles(None), ["c", "a", "b"]);
        let same = move_to(&conn, a.id, None, Some(2)).unwrap();
        assert_eq!(titles(None), ["c", "b", "a"]);
        assert_eq!(same.updated_at, a.updated_at); // reordering isn't an edit

        let moved = move_to(&conn, b.id, Some(f.id), None).unwrap();
        assert_eq!(moved.folder_id, Some(f.id));
        assert_eq!(titles(None), ["c", "a"]);
        assert_eq!(titles(Some(f.id)), ["b"]);
        move_to(&conn, c.id, Some(f.id), Some(0)).unwrap();
        assert_eq!(titles(Some(f.id)), ["c", "b"]);

        assert!(matches!(
            move_to(&conn, a.id, Some(999), None),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            move_to(&conn, 999, None, None),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn default_target_set_and_clear() {
        let conn = open_in_memory().unwrap();
        let target = LaunchTarget {
            browser_id: "firefox".into(),
            profile_id: Some("work".into()),
            private: true,
        };
        let mut n = new(None, "x", "x.com");
        n.default_target = Some(target.clone());
        let b = create(&conn, n).unwrap();
        assert_eq!(b.default_target.as_ref(), Some(&target));

        let untouched = update(
            &conn,
            b.id,
            BookmarkPatch {
                title: Some("y".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(untouched.default_target.as_ref(), Some(&target));
        let cleared = update(
            &conn,
            b.id,
            BookmarkPatch {
                default_target: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(cleared.default_target, None);
    }

    #[test]
    fn favicon_and_metadata() {
        let conn = open_in_memory().unwrap();
        let icon = "0123456789abcdef0123456789abcdef.png";
        let mut n = new(None, "", "x.com");
        n.favicon = Some("../evil.png".into());
        assert!(matches!(
            create(&conn, n.clone()),
            Err(AppError::Invalid(_))
        ));
        n.favicon = Some(icon.into());
        let b = create(&conn, n).unwrap();
        assert_eq!(b.favicon.as_deref(), Some(icon));

        // Metadata fills an empty title but never replaces one.
        let b = apply_metadata(&conn, b.id, Some("Fetched"), None).unwrap();
        assert_eq!(
            (b.title.as_str(), b.favicon.as_deref()),
            ("Fetched", Some(icon))
        );
        let b = apply_metadata(&conn, b.id, Some("Other"), None).unwrap();
        assert_eq!(b.title, "Fetched");
        assert_eq!(favicons_in_use(&conn).unwrap(), [icon.to_string()].into());
        let other = create(&conn, new(None, "", "y.com")).unwrap();
        create(&conn, new(None, "", "mailto:me@example.com")).unwrap();
        assert_eq!(
            missing_favicons(&conn).unwrap(),
            [(other.id, "https://y.com".to_string())]
        );

        let cleared = update(
            &conn,
            b.id,
            BookmarkPatch {
                favicon: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(cleared.favicon, None);
        assert!(matches!(
            apply_metadata(&conn, 999, None, None),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn delete_bookmark() {
        let conn = open_in_memory().unwrap();
        let b = create(&conn, new(None, "x", "x.com")).unwrap();
        delete(&conn, b.id).unwrap();
        assert!(matches!(get(&conn, b.id), Err(AppError::NotFound(_))));
        assert!(matches!(delete(&conn, b.id), Err(AppError::NotFound(_))));
    }

    #[test]
    fn record_open_counts() {
        let conn = open_in_memory().unwrap();
        let b = create(&conn, new(None, "x", "x.com")).unwrap();
        assert_eq!(b.last_opened_at, None);
        record_open(&conn, b.id).unwrap();
        record_open(&conn, b.id).unwrap();
        let after = get(&conn, b.id).unwrap();
        assert_eq!(after.open_count, 2);
        assert!(after.last_opened_at.is_some());
        assert_eq!(after.updated_at, b.updated_at);
        assert!(matches!(
            record_open(&conn, 999),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn patch_distinguishes_null_from_missing() {
        let p: BookmarkPatch = serde_json::from_str(r#"{"title":"t"}"#).unwrap();
        assert_eq!(p.folder_id, None);
        let p: BookmarkPatch = serde_json::from_str(r#"{"folderId":null}"#).unwrap();
        assert_eq!(p.folder_id, Some(None));
        let p: BookmarkPatch = serde_json::from_str(r#"{"folderId":3}"#).unwrap();
        assert_eq!(p.folder_id, Some(Some(3)));
    }

    #[test]
    fn url_normalization() {
        let cases = [
            ("example.com", "https://example.com"),
            ("  example.com/a?b=c  ", "https://example.com/a?b=c"),
            ("localhost:3000", "https://localhost:3000"),
            ("http://example.com", "http://example.com"),
            ("HTTPS://Example.com", "HTTPS://Example.com"),
            ("file:///C:/notes.txt", "file:///C:/notes.txt"),
            ("mailto:me@example.com", "mailto:me@example.com"),
        ];
        for (input, expected) in cases {
            assert_eq!(normalize_url(input).unwrap(), expected, "{input}");
        }
        assert!(matches!(normalize_url(""), Err(AppError::Invalid(_))));
        assert!(matches!(normalize_url(" \t "), Err(AppError::Invalid(_))));
    }
}
