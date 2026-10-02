# Link Store & Opener: Plan

## Context
Bookmarks are tied to whichever browser saved them, so you end up with copies spread across several browsers. This app keeps one bookmark library outside all browsers. When you open a link, it asks which **browser and profile** to use, with an optional private/incognito window. It must run on Windows, macOS and Linux. The project directory is empty, and Rust and Node are installed.

**Stack:** Tauri 2 + Rust backend, Svelte 5 + TypeScript (Vite) frontend, SQLite through `rusqlite` (bundled, FTS5).

**v1 scope (confirmed):** folders, add/edit/move, Ctrl/Cmd+K search, a global shortcut to show the app, a browser picker that includes profiles and private mode, title/favicon auto-fetch, tags, a per-bookmark or per-folder default browser, tray icon, and launch at login.

## Architecture

### Rust (`src-tauri/src/`)
- `lib.rs`: Tauri builder that registers these plugins: `global-shortcut`, `single-instance` (a second launch focuses the existing window), `autostart`, `window-state`, `opener`; plus the tray and the commands.
- `db/`: `mod.rs` (connection in `Mutex<Connection>` in Tauri state, WAL mode), `migrations.rs` (`rusqlite_migration`), `bookmarks.rs`, `folders.rs`, `tags.rs`, `search.rs`.
- `browsers/`:
  - `mod.rs`: `Browser { id, name, kind: Chromium|Firefox|Safari|Other, exe, icon }`, `Profile { id, name }`, `LaunchTarget { browser_id, profile_id?, private }`.
  - `windows.rs`: read the registry at `HKLM/HKCU\SOFTWARE\Clients\StartMenuInternet\*\shell\open\command` (`winreg` crate).
  - `macos.rs`: scan `/Applications` and `~/Applications` for bundles whose `Info.plist` (`plist` crate) lists `https` in `CFBundleURLTypes`. This replaced `LSCopyApplicationURLsForURL`: no FFI, and the parsing is unit-tested on any OS.
  - `linux.rs`: parse `.desktop` files in `/usr/share/applications`, `~/.local/share/applications`, and the flatpak/snap export dirs whose MimeType includes `x-scheme-handler/https`.
  - `profiles.rs`: Chromium family (Chrome, Edge, Brave, Vivaldi, Chromium, Opera): read `profile.info_cache` from the `Local State` JSON in each browser's user-data dir. Firefox (and LibreWolf/Zen/Waterfox): read `profiles.ini`.
  - `launch.rs`: build the command line. Chromium: `--profile-directory="Profile 1"`, plus `--incognito` (or `--inprivate` for Edge). Firefox: `-P <name> -new-tab <url>` or `--private-window <url>`. Safari: `open -a Safari <url>` only (no profile or private flag available). On macOS, launch with `open -na <App> --args …`.
  - Cache detection results and re-scan on demand from a "Refresh browsers" button.
- `metadata.rs`: `reqwest` + `scraper`. Fetch the page, take `<title>` and `og:title`, then the favicon from `<link rel=icon>` with `/favicon.ico` as a fallback. Save icons to `appDataDir/favicons/<hash>.png`. Use a 5s timeout and never block saving a bookmark on it.
- `commands.rs`: the `#[tauri::command]` surface: CRUD for bookmarks/folders/tags, `move_items`, `reorder`, `search`, `list_browsers`, `open_url(bookmark_id, target)`, `fetch_metadata`, `get/set_settings`, `set_global_shortcut`.
- `tray.rs`: tray menu with Show, Quick search, Settings and Quit. Closing the window hides it to the tray.

### Data model (SQLite)
- `folders(id, parent_id, name, position, default_target JSON NULL)`
- `bookmarks(id, folder_id, title, url, notes, favicon, position, default_target JSON NULL, created_at, updated_at, last_opened_at, open_count)`
- `tags(id, name UNIQUE)`, `bookmark_tags(bookmark_id, tag_id)`
- `bookmarks_fts`: an FTS5 virtual table over title, url, notes and tags, using the `trigram` tokenizer so substring matches work, kept in sync by triggers.
- `settings(key, value)`: global shortcut, theme, hidden browsers, last-used target.
- Default browser lookup order: bookmark target, then the nearest ancestor folder's target, then the picker.

### Frontend (`src/`)
- `lib/api.ts`: typed `invoke` wrappers, so no component calls `invoke` directly.
- Components:
  - `FolderTree`: nested tree with drag-and-drop to move and reorder.
  - `BookmarkList`
  - `BookmarkEditor`: add/edit modal. Pasting a URL triggers the auto-fetch. It also has a tag input, a folder select, and a default browser select.
  - `CommandPalette`: Ctrl/Cmd+K, results as you type, Enter opens the browser picker, Ctrl+Enter edits the bookmark.
  - `BrowserPicker`: a list of browser × profile options, each with a private toggle.
  - `MoveDialog`: a "Move to…" folder search, so moving also works from the keyboard.
  - `Settings`
- `BrowserPicker` UX:
  - Number keys 1–9 pick an option; Shift selects private.
  - A "Remember for this bookmark / folder" checkbox saves `default_target`.
  - Esc cancels.
  - A setting decides what Enter does for a bookmark that has a default: open it directly, or show the picker with that option preselected.

### Global shortcut
- Configurable in Settings, default `Ctrl+Alt+Space`. It shows and focuses the window with the palette open.
- **Linux Wayland caveat:** apps can't register global shortcuts on Wayland. Fallback: the binary accepts `--show`, which `single-instance` forwards to the running app. Settings explains how to bind this command in GNOME or KDE system shortcuts.

## Milestones
1. Scaffold with `npm create tauri-app` (Svelte-TS), add plugins, set up the DB and migrations, and build basic folder/bookmark CRUD with the tree and list views.
2. Browser and profile detection plus launch on Windows first (your dev machine), then Linux and macOS; then build the `BrowserPicker`.
3. FTS search and the Ctrl+K palette.
4. Move: drag-and-drop and the dialog. Tags. Per-bookmark/per-folder default browser.
5. Metadata/favicon fetch.
6. Tray, close-to-tray, global shortcut and its Settings UI, autostart, single instance.
7. Admin to manage which Browsers are available. Ability to refresh browser list.
8. GitHub Actions `tauri-action` matrix to build installers for all three OSes: MSI/NSIS, DMG, AppImage/deb.

## Dev environment note
Develop natively on Windows, not in WSL, so the app detects your real Windows browsers. Prerequisites:
- Rust with the MSVC toolchain (`rustup default stable-msvc`).
- Visual Studio Build Tools with the "Desktop development with C++" workload.
- WebView2 (preinstalled on Windows 10/11).
- Node.js LTS.

Linux and macOS builds come from CI, and macOS also needs a Mac to test on.

## Suggested later additions (not v1)
- **Register as the OS default browser:** links clicked in Slack, email and other apps would also go through the picker. This fits the main goal well and reuses the same picker.
- Import from browser HTML bookmark exports, and export JSON/HTML for backups.
- Sync between your machines, e.g. putting the DB/export in a synced folder.
- "Open all in folder", dead-link checker, duplicate URL detection.

## Verification
- `cargo test` covers:
  - DB CRUD, moves, FTS search and the default-target lookup order.
  - Profile parsing against fixture `Local State` and `profiles.ini` files.
  - Unit tests of the launch-arg builder for each browser kind.
- `npm run tauri dev` with this manual checklist:
  - Create nested folders, add/edit/move bookmarks.
  - Ctrl+K finds a bookmark by part of its title or URL.
  - Open a link in each detected browser/profile and in private mode.
  - "Remember" skips the picker next time.
  - Tray hide/show works, the global shortcut shows the window, and relaunching focuses the existing instance.
- The CI matrix builds succeed on all three OSes. Smoke-test the installers on Windows and macOS.
