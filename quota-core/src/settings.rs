// Settings persistence + autostart, 1:1 port of SettingsService (SPEC section 18).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
#[serde(default)] // tolerate a partial settings.json: missing fields take defaults
pub struct SettingsData {
    pub theme: String,        // system | light | dark
    pub refresh_minutes: i64, // 1 | 5 | 10 | 30
    pub auto_start: bool,
    /// Provider tab the panel shows: `kimi` | `deepseek` (PLAN §4 / §7).
    pub active_tab: String,
}

impl Default for SettingsData {
    fn default() -> Self {
        SettingsData {
            theme: "system".to_string(),
            refresh_minutes: 5,
            auto_start: false,
            active_tab: "kimi".to_string(),
        }
    }
}

/// Portable mode: a `portable.dat` next to the exe (SPEC 18.1).
fn portable_mode() -> bool {
    if crate::app_data_dir_override().is_some() {
        return false;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("portable.dat").exists()))
        .unwrap_or(false)
}

/// This app's own state: settings.json, logs, window state. Deliberately NOT the
/// Tauri build's directory (%APPDATA%\QuotaTray) — two apps, two sets of
/// preferences. `QUOTA_TRAY_APP_DATA_DIR` (SPEC 22.5) still wins for tests.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = crate::app_data_dir_override() {
        return dir;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = &exe_dir {
        if portable_mode() {
            return dir.clone();
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("QuotaTray-Slint");
        }
    }
    exe_dir.unwrap_or_else(|| PathBuf::from("."))
}

/// Account-side state shared with the Tauri build: credentials and the
/// network-derived caches. A FIXED absolute location (the other app's own
/// directory), because `config_dir()` is per app. Portable mode never shares.
pub fn shared_dir() -> PathBuf {
    if portable_mode() {
        return config_dir();
    }
    if let Some(dir) = crate::app_data_dir_override() {
        return dir.join("shared");
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("QuotaTray").join("shared");
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if !profile.is_empty() {
            return PathBuf::from(profile)
                .join("AppData")
                .join("Roaming")
                .join("QuotaTray")
                .join("shared");
        }
    }
    config_dir().join("shared")
}

fn file_path() -> PathBuf {
    config_dir().join("settings.json")
}

pub fn load() -> SettingsData {
    let Ok(text) = fs::read_to_string(file_path()) else {
        return SettingsData::default();
    };
    let mut data: SettingsData = serde_json::from_str(&text).unwrap_or_default();
    // Normalise once here so every reader — panel, commands — sees a valid tab
    // even after a hand edit (PLAN §7).
    data.active_tab = normalize_tab(&data.active_tab).to_string();
    data
}

pub fn save(data: &SettingsData) {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(data) {
        // Temp + rename: a crash mid-write must not leave a truncated file,
        // which `load` would silently turn into every setting's default.
        let tmp = dir.join("settings.json.tmp");
        if fs::write(&tmp, json).is_ok() && fs::rename(&tmp, file_path()).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }
}

/// Provider tab names as persisted in `ActiveTab` (PLAN §7). Anything that is
/// not `deepseek` reads as `kimi`, so a hand-edited or stale value can never
/// leave the panel with no pane selected.
pub fn normalize_tab(tab: &str) -> &'static str {
    if tab == "deepseek" {
        "deepseek"
    } else {
        "kimi"
    }
}

/// HKCU Run key autostart (per-user, no UAC). All errors silently swallowed.
pub fn apply_auto_start(data: &SettingsData) {
    // A diagnostic/test instance must not rewrite the real autostart entry.
    if crate::app_data_dir_override().is_some() {
        return;
    }
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey_with_flags(
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        winreg::enums::KEY_SET_VALUE,
    ) else {
        return;
    };
    if data.auto_start {
        if let Ok(exe) = std::env::current_exe() {
            let value = format!("\"{}\"", exe.to_string_lossy());
            // Its own value name: the Tauri build owns `QuotaTray`, and sharing
            // the name would let whichever saved last redirect the other's
            // autostart (PLAN §4).
            let _ = key.set_value("QuotaTray.Slint", &value);
        }
    } else {
        let _ = key.delete_value("QuotaTray.Slint");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The persisted shape and the fallback are a contract (PLAN §7): a
    // settings.json written before `ActiveTab` existed must still load, and a
    // saved tab must survive the round trip.
    #[test]
    fn settings_without_active_tab_falls_back_to_kimi() {
        let d: SettingsData =
            serde_json::from_str(r#"{"Theme":"dark","RefreshMinutes":5,"AutoStart":true}"#)
                .expect("legacy settings.json parses");
        assert_eq!(d.active_tab, "kimi");
        assert_eq!(d.theme, "dark");
        assert!(d.auto_start);
    }

    #[test]
    fn settings_keeps_the_saved_active_tab() {
        let d: SettingsData = serde_json::from_str(
            r#"{"Theme":"light","RefreshMinutes":30,"AutoStart":false,"ActiveTab":"deepseek"}"#,
        )
        .expect("settings with ActiveTab parses");
        assert_eq!(d.active_tab, "deepseek");
    }

    #[test]
    fn unknown_tab_names_normalise_to_kimi() {
        assert_eq!(normalize_tab("deepseek"), "deepseek");
        assert_eq!(normalize_tab("kimi"), "kimi");
        assert_eq!(normalize_tab(""), "kimi");
        assert_eq!(normalize_tab("DeepSeek"), "kimi");
    }
}
