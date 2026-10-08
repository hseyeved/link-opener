# Changelog

All notable changes to Link Opener. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [0.3.1] - 2026-10-08

### Changed

- The macOS app is now signed and notarized, so it opens without a Gatekeeper warning.

### Fixed

- macOS: deleting a bookmark did nothing after clicking "Confirm?".
- macOS: the global shortcut in Settings couldn't be changed; key presses weren't recorded.

## [0.3.0] - 2026-10-02

### Added

- Licensed under the GPL-3.0-or-later; first public release.
- The version number is shown at the bottom of Settings.
- Contributing and security guides, issue templates, and weekly dependency updates.
- CI checks Rust formatting and that every dependency's license is compatible with the GPL.

### Security

- A strict Content Security Policy for the app window.

## [0.2.0]

### Added

- **Browsers** screen: hide browsers or single profiles from the picker, reorder them (which
  sets the 1–9 keys), rename them, and add browsers the scan doesn't find.
- **Backups and import**: back up the whole library (including icons) to a file, restore it
  (merge or replace, with an automatic safety backup), import bookmarks exported from Chrome,
  Edge, Firefox or Safari, and export a bookmarks file any browser can import.
- Windows ARM64 installers.

### Changed

- Installers are built in CI only for version tags (or when started by hand).

### Fixed

- "Remember for this bookmark/folder" in the browser picker failed with an error.
- Top-level folders were indented further than "Unfiled".
- Builds didn't pick up a changed app icon.

## [0.1.0]

First version.

- Folders and bookmarks with notes and tags, drag-and-drop, and a "Move to…" dialog.
- Detects installed browsers and their profiles on Windows, macOS and Linux, and opens links in
  the one you pick, optionally in a private window.
- Default browser per bookmark or folder.
- `Ctrl/Cmd+K` search across titles, URLs, notes and tags.
- Fetches page titles and favicons.
- Tray icon, close to tray, global shortcut, launch at login.
