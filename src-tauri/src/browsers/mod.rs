//! Installed browser detection, their profiles, and launching a URL in one.

mod known;
pub mod launch;
pub mod prefs;
mod profiles;

#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(any(target_os = "macos", test))]
mod macos;
#[cfg(windows)]
mod windows;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub use launch::LaunchTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Chromium,
    Firefox,
    Safari,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    Mac,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Chromium: profile directory ("Profile 1"). Firefox: profile name.
    pub id: String,
    /// Display name: the user's label if set, else `default_name`.
    pub name: String,
    pub email: Option<String>,
    /// Name as detected.
    pub default_name: String,
    /// Left out of the picker.
    pub hidden: bool,
}

impl Profile {
    pub fn new(id: impl Into<String>, name: impl Into<String>, email: Option<String>) -> Self {
        let name = name.into();
        Self { id: id.into(), default_name: name.clone(), name, email, hidden: false }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    pub id: String,
    /// Display name: the user's label if set, else `default_name`.
    pub name: String,
    pub kind: BrowserKind,
    pub profiles: Vec<Profile>,
    pub supports_private: bool,
    /// Name as detected (or as given for a custom browser).
    pub default_name: String,
    /// Left out of the picker; it can still be opened through a saved default.
    pub hidden: bool,
    /// Added by the user rather than detected.
    pub custom: bool,
    /// The command, for display.
    pub path: String,
    /// Program and fixed leading args (Linux `Exec=`); on macOS, the `.app` bundle path.
    #[serde(skip)]
    pub exec: Vec<String>,
    #[serde(skip)]
    pub private_flag: Option<String>,
}

impl Browser {
    pub fn new(
        id: String,
        name: String,
        kind: BrowserKind,
        profiles: Vec<Profile>,
        exec: Vec<String>,
        private_flag: Option<String>,
    ) -> Self {
        Self {
            supports_private: kind == BrowserKind::Firefox || private_flag.is_some(),
            default_name: name.clone(),
            path: exec.join(" "),
            id,
            name,
            kind,
            profiles,
            hidden: false,
            custom: false,
            exec,
            private_flag,
        }
    }
}

/// What a platform scan found, before profiles are read and ids assigned.
pub(crate) struct Detected {
    pub name: String,
    pub exec: Vec<String>,
    /// Registry key, desktop file id or bundle id. Names unknown browsers.
    pub source_id: String,
    pub known: Option<&'static known::Known>,
}

/// Detection results, kept until the user asks for a re-scan.
#[derive(Default)]
pub struct BrowserCache(Mutex<Option<Vec<Browser>>>);

impl BrowserCache {
    pub fn get(&self, refresh: bool) -> Vec<Browser> {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if refresh || cache.is_none() {
            *cache = Some(detect());
        }
        cache.clone().unwrap_or_default()
    }
}

pub fn detect() -> Vec<Browser> {
    #[cfg(windows)]
    let found = windows::detect();
    #[cfg(target_os = "linux")]
    let found = linux::detect();
    #[cfg(target_os = "macos")]
    let found = macos::detect();
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    let found: Vec<Detected> = Vec::new();

    finalize(found, Os::current())
}

fn finalize(found: Vec<Detected>, os: Os) -> Vec<Browser> {
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut browsers: Vec<Browser> = found
        .into_iter()
        .map(|d| {
            let base_id = match d.known {
                Some(k) => k.id.to_string(),
                None => slug(&d.source_id),
            };
            // Two installs of one family (e.g. native + Flatpak) get "firefox", "firefox-2".
            let n = used.entry(base_id.clone()).or_default();
            *n += 1;
            let id = if *n == 1 { base_id } else { format!("{base_id}-{n}") };

            let kind = d.known.map_or(BrowserKind::Other, |k| k.kind);
            let profiles = match d.known {
                Some(k) => profiles::load(kind, &data_dirs(k, os, &d.source_id)),
                None => Vec::new(),
            };
            let private_flag = d.known.and_then(|k| k.private_flag).map(str::to_string);
            Browser::new(id, d.name, kind, profiles, d.exec, private_flag)
        })
        .collect();
    browsers.sort_by_key(|b| b.name.to_lowercase());
    browsers
}

/// Existing candidate data dirs, those belonging to this install (e.g. its Flatpak id) first.
fn data_dirs(known: &known::Known, os: Os, source_id: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<String> = known.data_dirs.get(os).iter().map(|s| s.to_string()).collect();
    dirs.sort_by_key(|d| !d.contains(source_id));
    dirs.iter().filter_map(|d| expand(d)).filter(|p| p.is_dir()).collect()
}

/// Expands a leading `~/`, `$XDG_CONFIG/` or `%VAR%`.
fn expand(path: &str) -> Option<PathBuf> {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(rest) = path.strip_prefix("~/") {
        return Some(env("HOME")?.join(rest));
    }
    if let Some(rest) = path.strip_prefix("$XDG_CONFIG/") {
        let base = env("XDG_CONFIG_HOME").or_else(|| Some(env("HOME")?.join(".config")))?;
        return Some(base.join(rest));
    }
    if let Some(rest) = path.strip_prefix('%') {
        let (var, rest) = rest.split_once('%')?;
        let rest = rest.trim_start_matches(['\\', '/']);
        let base = env(var)?;
        return Some(if rest.is_empty() { base } else { base.join(rest) });
    }
    Some(PathBuf::from(path))
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-');
    if out.is_empty() { "browser".into() } else { out.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected(name: &str, source_id: &str, os: Os, ident: &str) -> Detected {
        Detected {
            name: name.into(),
            exec: vec!["prog".into()],
            source_id: source_id.into(),
            known: known::find(os, ident),
        }
    }

    #[test]
    fn finalize_assigns_ids_and_kinds() {
        let os = Os::Linux;
        let browsers = finalize(
            vec![
                detected("Firefox", "firefox", os, "firefox"),
                detected("Firefox (Flatpak)", "org.mozilla.firefox", os, "org.mozilla.firefox"),
                detected("Some Viewer", "Some.Viewer App", os, "some.viewer app"),
                detected("Brave", "brave-browser", os, "brave-browser"),
            ],
            os,
        );
        let summary: Vec<_> = browsers
            .iter()
            .map(|b| (b.id.as_str(), b.kind, b.supports_private))
            .collect();
        assert_eq!(
            summary,
            [
                ("brave", BrowserKind::Chromium, true),
                ("firefox", BrowserKind::Firefox, true),
                ("firefox-2", BrowserKind::Firefox, true),
                ("some-viewer-app", BrowserKind::Other, false),
            ]
        );
        assert_eq!(browsers[0].private_flag.as_deref(), Some("--incognito"));
    }

    /// `cargo test print_detected -- --ignored --nocapture` lists this machine's browsers.
    #[test]
    #[ignore]
    fn print_detected() {
        for b in detect() {
            println!("{} [{}] {:?} {:?} private={}", b.name, b.id, b.kind, b.exec, b.supports_private);
            for p in &b.profiles {
                println!("    {} = {} {:?}", p.id, p.name, p.email);
            }
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Firefox-308046B0AF4A39CB"), "firefox-308046b0af4a39cb");
        assert_eq!(slug("  My Browser!! "), "my-browser");
        assert_eq!(slug("!!!"), "browser");
    }

    #[test]
    fn expand_prefixes() {
        assert_eq!(expand("/abs/path"), Some(PathBuf::from("/abs/path")));
        let path_var = std::env::var_os("PATH").map(PathBuf::from);
        assert_eq!(expand("%PATH%"), path_var);
        assert_eq!(expand("%LINK_OPENER_SURELY_UNSET%\\x"), None);
    }
}
