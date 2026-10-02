# Contributing

Thanks for helping! Bug reports, fixes and testing on other systems are all welcome.

## Especially useful

- **Testing on Linux and macOS.** The app is developed on Windows; Linux and macOS are built
  in CI but get far less hands-on use. Reports from real setups (Flatpak, Snap, Wayland,
  different desktops) help a lot.
- **Browser detection.** If a browser or profile is missing or wrong, open an issue with your
  OS, the browser and how it was installed. Running
  `cargo test print_detected -- --ignored --nocapture` in `src-tauri` prints what the app
  detects. Known browsers live in `src-tauri/src/browsers/known.rs`.

## Setting up

You need Node.js LTS, Rust (stable) and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```sh
npm install
npm run tauri dev
```

## Before opening a pull request

Please make sure these pass (CI runs them on Windows, macOS and Linux):

```sh
npm run check
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- Add tests for backend changes; most modules have a `tests` section to follow.
- Keep pull requests focused on one change, and describe what you tested and on which OS.
- Match the style of the code around your change.

## Where things are

- `src-tauri/src/db/`: SQLite schema and queries (folders, bookmarks, tags, search, settings)
- `src-tauri/src/browsers/`: browser and profile detection per OS, and launching
- `src-tauri/src/metadata.rs`: title and favicon fetching
- `src-tauri/src/transfer/`: backups, and browser bookmark file import/export
- `src/lib/`: the Svelte frontend; `api.ts` is the only place that calls the backend

## License

By contributing, you agree that your contributions are licensed under the
[GPL-3.0-or-later](LICENSE), like the rest of the project.
