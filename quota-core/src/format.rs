// Formatting shared by both shells (HANDOFF §4 / §6.3): the strings and
// thresholds that used to live only in `src/common.ts`. The Slint shell has no
// TypeScript, so a second UI must not re-derive them. Every function has a test
// pinning the shipped behaviour; while both shells exist, a change here has to
// be mirrored in `src/common.ts` (the Tauri frontend still renders from it).

use crate::deepseek::DeepseekResult;
use crate::quota::{QuotaResult, QuotaSegment};
use chrono::{DateTime, Datelike, Local, Timelike};

/// Balance under this many yuan is a failure, not a number (PLAN §5.2).
pub const LOW_BALANCE_YUAN: f64 = 10.0;
/// Cache hit rate under this percentage is a failure (PLAN §5.2).
pub const LOW_CACHE_HIT: f64 = 60.0;

/// FmtYuan: Kimi money is cents, always two decimals — the panel never shows
/// `¥10` or `¥8.2` next to `¥49.52` (user decision 2026-11-14).
pub fn fmt_yuan(cents: i64) -> String {
    let (sign, cents) = if cents < 0 {
        ("-", cents.unsigned_abs())
    } else {
        ("", cents as u64)
    };
    format!("{sign}¥{}.{:02}", cents / 100, cents % 100)
}

/// FmtMoney: DeepSeek money is yuan (not cents) — always two decimals, same
/// rule as `fmt_yuan` (user decision 2026-11-14). `$` for USD, `¥` otherwise.
pub fn fmt_money(yuan: f64, currency: &str) -> String {
    let symbol = if currency == "USD" { "$" } else { "¥" };
    format!("{symbol}{yuan:.2}")
}

/// FmtTokens: `1.33M` / `284K` / `96` — two decimals at most, no trailing
/// zeros.
pub fn fmt_tokens(tokens: i64) -> String {
    if tokens >= 1_000_000 {
        format!(
            "{}M",
            trim_zeros(&format!("{:.2}", tokens as f64 / 1_000_000.0))
        )
    } else if tokens >= 1_000 {
        format!(
            "{}K",
            trim_zeros(&format!("{:.1}", tokens as f64 / 1_000.0))
        )
    } else {
        tokens.to_string()
    }
}

/// `cache 80%` (one decimal, no trailing zero); `cache --` when unknown.
pub fn fmt_hit_rate(rate: Option<f64>) -> String {
    match rate {
        None => "cache --".to_string(),
        Some(rate) => format!("cache {}%", one_decimal(rate)),
    }
}

/// One decimal, trailing `.0` dropped: `99.4`, `100`; `None` stays `None`.
pub fn fmt_pct(value: Option<f64>) -> Option<String> {
    value.map(one_decimal)
}

/// Thousands separator for request / call counts: `3,097`.
pub fn fmt_count(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if value < 0 {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `formatReset`: span = at - now, English countdown text.
pub fn format_reset(at: DateTime<Local>, now: DateTime<Local>) -> String {
    let span = at.signed_duration_since(now);
    if span < chrono::Duration::zero() {
        return "Resets soon".to_string();
    }
    let total_sec = span.num_milliseconds() / 1000;
    let days = total_sec / 86_400;
    if days >= 1 {
        let hours = (total_sec / 3_600) % 24;
        return format!("Resets in {days}d {hours}h");
    }
    let total_hours = total_sec / 3_600;
    if total_hours >= 1 {
        let minutes = (total_sec / 60) % 60;
        return format!("Resets in {total_hours}h {minutes}m");
    }
    let minutes = total_sec / 60;
    format!("Resets in {}m", minutes.max(1))
}

/// Panel stamp for a fetch time: `Updated HH:MM` in local time, prefixed
/// `M/D ` when the fetch happened on an earlier local day — a cache reused
/// across a restart must not read as if it were fetched today (PLAN §5.2).
pub fn fmt_updated(at: DateTime<Local>, now: DateTime<Local>) -> String {
    let day = if at.date_naive() == now.date_naive() {
        String::new()
    } else {
        format!("{}/{} ", at.month(), at.day())
    };
    format!("Updated {day}{:02}:{:02}", at.hour(), at.minute())
}

pub fn clamp_percent(percent: f64) -> f64 {
    percent.clamp(0.0, 100.0)
}

/// PLAN §5.2: an unavailable or nearly empty balance is red.
pub fn is_low_balance(total: f64, is_available: bool) -> bool {
    !is_available || total < LOW_BALANCE_YUAN
}

/// PLAN §5.2: a cache hit rate under 60% is red — low means wasted cache.
pub fn is_low_cache_hit(rate: Option<f64>) -> bool {
    rate.is_some_and(|rate| rate < LOW_CACHE_HIT)
}

/// Tray tooltip (PLAN §5.3): product name, then one line per provider. Each
/// line carries its own failure suffix, so a dead Kimi fetch never hides a
/// healthy DeepSeek balance. Both pollers rewrite the same string, which is why
/// both providers are read here together.
pub fn tray_tooltip(kimi: Option<&QuotaResult>, deepseek: Option<&DeepseekResult>) -> String {
    format!(
        "Quota Tray\n{}\n{}",
        kimi_tooltip_line(kimi),
        deepseek_tooltip_line(deepseek)
    )
}

fn kimi_tooltip_line(last: Option<&QuotaResult>) -> String {
    match last {
        None => "Kimi --".to_string(),
        Some(l) => {
            let mut text = format!(
                "Kimi 5h {} · week {}",
                segment_percent(&l.five_hour),
                segment_percent(&l.week)
            );
            if l.error.is_some() {
                text.push_str(" (fetch failed)");
            }
            text
        }
    }
}

fn deepseek_tooltip_line(last: Option<&DeepseekResult>) -> String {
    match last {
        None => "DeepSeek --".to_string(),
        Some(r) => {
            let mut text = match &r.balance {
                Some(b) => format!("DeepSeek {}", fmt_money(b.total, &b.currency)),
                None => "DeepSeek --".to_string(),
            };
            if r.has_error() {
                text.push_str(" (fetch failed)");
            }
            text
        }
    }
}

fn segment_percent(seg: &Option<QuotaSegment>) -> String {
    match seg {
        Some(s) => format!("{}%", s.percent.round() as i64),
        None => "?".to_string(),
    }
}

/// `toFixed(1)` with a trailing `.0` dropped.
fn one_decimal(value: f64) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").unwrap_or(&text).to_string()
}

/// Trim the trailing zeros of a fixed-point string (`1.00` -> `1`, `1.30` ->
/// `1.3`); integers are left alone.
fn trim_zeros(text: &str) -> String {
    if !text.contains('.') {
        return text.to_string();
    }
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek::{BalanceInfo, DeepseekResult};
    use crate::quota::{ExtraState, QuotaResult};
    use chrono::TimeZone;

    fn local(y: i32, m: u32, d: u32, h: u32, mi: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, 0)
            .single()
            .expect("unambiguous local time")
    }

    fn quota(percent: f64, error: Option<&str>) -> QuotaResult {
        QuotaResult {
            five_hour: Some(QuotaSegment {
                percent,
                reset_at: None,
            }),
            week: Some(QuotaSegment {
                percent: percent / 2.0,
                reset_at: None,
            }),
            extra: Some(crate::quota::ExtraInfo {
                state: ExtraState::Ready,
                balance_cents: Some(1000),
                monthly_enabled: false,
                monthly_used_cents: None,
                monthly_limit_cents: None,
            }),
            fetched_at: local(2026, 11, 15, 10, 30),
            error: error.map(str::to_string),
        }
    }

    fn deepseek(total: f64, currency: &str, error: Option<&str>) -> DeepseekResult {
        DeepseekResult {
            balance: Some(BalanceInfo {
                is_available: true,
                currency: currency.to_string(),
                total,
                granted: 0.0,
                topped_up: 0.0,
            }),
            usage: None,
            balance_error: error.map(str::to_string),
            usage_error: None,
            fetched_at: local(2026, 11, 15, 10, 30),
        }
    }

    // Two decimals always, and the negative form keeps the sign outside the
    // symbol (same rule as common.ts).
    #[test]
    fn yuan_is_cents_with_two_decimals() {
        assert_eq!(fmt_yuan(0), "¥0.00");
        assert_eq!(fmt_yuan(5), "¥0.05");
        assert_eq!(fmt_yuan(100), "¥1.00");
        assert_eq!(fmt_yuan(1050), "¥10.50");
        assert_eq!(fmt_yuan(4952), "¥49.52");
        assert_eq!(fmt_yuan(-150), "-¥1.50");
    }

    // DeepSeek payloads are yuan, not cents; the tray uses this too, so a
    // DeepSeek-only refresh cannot print another format (PLAN §5.3).
    #[test]
    fn money_is_yuan_with_two_decimals() {
        assert_eq!(fmt_money(45.44, "CNY"), "¥45.44");
        assert_eq!(fmt_money(0.0, "CNY"), "¥0.00");
        assert_eq!(fmt_money(3.0, "CNY"), "¥3.00");
        assert_eq!(fmt_money(1.5, "USD"), "$1.50");
    }

    #[test]
    fn tokens_switch_unit_and_drop_trailing_zeros() {
        assert_eq!(fmt_tokens(0), "0");
        assert_eq!(fmt_tokens(96), "96");
        assert_eq!(fmt_tokens(999), "999");
        assert_eq!(fmt_tokens(1000), "1K");
        assert_eq!(fmt_tokens(284_000), "284K");
        assert_eq!(fmt_tokens(1_330_000), "1.33M");
        assert_eq!(fmt_tokens(1_000_000), "1M");
    }

    #[test]
    fn hit_rate_and_percent_share_one_decimal() {
        assert_eq!(fmt_hit_rate(None), "cache --");
        assert_eq!(fmt_hit_rate(Some(80.0)), "cache 80%");
        assert_eq!(fmt_hit_rate(Some(99.5)), "cache 99.5%");
        assert_eq!(fmt_pct(None), None);
        assert_eq!(fmt_pct(Some(100.0)).as_deref(), Some("100"));
        assert_eq!(fmt_pct(Some(99.44)).as_deref(), Some("99.4"));
    }

    #[test]
    fn counts_group_thousands() {
        assert_eq!(fmt_count(0), "0");
        assert_eq!(fmt_count(96), "96");
        assert_eq!(fmt_count(3097), "3,097");
        assert_eq!(fmt_count(1_234_567), "1,234,567");
        assert_eq!(fmt_count(-3097), "-3,097");
    }

    #[test]
    fn reset_counts_down_in_days_hours_minutes() {
        let now = local(2026, 11, 15, 10, 0);
        assert_eq!(
            format_reset(local(2026, 11, 15, 10, 0), now),
            "Resets in 1m",
            "a span under a minute still reads as one minute"
        );
        assert_eq!(
            format_reset(local(2026, 11, 15, 10, 42), now),
            "Resets in 42m"
        );
        assert_eq!(
            format_reset(local(2026, 11, 15, 12, 5), now),
            "Resets in 2h 5m"
        );
        assert_eq!(
            format_reset(local(2026, 11, 17, 13, 30), now),
            "Resets in 2d 3h"
        );
        assert_eq!(
            format_reset(local(2026, 11, 15, 9, 59), now),
            "Resets soon",
            "a past reset time is not a countdown"
        );
    }

    #[test]
    fn stamp_carries_the_day_only_when_it_is_not_today() {
        let now = local(2026, 11, 15, 10, 0);
        assert_eq!(fmt_updated(local(2026, 11, 15, 9, 5), now), "Updated 09:05");
        assert_eq!(
            fmt_updated(local(2026, 11, 14, 23, 59), now),
            "Updated 11/14 23:59",
            "yesterday's numbers must not read as if fetched today"
        );
    }

    #[test]
    fn thresholds_match_the_frontend() {
        assert!(is_low_balance(9.99, true));
        assert!(!is_low_balance(10.0, true));
        assert!(
            is_low_balance(500.0, false),
            "unavailable is always a failure"
        );
        assert!(is_low_cache_hit(Some(59.9)));
        assert!(!is_low_cache_hit(Some(60.0)));
        assert!(!is_low_cache_hit(None), "unknown is not a failure");
        assert_eq!(clamp_percent(-5.0), 0.0);
        assert_eq!(clamp_percent(120.0), 100.0);
        assert_eq!(clamp_percent(42.5), 42.5);
    }

    // PLAN §5.3: three lines, one per provider, each with its own suffix.
    #[test]
    fn tray_tooltip_is_three_lines_with_per_provider_failures() {
        assert_eq!(tray_tooltip(None, None), "Quota Tray\nKimi --\nDeepSeek --");
        assert_eq!(
            tray_tooltip(
                Some(&quota(31.4, None)),
                Some(&deepseek(45.44, "CNY", None))
            ),
            "Quota Tray\nKimi 5h 31% · week 16%\nDeepSeek ¥45.44"
        );
        assert_eq!(
            tray_tooltip(
                Some(&quota(31.0, Some("HttpRequestException"))),
                Some(&deepseek(45.44, "CNY", Some("token-expired")))
            ),
            "Quota Tray\nKimi 5h 31% · week 16% (fetch failed)\nDeepSeek ¥45.44 (fetch failed)"
        );
        assert_eq!(
            tray_tooltip(Some(&quota(0.0, None)), None),
            "Quota Tray\nKimi 5h 0% · week 0%\nDeepSeek --"
        );
    }
}
