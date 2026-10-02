//! Which browser a bookmark opens with: its own default, else the nearest ancestor
//! folder's, else none (ask with the picker).

use rusqlite::Connection;
use serde::Serialize;

use super::bookmarks::{self, parse_target};
use crate::browsers::LaunchTarget;
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTarget {
    pub target: LaunchTarget,
    /// The folder the default comes from; `None` when it is set on the bookmark itself.
    pub folder_id: Option<i64>,
    pub folder_name: Option<String>,
}

pub fn resolve(conn: &Connection, bookmark_id: i64) -> AppResult<Option<ResolvedTarget>> {
    let bookmark = bookmarks::get(conn, bookmark_id)?;
    if let Some(target) = bookmark.default_target {
        return Ok(Some(ResolvedTarget { target, folder_id: None, folder_name: None }));
    }
    let Some(folder_id) = bookmark.folder_id else {
        return Ok(None);
    };
    // Walk up from the bookmark's folder; the first parseable target wins.
    let mut stmt = conn.prepare_cached(
        "WITH RECURSIVE up(id, parent_id, name, default_target, depth) AS (
             SELECT id, parent_id, name, default_target, 0 FROM folders WHERE id = ?1
             UNION ALL
             SELECT f.id, f.parent_id, f.name, f.default_target, up.depth + 1
             FROM folders f JOIN up ON f.id = up.parent_id
             WHERE up.depth < 1000
         )
         SELECT id, name, default_target FROM up WHERE default_target IS NOT NULL ORDER BY depth",
    )?;
    let found = stmt
        .query_map([folder_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?))
        })?
        .filter_map(|row| row.ok())
        .find_map(|(id, name, json)| {
            parse_target(json).map(|target| ResolvedTarget {
                target,
                folder_id: Some(id),
                folder_name: Some(name),
            })
        });
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::bookmarks::{BookmarkPatch, NewBookmark};
    use crate::db::{folders, open_in_memory};

    fn target(browser: &str) -> LaunchTarget {
        LaunchTarget { browser_id: browser.into(), profile_id: None, private: false }
    }

    fn add(conn: &Connection, folder_id: Option<i64>, default_target: Option<LaunchTarget>) -> i64 {
        let new = NewBookmark { folder_id, url: "x.com".into(), default_target, ..Default::default() };
        bookmarks::create(conn, new).unwrap().id
    }

    #[test]
    fn lookup_order() {
        let conn = open_in_memory().unwrap();
        let top = folders::create(&conn, None, "Top").unwrap();
        let mid = folders::create(&conn, Some(top.id), "Mid").unwrap();
        let leaf = folders::create(&conn, Some(mid.id), "Leaf").unwrap();

        let unfiled = add(&conn, None, None);
        let in_leaf = add(&conn, Some(leaf.id), None);
        let own = add(&conn, Some(leaf.id), Some(target("own")));

        assert_eq!(resolve(&conn, unfiled).unwrap(), None);
        assert_eq!(resolve(&conn, in_leaf).unwrap(), None);

        folders::set_default_target(&conn, top.id, Some(&target("top"))).unwrap();
        let r = resolve(&conn, in_leaf).unwrap().unwrap();
        assert_eq!((r.target.browser_id.as_str(), r.folder_id, r.folder_name.as_deref()), ("top", Some(top.id), Some("Top")));

        // The nearest ancestor wins.
        folders::set_default_target(&conn, mid.id, Some(&target("mid"))).unwrap();
        assert_eq!(resolve(&conn, in_leaf).unwrap().unwrap().target.browser_id, "mid");

        // The bookmark's own default beats every folder.
        let r = resolve(&conn, own).unwrap().unwrap();
        assert_eq!((r.target.browser_id.as_str(), r.folder_id), ("own", None));

        // Clearing it falls back to the folders again.
        let patch = BookmarkPatch { default_target: Some(None), ..Default::default() };
        bookmarks::update(&conn, own, patch).unwrap();
        assert_eq!(resolve(&conn, own).unwrap().unwrap().target.browser_id, "mid");
    }

    #[test]
    fn unparseable_targets_are_skipped() {
        let conn = open_in_memory().unwrap();
        let top = folders::create(&conn, None, "Top").unwrap();
        let mid = folders::create(&conn, Some(top.id), "Mid").unwrap();
        folders::set_default_target(&conn, top.id, Some(&target("top"))).unwrap();
        conn.execute("UPDATE folders SET default_target = 'garbage' WHERE id = ?1", [mid.id]).unwrap();
        let b = add(&conn, Some(mid.id), None);
        assert_eq!(resolve(&conn, b).unwrap().unwrap().target.browser_id, "top");
        assert!(matches!(resolve(&conn, 999), Err(crate::error::AppError::NotFound(_))));
    }
}
