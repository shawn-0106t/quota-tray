// Quota fetch + defensive parsing, 1:1 port of QuotaService (SPEC section 16).
// Traps honored here:
//  - server JSON numbers are modeled as strings, numeric fallback tolerated
//  - Extra Usage amountLeft unit is 1e-8 yuan -> cents = (raw + 500000) / 1000000
//  - isEnabled == false must be reported as NotActivated (KimiCodeBar v1.1.1 bug)

use chrono::{DateTime, Local};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

use crate::credentials;

const USAGES_URL: &str = "https://api.kimi.com/coding/v1/usages";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaSegment {
    pub percent: f64,
    pub reset_at: Option<DateTime<Local>>,
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
pub enum ExtraState {
    NotActivated,
    NoData,
    Ready,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraInfo {
    pub state: ExtraState,
    pub balance_cents: Option<i64>,
    pub monthly_enabled: bool,
    pub monthly_used_cents: Option<i64>,
    pub monthly_limit_cents: Option<i64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaResult {
    pub five_hour: Option<QuotaSegment>,
    pub week: Option<QuotaSegment>,
    pub extra: Option<ExtraInfo>,
    pub fetched_at: DateTime<Local>,
    pub error: Option<String>,
}

impl QuotaResult {
    fn failed(kind: &str) -> Self {
        QuotaResult {
            five_hour: None,
            week: None,
            extra: None,
            fetched_at: Local::now(),
            error: Some(kind.to_string()),
        }
    }

    /// On failure keep last-known-good data: fill null fields from `last`
    /// (SPEC 16.5 step 2; the UI only shows the failure hint in the status line).
    pub fn fill_missing_from(&mut self, last: &QuotaResult) {
        let mut refilled = 0;
        if self.five_hour.is_none() {
            self.five_hour = last.five_hour.clone();
            refilled += 1;
        }
        if self.week.is_none() {
            self.week = last.week.clone();
            refilled += 1;
        }
        if self.extra.is_none() {
            self.extra = last.extra.clone();
            refilled += 1;
        }
        // Nothing was fetched: keep the last good timestamp with the data, so a
        // failure cannot make yesterday's numbers look freshly fetched (PLAN
        // §3.1 / §5.2).
        if refilled == 3 {
            self.fetched_at = last.fetched_at;
        }
    }
}

fn http_client() -> Option<&'static Client> {
    static CLIENT: OnceLock<Option<Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .ok()
        })
        .as_ref()
}

pub async fn fetch() -> QuotaResult {
    let Some(token) = credentials::load_token() else {
        return QuotaResult::failed("no-token");
    };
    let Some(client) = http_client() else {
        return QuotaResult::failed("HttpRequestException");
    };
    let resp = match client
        .get(USAGES_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            // Keep .NET exception-type names so --test-fetch output matches the WPF reference
            return QuotaResult::failed(if e.is_timeout() {
                "TaskCanceledException"
            } else {
                "HttpRequestException"
            });
        }
    };
    if !resp.status().is_success() {
        return QuotaResult::failed("HttpRequestException");
    }
    let root: Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return QuotaResult::failed("JsonException"),
    };

    QuotaResult {
        five_hour: parse_five_hour(&root),
        week: parse_week(&root),
        extra: Some(parse_extra_from_root(&root)),
        fetched_at: Local::now(),
        error: None,
    }
}

/// 5-hour window: `limits[0].detail` (PLAN §5.1).
fn parse_five_hour(root: &Value) -> Option<QuotaSegment> {
    root.get("limits")
        .and_then(|l| l.as_array())
        .and_then(|a| a.first())
        .and_then(|first| first.get("detail"))
        .filter(|d| d.is_object())
        .map(parse_segment)
}

/// Weekly window. The percentage has three possible sources, in order of
/// authority (PLAN §5.1; payload shape verified 2026-11-19):
///  1. `usages.limit_7d.used_ratio` — a fraction (0.0013 = 0.13 %), the
///     console's own number;
///  2. `usage.used` / `usage.limit` — the older shape;
///  3. `usage.limit - usage.remaining` — the live shape, integers, so it cannot
///     show sub-1 % detail.
///
/// `usage` also carries `resetTime`; a payload with only the ratio still yields
/// a segment (without a reset time) rather than dropping the card.
fn parse_week(root: &Value) -> Option<QuotaSegment> {
    let ratio = root
        .get("usages")
        .and_then(|v| v.get("limit_7d"))
        .and_then(|v| v.get("used_ratio"))
        .and_then(number)
        .filter(|r| r.is_finite())
        .map(|r| r * 100.0);
    match (root.get("usage").filter(|u| u.is_object()), ratio) {
        (Some(usage), ratio) => {
            let mut seg = parse_segment(usage);
            if let Some(percent) = ratio {
                seg.percent = percent;
            }
            Some(seg)
        }
        (None, Some(percent)) => Some(QuotaSegment {
            percent,
            reset_at: None,
        }),
        (None, None) => None,
    }
}

/// Extra Usage. The live payload keys the wallet `booster_wallet` (proto
/// snake_case at the top level); `boosterWallet` is the shape every document
/// used before 2026-11-19 and is kept so an older payload still parses.
fn parse_extra_from_root(root: &Value) -> ExtraInfo {
    let wallet = root
        .get("booster_wallet")
        .or_else(|| root.get("boosterWallet"));
    parse_extra(wallet)
}

/// JSON number-or-string -> f64, `None` when the value is absent or unusable.
fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// JSON number-or-string -> f64, missing -> 0.
fn get_f64(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(number).unwrap_or(0.0)
}

/// JSON number-or-string -> i64.
fn get_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

/// resetTime: RFC3339 first, then the looser shapes DateTimeOffset.TryParse
/// accepts (space separator, with/without offset; no offset = local time).
fn parse_reset_time(s: &str) -> Option<DateTime<Local>> {
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Local));
    }
    for fmt in [
        "%Y-%m-%d %H:%M:%S %:z",
        "%Y-%m-%d %H:%M:%S %z",
        "%Y-%m-%dT%H:%M:%S %:z",
        "%Y-%m-%d %H:%M:%S%.f %:z",
    ] {
        if let Ok(d) = DateTime::parse_from_str(s, fmt) {
            return Some(d.with_timezone(&Local));
        }
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            if let Some(d) = n.and_local_timezone(Local).single() {
                return Some(d);
            }
        }
    }
    None
}

fn parse_segment(v: &Value) -> QuotaSegment {
    // `used` when the payload has it; otherwise the live shape's integer pair.
    // Without this the weekly bar reads 0 forever — see `parse_week`.
    let used = if v.get("used").is_some() {
        get_f64(v, "used")
    } else {
        (get_f64(v, "limit") - get_f64(v, "remaining")).max(0.0)
    };
    let mut limit = get_f64(v, "limit");
    if limit <= 0.0 {
        limit = 1.0; // guard against division by zero
    }
    let reset_at = v
        .get("resetTime")
        .and_then(|x| x.as_str())
        .and_then(parse_reset_time);
    // get_f64 can yield inf/NaN from hostile strings ("1e999", "NaN"); keep
    // percent finite so serialization never emits a non-finite double
    let percent = used / limit * 100.0;
    QuotaSegment {
        percent: if percent.is_finite() { percent } else { 0.0 },
        reset_at,
    }
}

fn parse_cents(money: Option<&Value>) -> Option<i64> {
    money
        .filter(|m| m.is_object())
        .and_then(|m| m.get("priceInCents"))
        .and_then(get_i64)
}

fn parse_extra(wallet: Option<&Value>) -> ExtraInfo {
    let mut info = ExtraInfo {
        state: ExtraState::NotActivated,
        balance_cents: None,
        monthly_enabled: false,
        monthly_used_cents: None,
        monthly_limit_cents: None,
    };
    let Some(w) = wallet.filter(|w| w.is_object()) else {
        return info; // not an object -> NotActivated
    };

    // Not-activated defence, two shapes. The documented one is `isEnabled:
    // false`, where amountLeft is an estimate (monthly limit minus used) and NOT
    // the real balance. The live wallet (2026-11-19) has no `isEnabled` at all
    // and carries `status` instead, so anything but STATUS_ACTIVE must be read
    // as not activated too — otherwise a paused booster's estimate would be
    // printed as money.
    if let Some(enabled) = w.get("isEnabled").and_then(|v| v.as_bool()) {
        if !enabled {
            return info;
        }
    } else if let Some(status) = w.get("status").and_then(|v| v.as_str()) {
        if !status.eq_ignore_ascii_case("STATUS_ACTIVE") {
            return info;
        }
    }

    if let Some(raw) = w
        .get("balance")
        .filter(|b| b.is_object())
        .and_then(|b| b.get("amountLeft"))
        .and_then(get_i64)
    {
        info.state = ExtraState::Ready;
        // saturating_add: a pathological amountLeft near i64::MAX must not overflow
        info.balance_cents = Some(raw.saturating_add(500_000) / 1_000_000); // 1e-8 yuan -> cents, rounded
    } else {
        info.state = ExtraState::NoData;
    }

    if w.get("monthlyChargeLimitEnabled").and_then(|v| v.as_bool()) == Some(true) {
        info.monthly_enabled = true;
        info.monthly_used_cents = parse_cents(w.get("monthlyUsed"));
        info.monthly_limit_cents = parse_cents(w.get("monthlyChargeLimit"));
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Shape of the live payload (2026-11-19) with placeholder numbers: the
    // top-level key is snake_case while the wallet's own fields are camelCase.
    #[test]
    fn extra_reads_the_live_snake_case_wallet() {
        let root = json!({
            "booster_wallet": {
                "status": "STATUS_ACTIVE",
                "balance": { "amountLeft": "14821300" },
                "monthlyChargeLimitEnabled": true,
                "monthlyUsed": { "priceInCents": "0" },
                "monthlyChargeLimit": { "priceInCents": "10000" }
            }
        });
        let extra = parse_extra_from_root(&root);
        assert!(matches!(extra.state, ExtraState::Ready));
        // 1e-8 yuan -> cents, rounded: (14_821_300 + 500_000) / 1_000_000 = 15
        assert_eq!(extra.balance_cents, Some(15));
        assert!(extra.monthly_enabled);
        assert_eq!(extra.monthly_used_cents, Some(0));
        assert_eq!(extra.monthly_limit_cents, Some(10000));
    }

    #[test]
    fn extra_still_accepts_the_camel_case_wallet() {
        let root = json!({
            "boosterWallet": { "balance": { "amountLeft": "100000000" } }
        });
        let extra = parse_extra_from_root(&root);
        assert!(matches!(extra.state, ExtraState::Ready));
        assert_eq!(extra.balance_cents, Some(100));
    }

    #[test]
    fn extra_is_not_activated_unless_the_wallet_is_active() {
        // The live shape: no isEnabled, a status that is not ACTIVE.
        let paused = json!({
            "booster_wallet": { "status": "STATUS_PAUSED", "balance": { "amountLeft": "1" } }
        });
        assert!(matches!(
            parse_extra_from_root(&paused).state,
            ExtraState::NotActivated
        ));
        // The documented shape: isEnabled=false means amountLeft is an estimate.
        let disabled = json!({
            "boosterWallet": { "isEnabled": false, "balance": { "amountLeft": "1" } }
        });
        assert!(matches!(
            parse_extra_from_root(&disabled).state,
            ExtraState::NotActivated
        ));
    }

    #[test]
    fn extra_without_a_wallet_is_not_activated() {
        assert!(matches!(
            parse_extra_from_root(&json!({})).state,
            ExtraState::NotActivated
        ));
    }

    #[test]
    fn week_percent_prefers_the_used_ratio() {
        let root = json!({
            "usage": { "limit": "100", "remaining": "100", "resetTime": "2026-11-26T20:11:15+08:00" },
            "usages": { "limit_7d": { "used_ratio": "0.0013" } }
        });
        let week = parse_week(&root).expect("the week card must survive");
        assert!((week.percent - 0.13).abs() < 1e-9, "got {}", week.percent);
        assert!(
            week.reset_at.is_some(),
            "resetTime still comes from `usage`"
        );
    }

    #[test]
    fn week_falls_back_to_limit_minus_remaining() {
        let root = json!({ "usage": { "limit": "40", "remaining": "30" } });
        let week = parse_week(&root).expect("week");
        assert!((week.percent - 25.0).abs() < 1e-9, "got {}", week.percent);
    }

    #[test]
    fn week_keeps_reading_used_when_the_payload_has_it() {
        let root = json!({ "usage": { "used": "12", "limit": "48" } });
        let week = parse_week(&root).expect("week");
        assert!((week.percent - 25.0).abs() < 1e-9);
    }

    #[test]
    fn week_survives_a_missing_usage_object() {
        let root = json!({ "usages": { "limit_7d": { "used_ratio": 0.4 } } });
        let week = parse_week(&root).expect("the ratio alone still yields a card");
        assert!((week.percent - 40.0).abs() < 1e-9);
        assert!(week.reset_at.is_none());
    }

    // The payload carries no window identifier: `limits[0]` is taken as the
    // 5-hour window (PLAN §5.1). This pins that, and proves a second entry is
    // ignored rather than averaged in.
    #[test]
    fn the_five_hour_window_is_the_first_limit_entry() {
        let root = json!({
            "limits": [
                { "detail": { "used": "1", "limit": "100" } },
                { "detail": { "used": "60", "limit": "100" } }
            ]
        });
        let seg = parse_five_hour(&root).expect("first entry");
        assert!((seg.percent - 1.0).abs() < 1e-9, "got {}", seg.percent);
    }

    #[test]
    fn hostile_numbers_never_emit_a_non_finite_percent() {
        let root = json!({ "usage": { "used": "1e999", "limit": "1" } });
        let week = parse_week(&root).expect("week");
        assert!(week.percent.is_finite(), "got {}", week.percent);
    }
}
