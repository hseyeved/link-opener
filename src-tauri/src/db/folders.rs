use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::bookmarks::{parse_target, target_json};
use super::tags;
use crate::browsers::LaunchTarget;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
    pub position: i64,
    /// Browser for bookmarks in this folder and its subfolders, unless they set their own.
    pub default_target: Option<LaunchTarget>,
}

const COLUMNS: &str = "id, parent_id, name, position, default_target";

impl Folder {
    fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            name: row.get(2)?,
            position: row.get(3)?,
            default_target: parse_target(row.get(4)?),
        })
    }
}

/// All folders, flat, ordered by parent then position. The frontend builds the tree.
pub fn list_all(conn: &Connection) -> AppResult<Vec<Folder>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COLUMNS} FROM folders ORDER BY parent_id, position, id"
    ))?;
    let folders = stmt
        .query_map([], Folder::from_row)?
        .collect::<Result<_, _>>()?;
    Ok(folders)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<Folder> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM folders WHERE id = ?1"),
        [id],
        Folder::from_row,
    )
    .optional()?
    .ok_or(AppError::NotFound("folder"))
}

pub fn exists(conn: &Connection, id: i64) -> AppResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?1)",
        [id],
        |r| r.get(0),
    )?)
}

pub fn create(conn: &Connection, parent_id: Option<i64>, name: &str) -> AppResult<Folder> {
    let name = validate_name(name)?;
    if let Some(parent_id) = parent_id {
        if !exists(conn, parent_id)? {
            return Err(AppError::NotFound("parent folder"));
        }
    }
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM folders WHERE parent_id IS ?1",
        [parent_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO folders (parent_id, name, position) VALUES (?1, ?2, ?3)",
        params![parent_id, name, position],
    )?;
    get(conn, conn.last_insert_rowid())
}

pub fn rename(conn: &Connection, id: i64, name: &str) -> AppResult<Folder> {
    let name = validate_name(name)?;
    let changed = conn.execute(
        "UPDATE folders SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("folder"));
    }
    get(conn, id)
}

/// Deletes the folder; subfolders and their bookmarks go with it (FK cascade).
pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    if conn.execute("DELETE FROM folders WHERE id = ?1", [id])? == 0 {
        return Err(AppError::NotFound("folder"));
    }
    tags::delete_unused(conn)
}

pub fn set_default_target(
    conn: &Connection,
    id: i64,
    target: Option<&LaunchTarget>,
) -> AppResult<Folder> {
    let changed = conn.execute(
        "UPDATE folders SET default_target = ?2 WHERE id = ?1",
        params![id, target_json(target)?],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("folder"));
    }
    get(conn, id)
}

/// Moves the folder under `parent_id` (`None` = top level) at `index` among its new
/// siblings (`None` or past the end = last). Also reorders among the same siblings.
pub fn move_to(
    conn: &Connection,
    id: i64,
    parent_id: Option<i64>,
    index: Option<usize>,
) -> AppResult<Folder> {
    let folder = get(conn, id)?;
    if let Some(parent_id) = parent_id {
        if !exists(conn, parent_id)? {
            return Err(AppError::NotFound("parent folder"));
        }
        if is_within(conn, parent_id, id)? {
            return Err(AppError::Invalid("can't move a folder into itself".into()));
        }
    }
    let tx = conn.unchecked_transaction()?;
    let mut ids: Vec<i64> = tx
        .prepare_cached(
            "SELECT id FROM folders WHERE parent_id IS ?1 AND id != ?2 ORDER BY position, id",
        )?
        .query_map(params![parent_id, id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    ids.insert(index.unwrap_or(ids.len()).min(ids.len()), id);
    if parent_id != folder.parent_id {
        tx.execute(
            "UPDATE folders SET parent_id = ?2 WHERE id = ?1",
            params![id, parent_id],
        )?;
    }
    super::renumber(&tx, "folders", &ids)?;
    tx.commit()?;
    get(conn, id)
}

/// Whether `folder_id` is `ancestor_id` or inside it.
fn is_within(conn: &Connection, folder_id: i64, ancestor_id: i64) -> AppResult<bool> {
    Ok(conn.query_row(
        "WITH RECURSIVE up(id, parent_id, depth) AS (
             SELECT id, parent_id, 0 FROM folders WHERE id = ?1
             UNION ALL
             SELECT f.id, f.parent_id, up.depth + 1 FROM folders f JOIN up ON f.id = up.parent_id
             WHERE up.depth < 1000
         )
         SELECT EXISTS(SELECT 1 FROM up WHERE id = ?2)",
        params![folder_id, ancestor_id],
        |r| r.get(0),
    )?)
}

fn validate_name(name: &str) -> AppResult<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("folder name must not be empty".into()));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::bookmarks::{self, NewBookmark};
    use crate::db::open_in_memory;

    fn new_bookmark(folder_id: Option<i64>, url: &str) -> NewBookmark {
        NewBookmark {
            folder_id,
            url: url.into(),
            ..Default::default()
        }
    }

    fn names(conn: &Connection, parent_id: Option<i64>) -> Vec<String> {
        list_all(conn)
            .unwrap()
            .into_iter()
            .filter(|f| f.parent_id == parent_id)
            .map(|f| f.name)
            .collect()
    }

    #[test]
    fn move_reorders_and_reparents() {
        let conn = open_in_memory().unwrap();
        let a = create(&conn, None, "A").unwrap();
        let b = create(&conn, None, "B").unwrap();
        let c = create(&conn, None, "C").unwrap();

        move_to(&conn, c.id, None, Some(0)).unwrap();
        assert_eq!(names(&conn, None), ["C", "A", "B"]);
        move_to(&conn, c.id, None, None).unwrap();
        assert_eq!(names(&conn, None), ["A", "B", "C"]);

        let moved = move_to(&conn, b.id, Some(a.id), None).unwrap();
        assert_eq!(moved.parent_id, Some(a.id));
        assert_eq!(names(&conn, None), ["A", "C"]);
        assert_eq!(names(&conn, Some(a.id)), ["B"]);

        // Back to the top level, between A and C; an index past the end clamps.
        move_to(&conn, b.id, None, Some(1)).unwrap();
        assert_eq!(names(&conn, None), ["A", "B", "C"]);
        move_to(&conn, a.id, None, Some(99)).unwrap();
        assert_eq!(names(&conn, None), ["B", "C", "A"]);
    }

    #[test]
    fn move_rejects_cycles_and_missing() {
        let conn = open_in_memory().unwrap();
        let a = create(&conn, None, "A").unwrap();
        let child = create(&conn, Some(a.id), "Child").unwrap();
        let grandchild = create(&conn, Some(child.id), "Grandchild").unwrap();
        assert!(matches!(
            move_to(&conn, a.id, Some(a.id), None),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            move_to(&conn, a.id, Some(grandchild.id), None),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            move_to(&conn, a.id, Some(999), None),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            move_to(&conn, 999, None, None),
            Err(AppError::NotFound(_))
        ));
        // Moving a grandchild up is fine.
        move_to(&conn, grandchild.id, Some(a.id), Some(0)).unwrap();
        assert_eq!(names(&conn, Some(a.id)), ["Grandchild", "Child"]);
    }

    #[test]
    fn default_target_round_trip() {
        let conn = open_in_memory().unwrap();
        let f = create(&conn, None, "F").unwrap();
        assert_eq!(f.default_target, None);
        let target = LaunchTarget {
            browser_id: "edge".into(),
            profile_id: Some("Default".into()),
            private: false,
        };
        let f = set_default_target(&conn, f.id, Some(&target)).unwrap();
        assert_eq!(f.default_target, Some(target));
        assert_eq!(
            set_default_target(&conn, f.id, None)
                .unwrap()
                .default_target,
            None
        );
        assert!(matches!(
            set_default_target(&conn, 999, None),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn create_and_nest() {
        let conn = open_in_memory().unwrap();
        let work = create(&conn, None, "  Work ").unwrap();
        assert_eq!(work.name, "Work");
        assert_eq!(work.parent_id, None);
        let docs = create(&conn, Some(work.id), "Docs").unwrap();
        assert_eq!(docs.parent_id, Some(work.id));
        assert_eq!(list_all(&conn).unwrap().len(), 2);
    }

    #[test]
    fn positions_append_among_siblings() {
        let conn = open_in_memory().unwrap();
        let a = create(&conn, None, "A").unwrap();
        let b = create(&conn, None, "B").unwrap();
        let a1 = create(&conn, Some(a.id), "A1").unwrap();
        let a2 = create(&conn, Some(a.id), "A2").unwrap();
        assert_eq!((a.position, b.position), (0, 1));
        assert_eq!((a1.position, a2.position), (0, 1));
    }

    #[test]
    fn rejects_empty_name() {
        let conn = open_in_memory().unwrap();
        assert!(matches!(
            create(&conn, None, "   "),
            Err(AppError::Invalid(_))
        ));
        let f = create(&conn, None, "Ok").unwrap();
        assert!(matches!(rename(&conn, f.id, ""), Err(AppError::Invalid(_))));
    }

    #[test]
    fn rejects_missing_parent() {
        let conn = open_in_memory().unwrap();
        assert!(matches!(
            create(&conn, Some(42), "Orphan"),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn rename_folder() {
        let conn = open_in_memory().unwrap();
        let f = create(&conn, None, "Old").unwrap();
        assert_eq!(rename(&conn, f.id, "New").unwrap().name, "New");
        assert!(matches!(
            rename(&conn, 999, "X"),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn delete_cascades_to_children_and_bookmarks() {
        let conn = open_in_memory().unwrap();
        let top = create(&conn, None, "Top").unwrap();
        let child = create(&conn, Some(top.id), "Child").unwrap();
        let grandchild = create(&conn, Some(child.id), "Grandchild").unwrap();
        let other = create(&conn, None, "Other").unwrap();
        bookmarks::create(&conn, new_bookmark(Some(top.id), "a.com")).unwrap();
        bookmarks::create(&conn, new_bookmark(Some(grandchild.id), "b.com")).unwrap();
        let kept = bookmarks::create(&conn, new_bookmark(Some(other.id), "c.com")).unwrap();
        let unfiled = bookmarks::create(&conn, new_bookmark(None, "d.com")).unwrap();

        delete(&conn, top.id).unwrap();

        let remaining: Vec<i64> = list_all(&conn).unwrap().iter().map(|f| f.id).collect();
        assert_eq!(remaining, vec![other.id]);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        bookmarks::get(&conn, kept.id).unwrap();
        bookmarks::get(&conn, unfiled.id).unwrap();
        assert!(matches!(delete(&conn, top.id), Err(AppError::NotFound(_))));
    }
}
