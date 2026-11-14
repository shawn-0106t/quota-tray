// Refresh scheduling, 1:1 port of QuotaService.SafeRefresh/Reschedule (SPEC 16.5):
// first refresh 2s after start, period = max(1, RefreshMinutes), failure keeps
// last-known-good data and retries fast after 30s. Both providers share that
// contract but run their own loop, schedule and last-good slot (PLAN §5.3).

use crate::deepseek::{self, DeepseekResult};
use crate::quota::{self, QuotaResult};
use crate::state::{AppState, Provider};
use crate::tray;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// The configured interval, clamped to at least one minute.
fn period(app: &AppHandle) -> Duration {
    let mins = app
        .state::<AppState>()
        .settings
        .read()
        .unwrap()
        .refresh_minutes
        .max(1) as u64;
    Duration::from_secs(mins.saturating_mul(60))
}

pub async fn safe_refresh(app: &AppHandle) -> QuotaResult {
    let mut r = quota::fetch().await;
    let st = app.state::<AppState>();
    if r.error.is_some() {
        let last = st.last_quota.read().unwrap().clone();
        if let Some(l) = &last {
            r.fill_missing_from(l);
        }
    }
    *st.last_quota.write().unwrap() = Some(r.clone());
    let _ = app.emit("quota-updated", &r);
    tray::update_tooltip(app);
    // Every refresh (scheduled, manual, or hover) moves the next scheduled
    // tick: 30s after a failure, one period after a success (SPEC 16.5 step 3,
    // mirrors _timer.Change in QuotaService.SafeRefresh)
    let delay = if r.error.is_some() {
        Duration::from_secs(30)
    } else {
        period(app)
    };
    st.schedule(Provider::Kimi).retime(delay);
    r
}

/// DeepSeek counterpart of `safe_refresh`. Same failure semantics, separate
/// last-good slot: the two providers never overwrite each other (PLAN §5.3).
pub async fn safe_refresh_deepseek(app: &AppHandle) -> DeepseekResult {
    let mut r = deepseek::fetch().await;
    let st = app.state::<AppState>();
    if r.has_error() {
        let last = st.last_deepseek.read().unwrap().clone();
        if let Some(l) = &last {
            r.fill_missing_from(l);
        }
    }
    *st.last_deepseek.write().unwrap() = Some(r.clone());
    let _ = app.emit("deepseek-updated", &r);
    if r.balance.is_some() || r.usage.is_some() {
        deepseek::save_cache(&r);
    }
    let delay = if r.has_error() {
        Duration::from_secs(30)
    } else {
        period(app)
    };
    st.schedule(Provider::Deepseek).retime(delay);
    r
}

/// Polling loop. `reschedule` notify restarts the cycle with the 2s first
/// delay (mirrors QuotaService.Reschedule's timer.Change(2s, period)); entries
/// on the Manual or Hover path re-time the next tick through `retime`.
pub async fn run(app: AppHandle) {
    run_loop(app, Provider::Kimi).await;
}

pub async fn run_deepseek(app: AppHandle) {
    run_loop(app, Provider::Deepseek).await;
}

async fn run_loop(app: AppHandle, provider: Provider) {
    let mut next = Duration::from_secs(2);
    loop {
        let (reschedule, retime) = {
            let st = app.state::<AppState>();
            let schedule = st.schedule(provider);
            (schedule.reschedule.clone(), schedule.retime.clone())
        };
        let wall_start = std::time::SystemTime::now();
        tokio::select! {
            _ = tokio::time::sleep(next) => {}
            _ = reschedule.notified() => {
                // Drain any stale retime hint queued before this reschedule:
                // the notify permit survives `continue`, and a leftover delay
                // would otherwise overwrite the 2s first-refresh tick.
                app.state::<AppState>().schedule(provider).clear_delay();
                next = Duration::from_secs(2);
                continue;
            }
            _ = retime.notified() => {
                let delay = app.state::<AppState>().schedule(provider).take_delay();
                if let Some(d) = delay {
                    next = d;
                }
                continue;
            }
        }
        // Diagnostic only (SPEC 22.1): a wall-clock jump far beyond the
        // scheduled sleep usually means the machine slept or was suspended.
        if let Ok(wall) = wall_start.elapsed() {
            if wall > next + Duration::from_secs(60) {
                crate::recovery::note_ui_event(
                    &app,
                    "app",
                    &format!("{}-wake-gap={}s", provider.label(), wall.as_secs()),
                );
            }
        }
        let errored = match provider {
            Provider::Kimi => safe_refresh(&app).await.error.is_some(),
            Provider::Deepseek => safe_refresh_deepseek(&app).await.has_error(),
        };
        next = if errored {
            Duration::from_secs(30) // fast retry after failure
        } else {
            period(&app)
        };
    }
}
