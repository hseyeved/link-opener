//! Linux: `.desktop` entries that handle `x-scheme-handler/https`.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use super::{known, Detected, Os};

/// Most specific first: a user entry overrides a system one with the same id.
#[cfg(target_os = "linux")]
pub fn detect() -> Vec<Detected> {
    use super::expand;
    use std::path::Path;

    let mut dirs: Vec<PathBuf> = Vec::new();
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| expand("~/.local/share"));
    dirs.extend(data_home.map(|d| d.join("applications")));
    dirs.extend(expand("~/.local/share/flatpak/exports/share/applications"));
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|d| !d.is_empty())
            .map(|d| Path::new(d).join("applications")),
    );
    dirs.push("/var/lib/flatpak/exports/share/applications".into());
    dirs.push("/var/lib/snapd/desktop/applications".into());
    detect_in(&dirs)
}

pub fn detect_in(dirs: &[PathBuf]) -> Vec<Detected> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let Some(id) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".desktop"))
            else {
                continue;
            };
            if !seen.insert(id.to_string()) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Some(entry) = parse_desktop_entry(&text) else {
                continue;
            };
            if !entry.handles_https || id == "dev.linkopener" {
                continue;
            }
            let exec = parse_exec(&entry.exec);
            if exec.is_empty() {
                continue;
            }
            found.push(Detected {
                name: entry.name,
                exec,
                source_id: id.to_string(),
                known: known::find(Os::Linux, id),
            });
        }
    }
    found
}

#[derive(Debug, PartialEq)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub handles_https: bool,
}

/// The `[Desktop Entry]` group of a visible application entry.
pub fn parse_desktop_entry(text: &str) -> Option<DesktopEntry> {
    let mut in_main = false;
    let (mut name, mut exec, mut mime) = (None, None, String::new());
    let (mut app, mut hidden) = (false, false);
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Name" => name = Some(value.to_string()),
            "Exec" => exec = Some(value.to_string()),
            "MimeType" => mime = value.to_string(),
            "Type" => app = value == "Application",
            "Hidden" | "NoDisplay" if value == "true" => hidden = true,
            _ => {}
        }
    }
    if !app || hidden {
        return None;
    }
    Some(DesktopEntry {
        name: name?,
        exec: exec?,
        handles_https: mime
            .split(';')
            .any(|m| m.trim() == "x-scheme-handler/https"),
    })
}

/// Splits an `Exec=` value into arguments and drops field codes (`%u`, `%U`, …) and
/// Flatpak's `@@u`/`@@` markers.
pub fn parse_exec(exec: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut has_token = false;
    let mut in_quotes = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            '\\' if in_quotes => {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    args.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        args.push(cur);
    }
    args.into_iter()
        .filter(|a| !is_field_code(a) && a != "@@u" && a != "@@")
        .map(|a| a.replace("%%", "%"))
        .collect()
}

fn is_field_code(arg: &str) -> bool {
    let b = arg.as_bytes();
    b.len() == 2 && b[0] == b'%' && b"fFuUdDnNickvm".contains(&b[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const CHROME: &str = "\
[Desktop Entry]
Version=1.0
Name=Google Chrome
Name[de]=Google Chrome DE
Exec=/usr/bin/google-chrome-stable %U
Type=Application
MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;

[Desktop Action new-private-window]
Name=New Incognito Window
Exec=/usr/bin/google-chrome-stable --incognito
";

    #[test]
    fn desktop_entry() {
        assert_eq!(
            parse_desktop_entry(CHROME),
            Some(DesktopEntry {
                name: "Google Chrome".into(),
                exec: "/usr/bin/google-chrome-stable %U".into(),
                handles_https: true,
            })
        );
        let hidden = "[Desktop Entry]\nName=X\nExec=x\nType=Application\nNoDisplay=true\n";
        assert_eq!(parse_desktop_entry(hidden), None);
        let link = "[Desktop Entry]\nName=X\nURL=https://x\nType=Link\n";
        assert_eq!(parse_desktop_entry(link), None);
    }

    #[test]
    fn exec_lines() {
        assert_eq!(
            parse_exec("/usr/bin/google-chrome-stable %U"),
            ["/usr/bin/google-chrome-stable"]
        );
        assert_eq!(
            parse_exec("/usr/bin/flatpak run --branch=stable --arch=x86_64 --command=firefox --file-forwarding org.mozilla.firefox @@u %u @@"),
            ["/usr/bin/flatpak", "run", "--branch=stable", "--arch=x86_64", "--command=firefox", "--file-forwarding", "org.mozilla.firefox"]
        );
        assert_eq!(
            parse_exec(r#""/opt/My Browser/browser" --name "a \"b\"" 100%%"#),
            ["/opt/My Browser/browser", "--name", r#"a "b""#, "100%"]
        );
        assert_eq!(parse_exec(r#"env "" x"#), ["env", "", "x"]);
    }

    #[test]
    fn detect_in_dirs() {
        let user = tempfile::tempdir().unwrap();
        let system = tempfile::tempdir().unwrap();
        let write = |dir: &Path, file: &str, text: &str| fs::write(dir.join(file), text).unwrap();
        write(system.path(), "google-chrome.desktop", CHROME);
        write(
            system.path(),
            "org.mozilla.firefox.desktop",
            "[Desktop Entry]\nName=Firefox\nExec=/usr/bin/flatpak run org.mozilla.firefox @@u %u @@\nType=Application\nMimeType=x-scheme-handler/https;\n",
        );
        write(
            system.path(),
            "editor.desktop",
            "[Desktop Entry]\nName=Editor\nExec=ed %F\nType=Application\nMimeType=text/plain;\n",
        );
        // The user's copy of the Chrome entry hides it.
        write(
            user.path(),
            "google-chrome.desktop",
            "[Desktop Entry]\nName=Chrome\nExec=x\nType=Application\nHidden=true\n",
        );

        let found = detect_in(&[
            user.path().into(),
            system.path().into(),
            "/nonexistent".into(),
        ]);
        let summary: Vec<_> = found
            .iter()
            .map(|d| (d.source_id.as_str(), d.name.as_str(), d.known.map(|k| k.id)))
            .collect();
        assert_eq!(
            summary,
            [("org.mozilla.firefox", "Firefox", Some("firefox"))]
        );
        assert_eq!(
            found[0].exec,
            ["/usr/bin/flatpak", "run", "org.mozilla.firefox"]
        );
    }
}
