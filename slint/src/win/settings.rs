// Settings sheet (PLAN §4): the three persisted preferences, the read-only
// Kimi credential section, and the DeepSeek statuses with the manual fallback.
//
// Behaviour is the Tauri window's, 1:1:
//   * every open backfills the controls from settings.json (SPEC 13.2);
//   * Save persists -> applies autostart -> applies the theme -> restarts the
//     poll schedules -> closes the sheet;
//   * a pasted credential is verified before it is saved (PLAN §5.2), and a
//     Clear cancels any verification already in flight so it cannot resurrect
//     the token (the same `epoch` guard the sign-in window uses).
//
// Heights come from the Slint layout (`SettingsWindow.natural-h`), which sums
// the constants measured from the shipped DOM; this module only clamps that to
// 90% of the work area and centres the window.

use crate::app::{self, Msg};
use crate::shell::{self, Shell};
use crate::view;
use quota_core::deepseek::credentials as ds_credentials;
use quota_core::{credentials, deepseek, settings};
use slint::{ComponentHandle, LogicalSize, PhysicalPosition};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub const SETTINGS_W: f32 = 404.0;
/// Transparent shadow fade room per side (`theme.css` #root margin).
const MARGIN: f32 = 28.0;
const SHADOW_ROOM: f32 = 2.0 * MARGIN;
/// PLAN §4: the sheet never exceeds 90% of the work area's height.
const WORK_AREA_FRACTION: f32 = 0.9;
const MIN_H: f32 = 120.0;

/// Radio order in the sheet (theme names as persisted).
pub const THEMES: [&str; 3] = ["system", "light", "dark"];
/// Radio order in the sheet (`RefreshMinutes`).
pub const INTERVALS: [i64; 4] = [1, 5, 10, 30];

const ENV_NOTE: &str =
    "Values come from DEEPSEEK_API_KEY / DEEPSEEK_USER_TOKEN — Clear does not remove them.";
const CLEAR_NOTE: &str =
    "Cleared. Signing in again stores the usage token the console already has.";

pub fn theme_index(theme: &str) -> i32 {
    THEMES
        .iter()
        .position(|t| *t == theme)
        .map(|i| i as i32)
        .unwrap_or(0)
}

pub fn interval_index(minutes: i64) -> i32 {
    // The shipped sheet has no "unknown" state: a hand-edited interval falls
    // back to the 5-minute cell the loader defaults to.
    INTERVALS
        .iter()
        .position(|m| *m == minutes)
        .map(|i| i as i32)
        .unwrap_or(1)
}

/// Read-only credential status (PLAN §4): labels and booleans, never a value.
pub struct Status {
    pub kimi_version: String,
    pub kimi_cred: String,
    pub kimi_cred_bad: bool,
    pub ds_key: String,
    pub ds_key_bad: bool,
    pub ds_token: String,
    pub ds_token_bad: bool,
    pub env_note: bool,
}

/// HH:MM for the OAuth expiry; these tokens live for hours, so the date never
/// matters (settings.ts `fmtClock`).
fn fmt_clock(unix_seconds: i64) -> String {
    use chrono::{Local, TimeZone};
    match Local.timestamp_opt(unix_seconds, 0).single() {
        Some(t) => t.format("%H:%M").to_string(),
        None => String::new(),
    }
}

pub fn status(update: &quota_core::update::UpdateStatus) -> Status {
    let kimi = credentials::describe();
    let ds = ds_credentials::status();

    let local = update
        .local_version
        .clone()
        .unwrap_or_else(|| "Not found".to_string());
    let kimi_version = match (&update.latest_version, update.update_available) {
        (Some(latest), true) => format!("{local} → {latest}"),
        _ => local,
    };

    let (kimi_cred, kimi_cred_bad) = match (kimi.source, kimi.expired) {
        ("oauth", _) => {
            let expires = kimi
                .expires_at
                .map(|at| format!(" · expires {}", fmt_clock(at)))
                .unwrap_or_default();
            (format!("OAuth token{expires}"), false)
        }
        ("api-key", _) => ("API key (config.toml)".to_string(), false),
        (_, true) => ("Expired — sign in with the Kimi CLI".to_string(), true),
        // Not configured, not broken: red is reserved for expired/unreadable
        // (PLAN §3.5); the shipped frontend paints this row muted too.
        _ => ("Not found — sign in with the Kimi CLI".to_string(), false),
    };

    let describe = |configured: bool| -> (String, bool) {
        if configured {
            ("Configured".to_string(), false)
        } else if ds.source == ds_credentials::Source::Undecryptable {
            // A blob that will not decrypt is a broken state, not an empty one:
            // re-entry is the fix (PLAN §5.2).
            ("Unreadable — re-enter".to_string(), true)
        } else {
            ("Not configured".to_string(), false)
        }
    };
    let (ds_key, ds_key_bad) = describe(ds.api_key);
    let (ds_token, ds_token_bad) = describe(ds.user_token);

    Status {
        kimi_version,
        kimi_cred,
        kimi_cred_bad,
        ds_key,
        ds_key_bad,
        ds_token,
        ds_token_bad,
        env_note: ds.source == ds_credentials::Source::Env,
    }
}

/// Push the persisted settings + credential statuses into the sheet.
pub fn apply_controls(shell: &Shell) {
    let ui = &shell.settings_ui;
    ui.set_theme_index(theme_index(&shell.data.settings.theme));
    ui.set_interval_index(interval_index(shell.data.settings.refresh_minutes));
    ui.set_autostart(shell.data.settings.auto_start);
    apply_status(shell);
}

/// Re-render the read-only sections only (after Clear / a verified paste /
/// while the window is open and a poll lands).
pub fn apply_status(shell: &Shell) {
    let ui = &shell.settings_ui;
    let st = status(&shell.data.update);
    ui.set_kimi_version(st.kimi_version.into());
    ui.set_kimi_cred(st.kimi_cred.into());
    ui.set_kimi_cred_bad(st.kimi_cred_bad);
    ui.set_ds_key(st.ds_key.into());
    ui.set_ds_key_bad(st.ds_key_bad);
    ui.set_ds_token(st.ds_token.into());
    ui.set_ds_token_bad(st.ds_token_bad);
    // The env note is regenerated on every status pass; a transient note
    // ("Cleared…", a verification result) is written after this call.
    ui.set_ds_note(if st.env_note { ENV_NOTE } else { "" }.into());
    ui.set_ds_note_bad(false);
}

pub fn open(shell: &Rc<RefCell<Shell>>) {
    if shell.borrow().settings_open {
        return;
    }
    {
        // The flag goes up BEFORE the window shows: the panel's blur (our own
        // focus steal) must already be suppressed when it arrives.
        let mut s = shell.borrow_mut();
        s.settings_open = true;
        apply_controls(&s);
    }
    resize(shell, false);
    let shown = shell.borrow().settings_ui.show().is_ok();
    if !shown {
        crate::logf("settings: show failed");
        shell.borrow_mut().settings_open = false;
        return;
    }
    // A missing HWND right after the first show is normal (the winit window is
    // created on the event-loop tick): it is not a failure, only no focus yet.
    let hwnd = shell::hwnd_of(shell.borrow().settings_ui.window());
    if let Some(h) = hwnd {
        shell::no_round(h);
        shell::focus(h);
    }
    let (h, content) = {
        let s = shell.borrow();
        (s.settings_h, s.settings_ui.get_content_h())
    };
    crate::logf(format!(
        "settings: open h={h:.1} content={content:.1} hwnd={hwnd:?}"
    ));
}

pub fn close(shell: &Rc<RefCell<Shell>>, collapse_panel: bool) {
    {
        let mut s = shell.borrow_mut();
        if !s.settings_open {
            return;
        }
        s.settings_open = false;
        let _ = s.settings_ui.hide();
        // Closing the sheet is a decision about the sheet: a sign-in window that
        // is still open must not pop it back up when it exits.
        s.login.reopen_sheet = false;
    }
    crate::logf("settings: hide");
    if collapse_panel {
        // The panel's blur was suppressed while the sheet was open and hiding
        // the sheet does not produce a new one (Tauri `collapse_after_settings_close`).
        collapse_panel_if_unfocused(shell);
    }
}

/// Ask the settings window to hide itself; the sheet's own OK / Save actions
/// use the same path as the title-bar close.
pub fn close_from_ui(shell: &Rc<RefCell<Shell>>) {
    close(shell, true);
}

fn collapse_panel_if_unfocused(shell: &Rc<RefCell<Shell>>) {
    let weak = Rc::downgrade(shell);
    slint::Timer::single_shot(Duration::from_millis(150), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, panel_hwnd, other_open) = {
            let s = shell.borrow();
            (
                s.visible,
                s.hwnd,
                s.settings_open || s.skills_open || s.menu_open,
            )
        };
        if other_open || !visible {
            return;
        }
        let Some(hwnd) = panel_hwnd else { return };
        // Windows usually hands focus back to the panel after the sheet
        // disappears; hiding on the instant check would fight that.
        if !shell::is_foreground(hwnd) {
            shell::start_hide(&shell);
        }
    });
}

/// Size to the Slint-derived content height, clamp to 90% of the work area and
/// centre. Re-centring only happens when the height changed, so a position the
/// user dragged the sheet to survives a reopen at the same height (PLAN §4).
pub fn resize(shell: &Rc<RefCell<Shell>>, force: bool) {
    let scale = shell.borrow().settings_ui.window().scale_factor() as f64;
    let (wa_left, wa_top, wa_right, wa_bottom) = shell::work_area_phys();
    let work_h = (wa_bottom - wa_top) / scale;
    let mut max_h = (work_h as f32 * WORK_AREA_FRACTION).max(MIN_H);
    let natural;
    {
        let s = shell.borrow();
        if let Some(cap) = s.settings_max_override {
            max_h = max_h.min(cap);
        }
        natural = s.settings_ui.get_natural_h();
    }
    let h = natural.clamp(MIN_H, max_h);
    let changed = {
        let s = shell.borrow();
        (s.settings_h - h).abs() >= 0.5
    };
    {
        let mut s = shell.borrow_mut();
        s.settings_ui.set_max_card_h((h - SHADOW_ROOM).max(1.0));
        s.settings_ui
            .window()
            .set_size(LogicalSize::new(SETTINGS_W, h));
        if changed || force {
            let x = wa_left + ((wa_right - wa_left) - SETTINGS_W as f64 * scale) / 2.0;
            let y = wa_top + ((wa_bottom - wa_top) - h as f64 * scale) / 2.0;
            s.settings_ui
                .window()
                .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
        }
        s.settings_h = h;
    }
    crate::logf(format!(
        "settings: size natural={natural:.1} applied={h:.1} max={max_h:.1}"
    ));
}

/// The disclosure changed the sheet's height: keep the window in step without
/// throwing away the position (the sheet is only re-centred if it must move).
pub fn manual_toggled(shell: &Rc<RefCell<Shell>>) {
    resize(shell, false);
}

/// Read the controls and persist them (SPEC 13.2): save -> autostart -> theme
/// -> reschedule -> close.
pub fn save_from_ui(shell: &Rc<RefCell<Shell>>) {
    let (theme_i, interval_i, autostart) = {
        let s = shell.borrow();
        (
            s.settings_ui.get_theme_index(),
            s.settings_ui.get_interval_index(),
            s.settings_ui.get_autostart(),
        )
    };
    let theme = THEMES
        .get(theme_i.clamp(0, 2) as usize)
        .copied()
        .unwrap_or("system")
        .to_string();
    let minutes = INTERVALS
        .get(interval_i.clamp(0, 3) as usize)
        .copied()
        .unwrap_or(5);
    {
        let mut s = shell.borrow_mut();
        s.data.settings.theme = theme.clone();
        s.data.settings.refresh_minutes = minutes;
        s.data.settings.auto_start = autostart;
        settings::save(&s.data.settings);
        settings::apply_auto_start(&s.data.settings);
        s.workers.set_period_minutes(minutes);
        s.workers.set_configured_theme(&theme);
        s.data.theme = app::effective_theme(&theme);
    }
    view::apply_theme(shell);
    crate::logf(format!(
        "settings: saved theme={theme} interval={minutes} autostart={autostart}"
    ));
    close_from_ui(shell);
}

/// Clear both stored DeepSeek credentials (PLAN §5.2). The file stays, empty,
/// so the one-time legacy import cannot bring them back.
pub fn clear(shell: &Rc<RefCell<Shell>>) {
    // Invalidate any in-flight paste verification BEFORE the wipe: the worker
    // compares this epoch between verify and save, so bumping it first closes
    // the window where a verification could write the token back after Clear.
    shell.borrow().cred_epoch.fetch_add(1, Ordering::SeqCst);
    // An open sign-in window is a live capture channel: kill it before the
    // credential goes, or the console's own session hands the hook a valid
    // token seconds later and the Clear looks like it did nothing (PLAN §4).
    crate::login::cancel(shell);
    deepseek::credentials::clear();
    let env_sourced = deepseek::credentials::status().source == ds_credentials::Source::Env;
    {
        let s = shell.borrow();
        s.settings_ui.set_verifying(false);
        s.settings_ui.set_cred_note("".into());
        s.settings_ui.set_cred_note_bad(false);
        apply_status(&s);
        // Env-sourced credentials survive Clear by design; the env note has to
        // win over "Cleared." or the sheet claims a removal that did not happen.
        if !env_sourced {
            s.settings_ui.set_ds_note(CLEAR_NOTE.into());
        }
        s.workers.refresh();
    }
    crate::logf("settings: credentials cleared");
    resize(shell, false);
}

/// PLAN §5.2 error code -> the text the manual fallback shows (settings.ts
/// `credError`).
fn cred_error(code: &str) -> String {
    match code {
        "key-invalid" => "invalid key — not saved".to_string(),
        "token-expired" => "invalid or expired — not saved".to_string(),
        "rate-limited" => "rate limited — try again later".to_string(),
        "network-error" => "network error — not saved".to_string(),
        "server-error" => "DeepSeek server error — not saved".to_string(),
        "empty" => "empty value".to_string(),
        "save-failed" => "could not write the credential — not saved".to_string(),
        other => format!("verification failed ({other}) — not saved"),
    }
}

/// Verify each non-empty field, save only what passes. Runs on the worker
/// runtime; the note comes back through `Msg::CredentialChecked`.
pub fn save_credentials(shell: &Rc<RefCell<Shell>>, api_key: String, user_token: String) {
    let api_key = api_key.trim().to_string();
    let user_token = user_token.trim().to_string();
    if api_key.is_empty() && user_token.is_empty() {
        {
            let s = shell.borrow();
            s.settings_ui
                .set_cred_note("Paste an API key or a usage token first.".into());
            s.settings_ui.set_cred_note_bad(true);
        }
        resize(shell, false);
        return;
    }
    let epoch = {
        let s = shell.borrow();
        s.settings_ui.set_verifying(true);
        s.settings_ui.set_cred_note("".into());
        s.settings_ui.set_cred_note_bad(false);
        // Bumping the epoch drops any older verification still in flight: only
        // the newest candidate may write (PLAN §4). `fetch_add` returns the
        // value BEFORE the bump, so the new generation is that value + 1 —
        // comparing against the old one made every verification look
        // superseded and the button never left "Verifying…".
        s.cred_epoch.fetch_add(1, Ordering::SeqCst) + 1
    };
    let guard: Arc<std::sync::atomic::AtomicU64> = shell.borrow().cred_epoch.clone();
    let tx = shell.borrow().workers.sender();
    shell.borrow().workers.spawn(async move {
        let mut saved_api = false;
        let mut saved_token = false;
        let mut problems: Vec<String> = Vec::new();
        if !api_key.is_empty() {
            match deepseek::client::fetch_balance(&api_key).await {
                Ok(_) => {
                    if guard.load(Ordering::SeqCst) == epoch {
                        saved_api = deepseek::credentials::save(Some(&api_key), None, None);
                        if !saved_api {
                            problems.push(
                                "API key: could not write the credential — not saved".to_string(),
                            );
                        }
                    }
                }
                Err(code) => problems.push(format!("API key: {}", cred_error(&code))),
            }
        }
        if !user_token.is_empty() {
            match deepseek::client::verify_user_token(&user_token).await {
                Ok(()) => {
                    if guard.load(Ordering::SeqCst) == epoch {
                        saved_token = deepseek::credentials::save(None, Some(&user_token), None);
                        if !saved_token {
                            problems.push(
                                "Usage token: could not write the credential — not saved"
                                    .to_string(),
                            );
                        }
                    }
                }
                Err(code) => problems.push(format!("Usage token: {}", cred_error(&code))),
            }
        }
        let superseded = guard.load(Ordering::SeqCst) != epoch;
        let _ = tx.send(Msg::CredentialChecked {
            saved_api,
            saved_token,
            problems,
            superseded,
        });
    });
}

/// A finished verification landed on the UI thread: drop it if a Clear or a
/// newer candidate superseded it, otherwise write the note and refresh.
pub fn on_checked(
    shell: &Rc<RefCell<Shell>>,
    saved_api: bool,
    saved_token: bool,
    problems: Vec<String>,
    superseded: bool,
) {
    if superseded {
        crate::logf("settings: verification superseded, result dropped");
        return;
    }
    let note = if problems.is_empty() {
        "Verified and saved.".to_string()
    } else {
        problems.join(" · ")
    };
    let bad = !problems.is_empty();
    {
        let s = shell.borrow();
        s.settings_ui.set_verifying(false);
        if saved_api {
            s.settings_ui.set_api_key_input("".into());
        }
        if saved_token {
            s.settings_ui.set_user_token_input("".into());
        }
        s.settings_ui.set_cred_note(note.clone().into());
        s.settings_ui.set_cred_note_bad(bad);
        apply_status(&s);
        s.workers.refresh();
    }
    crate::logf(format!(
        "settings: verification done saved-api={saved_api} saved-token={saved_token} note={note:?}"
    ));
    resize(shell, false);
}

/// "Sign in…": the console window is a child process (`--login-host`), built on
/// demand and destroyed when it closes (PLAN §4). The sheet hides while it runs
/// and comes back when the child exits (`login::settle`).
pub fn sign_in(shell: &Rc<RefCell<Shell>>) {
    crate::logf("settings: sign-in clicked");
    crate::login::open(shell);
}
