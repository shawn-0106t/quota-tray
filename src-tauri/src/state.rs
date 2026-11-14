use crate::deepseek::DeepseekResult;
use crate::quota::QuotaResult;
use crate::recovery::RecoveryState;
use crate::settings::SettingsData;
use crate::skills::SkillInfo;
use crate::update::UpdateStatus;
use std::sync::atomic::AtomicBool;
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

/// Shared application state (managed by Tauri).
pub struct AppState {
    pub settings: RwLock<SettingsData>,
    pub last_quota: RwLock<Option<QuotaResult>>,
    /// Last-known-good DeepSeek payload; kept apart from `last_quota` so a
    /// failure on one provider never blanks the other (PLAN §5.3)
    pub last_deepseek: RwLock<Option<DeepseekResult>>,
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
            last_quota: RwLock::new(None),
            last_deepseek: RwLock::new(None),
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
