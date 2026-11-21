// DeepSeek backend (PLAN §5.2 / §6): official balance, console usage, official
// display names and an atomic last-good cache. This stops at the IPC boundary —
// the panel pane renders it; the thresholds live in the frontend (PLAN §5.2).

pub mod client;
pub mod credentials;
pub mod parse;

use crate::settings;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

const CACHE_FILE: &str = "deepseek-cache.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BalanceInfo {
    pub is_available: bool,
    pub currency: String,
    pub total: f64,
    pub granted: f64,
    pub topped_up: f64,
}

impl Default for BalanceInfo {
    fn default() -> Self {
        BalanceInfo {
            is_available: false,
            currency: "CNY".to_string(),
            total: 0.0,
            granted: 0.0,
            topped_up: 0.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ModelUsage {
    pub model: String,
    pub display_name: String,
    pub month_tokens: i64,
    pub month_requests: i64,
    pub month_cost: f64,
    /// 0-100; null when the model has no prompt tokens at all.
    pub cache_hit_rate: Option<f64>,
    pub today_tokens: i64,
    pub today_cost: f64,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UsageInfo {
    pub month_tokens: i64,
    pub month_requests: i64,
    pub month_cost: f64,
    pub cache_hit_rate: Option<f64>,
    pub today_tokens: i64,
    pub today_cost: f64,
    /// Models worth rendering, already filtered and capped by `visible_models`
    /// at the end of `fetch_diag` — the pane draws this list as-is.
    pub by_model: Vec<ModelUsage>,
}

impl UsageInfo {
    /// Cards worth rendering (PLAN §5.2): the console sends all-zero placeholder
    /// rows, and the pane shows at most four models by month cost. Stable order,
    /// so equal-cost rows keep the response order. `fetch_diag` applies this to
    /// the payload it returns, so callers render `by_model` without re-filtering.
    pub fn visible_models(&self) -> Vec<&ModelUsage> {
        let mut list: Vec<&ModelUsage> = self
            .by_model
            .iter()
            .filter(|m| {
                m.month_tokens > 0
                    || m.month_requests > 0
                    || m.month_cost > 0.0
                    || m.today_tokens > 0
                    || m.today_cost > 0.0
            })
            .collect();
        list.sort_by(|a, b| {
            b.month_cost
                .partial_cmp(&a.month_cost)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        list.truncate(4);
        list
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeepseekResult {
    pub balance: Option<BalanceInfo>,
    pub usage: Option<UsageInfo>,
    /// Balance and usage fail independently: one dead endpoint must not blank
    /// the other half of the pane (PLAN §5.2).
    pub balance_error: Option<String>,
    pub usage_error: Option<String>,
    pub fetched_at: DateTime<Local>,
}

impl Default for DeepseekResult {
    fn default() -> Self {
        DeepseekResult {
            balance: None,
            usage: None,
            balance_error: None,
            usage_error: None,
            fetched_at: Local::now(),
        }
    }
}

impl DeepseekResult {
    pub fn has_error(&self) -> bool {
        self.balance_error.is_some() || self.usage_error.is_some()
    }

    /// An error the user cannot fix by editing settings: a real fetch failure.
    /// `no-api-key` / `no-user-token` mean "not configured", which must not
    /// paint "(fetch failed)" forever or retry every 30 s (PLAN §5.3).
    pub fn is_failure(&self) -> bool {
        let failed = |e: &Option<String>| e.as_deref().is_some_and(|s| s != "no-api-key");
        let failed_token = |e: &Option<String>| e.as_deref().is_some_and(|s| s != "no-user-token");
        failed(&self.balance_error) || failed_token(&self.usage_error)
    }

    /// Keep the last-known-good half on failure (PLAN §5.3): the status line
    /// reports the failure, the numbers stay put.
    pub fn fill_missing_from(&mut self, last: &DeepseekResult) {
        let mut refilled = 0;
        if self.balance.is_none() {
            self.balance = last.balance.clone();
            refilled += 1;
        }
        if self.usage.is_none() {
            self.usage = last.usage.clone();
            refilled += 1;
        }
        // Nothing fresh arrived, so this result still describes the last good
        // fetch and must not claim a new timestamp: `save_cache` writes it, and
        // a restart would then print the old numbers as "Updated <now>"
        // (PLAN §3.1's cross-session rule depends on this staying put).
        if refilled == 2 {
            self.fetched_at = last.fetched_at;
        }
    }
}

/// Diagnostics for `--test-deepseek --verbose` (PLAN §6.1). A no-op unless the
/// flag is set, and it never receives a token or an API key.
#[derive(Default)]
pub struct Diag {
    pub enabled: bool,
    pub lines: Vec<String>,
}

impl Diag {
    pub fn log(&mut self, line: impl Into<String>) {
        if self.enabled {
            self.lines.push(line.into());
        }
    }
}

/// Fetch once. `--test-deepseek` prints the JSON form of this.
pub async fn fetch() -> DeepseekResult {
    fetch_diag(false).await.0
}

/// Same fetch, plus the verbose trail when `verbose` is set.
pub async fn fetch_diag(verbose: bool) -> (DeepseekResult, Vec<String>) {
    let mut diag = Diag {
        enabled: verbose,
        lines: Vec::new(),
    };
    let creds = credentials::load();
    diag.log(format!("credential source: {}", creds.source.as_str()));

    let mut result = DeepseekResult::default();

    match creds.api_key.as_deref() {
        None => result.balance_error = Some("no-api-key".to_string()),
        Some(api_key) => match client::fetch_balance(api_key).await {
            Ok(balance) => result.balance = Some(balance),
            Err(error) => result.balance_error = Some(error),
        },
    }

    match creds.user_token.as_deref() {
        None => result.usage_error = Some("no-user-token".to_string()),
        Some(token) => match client::fetch_usage(token, &mut diag).await {
            Ok(usage) => result.usage = Some(usage),
            Err(error) => result.usage_error = Some(error),
        },
    }

    if let Some(usage) = result.usage.as_mut() {
        client::apply_display_names(usage, creds.api_key.as_deref(), &mut diag).await;
        // The pane renders `visible_models` only, so filter once here: the IPC
        // payload, the `deepseek-updated` event and the cache then carry
        // exactly what is drawn, and no second filter can drift from this one
        // (PLAN §5.2). The verbose trail above already logged every row.
        let visible: Vec<ModelUsage> = usage.visible_models().into_iter().cloned().collect();
        usage.by_model = visible;
    }

    (result, diag.lines)
}

/// Last successful payload, so the pane has numbers before the first poll
/// lands. Historical error strings are dropped: a stale failure must not show
/// up as "update failed" on a cold start.
pub fn load_cache() -> Option<DeepseekResult> {
    let text = std::fs::read_to_string(settings::config_dir().join(CACHE_FILE)).ok()?;
    let mut result: DeepseekResult = serde_json::from_str(&text).ok()?;
    result.balance_error = None;
    result.usage_error = None;
    Some(result)
}

/// Atomic write (temp + rename); a failure keeps the previous file (PLAN §5.2).
pub fn save_cache(result: &DeepseekResult) {
    let dir = settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(json) = serde_json::to_string(result) else {
        return;
    };
    let tmp = dir.join(format!("{CACHE_FILE}.tmp"));
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(CACHE_FILE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good(at: DateTime<Local>) -> DeepseekResult {
        DeepseekResult {
            balance: Some(BalanceInfo::default()),
            usage: None,
            balance_error: None,
            usage_error: None,
            fetched_at: at,
        }
    }

    #[test]
    fn a_total_failure_keeps_the_last_good_timestamp() {
        let earlier = Local::now() - chrono::Duration::hours(6);
        let mut failed = DeepseekResult::default(); // stamps `now`
        failed.fill_missing_from(&good(earlier));
        assert!(
            failed.balance.is_some(),
            "the last-good half is carried over"
        );
        assert_eq!(
            failed.fetched_at, earlier,
            "nothing was fetched, so the timestamp must not move"
        );
    }

    #[test]
    fn a_partial_success_keeps_its_own_timestamp() {
        let earlier = Local::now() - chrono::Duration::hours(6);
        let mut partial = DeepseekResult {
            // A fresh balance with a dead usage endpoint.
            balance: Some(BalanceInfo::default()),
            ..Default::default()
        };
        partial.fill_missing_from(&good(earlier));
        assert_ne!(partial.fetched_at, earlier, "one half really is new");
    }
}
