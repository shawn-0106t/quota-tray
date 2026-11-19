// Credential chain, 1:1 port of QuotaService.LoadToken (SPEC 16.2):
// 1) <kimi_home>/credentials/kimi-code.json -> access_token (expires_at > now+30s)
// 2) <kimi_home>/config.toml -> provider whose base_url contains api.kimi.com/coding
// 3) None -> caller reports "no-token"
// <kimi_home> honors the KIMI_CODE_HOME override (see kimi_home below).

use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn home_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("USERPROFILE") {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    // Fallback: HOMEDRIVE + HOMEPATH
    if let (Ok(d), Ok(p)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        return Some(PathBuf::from(format!("{d}{p}")));
    }
    None
}

/// JSON number-or-string -> f64 (server models numbers as strings).
fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// ~/.kimi-code, honoring the KIMI_CODE_HOME override (SPEC 16.2 / 21.2).
pub(crate) fn kimi_home(home: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("KIMI_CODE_HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    home.join(".kimi-code")
}

/// Which link of the chain answered, for the settings window's read-only Kimi
/// section (PLAN §4). Never carries the token itself.
pub struct Info {
    /// `oauth` | `api-key` | `none`
    pub source: &'static str,
    /// Unix seconds from the OAuth credential file; `None` for an API key.
    pub expires_at: Option<i64>,
    /// An OAuth entry exists but was rejected (expired / too close to expiry)
    /// and no API key took over — the one state the UI can explain precisely.
    pub expired: bool,
}

pub fn load_token() -> Option<String> {
    resolve().map(|(token, _)| token)
}

/// Read-only description of the chain for the settings window. Same walk as
/// `load_token`, so the status can never disagree with what a fetch would use.
pub fn describe() -> Info {
    let home = home_dir();
    let oauth = home.as_deref().and_then(oauth_entry);
    match resolve() {
        Some((_, source)) => Info {
            source,
            expires_at: if source == "oauth" {
                oauth.as_ref().map(|(_, exp)| *exp)
            } else {
                None
            },
            expired: false,
        },
        None => Info {
            source: "none",
            expires_at: oauth.as_ref().map(|(_, exp)| *exp),
            expired: oauth.is_some(),
        },
    }
}

/// `(token, source)` from the two steps of the chain.
fn resolve() -> Option<(String, &'static str)> {
    let home = home_dir()?;
    let kimi = kimi_home(&home);

    // 1) OAuth access token from the credentials store
    if let Some((token, exp)) = oauth_entry(&home) {
        let now = chrono::Utc::now().timestamp();
        if exp > now + 30 {
            return Some((token, "oauth"));
        }
    }

    // 2) config.toml fallback: line-by-line parse (not a full TOML parser)
    api_key_entry(&kimi).map(|key| (key, "api-key"))
}

/// `(access_token, expires_at seconds)` from `credentials/kimi-code.json`,
/// whether or not it is still valid.
fn oauth_entry(home: &Path) -> Option<(String, i64)> {
    let kimi = kimi_home(home);
    let cred = kimi.join("credentials").join("kimi-code.json");
    let text = fs::read_to_string(&cred).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let token = v.get("access_token").and_then(|x| x.as_str())?;
    if token.is_empty() {
        return None;
    }
    Some((
        token.to_string(),
        unix_seconds(v.get("expires_at").and_then(as_f64).unwrap_or(0.0)),
    ))
}

/// `expires_at` is documented (and observed) as Unix **seconds**. A value in
/// milliseconds would read as "valid for a few thousand years", so the app would
/// keep using a dead token and print `Fetch failed` instead of `Expired — sign in
/// with the Kimi CLI`. 1e11 seconds is the year 5138: anything past that is a
/// millisecond stamp.
fn unix_seconds(raw: f64) -> i64 {
    let seconds = if raw.abs() >= 1.0e11 {
        raw / 1000.0
    } else {
        raw
    };
    seconds as i64
}

fn api_key_entry(kimi: &Path) -> Option<String> {
    let text = fs::read_to_string(kimi.join("config.toml")).ok()?;
    let kv = Regex::new(r#"^(base_url|api_key)\s*=\s*"([^"]*)""#).ok()?;
    let mut section: Option<String> = None;
    let mut base_url: Option<String> = None;
    let mut api_key: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            // Settle the previous section before starting a new one
            if let Some(found) =
                match_provider(section.as_deref(), base_url.as_deref(), api_key.as_deref())
            {
                return Some(found);
            }
            section = Some(line.trim_matches(|c| c == '[' || c == ']').to_string());
            base_url = None;
            api_key = None;
            continue;
        }
        if let Some(m) = kv.captures(line) {
            if &m[1] == "base_url" {
                base_url = Some(m[2].to_string());
            } else {
                api_key = Some(m[2].to_string());
            }
        }
    }
    match_provider(section.as_deref(), base_url.as_deref(), api_key.as_deref())
}

fn match_provider(
    section: Option<&str>,
    base_url: Option<&str>,
    api_key: Option<&str>,
) -> Option<String> {
    match (section, base_url, api_key) {
        (Some(s), Some(b), Some(k))
            if s.starts_with("providers.")
                && b.contains("api.kimi.com/coding")
                && !k.is_empty() =>
        {
            Some(k.to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn expires_at_seconds_and_milliseconds_both_land_in_seconds() {
        assert_eq!(unix_seconds(1_763_600_000.0), 1_763_600_000);
        assert_eq!(unix_seconds(1_763_600_000_000.0), 1_763_600_000);
        // Junk that parses as a number must not panic or wrap.
        assert_eq!(unix_seconds(0.0), 0);
        assert_eq!(unix_seconds(-1.0), -1);
    }
}
