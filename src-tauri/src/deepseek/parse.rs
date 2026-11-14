// DeepSeek payload parsing, 1:1 port of DeepseekService.ParseBalance/ParseUsage.
// Traps honoured here (PLAN §5.2):
//  - data.biz_data is an object for /usage/amount and an array for /usage/cost
//  - every amount arrives as a string ("1000000.0"); numeric fallback tolerated
//  - REQUEST counts requests, never tokens or money
//  - money keeps its fraction, token counts are rounded (C# Math.Round ties to even)
//  - a model with a missing or empty usage[] still gets an all-zero entry
//  - unknown token types fall through to the token total
//  - a missing model id is reported as "unknown"

use crate::deepseek::{BalanceInfo, ModelUsage, UsageInfo};
use chrono::Local;
use serde_json::Value;
use std::collections::HashMap;

/// {"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"100.00"}]}
pub fn parse_balance(json: &str) -> Result<BalanceInfo, serde_json::Error> {
    let root: Value = serde_json::from_str(json)?;
    let mut info = BalanceInfo::default();
    if root.get("is_available").and_then(Value::as_bool) == Some(true) {
        info.is_available = true;
    }
    // Only the first currency block matters: the panel shows one balance.
    if let Some(b) = root
        .get("balance_infos")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .filter(|v| v.is_object())
    {
        if let Some(currency) = b.get("currency").and_then(Value::as_str) {
            info.currency = currency.to_string();
        }
        info.total = num(b.get("total_balance"));
        info.granted = num(b.get("granted_balance"));
        info.topped_up = num(b.get("topped_up_balance"));
    }
    Ok(info)
}

/// `amount` and `cost` are two independent console responses that must be
/// merged: tokens carry no money and money carries no tokens.
pub fn parse_usage(amount_json: &str, cost_json: &str) -> Result<UsageInfo, serde_json::Error> {
    let amount_root: Value = serde_json::from_str(amount_json)?;
    let cost_root: Value = serde_json::from_str(cost_json)?;
    let amount_biz = biz(&amount_root);
    let cost_biz = biz(&cost_root);
    let today = Local::now().format("%Y-%m-%d").to_string();

    let mut models = Models::default();

    // Month tokens: total[] aggregated per model. hit/miss feed the hit rate.
    let mut hit_all: i64 = 0;
    let mut miss_all: i64 = 0;
    for (model, entries) in model_entries(amount_biz, "total") {
        let mut hit: i64 = 0;
        let mut miss: i64 = 0;
        for (token_type, amount) in entries {
            let n = tokens(amount);
            let m = models.entry(&model);
            match token_type.as_str() {
                "REQUEST" => m.month_requests += n,
                "PROMPT_CACHE_HIT_TOKEN" => {
                    m.month_tokens += n;
                    hit += n;
                }
                "PROMPT_CACHE_MISS_TOKEN" => {
                    m.month_tokens += n;
                    miss += n;
                }
                // RESPONSE_TOKEN / PROMPT_TOKEN / anything new: count as tokens
                _ => m.month_tokens += n,
            }
        }
        // A model with an empty usage[] is still reported, with no hit rate.
        let m = models.entry(&model);
        m.cache_hit_rate = if hit + miss > 0 {
            Some(hit as f64 * 100.0 / (hit + miss) as f64)
        } else {
            None
        };
        hit_all += hit;
        miss_all += miss;
    }

    // Today's tokens: only the days[] row matching the local date.
    for (model, entries) in day_entries(amount_biz, "days", &today) {
        for (token_type, amount) in entries {
            if token_type == "REQUEST" {
                continue;
            }
            models.entry(&model).today_tokens += tokens(amount);
        }
    }

    // Money (yuan, fraction preserved). cost responses carry REQUEST too, and
    // including it would inflate the month total by the request count.
    for (model, entries) in model_entries(cost_biz, "total") {
        for (token_type, amount) in entries {
            if token_type == "REQUEST" {
                continue;
            }
            models.entry(&model).month_cost += amount;
        }
    }
    for (model, entries) in day_entries(cost_biz, "days", &today) {
        for (token_type, amount) in entries {
            if token_type == "REQUEST" {
                continue;
            }
            models.entry(&model).today_cost += amount;
        }
    }

    let mut month_tokens = 0i64;
    let mut month_requests = 0i64;
    let mut month_cost_units = 0i64;
    let mut today_tokens = 0i64;
    let mut today_cost_units = 0i64;
    // The console reports money to 1e-8 yuan. Summing in f64 drifts in the
    // last bits (24.279870719999998 instead of 24.27987072), so each aggregate
    // is snapped to that quantum: the pane has to show the number the console
    // and the WPF version show. Models are snapped before the total, so the
    // total always equals the sum of the cards.
    for m in &mut models.list {
        m.month_cost = snap_money(m.month_cost);
        m.today_cost = snap_money(m.today_cost);
    }
    for m in &models.list {
        month_tokens += m.month_tokens;
        month_requests += m.month_requests;
        month_cost_units = month_cost_units.saturating_add(money_units(m.month_cost));
        today_tokens += m.today_tokens;
        today_cost_units = today_cost_units.saturating_add(money_units(m.today_cost));
    }

    Ok(UsageInfo {
        month_tokens,
        month_requests,
        month_cost: month_cost_units as f64 / 1e8,
        cache_hit_rate: if hit_all + miss_all > 0 {
            Some(hit_all as f64 * 100.0 / (hit_all + miss_all) as f64)
        } else {
            None
        },
        today_tokens,
        today_cost: today_cost_units as f64 / 1e8,
        by_model: models.list,
    })
}

/// Money quantum of the console payload: 8 decimal places of a yuan, matching
/// Kimi's `amountLeft` unit (PLAN §5.1). Snapping an f64 aggregate to it
/// reproduces C#'s decimal sum for payloads at that precision.
fn snap_money(amount: f64) -> f64 {
    if !amount.is_finite() {
        return 0.0;
    }
    money_units(amount) as f64 / 1e8
}

fn money_units(amount: f64) -> i64 {
    (amount * 1e8).round() as i64
}

/// Per-model accumulator that keeps first-seen order, so `by_model` reads the
/// same way as the C# Dictionary enumeration the parent UI was written against.
#[derive(Default)]
struct Models {
    list: Vec<ModelUsage>,
    index: HashMap<String, usize>,
}

impl Models {
    fn entry(&mut self, model: &str) -> &mut ModelUsage {
        let at = match self.index.get(model) {
            Some(at) => *at,
            None => {
                let at = self.list.len();
                self.list.push(ModelUsage {
                    model: model.to_string(),
                    display_name: base_display_name(model),
                    ..Default::default()
                });
                self.index.insert(model.to_string(), at);
                at
            }
        };
        &mut self.list[at]
    }
}

/// data.biz_data: an object for amount, a one-element array for cost. Both
/// shapes are tolerated on either endpoint (PLAN §5.2).
fn biz(root: &Value) -> Option<&Value> {
    let data = root.get("data").filter(|d| d.is_object())?;
    match data.get("biz_data") {
        Some(Value::Array(list)) => list.first(),
        Some(v @ Value::Object(_)) => Some(v),
        _ => None,
    }
}

/// Walk biz.<section>[]: each item is {model, usage:[{type, amount}]}.
fn model_entries(biz: Option<&Value>, section: &str) -> Vec<(String, Vec<(String, f64)>)> {
    let mut out = Vec::new();
    let Some(list) = biz.and_then(|b| b.get(section)).and_then(Value::as_array) else {
        return out;
    };
    for item in list {
        out.push((model_of(item), entries_of(item.get("usage"))));
    }
    out
}

/// Walk biz.days[] for the row whose date is `date`: {date, data:[{model, usage}]}.
fn day_entries(
    biz: Option<&Value>,
    section: &str,
    date: &str,
) -> Vec<(String, Vec<(String, f64)>)> {
    let mut out = Vec::new();
    let Some(days) = biz.and_then(|b| b.get(section)).and_then(Value::as_array) else {
        return out;
    };
    for day in days {
        if day.get("date").and_then(Value::as_str) != Some(date) {
            continue;
        }
        let Some(list) = day.get("data").and_then(Value::as_array) else {
            continue;
        };
        for item in list {
            out.push((model_of(item), entries_of(item.get("usage"))));
        }
    }
    out
}

fn model_of(item: &Value) -> String {
    item.get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

fn entries_of(usage: Option<&Value>) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    let Some(list) = usage.and_then(Value::as_array) else {
        return out;
    };
    for entry in list {
        let token_type = entry
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        out.push((token_type, num(entry.get("amount"))));
    }
    out
}

/// Number-or-string -> finite f64, anything else -> 0. The server always sends
/// a dot decimal, and Rust's float parser is locale-independent.
/// Non-finite input ("NaN", "inf") is rejected the way C#'s TryParse rejects it:
/// a NaN must never reach the UI as a null serialized double.
fn num(v: Option<&Value>) -> f64 {
    let parsed = match v {
        Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    };
    parsed.filter(|x| x.is_finite()).unwrap_or(0.0)
}

/// Token counts. `round_ties_even` is exactly .NET's default Math.Round
/// (MidpointRounding.ToEven); token counts never land on .5, but the port keeps
/// C# semantics so no payload can drift from the WPF reference.
/// `as i64` saturates instead of overflowing on a hostile huge value.
fn tokens(amount: f64) -> i64 {
    amount.round_ties_even() as i64
}

/// Ids known when this build shipped. This is only an offline bootstrap: an
/// official name from `GET api.deepseek.com/models` always wins, so a vendor
/// rename reaches the panel without a new release (PLAN §6.1).
fn static_display_name(model: &str) -> Option<&'static str> {
    match model {
        "deepseek-v4-pro" => Some("V4 Pro"),
        "deepseek-v4-flash" => Some("V4 Flash"),
        "deepseek-chat" => Some("Chat"),
        "deepseek-reasoner" => Some("Reasoner"),
        _ => None,
    }
}

/// Static table -> rule-derived -> raw id. `client::apply_display_names` puts
/// the official name in front of this once `/models` has been read.
/// Derivation only touches `deepseek-*` ids: aggregate console rows
/// (`A & B`) and foreign ids are display strings already, and mangling them
/// would be worse than showing the raw id (PLAN §6.1).
pub fn base_display_name(model: &str) -> String {
    if let Some(name) = static_display_name(model) {
        return name.to_string();
    }
    // A console aggregate row ("A & B") is already a display string; real ids
    // never contain whitespace.
    if model.contains(char::is_whitespace) {
        return model.to_string();
    }
    let Some(rest) = model.strip_prefix("deepseek-") else {
        return model.to_string();
    };
    let derived = derive_display_name(rest);
    if derived.is_empty() {
        model.to_string()
    } else {
        derived
    }
}

/// `v4-pro` -> `V4 Pro`, `flash` -> `Flash`, `chat-20250324` -> `Chat`.
fn derive_display_name(rest: &str) -> String {
    strip_revision_suffix(rest)
        .split('-')
        .filter(|part| !part.is_empty())
        .map(capitalize)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Drop trailing `-0` revision markers and date stamps (`-20250324`,
/// `-2025-03-24`): they carry no display value (PLAN §6.1).
fn strip_revision_suffix(s: &str) -> &str {
    let mut end = s.len();
    loop {
        let cur = &s[..end];
        let Some((head, tail)) = cur.rsplit_once('-') else {
            return cur;
        };
        if tail == "0" || is_date_stamp(tail) {
            end = head.len();
            continue;
        }
        // The dash-separated date form needs all three groups removed together.
        if tail.len() == 2 && is_digits(tail) {
            if let Some((h2, month)) = head.rsplit_once('-') {
                if month.len() == 2 && is_digits(month) {
                    if let Some((h3, year)) = h2.rsplit_once('-') {
                        if year.len() == 4 && is_digits(year) {
                            end = h3.len();
                            continue;
                        }
                    }
                }
            }
        }
        return cur;
    }
}

fn is_date_stamp(s: &str) -> bool {
    s.len() == 8 && is_digits(s)
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn capitalize(part: &str) -> String {
    let mut chars = part.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ported from deepseek-planbar/tray-wpf/FixtureTests.cs. The fixtures are
    // inert JSON under src-tauri/tests/fixtures/ (cargo only compiles
    // tests/*.rs), so include_str! keeps them inside the test binary.
    const BALANCE_OK: &str = include_str!("../../tests/fixtures/balance_ok.json");
    const AMOUNT_OK: &str = include_str!("../../tests/fixtures/amount_ok.json");
    const AMOUNT_SPARSE: &str = include_str!("../../tests/fixtures/amount_sparse.json");
    const COST_OK: &str = include_str!("../../tests/fixtures/cost_ok.json");

    /// Fixtures mark "today" with @@TODAY@@ so the day-aggregation assertions
    /// do not depend on when the suite runs.
    fn fixture(raw: &str) -> String {
        raw.replace("@@TODAY@@", &Local::now().format("%Y-%m-%d").to_string())
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn model<'a>(u: &'a UsageInfo, id: &str) -> &'a ModelUsage {
        u.by_model
            .iter()
            .find(|m| m.model == id)
            .unwrap_or_else(|| panic!("model {id} missing from by_model"))
    }

    #[test]
    fn balance_fixture() {
        let b = parse_balance(&fixture(BALANCE_OK)).expect("balance parses");
        assert!(b.is_available);
        assert_eq!(b.currency, "CNY");
        assert_eq!(b.total, 100.00);
        assert_eq!(b.granted, 50.00);
        assert_eq!(b.topped_up, 50.00);
    }

    #[test]
    fn usage_fixture() {
        let u = parse_usage(&fixture(AMOUNT_OK), &fixture(COST_OK)).expect("usage parses");
        assert_eq!(
            u.by_model.len(),
            2,
            "two models, the placeholder row is not invented"
        );

        let pro = model(&u, "deepseek-v4-pro");
        assert_eq!(pro.month_tokens, 1_330_000);
        assert_eq!(pro.month_requests, 1_200);
        assert!(close(pro.cache_hit_rate.expect("pro hit rate"), 80.0));
        assert_eq!(pro.today_tokens, 17_000);
        assert_eq!(pro.month_cost, 2.50, "REQUEST is not money");
        assert_eq!(pro.today_cost, 0.30);
        assert_eq!(pro.display_name, "V4 Pro");

        let flash = model(&u, "deepseek-v4-flash");
        assert_eq!(flash.month_tokens, 1_120_000);
        assert!(close(flash.cache_hit_rate.expect("flash hit rate"), 50.0));
        assert_eq!(flash.today_tokens, 9_500);
        assert_eq!(flash.month_cost, 0.50);
        assert_eq!(flash.display_name, "V4 Flash");

        assert_eq!(u.month_tokens, 2_450_000);
        assert_eq!(u.month_requests, 4_200);
        assert_eq!(u.month_cost, 3.00);
        assert_eq!(u.today_tokens, 26_500);
        assert_eq!(u.today_cost, 0.30);
        assert!(close(
            u.cache_hit_rate.expect("total hit rate"),
            200.0 / 3.0
        ));
    }

    #[test]
    fn usage_sparse_fixture_keeps_defaults() {
        let u = parse_usage(&fixture(AMOUNT_SPARSE), &fixture(COST_OK)).expect("sparse parses");
        let pro = model(&u, "deepseek-v4-pro");
        assert_eq!(pro.cache_hit_rate, None, "no usage[] means no hit rate");
        assert_eq!(
            model(&u, "deepseek-v4-flash").month_tokens,
            42,
            "a typeless entry still counts"
        );
        assert_eq!(u.today_tokens, 0, "no days[] means no today");
    }

    #[test]
    fn usage_tolerates_cost_object_shape() {
        // cost's biz_data degrades to an object: still two models, no panic.
        let u = parse_usage(&fixture(AMOUNT_OK), &fixture(AMOUNT_OK)).expect("object shape parses");
        assert_eq!(u.by_model.len(), 2);
    }

    #[test]
    fn money_aggregates_are_snapped_to_the_console_quantum() {
        // Two 8-decimal money entries. Summed raw, f64 drifts in the last bits;
        // the pane has to print the same number the console does.
        let cost = r#"{
            "data": { "biz_data": { "total": [
                { "model": "deepseek-chat", "usage": [
                    { "type": "RESPONSE_TOKEN", "amount": "24.27987071" },
                    { "type": "RESPONSE_TOKEN", "amount": "0.00000001" }
                ] }
            ] } }
        }"#;
        let u = parse_usage(cost, cost).expect("parses");
        assert_eq!(u.by_model[0].month_cost, 24.27987072);
        assert_eq!(
            u.month_cost, u.by_model[0].month_cost,
            "the total must equal the sum of the cards"
        );
    }

    #[test]
    fn non_deepseek_id_passes_through() {
        assert_eq!(base_display_name("some-other-model"), "some-other-model");
        assert_eq!(
            base_display_name("deepseek-chat & deepseek-reasoner"),
            "deepseek-chat & deepseek-reasoner",
            "aggregate console rows are display strings already"
        );
    }

    #[test]
    fn derived_display_names() {
        assert_eq!(base_display_name("deepseek-v4-pro"), "V4 Pro");
        assert_eq!(base_display_name("deepseek-flash"), "Flash");
        assert_eq!(base_display_name("deepseek-chat-20250324"), "Chat");
        assert_eq!(base_display_name("deepseek-v3.2-2025-03-24"), "V3.2");
        assert_eq!(base_display_name("deepseek-v4-pro-0"), "V4 Pro");
        assert_eq!(
            base_display_name("deepseek-"),
            "deepseek-",
            "degenerate id stays raw"
        );
    }

    #[test]
    fn hostiles_cannot_produce_non_finite_numbers() {
        let payload = r#"{"data":{"biz_data":{"total":[
            {"model":"deepseek-chat","usage":[{"type":"RESPONSE_TOKEN","amount":"NaN"}]}
        ]}}}"#;
        let u = parse_usage(payload, payload).expect("hostile payload parses");
        assert_eq!(u.month_tokens, 0);
        assert!(u.month_cost.is_finite());
    }

    #[test]
    fn visible_models_filters_sorts_and_caps() {
        let mut u = UsageInfo::default();
        for (id, cost) in [
            ("m1", 1.0),
            ("m2", 0.0),
            ("m3", 4.0),
            ("m4", 2.0),
            ("m5", 3.0),
            ("m6", 5.0),
        ] {
            u.by_model.push(ModelUsage {
                model: id.to_string(),
                month_cost: cost,
                month_tokens: if cost > 0.0 { 1 } else { 0 },
                ..Default::default()
            });
        }
        let visible: Vec<&str> = u
            .visible_models()
            .iter()
            .map(|m| m.model.as_str())
            .collect();
        assert_eq!(
            visible,
            ["m6", "m3", "m5", "m4"],
            "zero row dropped, top 4 by cost"
        );
    }
}
