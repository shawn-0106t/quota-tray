// Application data: the two provider pollers, the local Kimi usage scan, the
// CLI version check, the system-theme watch and the on-demand jobs the windows
// ask for (skills scan, credential verification).
//
// Semantics are PLAN §5.3, unchanged from the Tauri shell: first fetch 2s after
// start, then the configured interval; a failure keeps the last-known-good
// numbers and retries after 30s. Each provider owns its own last-good slot, so a
// dead Kimi fetch can never blank the DeepSeek pane. The pollers run on a
// dedicated tokio runtime and hand finished values to the UI thread through a
// channel — the Slint event loop stays the only thing touching the UI.
//
// The refresh interval and the configured theme live in shared cells rather
// than being captured at startup: P3's settings sheet changes both while the
// pollers sleep (PLAN §4: save restarts the schedule and applies the theme
// immediately).

use quota_core::deepseek::{self, DeepseekResult};
use quota_core::kimi_usage::{self, KimiUsage};
use quota_core::quota::{self, QuotaResult};
use quota_core::settings::{self, SettingsData};
use quota_core::skills::SkillInfo;
use quota_core::update::{self, UpdateStatus};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;

/// One finished piece of work, delivered to the UI thread.
pub enum Msg {
    Quota(QuotaResult),
    Deepseek(DeepseekResult),
    Usage(KimiUsage),
    Update(UpdateStatus),
    Theme(String),
    /// The lazy skills scan finished (the skills window renders it if open).
    Skills(Vec<SkillInfo>),
    /// A manual credential verification finished: which fields were stored,
    /// the PLAN §5.2 failures the sheet turns into text, and whether a Clear
    /// (or a newer candidate) superseded it on the way.
    CredentialChecked {
        saved_api: bool,
        saved_token: bool,
        problems: Vec<String>,
        superseded: bool,
    },
}

/// Everything the panel renders, plus the settings it started from.
pub struct Data {
    pub settings: SettingsData,
    /// Effective theme: `light` | `dark` (already resolved from `system`).
    pub theme: String,
    pub quota: Option<QuotaResult>,
    pub deepseek: Option<DeepseekResult>,
    pub usage: Option<KimiUsage>,
    pub update: UpdateStatus,
}

impl Data {
    /// Startup state: settings, the DeepSeek last-good cache (so the pane has
    /// numbers before the first poll lands) and nothing else — Kimi has no
    /// on-disk cache, exactly like the shipped app.
    pub fn load() -> Self {
        let settings = settings::load();
        Data {
            theme: effective_theme(&settings.theme),
            settings,
            quota: None,
            deepseek: deepseek::load_cache(),
            usage: None,
            update: UpdateStatus::default(),
        }
    }

    pub fn dark(&self) -> bool {
        self.theme == "dark"
    }

    /// Persist the provider tab (PLAN §4/§7): only `ActiveTab` changes, the
    /// rest of settings.json is written back untouched.
    pub fn save_tab(&mut self, tab: &str) {
        let tab = settings::normalize_tab(tab);
        if self.settings.active_tab == tab {
            return;
        }
        self.settings.active_tab = tab.to_string();
        settings::save(&self.settings);
    }
}

/// `system` follows HKCU AppsUseLightTheme; anything else is used verbatim.
pub fn effective_theme(configured: &str) -> String {
    match configured {
        "light" => "light".to_string(),
        "dark" => "dark".to_string(),
        _ => system_theme().to_string(),
    }
}

fn system_theme() -> &'static str {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let value: u32 = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|k| k.get_value("AppsUseLightTheme"))
        .unwrap_or(1);
    if value == 0 {
        "dark"
    } else {
        "light"
    }
}

/// The UI's inbox plus everything a window needs to ask for background work.
pub struct Workers {
    pub rx: Receiver<Msg>,
    /// "Refresh now" / tray-hover prefetch: one Notify per provider, with
    /// `notify_one` so a click that lands while a fetch is running leaves a
    /// stored permit instead of being lost (`notify_waiters` wakes only the
    /// waiters registered at that instant).
    kimi_refresh: Arc<Notify>,
    ds_refresh: Arc<Notify>,
    tx: Sender<Msg>,
    handle: tokio::runtime::Handle,
    /// Current poll period, re-read after every wait so a Save applies at once.
    period: Arc<Mutex<Duration>>,
    /// The configured (not effective) theme: `system` follows Windows.
    configured_theme: Arc<Mutex<String>>,
    // Held so the runtime stays alive for the process lifetime.
    _rt: tokio::runtime::Runtime,
}

impl Workers {
    /// Run a job on the shared runtime (network verification, scans).
    pub fn spawn(&self, future: impl std::future::Future<Output = ()> + Send + 'static) {
        self.handle.spawn(future);
    }

    /// Apply a new `RefreshMinutes` (already normalised by the caller). The
    /// pollers re-read it after their current wait, and `refresh` cuts that
    /// wait short so the new interval starts now (SPEC 13.2: save restarts
    /// both schedules).
    pub fn set_period_minutes(&self, minutes: i64) {
        if let Ok(mut period) = self.period.lock() {
            *period = Duration::from_secs(minutes.clamp(1, 30) as u64 * 60);
        }
        self.refresh();
    }

    /// Ask both pollers to fetch now. `notify_one` stores a permit when a loop
    /// is inside a fetch, so the request cannot fall between fetch and wait.
    pub fn refresh(&self) {
        self.kimi_refresh.notify_one();
        self.ds_refresh.notify_one();
    }

    pub fn set_configured_theme(&self, theme: &str) {
        if let Ok(mut current) = self.configured_theme.lock() {
            *current = theme.to_string();
        }
    }

    /// Queue a finished value from any thread (the self-check uses it to drive
    /// the theme branch without waiting for Windows to change).
    pub fn post(&self, msg: Msg) {
        let _ = self.tx.send(msg);
    }

    /// Current poll period in seconds (the settings check asserts Save applied
    /// the new interval).
    pub fn period_seconds(&self) -> u64 {
        self.period.lock().map(|p| p.as_secs()).unwrap_or(0)
    }

    /// A clone of the message sender, for jobs that outlive this borrow.
    pub fn sender(&self) -> Sender<Msg> {
        self.tx.clone()
    }
}

/// Start the worker runtime. `poll` starts the four background loops; the
/// runtime, channel and shared cells exist either way so the settings sheet can
/// still verify a pasted credential under `--no-poll` (screenshots only need
/// the loops off, not the window's buttons dead).
pub fn start(interval_minutes: i64, poll: bool) -> Workers {
    let (tx, rx) = mpsc::channel();
    let kimi_refresh = Arc::new(Notify::new());
    let ds_refresh = Arc::new(Notify::new());
    let period = Arc::new(Mutex::new(Duration::from_secs(
        interval_minutes.clamp(1, 30) as u64 * 60,
    )));
    let configured_theme = Arc::new(Mutex::new(settings::load().theme));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    if poll {
        spawn_pollers(
            &rt,
            &tx,
            &kimi_refresh,
            &ds_refresh,
            &period,
            &configured_theme,
        );
    }

    Workers {
        rx,
        kimi_refresh,
        ds_refresh,
        tx,
        handle: rt.handle().clone(),
        period,
        configured_theme,
        _rt: rt,
    }
}

fn spawn_pollers(
    rt: &tokio::runtime::Runtime,
    tx: &Sender<Msg>,
    kimi_refresh: &Arc<Notify>,
    ds_refresh: &Arc<Notify>,
    period: &Arc<Mutex<Duration>>,
    configured_theme: &Arc<Mutex<String>>,
) {
    {
        let tx = tx.clone();
        let refresh = kimi_refresh.clone();
        let period = period.clone();
        rt.spawn(async move {
            let mut last: Option<QuotaResult> = None;
            wait_first().await;
            loop {
                let mut result = quota::fetch().await;
                if result.error.is_some() {
                    if let Some(previous) = &last {
                        result.fill_missing_from(previous);
                    }
                }
                let failed = result.error.is_some();
                last = Some(result.clone());
                if tx.send(Msg::Quota(result)).is_err() {
                    return;
                }
                wait(&refresh, current_period(&period, failed)).await;
            }
        });
    }
    {
        let tx = tx.clone();
        let refresh = ds_refresh.clone();
        let period = period.clone();
        rt.spawn(async move {
            let mut last: Option<DeepseekResult> = None;
            wait_first().await;
            loop {
                let mut result = deepseek::fetch().await;
                if result.has_error() {
                    if let Some(previous) = &last {
                        result.fill_missing_from(previous);
                    }
                }
                if result.balance.is_some() || result.usage.is_some() {
                    deepseek::save_cache(&result);
                }
                let failed = result.has_error();
                last = Some(result.clone());
                if tx.send(Msg::Deepseek(result)).is_err() {
                    return;
                }
                wait(&refresh, current_period(&period, failed)).await;
            }
        });
    }
    {
        let tx = tx.clone();
        rt.spawn(async move {
            // One walk, then stat-only ticks in between (PLAN §5.1: the tree
            // costs ~0.1s on 6.5k files). Only changed numbers are sent.
            let mut previous: Option<(u64, u64, u64, u64)> = None;
            let mut tick: u64 = 0;
            loop {
                let walked = tick.is_multiple_of(2);
                tick += 1;
                let scanned = tokio::task::spawn_blocking(move || {
                    if walked {
                        kimi_usage::scan()
                    } else {
                        kimi_usage::scan_known()
                    }
                })
                .await
                .unwrap_or_default();
                let signature = scanned.signature();
                if previous != Some(signature) {
                    previous = Some(signature);
                    if tx.send(Msg::Usage(scanned)).is_err() {
                        return;
                    }
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }
    {
        let tx = tx.clone();
        rt.spawn(async move {
            loop {
                let status = update::check().await;
                if tx.send(Msg::Update(status)).is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
            }
        });
    }
    {
        let tx = tx.clone();
        let configured = configured_theme.clone();
        rt.spawn(async move {
            let mut current =
                effective_theme(&configured.lock().map(|t| t.clone()).unwrap_or_default());
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let next =
                    effective_theme(&configured.lock().map(|t| t.clone()).unwrap_or_default());
                if next != current {
                    current = next.clone();
                    if tx.send(Msg::Theme(next)).is_err() {
                        return;
                    }
                }
            }
        });
    }
}

fn current_period(period: &Arc<Mutex<Duration>>, failed: bool) -> Duration {
    if failed {
        RETRY
    } else {
        period.lock().map(|p| *p).unwrap_or(RETRY)
    }
}

/// Failed fetches retry after 30s (PLAN §5.3).
const RETRY: Duration = Duration::from_secs(30);

async fn wait_first() {
    tokio::time::sleep(Duration::from_secs(2)).await;
}

async fn wait(refresh: &Notify, delay: Duration) {
    tokio::select! {
        _ = tokio::time::sleep(delay) => {}
        _ = refresh.notified() => {}
    }
}
