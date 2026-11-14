// Quota Tray — Tauri 2 shell hosting the Kimi quota and DeepSeek balance/usage
// panes behind a segmented provider tab. Imported from kimi-planbar-tray (rust
// edition); startup order, window model and WebView2 recovery are unchanged
// (docs/SPEC.md section 20). Self-check command line args are handled in
// main.rs BEFORE the single-instance check.

pub mod credentials;
pub mod deepseek;
pub mod kimi_usage;
pub mod login;
pub mod panel;
pub mod polling;
pub mod quota;
pub mod recovery;
pub mod settings;
pub mod skills;
pub mod state;
pub mod theme_watch;
pub mod tray;
pub mod update;

use deepseek::DeepseekResult;
use quota::QuotaResult;
use serde::{Deserialize, Serialize};
use settings::SettingsData;
use state::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use update::UpdateStatus;

// on_window_event delivers tauri::Window; commands receive WebviewWindow.

const RELEASES_URL: &str = "https://github.com/MoonshotAI/kimi-code/releases";
const CONSOLE_URL: &str = "https://www.kimi.com/code/console?from=kfc_overview_topbar";

/// Opt every window out of Windows 11 DWM auto corner rounding. The CSS draws
/// its own 14px/12px radius on a transparent window; DWM would otherwise clip
/// the window corners at ~8px and fight the painted radius.
pub(crate) fn apply_no_round(window: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    if let Ok(hwnd) = window.hwnd() {
        let pref = DWMWCP_DONOTROUND;
        unsafe {
            let _ = DwmSetWindowAttribute(
                HWND(hwnd.0 as _),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
}

fn disable_dwm_corner_rounding(app: &tauri::App) {
    for window in app.webview_windows().values() {
        apply_no_round(window);
    }
}

/// Native quit-only tray menu installed after auto-recovery is disabled: the
/// webview menu may be a dead shell, and the app must keep a way to exit
/// (SPEC 22.4). `show_menu` stands down once this is active.
pub(crate) fn install_fallback_tray_menu(app: &AppHandle) {
    use tauri::menu::{Menu, MenuItem};
    // Idempotent claim: the coordinator (cap branch) and the show gate can
    // race here within a few milliseconds.
    if app
        .state::<AppState>()
        .recovery
        .fallback_menu
        .swap(true, Ordering::SeqCst)
    {
        return;
    }
    let installed = (|| -> bool {
        let Ok(quit) = MenuItem::with_id(
            app,
            "quotatray-fallback-quit",
            "Quit Quota Tray",
            true,
            None::<&str>,
        ) else {
            return false;
        };
        let Ok(menu) = Menu::with_items(app, &[&quit]) else {
            return false;
        };
        let Some(tray) = app.tray_by_id("tray") else {
            return false;
        };
        if tray.set_menu(Some(menu)).is_err() {
            return false;
        }
        // A native menu must not hijack left-click (which toggles the panel).
        if tray.set_show_menu_on_left_click(false).is_err() {
            recovery::log("fallback menu: failed to disable show-on-left-click");
        }
        true
    })();
    if !installed {
        app.state::<AppState>()
            .recovery
            .fallback_menu
            .store(false, Ordering::SeqCst);
        return;
    }
    recovery::log("fallback tray menu installed (quit only)");
}

// ---- IPC DTOs ----

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub theme: String,
    pub refresh_minutes: i64,
    pub auto_start: bool,
    pub active_tab: String,
}

impl From<&SettingsData> for SettingsDto {
    fn from(d: &SettingsData) -> Self {
        SettingsDto {
            theme: d.theme.clone(),
            refresh_minutes: d.refresh_minutes,
            auto_start: d.auto_start,
            active_tab: d.active_tab.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    pub quota: Option<QuotaResult>,
    pub update: UpdateStatus,
    pub settings: SettingsDto,
    pub theme: String, // effective theme: light | dark
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsArgs {
    pub theme: String,
    pub refresh_minutes: i64,
    pub auto_start: bool,
}

/// Read-only Kimi credential state for the settings window (PLAN §4). Never
/// carries the token; `source` is `oauth` / `api-key` / `none`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiCredentialDto {
    pub source: String,
    /// Unix seconds, OAuth only
    pub expires_at: Option<i64>,
    /// An OAuth entry exists but is unusable and nothing took over
    pub expired: bool,
}

/// DeepSeek credential state: presence and origin, never a value (PLAN §5.2).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepseekCredentialDto {
    pub api_key: bool,
    pub user_token: bool,
    /// env | file | imported | undecryptable | none
    pub source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusDto {
    pub kimi: KimiCredentialDto,
    pub deepseek: DeepseekCredentialDto,
    /// `kimi --version` from the update check (PLAN §5.1)
    pub cli_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
}

// ---- Commands ----

#[tauri::command]
fn get_state(app: AppHandle) -> AppStateDto {
    let st = app.state::<AppState>();
    let quota = st.last_quota.read().unwrap().clone();
    let update = st.update.read().unwrap().clone();
    let settings = SettingsDto::from(&*st.settings.read().unwrap());
    let theme = st.effective_theme.read().unwrap().clone();
    AppStateDto {
        quota,
        update,
        settings,
        theme,
    }
}

/// Panel "Refresh" button / tray-menu "Refresh": SafeRefresh + CheckAsync (SPEC 12.7 / 14).
/// Debounced: invocations within 2s of the previous one are ignored.
#[tauri::command]
fn refresh_now(app: AppHandle) {
    {
        let st = app.state::<AppState>();
        let mut last = st.last_manual_refresh.lock().unwrap();
        let now = std::time::Instant::now();
        if last.is_some_and(|t| now.duration_since(t) < std::time::Duration::from_secs(2)) {
            return;
        }
        *last = Some(now);
    }
    let a1 = app.clone();
    tauri::async_runtime::spawn(async move {
        polling::safe_refresh(&a1).await;
    });
    // One Refresh command refreshes both providers (PLAN §5.3); they fail
    // independently, so a Kimi error never blocks the DeepSeek fetch.
    let a2 = app.clone();
    tauri::async_runtime::spawn(async move {
        polling::safe_refresh_deepseek(&a2).await;
    });
    tauri::async_runtime::spawn(async move {
        check_and_emit(&app).await;
    });
}

/// Kimi prompt-cache / decode-speed statistics for the Kimi pane (PLAN §5.1).
/// Kept out of `get_state` for the same reason as `get_deepseek`: only the panel
/// renders it, and the settings / skills windows call `get_state` too.
#[tauri::command]
fn get_kimi_usage(app: AppHandle) -> Option<kimi_usage::KimiUsage> {
    app.state::<AppState>().kimi_usage.read().unwrap().clone()
}

/// DeepSeek half of the state: the initial payload for the DeepSeek pane.
/// Kept out of `get_state` on purpose — the settings and skills windows also
/// call `get_state` and have no use for the per-model usage list.
#[tauri::command]
fn get_deepseek(app: AppHandle) -> Option<DeepseekResult> {
    app.state::<AppState>().last_deepseek.read().unwrap().clone()
}

/// Provider tab selected in the panel (PLAN §4). Persisted so the next launch
/// opens on the same tab; the pollers keep running for both providers either
/// way, so switching only changes what is rendered.
#[tauri::command]
fn set_active_tab(app: AppHandle, tab: String) {
    let tab = settings::normalize_tab(&tab);
    let st = app.state::<AppState>();
    let mut cur = st.settings.write().unwrap();
    if cur.active_tab == tab {
        return;
    }
    cur.active_tab = tab.to_string();
    settings::save(&cur);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    show_settings_window(&app);
}

/// Read-only credential status for the settings sections (PLAN §4). Statuses
/// only — a value never leaves the backend.
#[tauri::command]
fn credential_status(app: AppHandle) -> CredentialStatusDto {
    let kimi = credentials::describe();
    let ds = deepseek::credentials::status();
    let update = app.state::<AppState>().update.read().unwrap().clone();
    CredentialStatusDto {
        kimi: KimiCredentialDto {
            source: kimi.source.to_string(),
            expires_at: kimi.expires_at,
            expired: kimi.expired,
        },
        deepseek: DeepseekCredentialDto {
            api_key: ds.api_key,
            user_token: ds.user_token,
            source: ds.source.as_str().to_string(),
        },
        cli_version: update.local_version,
        latest_version: update.latest_version,
        update_available: update.update_available,
    }
}

/// Manual fallback for a user who cannot use the sign-in window (PLAN §4).
/// Verify first, save only on success: a broken key must not replace a working
/// one. Errors are PLAN §5.2 codes, which the frontend turns into text.
#[tauri::command]
async fn save_credential(app: AppHandle, field: String, value: String) -> Result<(), String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err("empty".to_string());
    }
    match field.as_str() {
        "apiKey" => {
            deepseek::client::fetch_balance(&value).await?;
            deepseek::credentials::save(Some(&value), None, None);
        }
        "userToken" => {
            deepseek::client::verify_user_token(&value).await?;
            deepseek::credentials::save(None, Some(&value), None);
        }
        _ => return Err("unknown-field".to_string()),
    }
    let _ = app.emit("deepseek-credentials-changed", ());
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        polling::safe_refresh_deepseek(&handle).await;
    });
    Ok(())
}

/// Remove both stored DeepSeek credentials. The file stays (empty) so the
/// one-time legacy import cannot bring them back (PLAN §5.2).
#[tauri::command]
fn clear_credentials(app: AppHandle) {
    deepseek::credentials::clear();
    let _ = app.emit("deepseek-credentials-changed", ());
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        polling::safe_refresh_deepseek(&handle).await;
    });
}

/// DeepSeek sign-in window (PLAN §4): a temporary window, never reused after
/// it closes.
#[tauri::command]
fn open_login(app: AppHandle) {
    login::open(&app);
}

#[tauri::command]
fn close_login(app: AppHandle) {
    login::close(&app);
}

/// Shared by the open_settings command and the recovery rebuild restore path.
pub(crate) fn show_settings_window(app: &AppHandle) {
    if !recovery::gate_show(app, "settings") {
        return;
    }
    app.state::<AppState>()
        .settings_open
        .store(true, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("settings") {
        // Content-sized sheet (PLAN §4): pin the size from the last report
        // before showing, so the window never flashes at a stale height.
        panel::apply_settings_size(app);
        let _ = w.show();
        let _ = w.set_focus();
    }
    // The settings window is reused (hidden, never destroyed): tell the
    // frontend to backfill the current settings every time it opens (SPEC 13.2)
    recovery::note_ui_event(app, "settings", "show");
    recovery::set_last_visible(app, "settings", true);
    let _ = app.emit("settings-show", ());
}

#[tauri::command]
fn close_settings(app: AppHandle) {
    app.state::<AppState>()
        .settings_open
        .store(false, Ordering::SeqCst);
    recovery::set_last_visible(&app, "settings", false);
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.hide();
    }
    recovery::note_ui_event(&app, "settings", "hide");
    // The suppressed blur never arrives: closing settings has to close the
    // panel's focus loop itself, or the panel sits there until the next blur.
    panel::collapse_after_settings_close(&app);
}

/// Skills window: same reuse pattern as settings (SPEC 21). The scan itself
/// is lazy — it only runs when the frontend calls get_skills.
#[tauri::command]
fn open_skills(app: AppHandle) {
    show_skills_window(&app);
}

/// Shared by the open_skills command and the recovery rebuild restore path.
pub(crate) fn show_skills_window(app: &AppHandle) {
    if !recovery::gate_show(app, "skills") {
        return;
    }
    app.state::<AppState>()
        .skills_open
        .store(true, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("skills") {
        // Same DPI-inflation guard as show_panel: pin the size on every open.
        let _ = w.set_size(tauri::LogicalSize::new(404.0, 520.0));
        let _ = w.show();
        let _ = w.set_focus();
    }
    recovery::note_ui_event(app, "skills", "show");
    recovery::set_last_visible(app, "skills", true);
    let _ = app.emit("skills-show", ());
}

#[tauri::command]
fn close_skills(app: AppHandle) {
    app.state::<AppState>()
        .skills_open
        .store(false, Ordering::SeqCst);
    recovery::set_last_visible(&app, "skills", false);
    if let Some(w) = app.get_webview_window("skills") {
        let _ = w.hide();
    }
    recovery::note_ui_event(&app, "skills", "hide");
}

/// Cached skill list; `refresh` forces a rescan. Read-only (SPEC 21.2).
/// Async so the file scan never blocks the main thread / webviews.
#[tauri::command]
async fn get_skills(app: AppHandle, refresh: bool) -> Result<Vec<skills::SkillInfo>, ()> {
    let st = app.state::<AppState>();
    if !refresh {
        if let Some(cached) = st.skills_cache.read().unwrap().clone() {
            return Ok(cached);
        }
    }
    let list = tauri::async_runtime::spawn_blocking(skills::scan)
        .await
        .unwrap_or_default();
    *st.skills_cache.write().unwrap() = Some(list.clone());
    Ok(list)
}

/// Save -> persist -> ApplyAutoStart -> apply theme -> Reschedule -> close (SPEC 13.2).
#[tauri::command]
fn save_settings(app: AppHandle, settings: SaveSettingsArgs) {
    {
        let st = app.state::<AppState>();
        let mut cur = st.settings.write().unwrap();
        cur.theme = settings.theme;
        // Clamp at the IPC boundary: polling multiplies this by 60 seconds
        cur.refresh_minutes = settings.refresh_minutes.clamp(1, 30);
        cur.auto_start = settings.auto_start;
        settings::save(&cur);
        settings::apply_auto_start(&cur);
    }
    theme_watch::apply_now(&app);
    // Both loops restart on the new interval; their own last-good slots and
    // in-flight fetches are untouched (PLAN §5.3).
    {
        let st = app.state::<AppState>();
        st.schedule(state::Provider::Kimi).restart();
        st.schedule(state::Provider::Deepseek).restart();
    }
    close_settings(app);
}

#[tauri::command]
fn open_releases(app: AppHandle) {
    use tauri_plugin_opener::OpenerExt;
    let _ = app.opener().open_url(RELEASES_URL, None::<&str>);
}

#[tauri::command]
fn open_console(app: AppHandle) {
    use tauri_plugin_opener::OpenerExt;
    let _ = app.opener().open_url(CONSOLE_URL, None::<&str>);
}

#[tauri::command]
fn finish_hide_panel(app: AppHandle) {
    panel::finish_hide(&app);
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Tray menu items (SPEC 14): Open/Refresh/Settings/Skills/Exit; menu window closes first.
#[tauri::command]
fn menu_action(app: AppHandle, action: String) {
    recovery::set_last_visible(&app, "menu", false);
    if let Some(w) = app.get_webview_window("menu") {
        let _ = w.hide();
    }
    recovery::note_ui_event(&app, "menu", "hide");
    match action.as_str() {
        "open" => panel::toggle_panel(&app, false),
        "refresh" => refresh_now(app),
        "settings" => open_settings(app),
        "skills" => open_skills(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

/// Frontend-reported content height of the menu (WPF SizeToContent equivalent).
#[tauri::command]
fn menu_height(app: AppHandle, height: f64) {
    if (20.0..600.0).contains(&height) {
        *app.state::<AppState>().menu_height.lock().unwrap() = height;
    }
}

/// Frontend-reported content height of the main panel (PLAN §4). Applied at
/// once so a height change while the panel is visible keeps the card's
/// bottom-right corner pinned instead of jumping on the next show.
#[tauri::command]
fn panel_height(app: AppHandle, height: f64) {
    if (80.0..2000.0).contains(&height) {
        *app.state::<AppState>().panel_height.lock().unwrap() = height;
        panel::apply_panel_size(&app);
    }
}

/// Frontend-reported content height of the settings window (PLAN §4).
#[tauri::command]
fn settings_height(app: AppHandle, height: f64) {
    if (80.0..2000.0).contains(&height) {
        *app.state::<AppState>().settings_height.lock().unwrap() = height;
        panel::apply_settings_size(&app);
    }
}

#[tauri::command]
fn start_drag(window: WebviewWindow) {
    let _ = window.start_dragging();
}

// ---- Update check with event emission ----

pub async fn check_and_emit(app: &AppHandle) {
    let st = update::check().await;
    *app.state::<AppState>().update.write().unwrap() = st.clone();
    let _ = app.emit("update-status", &st);
}

// ---- App wiring ----

/// Diagnostic/test override: redirect all app directories (including the
/// WebView2 user-data folder and `webview.log`) under one root. Used to run a
/// second instance or a sandboxed test without touching the user profile
/// (SPEC 22.5). Empty/unset means the normal per-user locations.
pub(crate) fn app_data_dir_override() -> Option<std::path::PathBuf> {
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

fn apply_app_data_dir_override(context: &mut tauri::Context<tauri::Wry>) {
    if let Some(dir) = app_data_dir_override() {
        context.config_mut().app.app_directories_override =
            Some(tauri::utils::config::AppDirectoriesOverride::Root(dir));
    }
}

fn build_state() -> AppState {
    let data = settings::load();
    let theme = theme_watch::effective(&data.theme);
    let state = AppState::new(data, theme);
    // Seed from the last successful payload so the pane has numbers before the
    // first poll lands (mirrors DeepseekService.StartAutoRefresh -> LoadCache).
    *state.last_deepseek.write().unwrap() = deepseek::load_cache();
    state
}

fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    match event {
        // Focus-loss: panel auto-hides (suppressed while settings is open),
        // menu closes immediately (SPEC 10.1 / 10.3)
        tauri::WindowEvent::Focused(false) => match window.label() {
            "main" => panel::on_main_blur(window.app_handle()),
            "menu" => {
                recovery::set_last_visible(window.app_handle(), "menu", false);
                let _ = window.hide();
                recovery::note_ui_event(window.app_handle(), "menu", "hide");
            }
            _ => {}
        },
        // Windows are single-instance reused: never destroy, only hide.
        // destroy() (recovery rebuild) bypasses CloseRequested on purpose.
        tauri::WindowEvent::CloseRequested { api, .. } => {
            // The DeepSeek sign-in window is the one temporary window: closing
            // it destroys it, and the next sign-in builds a fresh one (PLAN §4).
            if window.label() == login::LABEL {
                window.app_handle().state::<AppState>().login.reset();
                recovery::log("login: sign-in window closed");
                return;
            }
            api.prevent_close();
            let st = window.app_handle().state::<AppState>();
            match window.label() {
                "settings" => st.settings_open.store(false, Ordering::SeqCst),
                "skills" => st.skills_open.store(false, Ordering::SeqCst),
                _ => {}
            }
            recovery::set_last_visible(window.app_handle(), window.label(), false);
            let _ = window.hide();
            recovery::note_ui_event(window.app_handle(), window.label(), "hide");
        }
        tauri::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            recovery::note_ui_event(
                window.app_handle(),
                window.label(),
                &format!("scale={scale_factor:.2}"),
            );
        }
        _ => {}
    }
}

fn invoke_handlers() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        get_state,
        get_deepseek,
        get_kimi_usage,
        set_active_tab,
        refresh_now,
        open_settings,
        close_settings,
        credential_status,
        save_credential,
        clear_credentials,
        open_login,
        close_login,
        open_skills,
        close_skills,
        get_skills,
        save_settings,
        open_releases,
        open_console,
        finish_hide_panel,
        quit_app,
        menu_action,
        menu_height,
        panel_height,
        settings_height,
        start_drag,
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut context = tauri::generate_context!();
    apply_app_data_dir_override(&mut context);
    tauri::Builder::default()
        // Single instance first: a second launch exits immediately (SPEC 20)
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_opener::init())
        .manage(build_state())
        .invoke_handler(invoke_handlers())
        .on_menu_event(|app, event| {
            let id: &str = event.id().as_ref();
            if id == "quotatray-fallback-quit" {
                app.exit(0);
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            disable_dwm_corner_rounding(app);
            // WebView2 health: subscribe before any background task so a
            // failure during startup is still queued for the coordinator.
            recovery::install_all(&handle);
            recovery::start(&handle);
            tray::build(app.handle())?;
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move { polling::run(h).await });
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move { polling::run_deepseek(h).await });
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move { kimi_usage::run(h).await });
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move { check_and_emit(&h).await });
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move { theme_watch::watch(h).await });
            Ok(())
        })
        .on_window_event(on_window_event)
        .build(context)
        .expect("error while building tauri application")
        // Tauri exits when the last window is destroyed. A BrowserProcessExited
        // recovery destroys all four windows at once, so that implicit exit
        // must be prevented until the rebuild completes; explicit quits
        // (app.exit(code), code = Some) still exit normally.
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

/// Remove stale `--test-ui` WebView2 data directories from earlier runs.
/// A clean exit also tries to delete its own directory, but WebView2 can keep
/// files locked for ~10 s after the process exits; leftovers are swept here
/// once they are no longer plausibly owned by a running test instance.
fn cleanup_stale_test_dirs() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("quota-tray-testui-") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(created) = meta.created() else {
            continue;
        };
        let stale = now
            .duration_since(created)
            .map(|age| age > std::time::Duration::from_secs(600))
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn remove_test_dir(path: &std::path::Path) {
    // WebView2 child processes may still hold files when the app returns from
    // the event loop; retry briefly before leaving the directory to the stale
    // sweep on the next run.
    for _ in 0..25 {
        if std::fs::remove_dir_all(path).is_ok() || !path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// --test-ui: apply the current theme and construct all four windows to
/// validate resource resolution, printing one OK line per window (SPEC 19).
/// Runs WITHOUT the single-instance plugin so it works alongside a live GUI,
/// and with its own app-data root so the WebView2 user-data folder never
/// collides with a running instance (SPEC 22.5).
pub fn run_ui_test() {
    run_headless_windows(false);
}

/// --test-login (PLAN §8): verify the DeepSeek sign-in chain end to end.
/// The verification half runs first and is read-only; the window half builds
/// the real sign-in webview and proves the page -> host capture channel by
/// firing the hook's own sender with a probe.
pub fn run_login_test(verbose: bool) {
    let status = deepseek::credentials::status();
    println!("credentialSource={}", status.source.as_str());
    println!("apiKeyConfigured={}", status.api_key);
    println!("userTokenConfigured={}", status.user_token);
    if verbose {
        match deepseek::client::load_usage_base() {
            Some(base) => println!("usageBase={base}"),
            None => println!("usageBase=(not learned yet)"),
        }
    }
    let creds = deepseek::credentials::load();
    match creds.user_token.as_deref() {
        Some(token) => {
            match tauri::async_runtime::block_on(deepseek::client::verify_user_token(token)) {
                Ok(()) => println!("verify=ok"),
                Err(error) => println!("verify={error}"),
            }
        }
        None => println!("verify=skipped(no-user-token)"),
    }
    run_headless_windows(true);
}

/// --test-usage (PLAN §5.1): run the local cache scan twice and print the result.
/// The first call walks the session tree (or resumes from the index); the second
/// only re-stats known files — that difference is the incremental proof.
pub fn run_usage_test() {
    let first = kimi_usage::scan();
    println!(
        "scan1: files={} requests(7d)={} requests(today)={} ms={}",
        first.files, first.week.requests, first.today.requests, first.scan_ms
    );
    let second = kimi_usage::scan_known();
    println!(
        "scan2: requests(7d)={} requests(today)={} ms={}",
        second.week.requests, second.today.requests, second.scan_ms
    );
    match serde_json::to_string_pretty(&second) {
        Ok(json) => println!("{json}"),
        Err(e) => println!("serialize error: {e}"),
    }
}

/// Shared scaffolding for the two headless window tests. `with_login` adds the
/// on-demand DeepSeek sign-in window and its channel probe.
fn run_headless_windows(with_login: bool) {
    cleanup_stale_test_dirs();
    let test_root = std::env::temp_dir().join(format!("quota-tray-testui-{}", std::process::id()));
    let mut context = tauri::generate_context!();
    // Keep the diagnostic log inside the same isolated root (SPEC 22.5), and
    // inherit the user's saved theme/interval so the self-check still renders
    // with the "current theme" (SPEC 19) without touching the real file.
    let real_settings = settings::config_dir().join("settings.json");
    let _ = std::fs::create_dir_all(&test_root);
    let _ = std::fs::copy(real_settings, test_root.join("settings.json"));
    std::env::set_var("QUOTA_TRAY_APP_DATA_DIR", &test_root);
    context.config_mut().app.app_directories_override = Some(
        tauri::utils::config::AppDirectoriesOverride::Root(test_root.clone()),
    );
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(build_state())
        .invoke_handler(invoke_handlers())
        .setup(move |app| {
            let handle = app.handle().clone();
            recovery::install_all(&handle);
            recovery::start(&handle);
            for (label, name) in [
                ("main", "MainWindow"),
                ("settings", "SettingsWindow"),
                ("skills", "SkillsWindow"),
                ("menu", "TrayMenuWindow"),
            ] {
                match app.get_webview_window(label) {
                    Some(w) => {
                        let _ = w.show();
                        if label == "main" {
                            let _ = app.emit("panel-show", ());
                        }
                        println!("{name} OK");
                    }
                    None => println!("UI-FAIL: {name}: window was not created"),
                }
            }
            // --test-ui also exercises the local cache scan: it is the only
            // headless path that renders the Kimi pane with real session data.
            let usage_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move { kimi_usage::run(usage_handle).await });
            if with_login {
                login::open(app.handle());
                match app.get_webview_window(login::LABEL) {
                    Some(_) => println!("LoginWindow OK"),
                    None => println!("LOGIN-FAIL: LoginWindow: window was not created"),
                }
            }
            // Give the webviews a moment to actually load, then exit headless.
            // Also emit panel-show (after the page's listeners attach) and suppress
            // the focus-loss auto-hide so the panel stays visible for inspection.
            app.state::<AppState>()
                .settings_open
                .store(true, Ordering::SeqCst);
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(2500));
                let _ = handle.emit("panel-show", ());
                // Lets the skills window render its list in --test-ui too.
                let _ = handle.emit("skills-show", ());
            });
            if with_login {
                let probe = app.handle().clone();
                std::thread::spawn(move || {
                    // The page needs to be up before its injected hook exists.
                    std::thread::sleep(std::time::Duration::from_millis(3500));
                    if let Some(w) = probe.get_webview_window(login::LABEL) {
                        let _ = w.eval(login::probe_script());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1200));
                    if login::probe_seen(&probe) {
                        println!("LoginWindow capture channel OK");
                    } else {
                        println!("LOGIN-FAIL: capture channel: the hook never reached the host");
                    }
                    if login::candidate_seen(&probe) {
                        println!("LoginWindow fetch capture OK");
                    } else {
                        println!("LOGIN-FAIL: fetch capture: the hook saw no token on a fetch call");
                    }
                });
            }
            let exit_handle = app.handle().clone();
            std::thread::spawn(move || {
                let hold = if with_login { 7000 } else { 6000 };
                std::thread::sleep(std::time::Duration::from_millis(hold));
                // Graceful exit so app.run returns and the test data dir can
                // be removed; std::process::exit would skip that cleanup.
                exit_handle.exit(0);
            });
            Ok(())
        })
        .build(context);
    match result {
        Ok(app) => app.run(|_app, event| {
            // Same last-window guard as the GUI path: a test-triggered rebuild
            // must not let Tauri exit before the windows are recreated.
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        }),
        Err(e) => println!("UI-FAIL: {e}"),
    }
    remove_test_dir(&test_root);
}
