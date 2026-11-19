use crate::deepseek::DeepseekResult;
use crate::kimi_usage::KimiUsage;
use crate::quota::QuotaResult;
use crate::recovery::RecoveryState;
use crate::settings::SettingsData;
use crate::skills::SkillInfo;
use crate::update::UpdateStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Which provider a poller or a schedule belongs to.
#[derive(Clone, Copy)]
pub enum Provider {
    Kimi,
    Deepseek,
}

impl Provider {
    /// Short tag for diagnostics (`webview.log`), never user-facing text.
    pub fn label(self) -> &'static str {
        match self {
            Provider::Kimi => "kimi",
            Provider::Deepseek => "deepseek",
        }
    }
}

/// One provider's refresh schedule. Kimi and DeepSeek each own one:
/// `Notify::notify_one` wakes a single waiter, so a shared pair of Notifies
/// would starve whichever loop lost the race, and the two providers must retry
/// on independent timers (PLAN §5.3).
pub struct Schedule {
    /// Fired when the polling schedule must restart (settings saved)
    pub reschedule: Arc<Notify>,
    /// Fired by every safe_refresh (manual, hover, or scheduled) to move the
    /// next scheduled tick to now+delay — mirrors QuotaService's per-call
    /// _timer.Change(error ? 30s : period, period) (SPEC 16.5 step 3)
    pub retime: Arc<Notify>,
    pub retime_delay: Mutex<Option<Duration>>,
}

impl Schedule {
    fn new() -> Self {
        Schedule {
            reschedule: Arc::new(Notify::new()),
            retime: Arc::new(Notify::new()),
            retime_delay: Mutex::new(None),
        }
    }

    /// Restart the cycle with the 2s first-refresh delay.
    pub fn restart(&self) {
        self.reschedule.notify_one();
    }

    /// Move the next tick to now+delay, leaving the period unchanged.
    pub fn retime(&self, delay: Duration) {
        *self.retime_delay.lock().unwrap() = Some(delay);
        self.retime.notify_one();
    }

    pub fn take_delay(&self) -> Option<Duration> {
        self.retime_delay.lock().unwrap().take()
    }

    /// A reschedule drops any stale retime hint: the notify permit survives
    /// `continue`, and a leftover delay would overwrite the 2s first tick.
    pub fn clear_delay(&self) {
        let _ = self.retime_delay.lock().unwrap().take();
    }
}

/// DeepSeek sign-in state (PLAN §5.2). The window is temporary, so nothing here
/// outlives it: `reset` runs on every open and on close.
#[derive(Default)]
pub struct LoginState {
    /// A candidate token is being verified right now (single flight)
    pub verifying: AtomicBool,
    /// A candidate passed verification and was saved
    pub accepted: AtomicBool,
    /// `--test-login` saw the injected hook's probe arrive (channel proof)
    pub probe: AtomicBool,
    /// `--test-login` saw the hook lift a token off a real `fetch` call
    pub candidate: AtomicBool,
    /// Newest candidate that arrived while another one was being verified. The
    /// console emits a burst of tokens during sign-in; dropping the last one
    /// costs the user a whole sign-in, so it is retried once the current
    /// verification finishes (login.rs).
    pub pending: Mutex<Option<String>>,
    /// Bumped by Clear and by every real window boundary (a new session, or a
    /// close). A verification that started before the bump must not save its
    /// token, retitle, or close the window that now exists.
    pub epoch: std::sync::atomic::AtomicU64,
    /// Bumped by Clear alone. Opening or closing the sign-in window is not a
    /// clear, so a manual paste the user is waiting on must survive it — which
    /// is why the paste path cannot use `epoch`.
    pub clears: std::sync::atomic::AtomicU64,
}

impl LoginState {
    /// Clear the per-session flags without touching the epoch. Used when the
    /// console window is merely brought forward: the user is still in the same
    /// sign-in, and an in-flight verification of a good candidate must survive
    /// (the page never resends a token it already reported).
    pub fn reset(&self) {
        self.verifying.store(false, Ordering::SeqCst);
        self.accepted.store(false, Ordering::SeqCst);
        self.probe.store(false, Ordering::SeqCst);
        self.candidate.store(false, Ordering::SeqCst);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
    }

    /// A real window boundary: the console window was built or destroyed, so
    /// anything still in flight belongs to a session nobody is looking at.
    pub fn new_session(&self) {
        self.reset();
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    /// The credentials were cleared: a paste being verified must drop its value.
    pub fn note_clear(&self) {
        self.clears.fetch_add(1, Ordering::SeqCst);
    }

    /// Read the clear generation to compare against after an await.
    pub fn clears(&self) -> u64 {
        self.clears.load(Ordering::SeqCst)
    }
}

/// Shared application state (managed by Tauri).
pub struct AppState {
    pub settings: RwLock<SettingsData>,
    /// DeepSeek sign-in window state (temporary window, PLAN §4)
    pub login: LoginState,
    pub last_quota: RwLock<Option<QuotaResult>>,
    /// Last-known-good DeepSeek payload; kept apart from `last_quota` so a
    /// failure on one provider never blanks the other (PLAN §5.3)
    pub last_deepseek: RwLock<Option<DeepseekResult>>,
    /// Kimi prompt-cache / decode-speed statistics read from the local session
    /// logs (PLAN §5.1). Independent of the quota fetch: one has nothing to do
    /// with the other.
    pub kimi_usage: RwLock<Option<KimiUsage>>,
    pub update: RwLock<UpdateStatus>,
    /// Effective theme after resolving "system": "light" | "dark"
    pub effective_theme: RwLock<String>,
    /// Timestamp of the last auto-hide, for the 300ms tray-click re-entry guard
    pub last_hide: Mutex<Option<Instant>>,
    /// Timestamp of the last hover-triggered refresh (10s throttle)
    pub last_hover: Mutex<Option<Instant>>,
    /// Timestamp of the last manual "Refresh" command (2s debounce)
    pub last_manual_refresh: Mutex<Option<Instant>>,
    pub panel_hiding: AtomicBool,
    /// While the settings window is open, panel focus-loss hide is suppressed
    pub settings_open: AtomicBool,
    /// While the skills window is open, panel focus-loss hide is suppressed
    pub skills_open: AtomicBool,
    /// Lazy one-shot cache for the skills window (no background scanning)
    pub skills_cache: RwLock<Option<Vec<SkillInfo>>>,
    /// Content-measured menu height in DIP (WPF SizeToContent equivalent)
    pub menu_height: Mutex<f64>,
    /// Content-measured main-panel height in DIP (card + shadow fade room).
    /// The panel is bottom-anchored, so this drives both the window height and
    /// the re-pinned position (PLAN §4).
    pub panel_height: Mutex<f64>,
    /// Content-measured settings-window height in DIP (card + shadow fade
    /// room). The window is centred, so this only drives the size (PLAN §4).
    pub settings_height: Mutex<f64>,
    /// Height last applied to the settings window; a repeat does not re-centre
    /// it, so a position the user dragged to survives a close/reopen.
    pub settings_applied_height: Mutex<f64>,
    schedule_kimi: Schedule,
    schedule_deepseek: Schedule,
    /// WebView2 failure detection, diagnostics and window rebuild state
    /// (SPEC 22)
    pub recovery: RecoveryState,
}

impl AppState {
    pub fn new(settings: SettingsData, effective_theme: String) -> Self {
        AppState {
            settings: RwLock::new(settings),
            login: LoginState::default(),
            last_quota: RwLock::new(None),
            last_deepseek: RwLock::new(None),
            kimi_usage: RwLock::new(None),
            update: RwLock::new(UpdateStatus::default()),
            effective_theme: RwLock::new(effective_theme),
            last_hide: Mutex::new(None),
            last_hover: Mutex::new(None),
            last_manual_refresh: Mutex::new(None),
            panel_hiding: AtomicBool::new(false),
            settings_open: AtomicBool::new(false),
            skills_open: AtomicBool::new(false),
            skills_cache: RwLock::new(None),
            menu_height: Mutex::new(160.0),
            panel_height: Mutex::new(520.0),
            settings_height: Mutex::new(360.0),
            settings_applied_height: Mutex::new(0.0),
            schedule_kimi: Schedule::new(),
            schedule_deepseek: Schedule::new(),
            recovery: RecoveryState::new(),
        }
    }

    pub fn schedule(&self, provider: Provider) -> &Schedule {
        match provider {
            Provider::Kimi => &self.schedule_kimi,
            Provider::Deepseek => &self.schedule_deepseek,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window boundary and a clear invalidate different things; conflating
    /// them would cancel a paste every time the sign-in window opens.
    #[test]
    fn a_clear_and_a_window_boundary_move_different_counters() {
        let st = LoginState::default();
        st.new_session();
        assert_eq!(st.epoch.load(Ordering::SeqCst), 1);
        assert_eq!(st.clears(), 0);

        st.note_clear();
        assert_eq!(st.clears(), 1);
        assert_eq!(st.epoch.load(Ordering::SeqCst), 1);
    }
}
