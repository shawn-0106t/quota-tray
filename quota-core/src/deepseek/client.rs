// DeepSeek HTTP layer (PLAN §5.2 / §6):
//  - balance and model metadata live on api.deepseek.com behind the API key
//  - usage lives on the platform console behind the browser user token, so the
//    request has to look like the console's own fetch
//  - the usage base is a candidate chain: 404/410 walks to the next one and the
//    winner is remembered, so a path move only costs one extra request
// Every failure maps to a PLAN §5.2 error code and is never raised as a panic
// or a dialog.

use crate::deepseek::parse;
use crate::deepseek::{BalanceInfo, Diag, UsageInfo};
use crate::settings;
use chrono::{DateTime, Datelike, Duration, Local};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const BALANCE_URL: &str = "https://api.deepseek.com/user/balance";
/// Display names only; this endpoint never drives the usage numbers (PLAN §6.1).
const MODELS_URL: &str = "https://api.deepseek.com/models";

/// Fixed headers that make the console accept a plain fetch. If upstream
/// tightens validation these two constants are the only knobs (PLAN §6.2).
const APP_VERSION: &str = "1.0.0";
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

/// Tried in order; the first one that answers is remembered (PLAN §6.2).
const USAGE_BASES: &[&str] = &[
    "https://platform.deepseek.com/api/v0/usage",
    "https://platform.deepseek.com/api/v1/usage",
];

const ENDPOINTS_FILE: &str = "deepseek-endpoints.json";
const MODELS_FILE: &str = "deepseek-models.json";
const MODELS_TTL_HOURS: i64 = 24;

/// Claim the next `/models` attempt, atomically: a successful refresh is good
/// for the cache TTL, a failed one retries after an hour, and two concurrent
/// fetches cannot both issue it. The old "once per process" flag either froze
/// the official names for the session (a startup blip) or, after the first 24 h,
/// for good — PLAN §6.1 promises a vendor rename reaches the panel within a day.
static MODELS_ATTEMPT: std::sync::Mutex<Option<(std::time::Instant, bool)>> =
    std::sync::Mutex::new(None);

fn claim_models_attempt() -> bool {
    // A poisoned lock would mean a panic inside the critical section, which
    // cannot happen here (no panicking calls); recover instead of taking the
    // poller down with it.
    let mut guard = match MODELS_ATTEMPT.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some((at, landed)) = *guard {
        let wait = if landed {
            std::time::Duration::from_secs(MODELS_TTL_HOURS as u64 * 3600)
        } else {
            std::time::Duration::from_secs(3600)
        };
        if at.elapsed() < wait {
            return false;
        }
    }
    // Pessimistic stamp: a panic or an early return still counts as an attempt.
    *guard = Some((std::time::Instant::now(), false));
    true
}

fn note_models_landed() {
    if let Ok(mut guard) = MODELS_ATTEMPT.lock() {
        *guard = Some((std::time::Instant::now(), true));
    }
}

fn client() -> Option<&'static Client> {
    static CLIENT: OnceLock<Option<Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                // The parent app used a 15s HttpClient timeout.
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .ok()
        })
        .as_ref()
}

pub async fn fetch_balance(api_key: &str) -> Result<BalanceInfo, String> {
    let Some(client) = client() else {
        return Err("network-error".to_string());
    };
    let resp = client
        .get(BALANCE_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|_| "network-error".to_string())?;
    if !resp.status().is_success() {
        return Err(balance_error(resp.status().as_u16()));
    }
    let body = resp.text().await.map_err(|_| "network-error".to_string())?;
    parse::parse_balance(&body).map_err(|_| "parse-error".to_string())
}

pub async fn fetch_usage(user_token: &str, diag: &mut Diag) -> Result<UsageInfo, String> {
    let now = Local::now();
    let month = now.month();
    let year = now.year();
    let amount = fetch_usage_body("amount", month, year, user_token, diag).await?;
    let cost = fetch_usage_body("cost", month, year, user_token, diag).await?;
    let usage = parse::parse_usage(&amount, &cost).map_err(|_| "parse-error".to_string())?;
    if diag.enabled {
        for m in &usage.by_model {
            diag.log(format!(
                "model {}: tokens={} requests={} cost={:.8} hitRate={} todayTokens={} todayCost={:.8}",
                m.model,
                m.month_tokens,
                m.month_requests,
                m.month_cost,
                m.cache_hit_rate
                    .map(|v| format!("{v:.4}"))
                    .unwrap_or_else(|| "null".to_string()),
                m.today_tokens,
                m.today_cost,
            ));
        }
    }
    Ok(usage)
}

async fn fetch_usage_body(
    kind: &str,
    month: u32,
    year: i32,
    user_token: &str,
    diag: &mut Diag,
) -> Result<String, String> {
    let bases = candidate_bases();
    let mut last_error = "server-error".to_string();
    // A base answering with the platform's refusal is recorded, not returned:
    // the answer that matters is whether *any* base accepted the token.
    let mut refused = false;
    for base in &bases {
        let Some(client) = client() else {
            return Err("network-error".to_string());
        };
        let url = format!("{base}/{kind}?month={month}&year={year}");
        diag.log(format!("usage/{kind} GET {url}"));
        let resp = match console_get(client, &url, user_token).send().await {
            Ok(r) => r,
            Err(_) => return Err("network-error".to_string()),
        };
        let status = resp.status().as_u16();
        if !(200..300).contains(&status) {
            let error = usage_error(status);
            if status == 404 || status == 410 {
                diag.log(format!("usage/{kind} {url} -> {error}, next candidate"));
                last_error = error;
                continue;
            }
            diag.log(format!("usage/{kind} {url} -> {error}"));
            return Err(error);
        }
        let body = resp.text().await.map_err(|_| "network-error".to_string())?;
        // A 2xx body that is not JSON at all (a proxy page, a truncated
        // response) is useless, and remembering it as this month's endpoint
        // would pin the app to it: parsing fails, the next poll hits the same
        // remembered base, and only deleting the file recovers.
        if serde_json::from_str::<Value>(&body).is_err() {
            diag.log(format!(
                "usage/{kind} {url} -> 200 that is not JSON, next candidate"
            ));
            last_error = "parse-error".to_string();
            continue;
        }
        // A 2xx is not a success: the platform answers 200 with
        // `{"code":40003,…,"data":null}` for a dead token (the same envelope
        // `verify_user_token` rejects at sign-in). Without this check the error
        // body parses as an all-zero month, overwrites the last-good cache and
        // pins the failing base — the pane then reads ¥0.00 as if it were real.
        //
        // Do not stop the walk on it: a remembered endpoint can go stale and
        // answer with its own envelope, and returning here would pin that base
        // and fail every poll until the next sign-in.
        if !token_accepted(&body) {
            diag.log(format!(
                "usage/{kind} {url} -> 200 with an error envelope, next candidate"
            ));
            refused = true;
            continue;
        }
        log_top_level_keys(&body, kind, diag);
        if load_usage_base().as_deref() != Some(base.as_str()) {
            save_usage_base(base);
            diag.log(format!("usage/{kind} endpoint remembered: {base}"));
        }
        return Ok(body);
    }
    // A refusal from any base is the more useful answer than the last 404: it
    // is the one the pane renders as "sign in again".
    if refused {
        return Err("token-expired".to_string());
    }
    Err(last_error)
}

/// The remembered base first, then the compiled-in candidates (PLAN §6.2).
fn candidate_bases() -> Vec<String> {
    let mut bases: Vec<String> = Vec::new();
    if let Some(base) = load_usage_base() {
        bases.push(base);
    }
    for base in USAGE_BASES {
        bases.push((*base).to_string());
    }
    // `dedup()` only collapses neighbours, so a remembered base that matches a
    // compiled-in one would cost a duplicate request.
    let mut seen = std::collections::HashSet::new();
    bases.retain(|base| seen.insert(base.clone()));
    bases
}

/// A console-shaped GET: Bearer token, the console's app-version header, a
/// browser UA. If upstream tightens validation these are the knobs (PLAN §6).
fn console_get(client: &Client, url: &str, user_token: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("Authorization", format!("Bearer {user_token}"))
        .header("x-app-version", app_version())
        .header("User-Agent", BROWSER_UA)
        .header("Accept", "*/*")
}

/// Verify a candidate browser token with one real `amount` request before it is
/// saved (PLAN §5.2). The console hands out several intermediate tokens while
/// signing in; only one of them is finished, so a 2xx answer that the body
/// agrees with is the only pass.
///
/// A 2xx is NOT enough on its own: the platform answers 200 with its own error
/// envelope for a bad token (`{"code":40003,"msg":"Authorization Failed
/// (invalid token)","data":null}`), so the body has to be read too. Status-only
/// verification accepted every candidate — found by P4's sign-in probe
/// (`spike/p4/REPORT.md` §4.1).
pub async fn verify_user_token(token: &str) -> Result<(), String> {
    if token.len() < 20 {
        return Err("token-expired".to_string());
    }
    let Some(client) = client() else {
        return Err("network-error".to_string());
    };
    let now = Local::now();
    let mut last_error = "server-error".to_string();
    // A token the platform itself refused is the more useful answer than some
    // other base's 404, and the loop's last write would otherwise win: with a
    // remembered dead base first, a bad token used to be reported as `http-404`.
    let mut refused = false;
    for base in candidate_bases() {
        let url = format!("{base}/amount?month={}&year={}", now.month(), now.year());
        let resp = match console_get(client, &url, token).send().await {
            Ok(r) => r,
            // The base could not be reached at all. Same rule as the fetch path:
            // a transport failure is the caller's answer, not a reason to walk.
            Err(_) => return Err("network-error".to_string()),
        };
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            // A body that cannot be *read* is a network problem, not a pass: the
            // candidate stays unverified rather than being saved on a guess. A
            // body that is merely empty is not evidence either way and passes —
            // a token the pane then cannot use shows up as `Fetch failed`, and
            // the next sign-in overwrites it.
            let body = resp.text().await.map_err(|_| "network-error".to_string())?;
            // A non-empty body that is not JSON is not the platform answering —
            // a proxy or an interstitial returned 200. That says nothing about
            // the candidate, so walk on, and do not remember this base: doing so
            // is what pinned the app to a page it could never parse.
            let trimmed = body.trim();
            if !trimmed.is_empty() && serde_json::from_str::<Value>(trimmed).is_err() {
                last_error = "parse-error".to_string();
                continue;
            }
            if !token_accepted(&body) {
                // This base answers 200 with an error envelope, but the token
                // may still be fine on the next candidate (a dead base can be
                // remembered and answered with a soft 404). Keep walking —
                // stopping here would lock a valid token out for good.
                refused = true;
                continue;
            }
            // A base the console itself answers on is the best one to remember.
            if load_usage_base().as_deref() != Some(base.as_str()) {
                save_usage_base(&base);
            }
            return Ok(());
        }
        let error = usage_error(status);
        if status == 404 || status == 410 {
            last_error = error;
            continue;
        }
        // A refusal already seen outranks this base's failure — but NOT when
        // the failure is transport-level: a valid token whose remembered base
        // answered with an auth envelope and whose live base then blipped would
        // otherwise be dropped without the one re-try that exists for exactly
        // that case (a false rejection is the direction this must never take).
        if refused && !is_transient(&error) {
            return Err("token-expired".to_string());
        }
        return Err(error);
    }
    if refused {
        return Err("token-expired".to_string());
    }
    Err(last_error)
}

/// Is a 2xx `amount` body a success? The platform marks a failure with a
/// non-zero `code` in its envelope; a success is `{"data":{"biz_data":…}}`.
///
/// Only the explicit error code rejects. "Unrecognised" counts as success on
/// purpose: a false rejection locks the user out of signing in entirely, while
/// a false acceptance stores a token the pane then reports as `Fetch failed` and
/// the next sign-in overwrites. `data: null` alone is deliberately NOT a
/// rejection — a valid account with no usage this month may look like that.
fn token_accepted(body: &str) -> bool {
    let value = match serde_json::from_str::<Value>(body) {
        Ok(value) => value,
        // Not the platform's JSON at all (a proxy page, a truncated body): no
        // evidence either way, so the candidate is not refused.
        Err(_) => return true,
    };
    match value.get("code") {
        Some(code) => !code_is_error(code),
        None => true,
    }
}

/// A failure that says nothing about the token itself, so the caller may try
/// the same candidate again. A refusal (`token-expired`) is not one of these.
pub fn is_transient(error: &str) -> bool {
    // Every 5xx maps to `server-error` (usage_error), so a raw `http-5…` never
    // reaches here; listing it would be dead code.
    matches!(error, "network-error" | "rate-limited" | "server-error")
}

/// Is one `code` value an error? The envelope is third-party data, so both the
/// numeric shape the platform sends today (`40003`) and a numeric string have to
/// be read; anything else is not a signal and must not reject a candidate.
fn code_is_error(code: &Value) -> bool {
    // Any numeric value that is not zero: integer or float, number or string.
    match code {
        // `f64::from_str` accepts "NaN" and "inf", which are not zero — a
        // non-finite value must not reject a candidate (a false rejection locks
        // the user out; a false acceptance is overwritten by the next sign-in).
        Value::Number(number) => number
            .as_f64()
            .is_some_and(|value| value.is_finite() && value != 0.0),
        Value::String(text) => text
            .trim()
            .parse::<f64>()
            .map(|value| value.is_finite() && value != 0.0)
            .unwrap_or(false),
        _ => false,
    }
}

fn log_top_level_keys(body: &str, kind: &str, diag: &mut Diag) {
    if !diag.enabled {
        return;
    }
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(body) else {
        diag.log(format!("usage/{kind} body is not a JSON object"));
        return;
    };
    diag.log(format!(
        "usage/{kind} top-level keys: {}",
        join_keys(map.keys())
    ));
    if let Some(Value::Object(data)) = map.get("data") {
        diag.log(format!(
            "usage/{kind} data keys: {}",
            join_keys(data.keys())
        ));
        match data.get("biz_data") {
            Some(Value::Array(list)) => {
                diag.log(format!("usage/{kind} biz_data is an array[{}]", list.len()))
            }
            Some(Value::Object(biz)) => diag.log(format!(
                "usage/{kind} biz_data keys: {}",
                join_keys(biz.keys())
            )),
            _ => diag.log(format!("usage/{kind} biz_data missing")),
        }
    }
}

fn join_keys<'a>(keys: impl Iterator<Item = &'a String>) -> String {
    keys.map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
}

/// Balance and usage report 401 differently on purpose: the balance 401 means
/// a dead API key, the usage 401 means the browser session token expired.
fn balance_error(status: u16) -> String {
    match status {
        401 => "key-invalid".to_string(),
        429 => "rate-limited".to_string(),
        s if s >= 500 => "server-error".to_string(),
        s => format!("http-{s}"),
    }
}

fn usage_error(status: u16) -> String {
    match status {
        401 => "token-expired".to_string(),
        429 => "rate-limited".to_string(),
        s if s >= 500 => "server-error".to_string(),
        s => format!("http-{s}"),
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Endpoints {
    usage_base: String,
    /// `x-app-version` the console itself sent, learned from the login window
    /// (PLAN §6.3). Empty means "use the compiled-in constant".
    app_version: String,
    /// When the template last changed; diagnostics only.
    learned_at: Option<DateTime<Local>>,
}

fn load_endpoints() -> Endpoints {
    std::fs::read_to_string(settings::config_dir().join(ENDPOINTS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_endpoints(memo: &Endpoints) {
    let dir = settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(memo) {
        let _ = std::fs::write(dir.join(ENDPOINTS_FILE), json);
    }
}

/// The base the last successful request (or the console, via the sign-in
/// window) settled on; surfaced by `--test-login --verbose` (PLAN §6.2).
pub fn load_usage_base() -> Option<String> {
    Some(load_endpoints().usage_base).filter(|b| !b.is_empty())
}
fn save_usage_base(base: &str) {
    let mut memo = load_endpoints();
    if memo.usage_base == base {
        return;
    }
    memo.usage_base = base.to_string();
    memo.learned_at = Some(Local::now());
    save_endpoints(&memo);
}

/// The console's app-version header, learned first and compiled-in second.
fn app_version() -> String {
    let learned = load_endpoints().app_version;
    if sanitize_version(&learned).is_some() {
        learned
    } else {
        APP_VERSION.to_string()
    }
}

/// Accept only a short, header-safe version token: the value arrives from a
/// remote page, so it must not be able to inject a header of its own.
fn sanitize_version(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    let ok = !trimmed.is_empty()
        && trimmed.len() <= 24
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if ok {
        Some(trimmed)
    } else {
        None
    }
}

/// Learn the usage template from a request the console itself made (PLAN §6.3).
/// Only the URL shape and the non-sensitive app-version header are kept; the
/// Authorization header never leaves the page. Returns a diagnostic line when
/// anything changed.
pub fn learn_request(url: &str, version_header: &str) -> Option<String> {
    let base = usage_base_from(url)?;
    let mut memo = load_endpoints();
    let mut learned: Vec<String> = Vec::new();
    if memo.usage_base != base {
        learned.push(format!("usage base {base}"));
        memo.usage_base = base;
    }
    if let Some(version) = sanitize_version(version_header) {
        if memo.app_version != version {
            learned.push(format!("app version {version}"));
            memo.app_version = version.to_string();
        }
    }
    if learned.is_empty() {
        return None;
    }
    memo.learned_at = Some(Local::now());
    save_endpoints(&memo);
    Some(learned.join(", "))
}

/// `https://platform.deepseek.com/api/v0/usage/amount?month=…` -> `…/api/v0/usage`.
/// A relative URL is resolved against the console host it was captured on.
fn usage_base_from(url: &str) -> Option<String> {
    const HOST: &str = "https://platform.deepseek.com";
    let absolute = if url.starts_with('/') {
        format!("{HOST}{url}")
    } else {
        url.to_string()
    };
    let rest = absolute.strip_prefix(HOST)?;
    // The host must END at `HOST`: `https://platform.deepseek.com.evil.example/…`
    // also starts with it, and this base is where the user token goes.
    if !rest.starts_with('/') {
        return None;
    }
    let idx = rest.find("/usage")?;
    // Cut at "/usage", so the base cannot carry a query or a fragment.
    Some(format!("{HOST}{}", &rest[..idx + "/usage".len()]))
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ModelsCache {
    fetched_at: Option<DateTime<Local>>,
    /// id -> official display name. Only `id` and `name` are kept; the other
    /// fields on that endpoint are irrelevant to a quota panel (PLAN §6.1).
    names: BTreeMap<String, String>,
}

fn load_models() -> ModelsCache {
    std::fs::read_to_string(settings::config_dir().join(MODELS_FILE))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_models(cache: &ModelsCache) {
    let dir = settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(json) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(dir.join(MODELS_FILE), json);
    }
}

/// Put the official display name in front of the static/derived one (PLAN §6.1
/// amended 2026-11-14: the live `/models` name wins, so a vendor rename reaches
/// the panel without a new release). Everything here is best-effort: no API
/// key, a failed request or a missing name all fall through silently.
pub async fn apply_display_names(usage: &mut UsageInfo, api_key: Option<&str>, diag: &mut Diag) {
    let mut cache = load_models();
    let fresh = cache.fetched_at.is_some_and(|at| {
        Local::now().signed_duration_since(at) < Duration::hours(MODELS_TTL_HOURS)
    });
    // Refresh when the cache is stale, or when a model showed up that the
    // cache has never seen (a release can ship a new id mid-day).
    let unknown = usage
        .by_model
        .iter()
        .any(|m| !cache.names.contains_key(&m.model));
    let want = !fresh || (unknown && !cache.names.is_empty());
    diag.log(format!(
        "models cache: {} entries, fresh={fresh}, unknownId={unknown}",
        cache.names.len()
    ));
    if want {
        if let Some(key) = api_key {
            if claim_models_attempt() {
                if let Some(names) = fetch_models(key, diag).await {
                    cache.names = names;
                    cache.fetched_at = Some(Local::now());
                    save_models(&cache);
                    note_models_landed();
                }
            }
        }
    }
    for m in &mut usage.by_model {
        match cache.names.get(&m.model).filter(|n| !n.is_empty()) {
            Some(official) => {
                m.display_name = official.clone();
                diag.log(format!("model {}: name from /models", m.model));
            }
            None => diag.log(format!(
                "model {}: name {}=static/derived",
                m.model, m.display_name
            )),
        }
    }
}

async fn fetch_models(api_key: &str, diag: &mut Diag) -> Option<BTreeMap<String, String>> {
    let client = client()?;
    let resp = match client
        .get(MODELS_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            diag.log("models GET -> network-error");
            return None;
        }
    };
    if !resp.status().is_success() {
        diag.log(format!("models GET -> http-{}", resp.status().as_u16()));
        return None;
    }
    let body = match resp.text().await {
        Ok(b) => b,
        Err(_) => {
            diag.log("models GET -> unreadable body");
            return None;
        }
    };
    let root: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            diag.log("models GET -> parse-error");
            return None;
        }
    };
    let Some(list) = root.get("data").and_then(Value::as_array) else {
        diag.log("models GET -> no data array");
        return None;
    };
    let mut names = BTreeMap::new();
    for item in list {
        let (Some(id), Some(name)) = (
            item.get("id").and_then(Value::as_str),
            item.get("name").and_then(Value::as_str),
        ) else {
            continue;
        };
        if id.is_empty() || name.is_empty() {
            continue;
        }
        names.insert(id.to_string(), name.to_string());
    }
    diag.log(format!("models GET -> {} id/name pairs", names.len()));
    Some(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    // PLAN §6.3: the template captured from the console must yield the base the
    // fetch walks, and nothing else — a request to another host is not a hint.
    #[test]
    fn usage_base_is_derived_from_the_console_url() {
        assert_eq!(
            usage_base_from("https://platform.deepseek.com/api/v0/usage/amount?month=11&year=2026")
                .as_deref(),
            Some("https://platform.deepseek.com/api/v0/usage")
        );
        assert_eq!(
            usage_base_from("/api/v2/usage/cost?month=1&year=2026").as_deref(),
            Some("https://platform.deepseek.com/api/v2/usage")
        );
        assert_eq!(
            usage_base_from("https://evil.example.com/api/v0/usage/amount"),
            None
        );
        assert_eq!(
            usage_base_from("https://platform.deepseek.com/api/v0/models"),
            None
        );
    }

    // The value comes from a remote page and ends up in a request header.
    #[test]
    fn app_version_header_values_are_sanitised() {
        assert_eq!(sanitize_version(" 1.0.0 "), Some("1.0.0"));
        assert_eq!(sanitize_version(""), None);
        assert_eq!(sanitize_version("1.0.0\r\nX-Evil: 1"), None);
        assert_eq!(sanitize_version(&"9".repeat(25)), None);
    }

    // PLAN §5.2: the platform answers 200 for a bad token and says so in the
    // body, so "verified" has to mean "the envelope says so too".
    #[test]
    fn a_2xx_body_is_only_a_pass_when_the_envelope_is_clean() {
        // Reproduced against the live endpoint 2026-11-15 (spike/p4/REPORT.md).
        assert!(!token_accepted(
            r#"{"code":40003,"msg":"Authorization Failed (invalid token)","data":null}"#
        ));
        assert!(!token_accepted(
            r#"{"code":40002,"msg":"Missing Token","data":null}"#
        ));
        // The success shape the fetch path parses.
        assert!(token_accepted(
            r#"{"data":{"biz_data":{"total":[{"model":"m"}]}}}"#
        ));
        // A numeric string is the same signal as a number.
        assert!(!token_accepted(r#"{"code":"40003","data":null}"#));
        assert!(token_accepted(r#"{"code":"0","data":{}}"#));
        // An explicit zero status is a success too.
        assert!(token_accepted(r#"{"code":0,"data":{"biz_data":{}}}"#));
        // A negative code is still an error, not a pass.
        assert!(!token_accepted(r#"{"code":-1,"data":null}"#));
        // So is one that arrives as a float (JS integral numbers are integers,
        // but the envelope is third-party data and this costs two assertions).
        assert!(!token_accepted(r#"{"code":40003.0,"data":null}"#));
        assert!(!token_accepted(r#"{"code":"40003.0","data":null}"#));
        // A non-finite value is not an error code: rejecting on it would be the
        // one direction this predicate must never take.
        assert!(token_accepted(r#"{"code":"NaN","data":null}"#));
        assert!(token_accepted(r#"{"code":"inf","data":null}"#));
        // `data: null` WITHOUT an error code is not a rejection: a valid account
        // with no usage this month may look like that, and a false rejection
        // locks the user out of signing in entirely.
        assert!(token_accepted(r#"{"data":null}"#));
        // Unrecognised shapes must not lock the user out either.
        assert!(token_accepted("<html><body>proxy</body></html>"));
        assert!(token_accepted(""));
        assert!(token_accepted("{}"));
    }
}

#[cfg(test)]
mod host_check_tests {
    use super::*;

    #[test]
    fn a_suffix_domain_is_not_the_console_host() {
        assert_eq!(
            usage_base_from("https://platform.deepseek.com.evil.example/api/v0/usage/amount"),
            None
        );
        assert_eq!(
            usage_base_from("https://platform.deepseek.com/api/v0/usage/amount?month=11")
                .as_deref(),
            Some("https://platform.deepseek.com/api/v0/usage")
        );
    }
}
