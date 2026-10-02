//! Browsers we recognise, how to identify them on each OS, and where they keep profiles.

use super::{BrowserKind, Os};

pub struct Known {
    /// Stable id, used in saved launch targets.
    pub id: &'static str,
    pub name: &'static str,
    pub kind: BrowserKind,
    /// Chromium only: the flag that opens a private window.
    pub private_flag: Option<&'static str>,
    pub idents: PerOs,
    /// Directory holding `Local State` (Chromium) or `profiles.ini` (Firefox).
    /// Prefixes: `%VAR%\` (Windows env var), `~/` (home), `$XDG_CONFIG/`.
    pub data_dirs: PerOs,
}

/// Lowercase values per OS.
/// Windows idents are executable path suffixes, Linux idents are desktop file ids
/// (without `.desktop`), macOS idents are bundle ids.
pub struct PerOs {
    pub windows: &'static [&'static str],
    pub linux: &'static [&'static str],
    pub macos: &'static [&'static str],
}

impl PerOs {
    pub fn get(&self, os: Os) -> &'static [&'static str] {
        match os {
            Os::Windows => self.windows,
            Os::Linux => self.linux,
            Os::Mac => self.macos,
        }
    }
}

/// Finds the known browser for an executable path (Windows), desktop id (Linux) or bundle id (macOS).
pub fn find(os: Os, ident: &str) -> Option<&'static Known> {
    let ident = ident.to_lowercase().replace('/', "\\");
    KNOWN.iter().find(|k| {
        k.idents.get(os).iter().any(|pat| match os {
            // Match whole path components: `...\msedge.exe`.
            Os::Windows => ident == *pat || ident.ends_with(&format!("\\{pat}")),
            Os::Linux | Os::Mac => ident == *pat,
        })
    })
}

const NONE: &[&str] = &[];

pub static KNOWN: &[Known] = &[
    Known {
        id: "chrome",
        name: "Google Chrome",
        kind: BrowserKind::Chromium,
        private_flag: Some("--incognito"),
        idents: PerOs {
            windows: &["google\\chrome\\application\\chrome.exe"],
            linux: &["google-chrome", "google-chrome-stable", "com.google.chrome"],
            macos: &["com.google.chrome"],
        },
        data_dirs: PerOs {
            windows: &["%LOCALAPPDATA%\\Google\\Chrome\\User Data"],
            linux: &[
                "$XDG_CONFIG/google-chrome",
                "~/.var/app/com.google.Chrome/config/google-chrome",
            ],
            macos: &["~/Library/Application Support/Google/Chrome"],
        },
    },
    Known {
        id: "chromium",
        name: "Chromium",
        kind: BrowserKind::Chromium,
        private_flag: Some("--incognito"),
        idents: PerOs {
            windows: &["chromium\\application\\chrome.exe"],
            linux: &[
                "chromium",
                "chromium-browser",
                "chromium_chromium",
                "org.chromium.chromium",
            ],
            macos: &["org.chromium.chromium"],
        },
        data_dirs: PerOs {
            windows: &["%LOCALAPPDATA%\\Chromium\\User Data"],
            linux: &[
                "$XDG_CONFIG/chromium",
                "~/.var/app/org.chromium.Chromium/config/chromium",
                "~/snap/chromium/common/chromium",
            ],
            macos: &["~/Library/Application Support/Chromium"],
        },
    },
    Known {
        id: "edge",
        name: "Microsoft Edge",
        kind: BrowserKind::Chromium,
        private_flag: Some("--inprivate"),
        idents: PerOs {
            windows: &["msedge.exe"],
            linux: &[
                "microsoft-edge",
                "microsoft-edge-stable",
                "com.microsoft.edge",
            ],
            macos: &["com.microsoft.edgemac"],
        },
        data_dirs: PerOs {
            windows: &["%LOCALAPPDATA%\\Microsoft\\Edge\\User Data"],
            linux: &[
                "$XDG_CONFIG/microsoft-edge",
                "~/.var/app/com.microsoft.Edge/config/microsoft-edge",
            ],
            macos: &["~/Library/Application Support/Microsoft Edge"],
        },
    },
    Known {
        id: "brave",
        name: "Brave",
        kind: BrowserKind::Chromium,
        private_flag: Some("--incognito"),
        idents: PerOs {
            windows: &["brave.exe"],
            linux: &["brave-browser", "brave_brave", "com.brave.browser"],
            macos: &["com.brave.browser"],
        },
        data_dirs: PerOs {
            windows: &["%LOCALAPPDATA%\\BraveSoftware\\Brave-Browser\\User Data"],
            linux: &[
                "$XDG_CONFIG/BraveSoftware/Brave-Browser",
                "~/.var/app/com.brave.Browser/config/BraveSoftware/Brave-Browser",
                "~/snap/brave/current/.config/BraveSoftware/Brave-Browser",
            ],
            macos: &["~/Library/Application Support/BraveSoftware/Brave-Browser"],
        },
    },
    Known {
        id: "vivaldi",
        name: "Vivaldi",
        kind: BrowserKind::Chromium,
        private_flag: Some("--incognito"),
        idents: PerOs {
            windows: &["vivaldi.exe"],
            linux: &["vivaldi-stable", "vivaldi", "com.vivaldi.vivaldi"],
            macos: &["com.vivaldi.vivaldi"],
        },
        data_dirs: PerOs {
            windows: &["%LOCALAPPDATA%\\Vivaldi\\User Data"],
            linux: &[
                "$XDG_CONFIG/vivaldi",
                "~/.var/app/com.vivaldi.Vivaldi/config/vivaldi",
            ],
            macos: &["~/Library/Application Support/Vivaldi"],
        },
    },
    Known {
        // Opera's profiles don't use Chromium's `info_cache`, so none are listed.
        id: "opera",
        name: "Opera",
        kind: BrowserKind::Chromium,
        private_flag: Some("--private"),
        idents: PerOs {
            windows: &["opera\\launcher.exe", "opera\\opera.exe"],
            linux: &["opera", "opera_opera", "com.opera.opera"],
            macos: &["com.operasoftware.opera"],
        },
        data_dirs: PerOs {
            windows: NONE,
            linux: NONE,
            macos: NONE,
        },
    },
    Known {
        id: "opera-gx",
        name: "Opera GX",
        kind: BrowserKind::Chromium,
        private_flag: Some("--private"),
        idents: PerOs {
            windows: &["opera gx\\launcher.exe", "opera gx\\opera.exe"],
            linux: NONE,
            macos: &["com.operasoftware.operagx"],
        },
        data_dirs: PerOs {
            windows: NONE,
            linux: NONE,
            macos: NONE,
        },
    },
    Known {
        id: "blisk",
        name: "Blisk",
        kind: BrowserKind::Chromium,
        private_flag: Some("--incognito"),
        idents: PerOs {
            windows: &["blisk.exe"],
            linux: NONE,
            macos: &["org.blisk.blisk"],
        },
        data_dirs: PerOs {
            windows: &["%LOCALAPPDATA%\\Blisk\\User Data"],
            linux: NONE,
            macos: &["~/Library/Application Support/Blisk"],
        },
    },
    Known {
        id: "firefox",
        name: "Firefox",
        kind: BrowserKind::Firefox,
        private_flag: None,
        idents: PerOs {
            windows: &["firefox.exe"],
            linux: &[
                "firefox",
                "firefox-esr",
                "firefox_firefox",
                "org.mozilla.firefox",
            ],
            macos: &["org.mozilla.firefox"],
        },
        data_dirs: PerOs {
            windows: &["%APPDATA%\\Mozilla\\Firefox"],
            linux: &[
                "~/.mozilla/firefox",
                "~/.var/app/org.mozilla.firefox/.mozilla/firefox",
                "~/snap/firefox/common/.mozilla/firefox",
            ],
            macos: &["~/Library/Application Support/Firefox"],
        },
    },
    Known {
        id: "librewolf",
        name: "LibreWolf",
        kind: BrowserKind::Firefox,
        private_flag: None,
        idents: PerOs {
            windows: &["librewolf.exe"],
            linux: &["librewolf", "io.gitlab.librewolf-community"],
            macos: &[
                "io.gitlab.librewolf-community.librewolf",
                "org.mozilla.librewolf",
            ],
        },
        data_dirs: PerOs {
            windows: &["%APPDATA%\\librewolf"],
            linux: &[
                "~/.librewolf",
                "~/.var/app/io.gitlab.librewolf-community/.librewolf",
            ],
            macos: &["~/Library/Application Support/librewolf"],
        },
    },
    Known {
        id: "zen",
        name: "Zen",
        kind: BrowserKind::Firefox,
        private_flag: None,
        idents: PerOs {
            windows: &["zen.exe"],
            linux: &["zen", "zen-browser", "app.zen_browser.zen"],
            macos: &["app.zen-browser.zen"],
        },
        data_dirs: PerOs {
            windows: &["%APPDATA%\\zen"],
            linux: &["~/.zen", "~/.var/app/app.zen_browser.zen/.zen"],
            macos: &["~/Library/Application Support/zen"],
        },
    },
    Known {
        id: "waterfox",
        name: "Waterfox",
        kind: BrowserKind::Firefox,
        private_flag: None,
        idents: PerOs {
            windows: &["waterfox.exe"],
            linux: &["waterfox", "net.waterfox.waterfox"],
            macos: &["net.waterfox.waterfox"],
        },
        data_dirs: PerOs {
            windows: &["%APPDATA%\\Waterfox"],
            linux: &["~/.waterfox", "~/.var/app/net.waterfox.waterfox/.waterfox"],
            macos: &["~/Library/Application Support/Waterfox"],
        },
    },
    Known {
        id: "safari",
        name: "Safari",
        kind: BrowserKind::Safari,
        private_flag: None,
        idents: PerOs {
            windows: NONE,
            linux: NONE,
            macos: &["com.apple.safari"],
        },
        data_dirs: PerOs {
            windows: NONE,
            linux: NONE,
            macos: NONE,
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn id(os: Os, ident: &str) -> Option<&'static str> {
        find(os, ident).map(|k| k.id)
    }

    #[test]
    fn windows_paths() {
        let w = Os::Windows;
        assert_eq!(
            id(w, r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
            Some("chrome")
        );
        assert_eq!(
            id(
                w,
                r"C:\Users\me\AppData\Local\Chromium\Application\chrome.exe"
            ),
            Some("chromium")
        );
        assert_eq!(
            id(
                w,
                r"C:\Program Files (x86)\Microsoft\Edge\Application\MSEDGE.EXE"
            ),
            Some("edge")
        );
        assert_eq!(
            id(w, r"C:\Program Files\Mozilla Firefox\firefox.exe"),
            Some("firefox")
        );
        assert_eq!(
            id(
                w,
                r"C:\Users\me\AppData\Local\Programs\Opera GX\launcher.exe"
            ),
            Some("opera-gx")
        );
        assert_eq!(
            id(w, r"C:\Users\me\AppData\Local\Programs\Opera\launcher.exe"),
            Some("opera")
        );
        // Suffixes match whole components only.
        assert_eq!(id(w, r"C:\tools\notfirefox.exe"), None);
        assert_eq!(
            id(
                w,
                r"C:\Program Files\Google\Chrome Beta\Application\chrome.exe"
            ),
            None
        );
    }

    #[test]
    fn linux_and_macos_ids() {
        assert_eq!(id(Os::Linux, "org.mozilla.firefox"), Some("firefox"));
        assert_eq!(id(Os::Linux, "com.google.Chrome"), Some("chrome"));
        assert_eq!(id(Os::Linux, "firefox_firefox"), Some("firefox"));
        assert_eq!(id(Os::Mac, "com.apple.Safari"), Some("safari"));
        assert_eq!(id(Os::Mac, "com.microsoft.edgemac"), Some("edge"));
        assert_eq!(id(Os::Mac, "com.example.other"), None);
    }

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = KNOWN.iter().map(|k| k.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), KNOWN.len());
    }
}
