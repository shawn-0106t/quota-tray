// Application data: the two provider pollers, the local Kimi usage scan, the
// CLI version check and the system-theme watch.
//
// Semantics are PLAN §5.3, unchanged from the Tauri shell: first fetch 2s after
// start, then the configured interval; a failure keeps the last-known-good
// numbers and retries after 30s. Each provider owns its own last-good slot, so a
// dead Kimi fetch can never blank the DeepSeek pane. The pollers run on a
// dedicated tokio runtime and hand finished values to the UI thread through a
// channel — the Slint event loop stays the only thing touching the UI.

use quota_core::deepseek::{self, DeepseekResult};
use quota_core::kimi_usage::{self, KimiUsage};
use quota_core::quota::{self, QuotaResult};
use quota_core::settings::{self, SettingsData};
use quota_core::update::{self, UpdateStatus};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// One finished piece of work, delivered to the UI thread.
pub enum Msg {
    Quota(QuotaResult),
    Deepseek(DeepseekResult),
    Usage(KimiUsage),
    Update(UpdateStatus),
    Theme(String),
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

/// The UI's inbox plus the manual-refresh broadcast.
pub struct Workers {
    pub rx: Receiver<Msg>,
    /// Every "Refresh now" / tray-hover prefetch fires this; both poll loops
    /// wake up and fetch immediately.
    pub refresh: Arc<Notify>,
    // Held so the runtime stays alive for the process lifetime.
    _rt: tokio::runtime::Runtime,
}

/// Start the background pollers. `interval_minutes` is the configured refresh
/// period (already normalised to >= 1 by the settings loader).
pub fn spawn(interval_minutes: i64) -> Workers {
    let (tx, rx) = mpsc::channel();
    let refresh = Arc::new(Notify::new());
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    let period = Duration::from_secs(interval_minutes.max(1) as u64 * 60);

    {
        let tx = tx.clone();
        let refresh = refresh.clone();
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
                wait(&refresh, if failed { RETRY } else { period }).await;
            }
        });
    }
    {
        let tx = tx.clone();
        let refresh = refresh.clone();
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
                wait(&refresh, if failed { RETRY } else { period }).await;
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
        let tx = tx;
        // The configured theme is read once at startup: P2 has no settings
        // window, so it cannot change under us; the system value still can.
        let configured = settings::load().theme;
        rt.spawn(async move {
            let mut current = effective_theme(&configured);
            loop {
                tokio::time::sleep(Duration::from_secs(3)).await;
                let next = effective_theme(&configured);
                if next != current {
                    current = next.clone();
                    if tx.send(Msg::Theme(next)).is_err() {
                        return;
                    }
                }
            }
        });
    }

    Workers {
        rx,
        refresh,
        _rt: rt,
    }
}

/// Both providers wait 2s before their first fetch (SPEC 16.5).
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
