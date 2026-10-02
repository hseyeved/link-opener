//! Windows: browsers register under `SOFTWARE\Clients\StartMenuInternet`.

use std::collections::HashSet;
use std::path::Path;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use super::{expand, known, Detected, Os};

const ROOTS: [(winreg::HKEY, &str); 3] = [
    (HKEY_CURRENT_USER, r"SOFTWARE\Clients\StartMenuInternet"),
    (HKEY_LOCAL_MACHINE, r"SOFTWARE\Clients\StartMenuInternet"),
    (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Clients\StartMenuInternet"),
];

pub fn detect() -> Vec<Detected> {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for (hive, path) in ROOTS {
        let Ok(clients) = RegKey::predef(hive).open_subkey(path) else {
            continue;
        };
        for key_name in clients.enum_keys().flatten() {
            let Ok(client) = clients.open_subkey(&key_name) else {
                continue;
            };
            let Ok(command) = client
                .open_subkey(r"shell\open\command")
                .and_then(|k| k.get_value::<String, _>(""))
            else {
                continue;
            };
            let Some(exe) = parse_command_exe(&command) else {
                continue;
            };
            // HKCU and HKLM often list the same install.
            if !Path::new(&exe).is_file() || !seen.insert(exe.to_lowercase()) {
                continue;
            }
            if exe.to_lowercase().ends_with("\\iexplore.exe") {
                continue;
            }
            let known = known::find(Os::Windows, &exe);
            let name = client
                .get_value::<String, _>("")
                .ok()
                .filter(|n| !n.trim().is_empty())
                .or_else(|| known.map(|k| k.name.to_string()))
                .unwrap_or_else(|| key_name.clone());
            found.push(Detected { name, exec: vec![exe], source_id: key_name, known });
        }
    }
    found
}

/// The executable from a `shell\open\command` value: `"C:\x\b.exe" --flag "%1"` or `C:\x\b.exe`.
fn parse_command_exe(command: &str) -> Option<String> {
    let command = command.trim();
    let exe = if let Some(rest) = command.strip_prefix('"') {
        &rest[..rest.find('"')?]
    } else {
        let end = command
            .to_lowercase()
            .find(".exe")
            .map(|i| i + 4)
            .unwrap_or(command.len());
        &command[..end]
    };
    let exe = exe.trim();
    if exe.is_empty() {
        return None;
    }
    // REG_EXPAND_SZ values come back unexpanded.
    Some(expand(exe)?.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_values() {
        assert_eq!(
            parse_command_exe(r#""C:\Program Files\Google\Chrome\Application\chrome.exe""#).as_deref(),
            Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe")
        );
        assert_eq!(
            parse_command_exe(r#""C:\Program Files\Mozilla Firefox\firefox.exe" -osint -url "%1""#).as_deref(),
            Some(r"C:\Program Files\Mozilla Firefox\firefox.exe")
        );
        assert_eq!(
            parse_command_exe(r"C:\Apps\Browser.EXE --new-window").as_deref(),
            Some(r"C:\Apps\Browser.EXE")
        );
        assert_eq!(parse_command_exe(r#""""#), None);
        assert_eq!(parse_command_exe(r#""unterminated"#), None);
    }

    #[test]
    fn expands_env_vars() {
        let local = std::env::var("LOCALAPPDATA").unwrap();
        assert_eq!(
            parse_command_exe(r#""%LOCALAPPDATA%\Programs\Opera\launcher.exe""#),
            Some(format!(r"{local}\Programs\Opera\launcher.exe"))
        );
    }

    /// Smoke test against this machine's registry: whatever is found must be well-formed.
    #[test]
    fn detect_runs() {
        for d in detect() {
            assert!(!d.name.is_empty());
            assert!(Path::new(&d.exec[0]).is_file(), "{}", d.exec[0]);
        }
    }
}
