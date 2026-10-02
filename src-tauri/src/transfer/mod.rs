//! Backups and imports. Link Opener backups (JSON) and browser bookmark files (HTML) are
//! both read into a [`Library`], and one importer adds that to the database.

pub mod html;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use base64::Engine as _;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::browsers::LaunchTarget;
use crate::db::bookmarks::{self, normalize_url, target_json};
use crate::db::{folders, now_ms, settings, tags};
use crate::error::{AppError, AppResult};
use crate::metadata;

pub const BACKUP_FORMAT: &str = "link-opener-backup";
pub const BACKUP_VERSION: u32 = 1;
const MAX_FILE_BYTES: u64 = 200 * 1024 * 1024;

/// Settings that belong to the library (restored with it), not to this machine.
const LIBRARY_SETTINGS: [&str; 3] = ["unfiled_name", "open_direct", settings::BROWSER_PREFS];

/// Folders and bookmarks, independent of database ids.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub folders: Vec<FolderRecord>,
    pub bookmarks: Vec<BookmarkRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRecord {
    /// Only links bookmarks and subfolders to this folder within the file.
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub default_target: Option<LaunchTarget>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookmarkRecord {
    pub folder_id: Option<i64>,
    pub title: String,
    pub url: String,
    pub notes: String,
    pub tags: Vec<String>,
    /// File name in the favicons dir (from a backup), or a `data:` URI (from a browser file).
    pub favicon: Option<String>,
    pub position: i64,
    pub default_target: Option<LaunchTarget>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub last_opened_at: Option<i64>,
    pub open_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub format: String,
    pub version: u32,
    pub exported_at: i64,
    #[serde(flatten)]
    pub library: Library,
    /// Favicon file name → base64 contents, so the backup is self-contained.
    #[serde(default)]
    pub favicons: BTreeMap<String, String>,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
}

/// The whole library as it is now.
pub fn read_library(conn: &Connection) -> AppResult<Library> {
    let folders = folders::list_all(conn)?
        .into_iter()
        .map(|f| FolderRecord {
            id: f.id,
            parent_id: f.parent_id,
            name: f.name,
            position: f.position,
            default_target: f.default_target,
        })
        .collect();
    let bookmarks = bookmarks::list_all(conn)?
        .into_iter()
        .map(|b| BookmarkRecord {
            folder_id: b.folder_id,
            title: b.title,
            url: b.url,
            notes: b.notes,
            tags: b.tags,
            favicon: b.favicon,
            position: b.position,
            default_target: b.default_target,
            created_at: Some(b.created_at),
            updated_at: Some(b.updated_at),
            last_opened_at: b.last_opened_at,
            open_count: b.open_count,
        })
        .collect();
    Ok(Library { folders, bookmarks })
}

pub fn create_backup(conn: &Connection, favicon_dir: &Path) -> AppResult<BackupFile> {
    let library = read_library(conn)?;
    let engine = base64::engine::general_purpose::STANDARD;
    let mut favicons = BTreeMap::new();
    for name in library
        .bookmarks
        .iter()
        .filter_map(|b| b.favicon.as_deref())
    {
        if favicons.contains_key(name) || !metadata::is_favicon_name(name) {
            continue;
        }
        // A missing icon file just leaves the bookmark without an icon after restore.
        if let Ok(bytes) = fs::read(favicon_dir.join(name)) {
            favicons.insert(name.to_string(), engine.encode(bytes));
        }
    }
    let mut saved_settings = BTreeMap::new();
    for key in LIBRARY_SETTINGS {
        if let Some(value) = settings::get(conn, key)? {
            saved_settings.insert(key.to_string(), value);
        }
    }
    Ok(BackupFile {
        format: BACKUP_FORMAT.into(),
        version: BACKUP_VERSION,
        exported_at: now_ms(),
        library,
        favicons,
        settings: saved_settings,
    })
}

pub fn write_backup(path: &Path, backup: &BackupFile) -> AppResult<()> {
    let json = serde_json::to_vec_pretty(backup).map_err(|e| AppError::Invalid(e.to_string()))?;
    write_atomically(path, &json)
}

/// Writes next to the target and renames, so a failed write never leaves a half file.
pub fn write_atomically(path: &Path, contents: &[u8]) -> AppResult<()> {
    let tmp = path.with_extension("tmp-link-opener");
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    Ok(())
}

/// What a file contains.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportSource {
    Backup(BackupFile),
    Browser(Library),
}

impl ImportSource {
    pub fn library(&self) -> &Library {
        match self {
            ImportSource::Backup(b) => &b.library,
            ImportSource::Browser(l) => l,
        }
    }
}

/// Reads a Link Opener backup or a browser bookmark file, telling them apart by content.
pub fn read_file(path: &Path) -> AppResult<ImportSource> {
    if fs::metadata(path)?.len() > MAX_FILE_BYTES {
        return Err(AppError::Invalid("the file is too large to import".into()));
    }
    let bytes = fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    parse_import(&text)
}

pub fn parse_import(text: &str) -> AppResult<ImportSource> {
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    if trimmed.starts_with('{') {
        let backup: BackupFile = serde_json::from_str(trimmed)
            .map_err(|e| AppError::Invalid(format!("this isn't a Link Opener backup ({e})")))?;
        if backup.format != BACKUP_FORMAT {
            return Err(AppError::Invalid("this isn't a Link Opener backup".into()));
        }
        if backup.version > BACKUP_VERSION {
            return Err(AppError::Invalid(
                "this backup is from a newer version of Link Opener; update the app to import it"
                    .into(),
            ));
        }
        return Ok(ImportSource::Backup(backup));
    }
    let lower = trimmed
        .get(..trimmed.len().min(4096))
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    if lower.contains("netscape-bookmark-file") || lower.contains("<dl") {
        return Ok(ImportSource::Browser(html::parse(trimmed)));
    }
    Err(AppError::Invalid(
        "unrecognised file: choose a Link Opener backup (.json) or a browser bookmarks export (.html)".into(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportMode {
    /// Add to the library; folders with the same name and parent are reused.
    Merge,
    /// Delete everything first and restore the file exactly.
    Replace,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub folders_created: usize,
    pub bookmarks_added: usize,
    pub duplicates_skipped: usize,
    pub invalid_skipped: usize,
}

/// Stores a source's icons as favicon files and points the bookmarks at them. Backup icons
/// are checked and rewritten under their content hash; data URIs from browser files are decoded.
pub fn store_icons(source: &mut ImportSource, favicon_dir: &Path) -> AppResult<()> {
    let engine = base64::engine::general_purpose::STANDARD;
    match source {
        ImportSource::Backup(backup) => {
            let mut stored: HashMap<String, Option<String>> = HashMap::new();
            for (name, data) in &backup.favicons {
                let file = match engine.decode(data) {
                    Ok(bytes) => metadata::store_icon(favicon_dir, &bytes)?,
                    Err(_) => None,
                };
                stored.insert(name.clone(), file);
            }
            for b in &mut backup.library.bookmarks {
                b.favicon = b
                    .favicon
                    .take()
                    .and_then(|name| stored.get(&name).cloned().flatten());
            }
        }
        ImportSource::Browser(lib) => {
            for b in &mut lib.bookmarks {
                b.favicon = match b.favicon.take() {
                    Some(uri) => decode_data_uri(&uri)
                        .map(|bytes| metadata::store_icon(favicon_dir, &bytes))
                        .transpose()?
                        .flatten(),
                    None => None,
                };
            }
        }
    }
    Ok(())
}

/// The bytes of a base64 `data:` URI.
fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let (header, data) = uri.strip_prefix("data:")?.split_once(',')?;
    if !header.ends_with(";base64") {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .ok()
}

/// Adds `lib` to the database in one transaction. `wrap_in` puts everything inside a new
/// top-level folder of that name. With `skip_duplicates` (merge only), bookmarks whose URL
/// is already in the library, or earlier in the file, are left out.
pub fn import_library(
    conn: &Connection,
    lib: &Library,
    mode: ImportMode,
    skip_duplicates: bool,
    wrap_in: Option<&str>,
) -> AppResult<ImportSummary> {
    let mut summary = ImportSummary::default();
    let tx = conn.unchecked_transaction()?;
    let replace = mode == ImportMode::Replace;
    if replace {
        tx.execute_batch("DELETE FROM bookmarks; DELETE FROM folders; DELETE FROM tags;")?;
    }

    let root = match wrap_in {
        Some(name) => {
            summary.folders_created += 1;
            Some(insert_folder(&tx, None, name, None, None)?)
        }
        None => None,
    };

    // Folders, parents first. A folder whose parent is missing (or in a cycle) goes to the root.
    let mut ids: HashMap<i64, i64> = HashMap::new();
    let known: HashSet<i64> = lib.folders.iter().map(|f| f.id).collect();
    let mut pending: Vec<&FolderRecord> = lib.folders.iter().collect();
    pending.sort_by_key(|f| f.position);
    while !pending.is_empty() {
        let ready = pending
            .iter()
            .position(|f| {
                f.parent_id
                    .is_none_or(|p| !known.contains(&p) || ids.contains_key(&p))
            })
            // Only a cycle is left: break it at the first folder.
            .unwrap_or(0);
        let f = pending.remove(ready);
        let parent = f.parent_id.and_then(|p| ids.get(&p).copied()).or(root);
        let name = f.name.trim();
        let name = if name.is_empty() {
            "Untitled folder"
        } else {
            name
        };
        let existing = if replace {
            None
        } else {
            tx.query_row(
                "SELECT id FROM folders WHERE parent_id IS ?1 AND name = ?2 COLLATE NOCASE",
                params![parent, name],
                |r| r.get(0),
            )
            .optional()?
        };
        let id = match existing {
            Some(id) => id,
            None => {
                summary.folders_created += 1;
                let position = replace.then_some(f.position);
                insert_folder(&tx, parent, name, position, f.default_target.as_ref())?
            }
        };
        ids.insert(f.id, id);
    }

    let mut seen_urls: HashSet<String> = HashSet::new();
    let now = now_ms();
    for b in &lib.bookmarks {
        let Ok(url) = normalize_url(&b.url) else {
            summary.invalid_skipped += 1;
            continue;
        };
        if skip_duplicates && !replace {
            let in_library: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM bookmarks WHERE url = ?1)",
                [&url],
                |r| r.get(0),
            )?;
            if in_library || !seen_urls.insert(url.clone()) {
                summary.duplicates_skipped += 1;
                continue;
            }
        }
        let folder = b.folder_id.and_then(|id| ids.get(&id).copied()).or(root);
        let position = if replace {
            b.position
        } else {
            bookmarks::next_position(&tx, folder)?
        };
        let favicon = b
            .favicon
            .as_deref()
            .filter(|f| metadata::is_favicon_name(f));
        let created = b.created_at.unwrap_or(now);
        tx.execute(
            "INSERT INTO bookmarks (folder_id, title, url, notes, favicon, position, default_target,
                                    created_at, updated_at, last_opened_at, open_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                folder,
                b.title.trim(),
                url,
                b.notes,
                favicon,
                position,
                target_json(b.default_target.as_ref())?,
                created,
                b.updated_at.unwrap_or(created),
                b.last_opened_at,
                b.open_count.max(0),
            ],
        )?;
        let id = tx.last_insert_rowid();
        // Over-long tags are dropped rather than failing the whole import.
        let names: Vec<String> = b
            .tags
            .iter()
            .filter(|t| t.chars().count() <= tags::MAX_TAG_LEN)
            .cloned()
            .collect();
        tags::set_for_bookmark(&tx, id, &tags::normalize(&names)?)?;
        summary.bookmarks_added += 1;
    }
    tags::delete_unused(&tx)?;
    tx.commit()?;
    Ok(summary)
}

/// Restores a backup's library settings (replace mode).
pub fn restore_settings(conn: &Connection, backup: &BackupFile) -> AppResult<()> {
    for (key, value) in &backup.settings {
        if LIBRARY_SETTINGS.contains(&key.as_str()) {
            settings::set(conn, key, value)?;
        }
    }
    Ok(())
}

fn insert_folder(
    conn: &Connection,
    parent: Option<i64>,
    name: &str,
    position: Option<i64>,
    target: Option<&LaunchTarget>,
) -> AppResult<i64> {
    let position = match position {
        Some(p) => p,
        None => conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM folders WHERE parent_id IS ?1",
            [parent],
            |r| r.get(0),
        )?,
    };
    conn.execute(
        "INSERT INTO folders (parent_id, name, position, default_target) VALUES (?1, ?2, ?3, ?4)",
        params![parent, name, position, target_json(target)?],
    )?;
    Ok(conn.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::bookmarks::NewBookmark;
    use crate::db::{open_in_memory, search};

    const ICON: &str = "0123456789abcdef0123456789abcdef.png";

    fn add(conn: &Connection, folder_id: Option<i64>, url: &str, tags: &[&str]) -> i64 {
        let new = NewBookmark {
            folder_id,
            url: url.into(),
            title: format!("T {url}"),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            ..Default::default()
        };
        bookmarks::create(conn, new).unwrap().id
    }

    /// Work/Docs nesting, a default target, tags, an unfiled bookmark and usage stats.
    fn sample(conn: &Connection) {
        let work = folders::create(conn, None, "Work").unwrap();
        let docs = folders::create(conn, Some(work.id), "Docs").unwrap();
        let target = LaunchTarget {
            browser_id: "edge".into(),
            profile_id: None,
            private: false,
        };
        folders::set_default_target(conn, work.id, Some(&target)).unwrap();
        add(conn, Some(work.id), "work.example", &["job"]);
        let d = add(conn, Some(docs.id), "docs.example", &["job", "ref"]);
        bookmarks::record_open(conn, d).unwrap();
        add(conn, None, "unfiled.example", &[]);
        conn.execute(
            "UPDATE bookmarks SET favicon = ?1 WHERE url = 'https://docs.example'",
            [ICON],
        )
        .unwrap();
    }

    fn snapshot(conn: &Connection) -> Vec<(String, String, Vec<String>, i64)> {
        let lib = read_library(conn).unwrap();
        let path = |id: Option<i64>| {
            let mut names = Vec::new();
            let mut cur = id;
            while let Some(f) = cur.and_then(|id| lib.folders.iter().find(|f| f.id == id)) {
                names.insert(0, f.name.clone());
                cur = f.parent_id;
            }
            names.join("/")
        };
        let mut rows: Vec<_> = lib
            .bookmarks
            .iter()
            .map(|b| {
                (
                    path(b.folder_id),
                    b.url.clone(),
                    b.tags.clone(),
                    b.open_count,
                )
            })
            .collect();
        rows.sort();
        rows
    }

    #[test]
    fn backup_json_round_trip_and_replace() {
        let source = open_in_memory().unwrap();
        sample(&source);
        let dir = tempfile::tempdir().unwrap();
        // A real (tiny) PNG for the favicon, so it survives the content check on restore.
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR....";
        fs::write(dir.path().join(ICON), png).unwrap();

        let backup = create_backup(&source, dir.path()).unwrap();
        assert_eq!(backup.favicons.len(), 1);
        let file = dir.path().join("backup.json");
        write_backup(&file, &backup).unwrap();

        let target = open_in_memory().unwrap();
        add(&target, None, "will-be-deleted.example", &["gone"]);
        let restore_dir = tempfile::tempdir().unwrap();
        let mut source_file = read_file(&file).unwrap();
        assert!(matches!(source_file, ImportSource::Backup(_)));
        store_icons(&mut source_file, restore_dir.path()).unwrap();
        let summary = import_library(
            &target,
            source_file.library(),
            ImportMode::Replace,
            true,
            None,
        )
        .unwrap();
        assert_eq!((summary.folders_created, summary.bookmarks_added), (2, 3));
        assert_eq!(snapshot(&target), snapshot(&source));

        // Default target and favicon came along; the old tag is gone.
        let work = folders::list_all(&target)
            .unwrap()
            .into_iter()
            .find(|f| f.name == "Work")
            .unwrap();
        assert_eq!(work.default_target.unwrap().browser_id, "edge");
        let docs = search::search(&target, "docs.example", 5).unwrap();
        let icon = docs[0].favicon.clone().unwrap();
        assert_eq!(fs::read(restore_dir.path().join(icon)).unwrap(), png);
        assert!(tags::list_all(&target)
            .unwrap()
            .iter()
            .all(|t| t.name != "gone"));
    }

    #[test]
    fn merge_reuses_folders_and_skips_duplicates() {
        let source = open_in_memory().unwrap();
        sample(&source);
        let lib = read_library(&source).unwrap();

        let target = open_in_memory().unwrap();
        let work = folders::create(&target, None, "work").unwrap(); // same name, other case
        add(&target, Some(work.id), "work.example", &[]); // already there

        let summary = import_library(&target, &lib, ImportMode::Merge, true, None).unwrap();
        assert_eq!(summary.folders_created, 1); // only Docs
        assert_eq!(summary.bookmarks_added, 2);
        assert_eq!(summary.duplicates_skipped, 1);
        assert_eq!(folders::list_all(&target).unwrap().len(), 2);

        // Importing again adds nothing new.
        let again = import_library(&target, &lib, ImportMode::Merge, true, None).unwrap();
        assert_eq!(
            (
                again.bookmarks_added,
                again.duplicates_skipped,
                again.folders_created
            ),
            (0, 3, 0)
        );

        // Without skipping, duplicates are added.
        let dupes = import_library(&target, &lib, ImportMode::Merge, false, None).unwrap();
        assert_eq!(dupes.bookmarks_added, 3);
    }

    #[test]
    fn browser_file_imports_into_wrapper_folder() {
        let html = r#"<DL><p>
            <DT><H3>Bar</H3>
            <DL><p>
                <DT><A HREF="https://a.example/" TAGS="x">A</A>
                <DT><A HREF="https://a.example/">A again</A>
                <DT><A HREF="not a url">Broken</A>
                <DT><A HREF="https://icon.example/" ICON="data:image/png;base64,iVBORw0KGgoAAAANSUhEUg==">Icon</A>
            </DL><p>
            <DT><A HREF="https://top.example/">Top</A>
        </DL><p>"#;
        let mut source = parse_import(html).unwrap();
        let dir = tempfile::tempdir().unwrap();
        store_icons(&mut source, dir.path()).unwrap();
        let conn = open_in_memory().unwrap();
        let summary = import_library(
            &conn,
            source.library(),
            ImportMode::Merge,
            true,
            Some("Imported bookmarks"),
        )
        .unwrap();
        assert_eq!(summary.folders_created, 2);
        // "not a url" is kept: it gets https:// like any typed URL.
        assert_eq!(summary.bookmarks_added, 4);
        assert_eq!(summary.duplicates_skipped, 1);

        let rows = snapshot(&conn);
        let paths: Vec<(&str, &str)> = rows
            .iter()
            .map(|(p, u, _, _)| (p.as_str(), u.as_str()))
            .collect();
        assert!(paths.contains(&("Imported bookmarks/Bar", "https://a.example/")));
        assert!(paths.contains(&("Imported bookmarks", "https://top.example/")));
        // Nothing went to Unfiled.
        assert!(bookmarks::list(&conn, None).unwrap().is_empty());
        let icon = search::search(&conn, "icon.example", 5).unwrap()[0]
            .favicon
            .clone();
        assert!(icon.is_some_and(|f| dir.path().join(f).exists()));
    }

    #[test]
    fn folder_cycles_and_orphans_do_not_hang() {
        let lib = Library {
            folders: vec![
                FolderRecord {
                    id: 1,
                    parent_id: Some(2),
                    name: "A".into(),
                    position: 0,
                    default_target: None,
                },
                FolderRecord {
                    id: 2,
                    parent_id: Some(1),
                    name: "B".into(),
                    position: 1,
                    default_target: None,
                },
                FolderRecord {
                    id: 3,
                    parent_id: Some(99),
                    name: "Orphan".into(),
                    position: 2,
                    default_target: None,
                },
            ],
            bookmarks: vec![BookmarkRecord {
                folder_id: Some(2),
                url: "b.example".into(),
                ..Default::default()
            }],
        };
        let conn = open_in_memory().unwrap();
        let summary = import_library(&conn, &lib, ImportMode::Replace, false, None).unwrap();
        assert_eq!((summary.folders_created, summary.bookmarks_added), (3, 1));
    }

    #[test]
    fn rejects_unknown_files() {
        assert!(matches!(parse_import("hello"), Err(AppError::Invalid(_))));
        assert!(matches!(
            parse_import(r#"{"format":"other","version":1,"exportedAt":0}"#),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            parse_import(r#"{"format":"link-opener-backup","version":99,"exportedAt":0}"#),
            Err(AppError::Invalid(_))
        ));
        let empty =
            parse_import(r#"{"format":"link-opener-backup","version":1,"exportedAt":0}"#).unwrap();
        assert_eq!(empty.library(), &Library::default());
    }

    #[test]
    fn restore_settings_only_library_keys() {
        let conn = open_in_memory().unwrap();
        let backup = BackupFile {
            format: BACKUP_FORMAT.into(),
            version: 1,
            exported_at: 0,
            library: Library::default(),
            favicons: BTreeMap::new(),
            settings: BTreeMap::from([
                ("unfiled_name".to_string(), "Inbox".to_string()),
                ("global_shortcut".to_string(), "Ctrl+Q".to_string()),
            ]),
        };
        restore_settings(&conn, &backup).unwrap();
        assert_eq!(
            settings::get(&conn, "unfiled_name").unwrap().as_deref(),
            Some("Inbox")
        );
        assert_eq!(settings::get(&conn, "global_shortcut").unwrap(), None);
    }
}
