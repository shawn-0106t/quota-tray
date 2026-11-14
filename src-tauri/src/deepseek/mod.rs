// DeepSeek backend (PLAN §5.2 / §6): official balance, console usage, official
// display names and an atomic last-good cache. The provider tab and the pane
// are the remaining Phase 2 work, so this stops at the IPC boundary for now —
// nothing here renders.

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
    /// First-seen order, as the console reports it. `visible_models` is what a
    /// pane should render.
    pub by_model: Vec<ModelUsage>,
}

impl UsageInfo {
    /// Cards worth rendering (PLAN §5.2): the console sends all-zero placeholder
    /// rows, and the pane shows at most four models by month cost. Stable order,
    /// so equal-cost rows keep the response order.
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

    /// Keep the last-known-good half on failure (PLAN §5.3): the status line
    /// reports the failure, the numbers stay put.
    pub fn fill_missing_from(&mut self, last: &DeepseekResult) {
        if self.balance.is_none() {
            self.balance = last.balance.clone();
        }
        if self.usage.is_none() {
            self.usage = last.usage.clone();
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
