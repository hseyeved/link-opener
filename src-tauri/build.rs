fn main() {
    // tauri-build only re-runs when tauri.conf.json or capabilities/ change, so new icons
    // (e.g. from `npm run tauri icon`) would keep the old exe icon and window/tray icon.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
