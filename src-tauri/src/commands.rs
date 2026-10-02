use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_autostart::ManagerExt;

use crate::browsers::prefs::{self, BrowserPrefs};
use crate::browsers::{launch, Browser, BrowserCache, LaunchTarget, Os};
use crate::db::bookmarks::{self, Bookmark, BookmarkPatch, NewBookmark};
use crate::db::folders::{self, Folder};
use crate::db::tags::{self, Tag};
use crate::db::targets::{self, ResolvedTarget};
use crate::db::{settings, Db};
use crate::error::{AppError, AppResult};
use crate::metadata::{Fetcher, PageMeta};
use crate::shortcut::{self, ActiveShortcut};

#[tauri::command]
pub fn list_folders(db: State<'_, Db>) -> AppResult<Vec<Folder>> {
    folders::list_all(&db.conn())
}

#[tauri::command]
pub fn create_folder(db: State<'_, Db>, parent_id: Option<i64>, name: String) -> AppResult<Folder> {
    folders::create(&db.conn(), parent_id, &name)
}

#[tauri::command]
pub fn rename_folder(db: State<'_, Db>, id: i64, name: String) -> AppResult<Folder> {
    folders::rename(&db.conn(), id, &name)
}

#[tauri::command]
pub fn delete_folder(db: State<'_, Db>, id: i64) -> AppResult<()> {
    folders::delete(&db.conn(), id)
}

#[tauri::command]
pub fn list_bookmarks(db: State<'_, Db>, folder_id: Option<i64>) -> AppResult<Vec<Bookmark>> {
    bookmarks::list(&db.conn(), folder_id)
}

#[tauri::command]
pub fn create_bookmark(db: State<'_, Db>, bookmark: NewBookmark) -> AppResult<Bookmark> {
    bookmarks::create(&db.conn(), bookmark)
}

#[tauri::command]
pub fn update_bookmark(db: State<'_, Db>, id: i64, patch: BookmarkPatch) -> AppResult<Bookmark> {
    bookmarks::update(&db.conn(), id, patch)
}

#[tauri::command]
pub fn delete_bookmark(db: State<'_, Db>, id: i64) -> AppResult<()> {
    bookmarks::delete(&db.conn(), id)
}

// Async so detection's registry/file reads run off the main thread.
#[tauri::command]
pub async fn list_browsers(
    db: State<'_, Db>,
    cache: State<'_, BrowserCache>,
    refresh: bool,
) -> AppResult<Vec<Browser>> {
    Ok(all_browsers(&db, &cache, refresh))
}

/// Detected and custom browsers with the user's order, labels and hidden flags. Hidden ones
/// are included (the picker leaves them out) so saved defaults can still open them.
fn all_browsers(db: &Db, cache: &BrowserCache, refresh: bool) -> Vec<Browser> {
    let detected = cache.get(refresh);
    prefs::apply(detected, &load_browser_prefs(db))
}

/// Unreadable prefs fall back to the defaults rather than breaking the browser list.
fn load_browser_prefs(db: &Db) -> BrowserPrefs {
    settings::get(&db.conn(), settings::BROWSER_PREFS)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_browser_prefs(db: State<'_, Db>) -> BrowserPrefs {
    load_browser_prefs(&db)
}

/// Saves browser prefs (new custom browsers get ids) and returns the updated browser list.
#[tauri::command]
pub async fn set_browser_prefs(
    db: State<'_, Db>,
    cache: State<'_, BrowserCache>,
    prefs: BrowserPrefs,
) -> AppResult<Vec<Browser>> {
    let detected_ids: Vec<String> = cache.get(false).into_iter().map(|b| b.id).collect();
    let prefs = prefs::validate(prefs, &detected_ids)?;
    let json = serde_json::to_string(&prefs).map_err(|e| AppError::Invalid(e.to_string()))?;
    settings::set(&db.conn(), settings::BROWSER_PREFS, &json)?;
    Ok(all_browsers(&db, &cache, false))
}

/// Opens the bookmark in the target browser, then records the open and remembers the target.
#[tauri::command]
pub async fn open_url(
    db: State<'_, Db>,
    cache: State<'_, BrowserCache>,
    bookmark_id: i64,
    target: LaunchTarget,
) -> AppResult<()> {
    let url = bookmarks::get(&db.conn(), bookmark_id)?.url;
    let browsers = all_browsers(&db, &cache, false);
    let browser = browsers
        .iter()
        .find(|b| b.id == target.browser_id)
        .ok_or(AppError::NotFound("browser"))?;
    let cmd = launch::build(browser, target.profile_id.as_deref(), target.private, &url, Os::current())?;
    launch::spawn(&cmd, &browser.name)?;

    let conn = db.conn();
    bookmarks::record_open(&conn, bookmark_id)?;
    let target = serde_json::to_string(&target).map_err(|e| AppError::Invalid(e.to_string()))?;
    settings::set(&conn, settings::LAST_TARGET, &target)
}

#[tauri::command]
pub fn get_setting(db: State<'_, Db>, key: String) -> AppResult<Option<String>> {
    settings::get(&db.conn(), &key)
}

#[tauri::command]
pub fn set_setting(db: State<'_, Db>, key: String, value: String) -> AppResult<()> {
    settings::set(&db.conn(), &key, &value)
}

#[tauri::command]
pub fn search(db: State<'_, Db>, query: String, limit: Option<usize>) -> AppResult<Vec<Bookmark>> {
    crate::db::search::search(&db.conn(), &query, limit.unwrap_or(50))
}

/// Moves a bookmark to `folder_id` (`null` = Unfiled) at `index` among its new siblings
/// (`null` = last). Also used to reorder within a folder.
#[tauri::command]
pub fn move_bookmark(
    db: State<'_, Db>,
    id: i64,
    folder_id: Option<i64>,
    index: Option<usize>,
) -> AppResult<Bookmark> {
    bookmarks::move_to(&db.conn(), id, folder_id, index)
}

/// Moves a folder under `parent_id` (`null` = top level) at `index` among its new siblings.
#[tauri::command]
pub fn move_folder(
    db: State<'_, Db>,
    id: i64,
    parent_id: Option<i64>,
    index: Option<usize>,
) -> AppResult<Folder> {
    folders::move_to(&db.conn(), id, parent_id, index)
}

#[tauri::command]
pub fn set_folder_target(db: State<'_, Db>, id: i64, target: Option<LaunchTarget>) -> AppResult<Folder> {
    folders::set_default_target(&db.conn(), id, target.as_ref())
}

/// The bookmark's default browser (own or inherited from a folder), if any.
#[tauri::command]
pub fn resolve_target(db: State<'_, Db>, bookmark_id: i64) -> AppResult<Option<ResolvedTarget>> {
    targets::resolve(&db.conn(), bookmark_id)
}

#[tauri::command]
pub fn list_tags(db: State<'_, Db>) -> AppResult<Vec<Tag>> {
    tags::list_all(&db.conn())
}

/// Title and favicon for a URL (normalized like a bookmark URL). Stores the icon file.
#[tauri::command]
pub async fn fetch_metadata(fetcher: State<'_, Fetcher>, url: String) -> AppResult<PageMeta> {
    let url = bookmarks::normalize_url(&url)?;
    fetcher.fetch(&url).await
}

/// Fetches a saved bookmark's metadata and stores the favicon, filling the title if empty.
#[tauri::command]
pub async fn refresh_metadata(
    db: State<'_, Db>,
    fetcher: State<'_, Fetcher>,
    bookmark_id: i64,
) -> AppResult<Bookmark> {
    let url = bookmarks::get(&db.conn(), bookmark_id)?.url;
    let meta = fetcher.fetch(&url).await?;
    bookmarks::apply_metadata(&db.conn(), bookmark_id, meta.title.as_deref(), meta.favicon.as_deref())
}

/// Fetches icons (and empty titles) for every web bookmark without an icon, a few at a time.
/// Returns how many icons were found.
#[tauri::command]
pub async fn fetch_missing_favicons(db: State<'_, Db>, fetcher: State<'_, Fetcher>) -> AppResult<usize> {
    const CONCURRENT: usize = 4;
    let pending = bookmarks::missing_favicons(&db.conn())?;
    let mut found = 0;
    for chunk in pending.chunks(CONCURRENT) {
        let tasks: Vec<_> = chunk
            .iter()
            .cloned()
            .map(|(id, url)| {
                let fetcher = fetcher.inner().clone();
                tauri::async_runtime::spawn(async move { (id, fetcher.fetch(&url).await) })
            })
            .collect();
        for task in tasks {
            // One site failing doesn't stop the rest.
            let Ok((id, Ok(meta))) = task.await else { continue };
            if meta.favicon.is_some() {
                found += 1;
            }
            // The bookmark may have been deleted meanwhile.
            let _ = bookmarks::apply_metadata(&db.conn(), id, meta.title.as_deref(), meta.favicon.as_deref());
        }
    }
    Ok(found)
}

/// Absolute path of the favicons dir; the frontend shows icons from it via the asset protocol.
#[tauri::command]
pub fn favicon_dir(fetcher: State<'_, Fetcher>) -> String {
    fetcher.favicon_dir.to_string_lossy().into_owned()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    /// "" = no global shortcut.
    pub global_shortcut: String,
    /// Why the configured shortcut isn't active, if it isn't.
    pub shortcut_error: Option<String>,
    pub close_to_tray: bool,
    pub autostart: bool,
    /// Global shortcuts don't work under Wayland; Settings suggests a desktop shortcut instead.
    pub wayland: bool,
    /// For that desktop shortcut: `<executable> --search`.
    pub executable: String,
}

pub fn close_to_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    let db = app.state::<Db>();
    let value = settings::get(&db.conn(), settings::CLOSE_TO_TRAY).ok().flatten();
    value.as_deref() != Some("false")
}

#[tauri::command]
pub fn get_desktop_settings(app: AppHandle, db: State<'_, Db>) -> AppResult<DesktopSettings> {
    let global_shortcut = settings::get(&db.conn(), settings::GLOBAL_SHORTCUT)?
        .unwrap_or_else(|| shortcut::DEFAULT.to_string());
    let autostart = app
        .autolaunch()
        .is_enabled()
        .map_err(|e| AppError::Invalid(format!("couldn't read the launch-at-login setting: {e}")))?;
    let wayland = cfg!(target_os = "linux")
        && (std::env::var_os("WAYLAND_DISPLAY").is_some()
            || std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland"));
    Ok(DesktopSettings {
        global_shortcut,
        shortcut_error: app.state::<ActiveShortcut>().error(),
        close_to_tray: close_to_tray(&app),
        autostart,
        wayland,
        executable: std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

/// Registers and saves the global shortcut ("" = none). Nothing is saved if it can't be registered.
#[tauri::command]
pub fn set_global_shortcut(app: AppHandle, db: State<'_, Db>, shortcut: String) -> AppResult<DesktopSettings> {
    let shortcut = shortcut.trim().to_string();
    shortcut::apply(&app, Some(&shortcut))?;
    settings::set(&db.conn(), settings::GLOBAL_SHORTCUT, &shortcut)?;
    get_desktop_settings(app, db)
}

#[tauri::command]
pub fn set_close_to_tray(db: State<'_, Db>, enabled: bool) -> AppResult<()> {
    settings::set(&db.conn(), settings::CLOSE_TO_TRAY, if enabled { "true" } else { "false" })
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> AppResult<()> {
    let manager = app.autolaunch();
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| AppError::Invalid(format!("couldn't change launch at login: {e}")))
}
