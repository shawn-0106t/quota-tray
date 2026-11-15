// One real `amount` request, the only acceptance for a captured token
// (PLAN §5.2): the console hands out several intermediate tokens during
// sign-in and only the finished one answers 2xx.
//
// This is a spike-local copy of deepseek::client::verify_user_token trimmed to
// what the child needs; the shared `quota-core` crate in P1 replaces it.

use chrono::{Datelike, Local};
use std::time::Duration;

const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";
const APP_VERSION: &str = "1.0.0";
const BASES: &[&str] = &[
    "https://platform.deepseek.com/api/v0/usage",
    "https://platform.deepseek.com/api/v1/usage",
];

pub fn verify_user_token(token: &str) -> Result<(), String> {
    if token.len() < 20 {
        return Err("token-expired".to_string());
    }
    let now = Local::now();
    let mut last = "server-error".to_string();
    for base in BASES {
        let url = format!("{base}/amount?month={}&year={}", now.month(), now.year());
        let result = ureq::get(&url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("x-app-version", APP_VERSION)
            .header("User-Agent", BROWSER_UA)
            .header("Accept", "*/*")
            .config()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .call();
        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if (200..300).contains(&status) {
                    return Ok(());
                }
                if status == 404 || status == 410 {
                    last = "endpoint-moved".to_string();
                    continue;
                }
                return Err(explain(status));
            }
            Err(ureq::Error::StatusCode(status)) => {
                if status == 404 || status == 410 {
                    last = "endpoint-moved".to_string();
                    continue;
                }
                return Err(explain(status));
            }
            Err(ureq::Error::Timeout(_)) => return Err("network-error".to_string()),
            Err(_) => return Err("network-error".to_string()),
        }
    }
    Err(last)
}

fn explain(status: u16) -> String {
    match status {
        401 | 403 => "token-rejected".to_string(),
        429 => "rate-limited".to_string(),
        500..=599 => "server-error".to_string(),
        _ => "token-rejected".to_string(),
    }
}
