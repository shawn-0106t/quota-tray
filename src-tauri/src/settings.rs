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

/// Portable mode: a `portable.dat` next to the exe pins the config dir to the
/// exe directory; otherwise %APPDATA%\QuotaTray (SPEC 18.1).
/// `QUOTA_TRAY_APP_DATA_DIR` (diagnostic/test override, SPEC 22.5) wins over both so
/// an isolated instance never touches the user's settings.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = crate::app_data_dir_override() {
        return dir;
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = &exe_dir {
        if dir.join("portable.dat").exists() {
            return dir.clone();
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("QuotaTray");
        }
    }
    exe_dir.unwrap_or_else(|| PathBuf::from("."))
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
        let _ = fs::write(file_path(), json);
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
            let _ = key.set_value("QuotaTray", &value);
        }
    } else {
        let _ = key.delete_value("QuotaTray");
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
