// Quota Tray data layer, shared by both shells (HANDOFF §4): the Kimi credential
// chain and quota fetch, the DeepSeek credentials/parse/client stack, the local
// Kimi usage scan, settings/skills/update, and the formatting a UI needs.
//
// Nothing here may reference a GUI toolkit. The Tauri shell re-exports these
// modules as `crate::…` so its own call sites are unchanged; the Slint shell
// consumes them directly. PLAN.md stays the behaviour contract.

pub mod credentials;
pub mod deepseek;
pub mod format;
pub mod kimi_usage;
pub mod quota;
pub mod settings;
pub mod skills;
pub mod update;

/// Diagnostic/test override: redirect all app directories (settings, caches,
/// credentials — and, in the Tauri shell, the WebView2 user-data folder and
/// `webview.log`) under one root, so a second instance or a sandboxed test
/// never touches the user profile (SPEC 22.5). Unset or empty means the normal
/// per-user locations; lives here because `settings::config_dir` reads it.
pub fn app_data_dir_override() -> Option<std::path::PathBuf> {
    let dir = std::env::var("QUOTA_TRAY_APP_DATA_DIR").ok()?;
    if dir.is_empty() {
        return None;
    }
    let path = std::path::PathBuf::from(&dir);
    // Tauri resolves a relative Root against the exe directory, while a bare
    // PathBuf resolves against the CWD — reject relatives so settings/log/
    // WebView2 data cannot split across two places.
    if path.is_absolute() {
        return Some(path);
    }
    static WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    if WARNED.set(()).is_ok() {
        eprintln!("QUOTA_TRAY_APP_DATA_DIR ignored (not an absolute path): {dir}");
    }
    None
}
