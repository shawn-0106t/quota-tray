// DeepSeek credential chain (PLAN §5.2):
//   DEEPSEEK_API_KEY / DEEPSEEK_USER_TOKEN env
//   -> config_dir()/deepseek-credentials.json
//   -> one-time import of ~/.deepseek-planbar/credentials.json (never written back)
//   -> not configured.
// Values are DPAPI-protected behind a `dpapi:` prefix while key names stay
// readable; a bare value is accepted as plaintext (debug escape hatch).
// A decrypt failure means "not configured", never an error dialog (PLAN §7).

use crate::settings;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CRED_FILE: &str = "deepseek-credentials.json";
/// Where the parent WPF app kept its plaintext credentials. Imported once,
/// then left alone so the parent app keeps working (PLAN §5.2).
const OLD_FILE_LABEL: &str = "~/.deepseek-planbar/credentials.json";

/// Which link of the chain answered. `--test-deepseek --verbose` reports this
/// and never a value (PLAN §5.2 / AGENTS.md).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Env,
    File,
    Imported,
    Undecryptable,
    None,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Env => "env",
            Source::File => "file",
            Source::Imported => "imported",
            Source::Undecryptable => "undecryptable",
            Source::None => "none",
        }
    }
}

#[derive(Clone)]
pub struct Credentials {
    pub api_key: Option<String>,
    pub user_token: Option<String>,
    pub source: Source,
}

/// Resolve both values. A credential is never logged, printed or persisted
/// outside `credential_file()`.
pub fn load() -> Credentials {
    let env_key = env_value("DEEPSEEK_API_KEY");
    let env_token = env_value("DEEPSEEK_USER_TOKEN");
    if env_key.is_some() && env_token.is_some() {
        // Both present: env wins outright and no file is read (PLAN §5.2).
        return Credentials {
            api_key: env_key,
            user_token: env_token,
            source: Source::Env,
        };
    }

    let mut undecryptable = false;
    let existing = read_raw();
    let (file_key, file_token) = match &existing {
        Some(file) => (
            open(&file.api_key, &mut undecryptable),
            open(&file.user_token, &mut undecryptable),
        ),
        None => (None, None),
    };
    if file_key.is_some() || file_token.is_some() {
        // A half-configured env still gets its missing half from the file,
        // matching the parent app's LoadCredentials.
        let from_env = env_key.is_some() || env_token.is_some();
        return Credentials {
            api_key: env_key.or(file_key),
            user_token: env_token.or(file_token),
            source: if from_env { Source::Env } else { Source::File },
        };
    }

    // The legacy import is one-time (PLAN §5.2): the mere existence of our own
    // file — including one emptied by "Clear" — means it already ran. Reading
    // the old file again would resurrect credentials the user just removed.
    if existing.is_none() {
        let (old_key, old_token) = read_legacy();
        if old_key.is_some() || old_token.is_some() {
            save(
                old_key.as_deref(),
                old_token.as_deref(),
                Some(OLD_FILE_LABEL),
            );
            return Credentials {
                api_key: env_key.or(old_key),
                user_token: env_token.or(old_token),
                source: Source::Imported,
            };
        }
    }

    Credentials {
        api_key: env_key,
        user_token: env_token,
        source: if undecryptable {
            Source::Undecryptable
        } else {
            Source::None
        },
    }
}

/// Which values are configured, without ever handing a value to the caller.
/// The settings window renders this (PLAN §4).
pub struct Status {
    pub api_key: bool,
    pub user_token: bool,
    pub source: Source,
}

pub fn status() -> Status {
    let creds = load();
    Status {
        api_key: creds.api_key.is_some(),
        user_token: creds.user_token.is_some(),
        source: creds.source,
    }
}

/// Drop both stored values. The file stays behind, empty, so the one-time
/// legacy import cannot resurrect them on the next poll (see `load`).
pub fn clear() {
    let imported_from = read_raw()
        .and_then(|f| f.imported_from)
        .or_else(|| Some("cleared".to_string()));
    let file = CredFile {
        v: 1,
        api_key: String::new(),
        user_token: String::new(),
        imported_from,
    };
    write(&file);
}

/// Write both values, DPAPI-sealed, preserving fields the caller left alone
/// (including a blob that failed to decrypt and the import provenance).
pub fn save(api_key: Option<&str>, user_token: Option<&str>, imported_from: Option<&str>) {
    let existing = read_raw().unwrap_or_default();
    let file = CredFile {
        v: 1,
        api_key: match api_key {
            Some(v) if !v.is_empty() => seal(v),
            _ => existing.api_key,
        },
        user_token: match user_token {
            Some(v) if !v.is_empty() => seal(v),
            _ => existing.user_token,
        },
        imported_from: imported_from.map(str::to_string).or(existing.imported_from),
    };
    write(&file);
}

fn write(file: &CredFile) {
    let dir = settings::config_dir();
    let _ = fs::create_dir_all(&dir);
    let Ok(json) = serde_json::to_string_pretty(file) else {
        return;
    };
    // Temp + rename so a crash cannot leave a truncated credential file.
    let tmp = dir.join(format!("{CRED_FILE}.tmp"));
    if fs::write(&tmp, json).is_ok() {
        let _ = fs::rename(&tmp, dir.join(CRED_FILE));
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// `{"v":1,"apiKey":"dpapi:<base64>","userToken":"dpapi:<base64>","importedFrom":"..."}`
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CredFile {
    v: u32,
    api_key: String,
    user_token: String,
    imported_from: Option<String>,
}

fn read_raw() -> Option<CredFile> {
    let text = fs::read_to_string(settings::config_dir().join(CRED_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Decrypt both fields. `failed` is set when a `dpapi:` blob could not be
/// opened, which only matters for the diagnostic source string.
fn open(value: &str, failed: &mut bool) -> Option<String> {
    if value.is_empty() {
        return None;
    }
    let Some(encoded) = value.strip_prefix("dpapi:") else {
        return Some(value.to_string());
    };
    let Ok(blob) = base64::engine::general_purpose::STANDARD.decode(encoded.trim()) else {
        *failed = true;
        return None;
    };
    match dpapi::unprotect(&blob) {
        Some(text) => Some(text),
        None => {
            *failed = true;
            None
        }
    }
}

fn seal(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    match dpapi::protect(value.as_bytes()) {
        Some(blob) => format!(
            "dpapi:{}",
            base64::engine::general_purpose::STANDARD.encode(blob)
        ),
        // DPAPI unavailable: keep the credential usable rather than losing it.
        // `open` reads a bare value back as plaintext (PLAN §5.2).
        None => value.to_string(),
    }
}

/// The parent app's plaintext file: {"api_key": "...", "user_token": "..."}.
fn read_legacy() -> (Option<String>, Option<String>) {
    let Some(path) = legacy_path() else {
        return (None, None);
    };
    let Ok(text) = fs::read_to_string(path) else {
        return (None, None);
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return (None, None);
    };
    let field = |name: &str| {
        v.get(name)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    (field("api_key"), field("user_token"))
}

fn legacy_path() -> Option<PathBuf> {
    let home = crate::credentials::home_dir()?;
    Some(home.join(".deepseek-planbar").join("credentials.json"))
}

/// User-scoped DPAPI (CryptProtectData). The blob is bound to the current
/// Windows account: a portable copy to another account will not decrypt, and
/// re-entry is the expected fix (PLAN §5.2).
/// `CRYPTPROTECT_UI_FORBIDDEN` keeps a failure silent — the app never shows a
/// prompt or a dialog (PLAN §7).
mod dpapi {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: data.len() as u32,
                pbData: data.as_ptr() as *mut u8,
            };
            let mut out = CRYPT_INTEGER_BLOB::default();
            CryptProtectData(
                &input,
                None::<&PCWSTR>,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
            .ok()?;
            Some(take(&mut out))
        }
    }

    pub fn unprotect(blob: &[u8]) -> Option<String> {
        unsafe {
            let input = CRYPT_INTEGER_BLOB {
                cbData: blob.len() as u32,
                pbData: blob.as_ptr() as *mut u8,
            };
            let mut out = CRYPT_INTEGER_BLOB::default();
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
            .ok()?;
            String::from_utf8(take(&mut out)).ok()
        }
    }

    /// Copy the LocalAlloc'd buffer out, then release it.
    fn take(out: &mut CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if out.pbData.is_null() || out.cbData == 0 {
            return Vec::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe {
            LocalFree(Some(HLOCAL(out.pbData as _)));
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_value_survives_the_open_path() {
        // The escape hatch: a field without the prefix reads back verbatim.
        let mut failed = false;
        assert_eq!(open("sk-plain", &mut failed).as_deref(), Some("sk-plain"));
        assert!(!failed);
    }

    #[test]
    fn broken_blob_is_not_configured_not_an_error() {
        let mut failed = false;
        assert_eq!(open("dpapi:not-base64!!", &mut failed), None);
        assert!(failed, "the diagnostic source must distinguish this case");
    }

    #[test]
    fn empty_field_is_absent() {
        let mut failed = false;
        assert_eq!(open("", &mut failed), None);
        assert!(!failed);
    }

    #[test]
    fn dpapi_round_trips() {
        let blob = dpapi::protect(b"secret-token").expect("DPAPI available in this session");
        assert_ne!(
            blob, b"secret-token",
            "the blob must not contain the plaintext"
        );
        assert_eq!(dpapi::unprotect(&blob).as_deref(), Some("secret-token"));
    }
}
