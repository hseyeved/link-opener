//! macOS: `.app` bundles whose `Info.plist` declares the `https` URL scheme.

use std::fs;
use std::path::PathBuf;

use plist::Value;

use super::{known, Detected, Os};

#[cfg(target_os = "macos")]
pub fn detect() -> Vec<Detected> {
    let mut dirs = vec![PathBuf::from("/Applications")];
    dirs.extend(super::expand("~/Applications"));
    detect_in(&dirs)
}

pub fn detect_in(dirs: &[PathBuf]) -> Vec<Detected> {
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut apps: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "app"))
            .collect();
        apps.sort();
        for app in apps {
            let Ok(bytes) = fs::read(app.join("Contents/Info.plist")) else {
                continue;
            };
            let Some(info) = parse_info_plist(&bytes) else {
                continue;
            };
            if !info.handles_https || info.bundle_id.eq_ignore_ascii_case("com.devistry.linkopener") {
                continue;
            }
            let name = info.name.unwrap_or_else(|| {
                app.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
            });
            found.push(Detected {
                name,
                exec: vec![app.to_string_lossy().into_owned()],
                known: known::find(Os::Mac, &info.bundle_id),
                source_id: info.bundle_id,
            });
        }
    }
    found
}

#[derive(Debug, PartialEq)]
pub struct AppInfo {
    pub bundle_id: String,
    pub name: Option<String>,
    pub handles_https: bool,
}

/// Reads an XML or binary `Info.plist`.
pub fn parse_info_plist(bytes: &[u8]) -> Option<AppInfo> {
    let value = Value::from_reader(std::io::Cursor::new(bytes)).ok()?;
    let dict = value.as_dictionary()?;
    let text = |key: &str| {
        dict.get(key)
            .and_then(Value::as_string)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let handles_https = dict
        .get("CFBundleURLTypes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| t.as_dictionary()?.get("CFBundleURLSchemes")?.as_array())
        .flatten()
        .any(|s| s.as_string().is_some_and(|s| s.eq_ignore_ascii_case("https")));
    Some(AppInfo {
        bundle_id: text("CFBundleIdentifier")?,
        name: text("CFBundleDisplayName").or_else(|| text("CFBundleName")),
        handles_https,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHROME_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>com.google.Chrome</string>
    <key>CFBundleName</key><string>Chrome</string>
    <key>CFBundleDisplayName</key><string>Google Chrome</string>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key><string>Web site URL</string>
            <key>CFBundleURLSchemes</key><array><string>http</string><string>https</string></array>
        </dict>
        <dict>
            <key>CFBundleURLSchemes</key><array><string>mailto</string></array>
        </dict>
    </array>
</dict>
</plist>"#;

    const NOTES_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
    <key>CFBundleIdentifier</key><string>com.example.notes</string>
    <key>CFBundleName</key><string>Notes</string>
</dict></plist>"#;

    #[test]
    fn info_plist() {
        assert_eq!(
            parse_info_plist(CHROME_PLIST.as_bytes()),
            Some(AppInfo {
                bundle_id: "com.google.Chrome".into(),
                name: Some("Google Chrome".into()),
                handles_https: true,
            })
        );
        assert!(!parse_info_plist(NOTES_PLIST.as_bytes()).unwrap().handles_https);
        assert_eq!(parse_info_plist(b"garbage"), None);
    }

    #[test]
    fn binary_plist() {
        let value = Value::from_reader(std::io::Cursor::new(CHROME_PLIST.as_bytes())).unwrap();
        let mut binary = Vec::new();
        value.to_writer_binary(&mut binary).unwrap();
        assert_eq!(parse_info_plist(&binary).unwrap().bundle_id, "com.google.Chrome");
    }

    #[test]
    fn detect_in_dirs() {
        let apps = tempfile::tempdir().unwrap();
        for (app, plist) in [("Google Chrome.app", CHROME_PLIST), ("Notes.app", NOTES_PLIST)] {
            let contents = apps.path().join(app).join("Contents");
            fs::create_dir_all(&contents).unwrap();
            fs::write(contents.join("Info.plist"), plist).unwrap();
        }
        fs::create_dir_all(apps.path().join("Broken.app")).unwrap();

        let found = detect_in(&[apps.path().into()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Google Chrome");
        assert_eq!(found[0].known.map(|k| k.id), Some("chrome"));
        assert!(found[0].exec[0].ends_with("Google Chrome.app"));
    }
}
