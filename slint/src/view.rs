// Data -> panel view model. Every string the panel shows is produced here, by
// the same `quota_core::format` functions the Tauri frontend's `common.ts` is
// pinned against (`spike/p1/fmt_parity.py`), so the two shells cannot print
// different numbers for the same payload.
//
// The rules are PLAN §5.2 / §5.3: money always two decimals, `--` for missing
// values, red only for real failures, and the header stamp belongs to the
// provider on screen.

use crate::app::Data;
use crate::{ModelRow, Panel, Tokens};
use chrono::{DateTime, Local};
use quota_core::deepseek::{DeepseekResult, UsageInfo};
use quota_core::format::{
    clamp_percent, fmt_count, fmt_hit_rate, fmt_money, fmt_pct, fmt_tokens, fmt_updated, fmt_yuan,
    format_reset, is_low_balance, is_low_cache_hit,
};
use quota_core::kimi_usage::{CacheWindow, KimiUsage};
use quota_core::quota::{ExtraState, QuotaResult, QuotaSegment};
use quota_core::update::UpdateStatus;
use slint::{ComponentHandle, ModelRc, VecModel};

/// Without an API key nothing can work, so that one reads as configuration
/// rather than as a failed fetch; every other code is a failure (PLAN §5.2).
const NO_API_KEY: &str = "no-api-key";
/// The DeepSeek usage token's counterpart, and the Kimi credential's: an
/// unconfigured provider must not read as broken (PLAN §5.2 / §5.3).
const NO_USER_TOKEN: &str = "no-user-token";
const NO_TOKEN: &str = "no-token";

pub fn apply(ui: &Panel, data: &Data) {
    let now = Local::now();
    ui.set_dark(data.dark());
    ui.global::<Tokens>().set_dark(data.dark());
    apply_kimi(ui, data.quota.as_ref(), now);
    apply_usage(ui, data.usage.as_ref());
    apply_deepseek(ui, data.deepseek.as_ref());
    apply_update(ui, &data.update);
    apply_stamps(ui, data, now);
}

// ---- Kimi quota + Extra usage ----

fn apply_kimi(ui: &Panel, quota: Option<&QuotaResult>, now: DateTime<Local>) {
    let week = quota.and_then(|q| q.week.as_ref());
    let five = quota.and_then(|q| q.five_hour.as_ref());
    set_segment(week, now, |v, p, r| {
        ui.set_week(v.into());
        ui.set_week_pct(p);
        ui.set_week_reset(r.into());
    });
    set_segment(five, now, |v, p, r| {
        ui.set_five(v.into());
        ui.set_five_pct(p);
        ui.set_five_reset(r.into());
    });

    let extra = quota.and_then(|q| q.extra.as_ref());
    match extra {
        None => ui.set_extra_balance("--".into()),
        Some(e) => {
            let text = match e.state {
                ExtraState::Ready => e
                    .balance_cents
                    .map(fmt_yuan)
                    .unwrap_or_else(|| "--".to_string()),
                ExtraState::NoData => "No data".to_string(),
                ExtraState::NotActivated => "Not activated".to_string(),
            };
            ui.set_extra_balance(text.into());
            let monthly = e.monthly_enabled
                && e.monthly_limit_cents.is_some_and(|limit| limit > 0)
                && e.monthly_used_cents.is_some();
            ui.set_extra_monthly(monthly);
            if monthly {
                let used = e.monthly_used_cents.unwrap_or(0);
                let limit = e.monthly_limit_cents.unwrap_or(1);
                ui.set_extra_pct(clamp_percent(used as f64 / limit as f64 * 100.0) as f32);
                ui.set_extra_note(
                    format!(
                        "Used {} this month / {} limit",
                        fmt_yuan(used),
                        fmt_yuan(limit)
                    )
                    .into(),
                );
            }
        }
    }
}

fn set_segment(
    segment: Option<&QuotaSegment>,
    now: DateTime<Local>,
    mut set: impl FnMut(String, f32, String),
) {
    match segment {
        None => set("--".to_string(), 0.0, String::new()),
        Some(s) => set(
            // The unit lives next to the number in the markup, so only the
            // rounded value is written here.
            (s.percent.round() as i64).to_string(),
            clamp_percent(s.percent) as f32,
            s.reset_at
                .map(|at| format_reset(at, now))
                .unwrap_or_default(),
        ),
    }
}

// ---- Kimi local cache / decode figures (PLAN §5.1) ----

fn apply_usage(ui: &Panel, usage: Option<&KimiUsage>) {
    let week = usage.map(|u| &u.week);
    let today = usage.map(|u| &u.today);
    let week_rate = week.and_then(|w| w.hit_rate);
    let today_rate = today.and_then(|w| w.hit_rate);
    let has_cache = usage.is_some() && (week_rate.is_some() || today_rate.is_some());
    ui.set_has_cache(has_cache);
    if has_cache {
        let (value, pct, note, bad) = cache_cell(week_rate, week);
        ui.set_cache_week(value.into());
        ui.set_cache_week_pct(pct);
        ui.set_cache_week_note(note.into());
        ui.set_cache_week_bad(bad);
        let (value, pct, note, bad) = cache_cell(today_rate, today);
        ui.set_cache_today(value.into());
        ui.set_cache_today_pct(pct);
        ui.set_cache_today_note(note.into());
        ui.set_cache_today_bad(bad);
    }

    let tps = week.and_then(|w| w.decode_tps);
    ui.set_has_speed(has_cache && tps.is_some());
    if has_cache {
        if let (Some(tps), Some(w)) = (tps, week) {
            ui.set_speed_value(format!("{tps:.1} tok/s").into());
            let calls = fmt_count(w.calls as i64);
            let note = match w.first_token_ms {
                Some(first) => {
                    format!("First token {:.2} s · {calls} calls", first as f64 / 1000.0)
                }
                None => format!("{calls} calls"),
            };
            ui.set_speed_note(note.into());
        }
    }
}

fn cache_cell(rate: Option<f64>, window: Option<&CacheWindow>) -> (String, f32, String, bool) {
    let requests = window.map(|w| w.requests).unwrap_or(0);
    let note = if requests > 0 {
        format!("{} requests", fmt_count(requests as i64))
    } else {
        "No requests".to_string()
    };
    match rate {
        None => ("--".to_string(), 0.0, note, false),
        Some(rate) => (
            fmt_pct(Some(rate)).unwrap_or_else(|| "--".to_string()),
            clamp_percent(rate) as f32,
            note,
            is_low_cache_hit(Some(rate)),
        ),
    }
}

// ---- DeepSeek balance + usage (PLAN §3.1 / §5.2) ----

fn apply_deepseek(ui: &Panel, result: Option<&DeepseekResult>) {
    let balance = result.and_then(|r| r.balance.as_ref());
    if let Some(b) = balance {
        ui.set_ds_total(fmt_money(b.total, &b.currency).into());
        ui.set_ds_total_bad(is_low_balance(b.total, b.is_available));
        ui.set_ds_status(
            if b.is_available {
                "Available"
            } else {
                "Insufficient balance"
            }
            .into(),
        );
        ui.set_ds_status_bad(!b.is_available);
        ui.set_ds_detail(
            format!(
                "Granted {} · Topped up {}",
                fmt_money(b.granted, &b.currency),
                fmt_money(b.topped_up, &b.currency)
            )
            .into(),
        );
    } else {
        let code = result.and_then(|r| r.balance_error.as_deref());
        ui.set_ds_total("--".into());
        ui.set_ds_total_bad(false);
        ui.set_ds_status(balance_status_text(code).into());
        ui.set_ds_status_bad(code.is_some());
        ui.set_ds_detail(
            if code == Some(NO_API_KEY) {
                "API key not set"
            } else {
                ""
            }
            .into(),
        );
    }

    let currency = balance
        .map(|b| b.currency.clone())
        .unwrap_or_else(|| "CNY".to_string());
    // Balance and usage fail independently: the model list keeps its own hint.
    ui.set_ds_hint("".into());
    ui.set_ds_hint_bad(false);
    ui.set_ds_has_summary(false);
    ui.set_models(ModelRc::new(VecModel::from(Vec::<ModelRow>::new())));
    match result.and_then(|r| r.usage.as_ref()) {
        Some(usage) => apply_models(ui, usage, &currency),
        None => {
            if let Some(code) = result.and_then(|r| r.usage_error.as_deref()) {
                ui.set_ds_hint(usage_hint_text(code).into());
                ui.set_ds_hint_bad(true);
            }
        }
    }
}

fn balance_status_text(code: Option<&str>) -> &'static str {
    match code {
        None => "",
        Some(NO_API_KEY) => "Not configured",
        Some("key-invalid") => "Key invalid",
        Some("rate-limited") => "Rate limited",
        Some(_) => "Fetch failed",
    }
}

fn usage_hint_text(code: &str) -> &'static str {
    match code {
        // No "balance only" suffix: the balance block above already says
        // whether it worked (PLAN §5.2).
        "no-user-token" => "Usage token not configured",
        "token-expired" => "Usage token expired — sign in again to sync",
        "rate-limited" => "Usage was rate limited — retrying in 30s",
        _ => "Usage fetch failed — retrying in 30s",
    }
}

fn apply_models(ui: &Panel, usage: &UsageInfo, currency: &str) {
    let rows = &usage.by_model;
    if rows.is_empty() {
        // Not a failure: a quiet month draws no rows and no red.
        ui.set_ds_hint("No usage data this month".into());
        ui.set_ds_hint_bad(false);
        return;
    }
    // The bar compares models with each other by tokens, not by money.
    let max_tokens = rows
        .iter()
        .map(|m| m.month_tokens)
        .max()
        .unwrap_or(0)
        .max(1);
    let models: Vec<ModelRow> = rows
        .iter()
        .map(|m| ModelRow {
            name: if m.display_name.is_empty() {
                m.model.clone()
            } else {
                m.display_name.clone()
            }
            .into(),
            cost: fmt_money(m.month_cost, currency).into(),
            share: (m.month_tokens as f64 / max_tokens as f64) as f32,
            tokens: fmt_tokens(m.month_tokens).into(),
            hit: fmt_hit_rate(m.cache_hit_rate).into(),
            hit_bad: is_low_cache_hit(m.cache_hit_rate),
            today: format!("today {}", fmt_money(m.today_cost, currency)).into(),
        })
        .collect();
    ui.set_models(ModelRc::new(VecModel::from(models)));
    ui.set_ds_has_summary(true);
    ui.set_ds_sum(
        format!(
            "This month {} · {} tok",
            fmt_money(usage.month_cost, currency),
            fmt_tokens(usage.month_tokens)
        )
        .into(),
    );
    ui.set_ds_sum_today(format!("today {}", fmt_money(usage.today_cost, currency)).into());
}

// ---- CLI version ----

fn apply_update(ui: &Panel, update: &UpdateStatus) {
    let version = update
        .local_version
        .clone()
        .unwrap_or_else(|| "Not detected".to_string());
    ui.set_cli_version(version.into());
    ui.set_update_badge(update.update_available);
}

// ---- theme ----

/// Switch every window's token instance. Slint globals are per-component, so
/// each window owns a `Tokens` instance even though the file is shared.
pub fn apply_dark(
    panel: &Panel,
    settings: &crate::SettingsWindow,
    skills: &crate::SkillsWindow,
    menu: &crate::MenuWindow,
    dark: bool,
) {
    panel.global::<Tokens>().set_dark(dark);
    settings.global::<Tokens>().set_dark(dark);
    skills.global::<Tokens>().set_dark(dark);
    menu.global::<Tokens>().set_dark(dark);
    panel.set_dark(dark);
    settings.set_dark(dark);
    skills.set_dark(dark);
    menu.set_dark(dark);
}

/// Re-apply the effective theme to every window (settings Save, system theme
/// change). Nothing else in the panel depends on the theme, so no re-render.
pub fn apply_theme(shell: &std::rc::Rc<std::cell::RefCell<crate::shell::Shell>>) {
    let s = shell.borrow();
    let dark = s.data.dark();
    apply_dark(&s.ui, &s.settings_ui, &s.skills_ui, &s.menu_ui, dark);
}

// ---- Header stamp: it belongs to whichever provider tab is on screen ----

fn apply_stamps(ui: &Panel, data: &Data, now: DateTime<Local>) {
    let (kimi_text, kimi_bad) = match data.quota.as_ref() {
        None => (String::new(), false),
        Some(q) => match q.error.as_deref() {
            // Never configured: configuration, not a fault — red is reserved for
            // failures (PLAN §3.5 / §5.2).
            Some(NO_TOKEN) => ("Not configured".to_string(), false), // "Fetch failed", not "Update failed": this is the quota fetch; the
            // CLI version check is a separate path (PLAN §5.3).
            Some(_) => ("Fetch failed".to_string(), true),
            None => (fmt_updated(q.fetched_at, now), false),
        },
    };
    ui.set_stamp_kimi(kimi_text.as_str().into());
    ui.set_stamp_kimi_bad(kimi_bad);

    let (ds_text, ds_bad) = match data.deepseek.as_ref() {
        None => (String::new(), false),
        Some(r) => match (r.balance_error.as_deref(), r.usage_error.as_deref()) {
            // Nothing works without a key, so this is configuration too.
            (Some(NO_API_KEY), _) => ("Not configured".to_string(), false),
            // A missing usage token leaves the balance live: the stamp keeps the
            // honest timestamp and the usage block carries its own hint.
            (None, Some(NO_USER_TOKEN)) => (fmt_updated(r.fetched_at, now), false),
            (None, None) => (fmt_updated(r.fetched_at, now), false),
            _ => ("Fetch failed".to_string(), true),
        },
    };
    ui.set_stamp_ds(ds_text.as_str().into());
    ui.set_stamp_ds_bad(ds_bad);
}
