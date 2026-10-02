//! Builds and runs the command line that opens a URL in a browser/profile.

use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::{Browser, BrowserKind, Os};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTarget {
    pub browser_id: String,
    pub profile_id: Option<String>,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, PartialEq)]
pub struct LaunchCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn build(
    browser: &Browser,
    profile_id: Option<&str>,
    private: bool,
    url: &str,
    os: Os,
) -> AppResult<LaunchCommand> {
    // The URL is passed as a separate argument, never through a shell, but a leading
    // `-` would still be read as a browser flag.
    if url.is_empty() || url.starts_with('-') || url.starts_with(char::is_whitespace) {
        return Err(AppError::Invalid(format!("refusing to open URL {url:?}")));
    }
    if let Some(id) = profile_id {
        if !browser.profiles.iter().any(|p| p.id == id) {
            return Err(AppError::NotFound("browser profile"));
        }
    }
    if private && !browser.supports_private {
        return Err(AppError::Invalid(format!("{} has no private mode", browser.name)));
    }
    let Some((program, fixed_args)) = browser.exec.split_first() else {
        return Err(AppError::Invalid(format!("no command for {}", browser.name)));
    };

    let mut args: Vec<String> = Vec::new();
    match browser.kind {
        BrowserKind::Chromium => {
            if let Some(id) = profile_id {
                args.push(format!("--profile-directory={id}"));
            }
            if private {
                args.extend(browser.private_flag.clone());
            }
            args.push(url.into());
        }
        BrowserKind::Firefox => {
            if let Some(id) = profile_id {
                args.extend(["-P".into(), id.into()]);
            }
            args.push(if private { "--private-window" } else { "--new-tab" }.into());
            args.push(url.into());
        }
        BrowserKind::Safari | BrowserKind::Other => args.push(url.into()),
    }

    Ok(match os {
        // `program` is the .app bundle. Safari and unknown apps only take a URL.
        Os::Mac => match browser.kind {
            BrowserKind::Safari | BrowserKind::Other => LaunchCommand {
                program: "open".into(),
                args: vec!["-a".into(), program.clone(), url.into()],
            },
            _ => LaunchCommand {
                program: "open".into(),
                args: ["-na".into(), program.clone(), "--args".into()]
                    .into_iter()
                    .chain(args)
                    .collect(),
            },
        },
        Os::Windows | Os::Linux => LaunchCommand {
            program: program.clone(),
            args: fixed_args.iter().cloned().chain(args).collect(),
        },
    })
}

/// Starts the command detached from our stdio, without waiting for the browser to exit.
pub fn spawn(cmd: &LaunchCommand, browser_name: &str) -> AppResult<()> {
    let mut child = Command::new(&cmd.program)
        .args(&cmd.args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Launch(format!("couldn't start {browser_name}: {e}")))?;
    // Reap the process when it exits so it doesn't linger as a zombie on Unix.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browsers::Profile;

    fn browser(kind: BrowserKind, exec: &[&str], private_flag: Option<&str>) -> Browser {
        Browser::new(
            "b".into(),
            "B".into(),
            kind,
            vec![Profile::new("Default", "Me", None), Profile::new("Profile 1", "Work", None)],
            exec.iter().map(|s| s.to_string()).collect(),
            private_flag.map(str::to_string),
        )
    }

    fn args(cmd: &LaunchCommand) -> Vec<&str> {
        cmd.args.iter().map(String::as_str).collect()
    }

    const URL: &str = "https://example.com/a b?c=\"d\"";

    #[test]
    fn chromium() {
        let chrome = browser(BrowserKind::Chromium, &[r"C:\Chrome\chrome.exe"], Some("--incognito"));
        let cmd = build(&chrome, None, false, URL, Os::Windows).unwrap();
        assert_eq!(cmd.program, r"C:\Chrome\chrome.exe");
        assert_eq!(args(&cmd), [URL]);

        let cmd = build(&chrome, Some("Profile 1"), true, URL, Os::Windows).unwrap();
        assert_eq!(args(&cmd), ["--profile-directory=Profile 1", "--incognito", URL]);
    }

    #[test]
    fn edge_uses_inprivate() {
        let edge = browser(BrowserKind::Chromium, &["msedge.exe"], Some("--inprivate"));
        let cmd = build(&edge, Some("Default"), true, URL, Os::Windows).unwrap();
        assert_eq!(args(&cmd), ["--profile-directory=Default", "--inprivate", URL]);
    }

    #[test]
    fn firefox() {
        let ff = browser(BrowserKind::Firefox, &["/usr/bin/firefox"], None);
        let cmd = build(&ff, None, false, URL, Os::Linux).unwrap();
        assert_eq!(args(&cmd), ["--new-tab", URL]);
        let cmd = build(&ff, Some("Default"), true, URL, Os::Linux).unwrap();
        assert_eq!(args(&cmd), ["-P", "Default", "--private-window", URL]);
    }

    #[test]
    fn linux_fixed_args_come_first() {
        let flatpak = browser(
            BrowserKind::Firefox,
            &["/usr/bin/flatpak", "run", "--branch=stable", "org.mozilla.firefox"],
            None,
        );
        let cmd = build(&flatpak, None, true, URL, Os::Linux).unwrap();
        assert_eq!(cmd.program, "/usr/bin/flatpak");
        assert_eq!(args(&cmd), ["run", "--branch=stable", "org.mozilla.firefox", "--private-window", URL]);
    }

    #[test]
    fn macos_uses_open() {
        let chrome = browser(BrowserKind::Chromium, &["/Applications/Google Chrome.app"], Some("--incognito"));
        let cmd = build(&chrome, Some("Default"), true, URL, Os::Mac).unwrap();
        assert_eq!(cmd.program, "open");
        assert_eq!(
            args(&cmd),
            ["-na", "/Applications/Google Chrome.app", "--args", "--profile-directory=Default", "--incognito", URL]
        );

        let mut safari = browser(BrowserKind::Safari, &["/Applications/Safari.app"], None);
        safari.profiles.clear();
        let cmd = build(&safari, None, false, URL, Os::Mac).unwrap();
        assert_eq!(args(&cmd), ["-a", "/Applications/Safari.app", URL]);
    }

    #[test]
    fn other_gets_only_the_url() {
        let other = browser(BrowserKind::Other, &["viewer.exe"], None);
        assert_eq!(args(&build(&other, None, false, URL, Os::Windows).unwrap()), [URL]);
    }

    #[test]
    fn rejects_bad_input() {
        let chrome = browser(BrowserKind::Chromium, &["chrome"], Some("--incognito"));
        let safari = browser(BrowserKind::Safari, &["Safari.app"], None);
        let no_exec = browser(BrowserKind::Chromium, &[], Some("--incognito"));
        assert!(matches!(build(&chrome, None, false, "--remote-debugging-port=1", Os::Linux), Err(AppError::Invalid(_))));
        assert!(matches!(build(&chrome, None, false, "", Os::Linux), Err(AppError::Invalid(_))));
        assert!(matches!(build(&chrome, Some("Nope"), false, URL, Os::Linux), Err(AppError::NotFound(_))));
        assert!(matches!(build(&safari, None, true, URL, Os::Mac), Err(AppError::Invalid(_))));
        assert!(matches!(build(&no_exec, None, false, URL, Os::Linux), Err(AppError::Invalid(_))));
    }

    #[test]
    fn target_json() {
        let t: LaunchTarget = serde_json::from_str(r#"{"browserId":"edge","profileId":null}"#).unwrap();
        assert_eq!(t, LaunchTarget { browser_id: "edge".into(), profile_id: None, private: false });
    }
}
