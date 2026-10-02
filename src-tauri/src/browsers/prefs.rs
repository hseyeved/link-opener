//! The user's browser list preferences: order, hidden browsers/profiles, renamed labels and
//! browsers added by hand. Stored as JSON in the `browser_prefs` setting.
//!
//! Keys: a browser is `"<browser id>"`, a profile is `"<browser id>/<profile id>"`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Browser, BrowserKind};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BrowserPrefs {
    /// Browser ids, first to last. Browsers not listed follow, by name.
    pub order: Vec<String>,
    /// Browser and profile keys left out of the picker.
    pub hidden: Vec<String>,
    /// Browser and profile key → display name.
    pub labels: HashMap<String, String>,
    pub custom: Vec<CustomBrowser>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomBrowser {
    /// Assigned on save when empty.
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Executable (or `.app` bundle on macOS).
    pub path: String,
    /// Decides the private-window and profile flags; `Other` only passes the URL.
    pub kind: BrowserKind,
}

impl CustomBrowser {
    fn to_browser(&self) -> Browser {
        let private_flag = (self.kind == BrowserKind::Chromium).then(|| "--incognito".to_string());
        let mut browser = Browser::new(
            self.id.clone(),
            self.name.clone(),
            self.kind,
            Vec::new(),
            vec![self.path.clone()],
            private_flag,
        );
        browser.custom = true;
        browser
    }
}

pub fn profile_key(browser_id: &str, profile_id: &str) -> String {
    format!("{browser_id}/{profile_id}")
}

/// Detected plus custom browsers, with labels, hidden flags and order applied.
pub fn apply(detected: Vec<Browser>, prefs: &BrowserPrefs) -> Vec<Browser> {
    let hidden: HashSet<&str> = prefs.hidden.iter().map(String::as_str).collect();
    let mut browsers: Vec<Browser> = detected
        .into_iter()
        .chain(prefs.custom.iter().map(CustomBrowser::to_browser))
        .collect();

    for b in &mut browsers {
        if let Some(label) = prefs.labels.get(&b.id).filter(|l| !l.trim().is_empty()) {
            b.name = label.trim().to_string();
        }
        b.hidden = hidden.contains(b.id.as_str());
        for p in &mut b.profiles {
            let key = profile_key(&b.id, &p.id);
            if let Some(label) = prefs.labels.get(&key).filter(|l| !l.trim().is_empty()) {
                p.name = label.trim().to_string();
            }
            p.hidden = hidden.contains(key.as_str());
        }
    }

    let rank: HashMap<&str, usize> = prefs
        .order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    browsers.sort_by(|a, b| {
        let ra = rank.get(a.id.as_str()).copied().unwrap_or(usize::MAX);
        let rb = rank.get(b.id.as_str()).copied().unwrap_or(usize::MAX);
        ra.cmp(&rb)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    browsers
}

/// Checks custom browsers and gives new ones an id that clashes with nothing in `taken`
/// (the detected browser ids). Paths are only checked for new entries, so a browser that
/// was moved later doesn't make every save fail.
pub fn validate(mut prefs: BrowserPrefs, taken: &[String]) -> AppResult<BrowserPrefs> {
    let mut used: HashSet<String> = taken.iter().cloned().collect();
    used.extend(
        prefs
            .custom
            .iter()
            .filter(|c| !c.id.is_empty())
            .map(|c| c.id.clone()),
    );
    for custom in &mut prefs.custom {
        custom.name = custom.name.trim().to_string();
        custom.path = custom.path.trim().trim_matches('"').to_string();
        if custom.name.is_empty() {
            return Err(AppError::Invalid("a browser needs a name".into()));
        }
        if custom.path.is_empty() {
            return Err(AppError::Invalid(format!("{} needs a path", custom.name)));
        }
        if custom.id.is_empty() {
            if !Path::new(&custom.path).exists() {
                return Err(AppError::Invalid(format!("{} doesn't exist", custom.path)));
            }
            let base = format!("custom-{}", super::slug(&custom.name));
            let id = (1..)
                .map(|n| {
                    if n == 1 {
                        base.clone()
                    } else {
                        format!("{base}-{n}")
                    }
                })
                .find(|id| !used.contains(id))
                .expect("an unused id exists");
            used.insert(id.clone());
            custom.id = id;
        }
    }
    prefs.labels.retain(|_, label| !label.trim().is_empty());
    Ok(prefs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browsers::Profile;

    fn detected(id: &str, name: &str, profiles: &[(&str, &str)]) -> Browser {
        Browser::new(
            id.into(),
            name.into(),
            BrowserKind::Chromium,
            profiles
                .iter()
                .map(|(pid, pname)| Profile::new(*pid, *pname, None))
                .collect(),
            vec![format!("{id}.exe")],
            Some("--incognito".into()),
        )
    }

    fn sample() -> Vec<Browser> {
        vec![
            detected("chrome", "Google Chrome", &[]),
            detected(
                "edge",
                "Microsoft Edge",
                &[("Default", "Personal"), ("Profile 1", "Work")],
            ),
            detected("brave", "Brave", &[]),
        ]
    }

    fn ids(browsers: &[Browser]) -> Vec<&str> {
        browsers.iter().map(|b| b.id.as_str()).collect()
    }

    #[test]
    fn defaults_sort_by_name() {
        let browsers = apply(sample(), &BrowserPrefs::default());
        assert_eq!(ids(&browsers), ["brave", "chrome", "edge"]);
        assert!(browsers.iter().all(|b| !b.hidden && !b.custom));
    }

    #[test]
    fn order_hidden_and_labels() {
        let prefs = BrowserPrefs {
            // Unknown ids are ignored; unlisted browsers follow by name.
            order: vec!["edge".into(), "gone".into(), "chrome".into()],
            hidden: vec!["chrome".into(), "edge/Profile 1".into()],
            labels: HashMap::from([
                ("brave".into(), "  Brave (work)  ".into()),
                ("edge/Default".into(), "Me".into()),
                ("chrome".into(), "   ".into()),
            ]),
            custom: vec![],
        };
        let browsers = apply(sample(), &prefs);
        assert_eq!(ids(&browsers), ["edge", "chrome", "brave"]);
        let (edge, chrome, brave) = (&browsers[0], &browsers[1], &browsers[2]);
        assert!(chrome.hidden && !edge.hidden);
        assert_eq!(chrome.name, "Google Chrome"); // blank label ignored
        assert_eq!(
            (brave.name.as_str(), brave.default_name.as_str()),
            ("Brave (work)", "Brave")
        );
        assert_eq!(
            (
                edge.profiles[0].name.as_str(),
                edge.profiles[0].default_name.as_str()
            ),
            ("Me", "Personal")
        );
        assert!(!edge.profiles[0].hidden && edge.profiles[1].hidden);
    }

    #[test]
    fn custom_browsers_merge_and_validate() {
        let exe = tempfile::NamedTempFile::new().unwrap();
        let path = exe.path().to_string_lossy().into_owned();
        let prefs = BrowserPrefs {
            custom: vec![
                CustomBrowser {
                    id: String::new(),
                    name: " Portable Fox ".into(),
                    path: format!("\"{path}\""),
                    kind: BrowserKind::Firefox,
                },
                CustomBrowser {
                    id: String::new(),
                    name: "Portable Fox".into(),
                    path: path.clone(),
                    kind: BrowserKind::Chromium,
                },
            ],
            labels: HashMap::from([("x".into(), "".into())]),
            ..Default::default()
        };
        let taken = vec!["chrome".to_string(), "custom-portable-fox".to_string()];
        let prefs = validate(prefs, &taken).unwrap();
        assert_eq!(prefs.custom[0].id, "custom-portable-fox-2");
        assert_eq!(prefs.custom[1].id, "custom-portable-fox-3");
        assert_eq!(
            (prefs.custom[0].name.as_str(), prefs.custom[0].path.as_str()),
            ("Portable Fox", path.as_str())
        );
        assert!(prefs.labels.is_empty());

        // Saving again keeps ids, even if the file has since gone.
        let again = validate(prefs.clone(), &taken).unwrap();
        assert_eq!(again, prefs);

        let browsers = apply(sample(), &prefs);
        let fox = browsers
            .iter()
            .find(|b| b.id == "custom-portable-fox-2")
            .unwrap();
        assert!(fox.custom && fox.supports_private && fox.profiles.is_empty());
        assert_eq!(fox.exec, std::slice::from_ref(&path));
        let chromium = browsers
            .iter()
            .find(|b| b.id == "custom-portable-fox-3")
            .unwrap();
        assert_eq!(chromium.private_flag.as_deref(), Some("--incognito"));
    }

    #[test]
    fn validate_rejects_bad_custom() {
        let custom = |name: &str, path: &str| BrowserPrefs {
            custom: vec![CustomBrowser {
                id: String::new(),
                name: name.into(),
                path: path.into(),
                kind: BrowserKind::Other,
            }],
            ..Default::default()
        };
        assert!(matches!(
            validate(custom("", "x"), &[]),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            validate(custom("X", " "), &[]),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(
            validate(custom("X", "/definitely/not/here.exe"), &[]),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn prefs_json() {
        let prefs: BrowserPrefs = serde_json::from_str(r#"{"hidden":["edge"]}"#).unwrap();
        assert_eq!(prefs.hidden, ["edge"]);
        assert!(prefs.order.is_empty() && prefs.custom.is_empty());
        let custom: CustomBrowser =
            serde_json::from_str(r#"{"name":"X","path":"p","kind":"firefox"}"#).unwrap();
        assert_eq!(
            (custom.id.as_str(), custom.kind),
            ("", BrowserKind::Firefox)
        );
    }
}
