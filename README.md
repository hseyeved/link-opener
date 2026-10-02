# Link Opener

A bookmark library that lives outside your browsers. When you open a link, it asks which
**browser and profile** to use (optionally in a private window), or uses the default you've
set for that bookmark or folder. Runs on Windows, macOS and Linux.

- Folders, tags, notes, drag-and-drop, and a `Ctrl/Cmd+K` search palette
- Detects installed browsers and their profiles (Chromium family, Firefox family, Safari)
- Fetches page titles and favicons
- Tray icon, global shortcut (`Ctrl+Alt+Space` by default), launch at login
- Backups, plus import from (and export to) browser bookmark files

Built with Tauri 2 (Rust) and Svelte 5.

## Privacy

Link Opener has no accounts, analytics or telemetry. Your library stays in a local SQLite
database on your computer.

- **Reads, never writes, browser data:** to list browsers and profiles it reads the Windows
  registry (`StartMenuInternet`), `.desktop` files on Linux, app bundles' `Info.plist` on macOS,
  and each browser's profile list (`Local State` for Chromium browsers, `profiles.ini` for
  Firefox). It doesn't touch history, passwords or cookies.
- **Network:** it only contacts the pages you bookmark, to fetch their title and icon (when you
  add or edit a bookmark, or choose "Fetch missing icons"). Nothing else is sent anywhere.
- **Opening links** starts the browser you pick with the link as an argument; it never goes
  through a shell.

## Development

Prerequisites: Node.js LTS and Rust (stable), plus the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS. On Windows that is
Visual Studio Build Tools with the C++ workload; WebView2 ships with Windows 10/11.

```sh
npm install
npm run tauri dev      # run the app with hot reload
npm run check          # type-check the frontend
cd src-tauri
cargo test             # backend tests
cargo clippy --all-targets
```

Useful extras:

```sh
cargo test print_detected -- --ignored --nocapture   # list this machine's browsers/profiles
LIVE_URLS="https://example.com" cargo test live_fetch -- --ignored --nocapture   # try a metadata fetch
```

Data lives in the app data dir (`%APPDATA%\dev.linkopener` on Windows,
`~/Library/Application Support/dev.linkopener` on macOS,
`~/.local/share/dev.linkopener` on Linux): `links.db` (SQLite) and `favicons/`.

## Building installers

Locally, for the current OS:

```sh
npm run tauri build
```

Installers end up in `src-tauri/target/release/bundle/`.

### CI and releases

[`.github/workflows/build.yml`](.github/workflows/build.yml) has two jobs:

1. **Test**, on every push and pull request, on Windows, macOS and Linux: `npm run check`, a
   frontend build, clippy, `cargo test`.
2. **Installers**, only for version tags (`v*`) or when run by hand (Actions → Build → Run
   workflow), after the tests pass: Windows x64 and ARM64 (MSI and NSIS), macOS universal (DMG),
   Linux x64 (AppImage, deb, rpm). They're attached to the run as artifacts.

To publish a release:

1. Bump the version in `src-tauri/tauri.conf.json` (and `package.json` / `src-tauri/Cargo.toml`
   to match).
2. Add the changes to [CHANGELOG.md](CHANGELOG.md).
3. Commit, then tag and push: `git tag v0.3.0 && git push origin v0.3.0`.
4. The workflow creates a **draft** release with the installers; review it and publish.

### Code signing

The installers are not signed yet, so Windows SmartScreen and macOS Gatekeeper will warn on
first launch (on macOS: right-click → Open). To sign, add the certificates as repository
secrets and pass them to `tauri-action`; see Tauri's
[Windows](https://tauri.app/distribute/sign/windows/) and
[macOS](https://tauri.app/distribute/sign/macos/) signing guides. Open-source projects can get
free Windows signing from the [SignPath Foundation](https://signpath.org/).

## Contributing

Bug reports, browser-detection fixes and testing on Linux and macOS are especially welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md). To report a security problem, see [SECURITY.md](SECURITY.md).

## License

Copyright © 2026 hseyeved.

Link Opener is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version. It is distributed in the hope that it will be
useful, but WITHOUT ANY WARRANTY. See [LICENSE](LICENSE) for the full text.
