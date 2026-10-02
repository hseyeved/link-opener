//! The global shortcut that shows the window with search open.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

use crate::error::{AppError, AppResult};

pub const DEFAULT: &str = "Ctrl+Alt+Space";

/// What is registered now, and why the configured shortcut isn't (if it failed).
#[derive(Default)]
pub struct ActiveShortcut(Mutex<Status>);

#[derive(Default)]
struct Status {
    registered: Option<Shortcut>,
    error: Option<String>,
}

impl ActiveShortcut {
    pub fn error(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).error.clone()
    }
}

/// Parses a shortcut like "Ctrl+Alt+Space". Needs a modifier other than Shift alone,
/// except for function keys, so it doesn't swallow ordinary typing.
pub fn parse(text: &str) -> AppResult<Shortcut> {
    let shortcut: Shortcut = text
        .trim()
        .parse()
        .map_err(|e| AppError::Invalid(format!("invalid shortcut \"{text}\": {e}")))?;
    let function_key = matches!(
        shortcut.key,
        Code::F1 | Code::F2 | Code::F3 | Code::F4 | Code::F5 | Code::F6 | Code::F7 | Code::F8
            | Code::F9 | Code::F10 | Code::F11 | Code::F12 | Code::F13 | Code::F14 | Code::F15
            | Code::F16 | Code::F17 | Code::F18 | Code::F19 | Code::F20 | Code::F21 | Code::F22
            | Code::F23 | Code::F24
    );
    let weak = (shortcut.mods - Modifiers::SHIFT).is_empty();
    if weak && !function_key {
        return Err(AppError::Invalid(
            "a global shortcut needs Ctrl, Alt or Cmd/Win (or an F key)".into(),
        ));
    }
    Ok(shortcut)
}

/// Registers `text` (`None` or empty = no shortcut) in place of the current one. If the new
/// one can't be registered (e.g. another app owns it), the previous one stays active.
pub fn apply<R: Runtime>(app: &AppHandle<R>, text: Option<&str>) -> AppResult<()> {
    let text = text.map(str::trim).filter(|t| !t.is_empty());
    let new = text.map(parse).transpose()?;
    let state = app.state::<ActiveShortcut>();
    let mut status = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if new == status.registered {
        status.error = None;
        return Ok(());
    }

    let manager = app.global_shortcut();
    let previous = status.registered.take();
    if let Some(old) = previous {
        let _ = manager.unregister(old);
    }
    if let Some(shortcut) = new {
        if let Err(e) = manager.register(shortcut) {
            if let Some(old) = previous {
                if manager.register(old).is_ok() {
                    status.registered = Some(old);
                }
            }
            let message = format!(
                "couldn't register {}: {e}. Another app may be using it.",
                text.unwrap_or_default()
            );
            status.error = Some(message.clone());
            return Err(AppError::Invalid(message));
        }
    }
    status.registered = new;
    status.error = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rules() {
        assert!(parse("Ctrl+Alt+Space").is_ok());
        assert!(parse(" CommandOrControl+Shift+K ").is_ok());
        assert!(parse("Super+Digit1").is_ok());
        assert!(parse("F9").is_ok());
        assert!(parse("Shift+F2").is_ok());
        assert!(matches!(parse("K"), Err(AppError::Invalid(_))));
        assert!(matches!(parse("Shift+K"), Err(AppError::Invalid(_))));
        assert!(matches!(parse("Ctrl+"), Err(AppError::Invalid(_))));
        assert!(matches!(parse("Ctrl+Banana"), Err(AppError::Invalid(_))));
        assert_eq!(parse("ctrl+alt+space").unwrap(), parse(DEFAULT).unwrap());
    }
}
