//! Reads the profile lists of Chromium-family (`Local State`) and Firefox-family (`profiles.ini`) browsers.

use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::{BrowserKind, Profile};

/// Profiles from the first data dir that has a profile list. Unreadable files give no profiles.
pub fn load(kind: BrowserKind, data_dirs: &[PathBuf]) -> Vec<Profile> {
    let file = match kind {
        BrowserKind::Chromium => "Local State",
        BrowserKind::Firefox => "profiles.ini",
        BrowserKind::Safari | BrowserKind::Other => return Vec::new(),
    };
    for dir in data_dirs {
        let Ok(text) = fs::read_to_string(dir.join(file)) else {
            continue;
        };
        return match kind {
            BrowserKind::Chromium => parse_local_state(&text),
            _ => parse_profiles_ini(&text),
        };
    }
    Vec::new()
}

/// Chromium: `profile.info_cache` maps profile directory → info. The id is the directory name.
pub fn parse_local_state(json: &str) -> Vec<Profile> {
    let Ok(root) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let Some(cache) = root.pointer("/profile/info_cache").and_then(Value::as_object) else {
        return Vec::new();
    };
    let order: Vec<&str> = root
        .pointer("/profile/profiles_order")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut profiles: Vec<Profile> = cache
        .iter()
        .filter(|(_, info)| !info.get("is_ephemeral").and_then(Value::as_bool).unwrap_or(false))
        .map(|(dir, info)| {
            let text = |key: &str| {
                info.get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            Profile::new(dir.clone(), text("name").unwrap_or_else(|| dir.clone()), text("user_name"))
        })
        .collect();

    let rank = |p: &Profile| order.iter().position(|d| *d == p.id).unwrap_or(usize::MAX);
    profiles.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| chromium_dir_order(&a.id, &b.id)));
    profiles
}

/// "Default" first, then "Profile N" by number, then anything else by name.
fn chromium_dir_order(a: &str, b: &str) -> Ordering {
    fn key(dir: &str) -> (u8, u64, &str) {
        if dir == "Default" {
            (0, 0, dir)
        } else if let Some(n) = dir.strip_prefix("Profile ").and_then(|n| n.parse().ok()) {
            (1, n, dir)
        } else {
            (2, 0, dir)
        }
    }
    key(a).cmp(&key(b))
}

/// Firefox: `[ProfileN]` sections with `Name=` and `Path=`. The id is the name (used with `-P`).
/// The default profile comes first: the `[Install…]` section's `Default=` path, else `Default=1`.
pub fn parse_profiles_ini(text: &str) -> Vec<Profile> {
    struct Section {
        name: String,
        entries: Vec<(String, String)>,
    }
    let mut sections: Vec<Section> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push(Section { name: name.to_string(), entries: Vec::new() });
        } else if let (Some(section), Some((k, v))) = (sections.last_mut(), line.split_once('=')) {
            section.entries.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let get = |s: &Section, key: &str| {
        s.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    };

    let install_default = sections
        .iter()
        .filter(|s| s.name.starts_with("Install"))
        .find_map(|s| get(s, "Default"));

    let mut default_name = None;
    let mut profiles = Vec::new();
    for s in sections.iter().filter(|s| s.name.starts_with("Profile")) {
        let Some(name) = get(s, "Name").filter(|n| !n.is_empty()) else {
            continue;
        };
        let is_default = match &install_default {
            Some(path) => get(s, "Path").as_ref() == Some(path),
            None => get(s, "Default").as_deref() == Some("1"),
        };
        if is_default && default_name.is_none() {
            default_name = Some(name.clone());
        }
        profiles.push(Profile::new(name.clone(), name, None));
    }
    // Stable sort: the default moves to the front, the rest keep file order.
    profiles.sort_by_key(|p| Some(&p.id) != default_name.as_ref());
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCAL_STATE: &str = r#"{
        "browser": { "enabled_labs_experiments": [] },
        "profile": {
            "info_cache": {
                "Profile 10": { "name": "Ten", "user_name": "" },
                "Profile 2":  { "name": "Work", "user_name": "me@work.example" },
                "Default":    { "name": "Personal", "user_name": "me@example.com" },
                "Guest Profile": { "name": "Guest", "is_ephemeral": true },
                "Profile 3":  { "name": "  " }
            },
            "last_used": "Profile 2"
        }
    }"#;

    #[test]
    fn chromium_profiles() {
        let profiles = parse_local_state(LOCAL_STATE);
        let ids: Vec<_> = profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["Default", "Profile 2", "Profile 3", "Profile 10"]);
        assert_eq!(profiles[0].name, "Personal");
        assert_eq!(profiles[0].email.as_deref(), Some("me@example.com"));
        assert_eq!(profiles[1].name, "Work");
        // Blank name falls back to the directory; blank email is dropped.
        assert_eq!(profiles[2].name, "Profile 3");
        assert_eq!(profiles[3].email, None);
    }

    #[test]
    fn chromium_profiles_order_wins() {
        let json = r#"{"profile": {
            "info_cache": { "Default": {"name": "A"}, "Profile 1": {"name": "B"} },
            "profiles_order": ["Profile 1", "Default"]
        }}"#;
        let ids: Vec<_> = parse_local_state(json).into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["Profile 1", "Default"]);
    }

    #[test]
    fn chromium_bad_input() {
        assert!(parse_local_state("not json").is_empty());
        assert!(parse_local_state("{}").is_empty());
    }

    const PROFILES_INI: &str = "\
[Install308046B0AF4A39CB]
Default=Profiles/abcd.dev-edition
Locked=1

[Profile1]
Name=default
IsRelative=1
Path=Profiles/xyz.default
Default=1

[Profile0]
Name=Work
IsRelative=1
Path=Profiles/abcd.dev-edition

[Profile2]
Name=Testing
IsRelative=1
Path=Profiles/t.testing

[General]
StartWithLastProfile=1
Version=2
";

    #[test]
    fn firefox_profiles_install_default_first() {
        let names: Vec<_> = parse_profiles_ini(PROFILES_INI).into_iter().map(|p| p.id).collect();
        assert_eq!(names, ["Work", "default", "Testing"]);
    }

    #[test]
    fn firefox_profiles_default_flag_without_install_section() {
        let ini = "[Profile0]\nName=a\nPath=p/a\n\n[Profile1]\nName=b\nPath=p/b\nDefault=1\n";
        let names: Vec<_> = parse_profiles_ini(ini).into_iter().map(|p| p.id).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[test]
    fn load_reads_first_dir_with_a_profile_list() {
        let empty = tempfile::tempdir().unwrap();
        let ff = tempfile::tempdir().unwrap();
        std::fs::write(ff.path().join("profiles.ini"), PROFILES_INI).unwrap();
        let dirs = [empty.path().to_path_buf(), ff.path().to_path_buf()];
        assert_eq!(load(BrowserKind::Firefox, &dirs).len(), 3);
        assert!(load(BrowserKind::Chromium, &dirs).is_empty());
        assert!(load(BrowserKind::Safari, &dirs).is_empty());
    }
}
