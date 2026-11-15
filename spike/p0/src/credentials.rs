// DeepSeek credential file, ported from src-tauri/src/deepseek/credentials.rs.
//
// The file format is the shipped one (`{"v":1,"apiKey":"","userToken":"dpapi:…",
// "importedFrom":"…"}`) on purpose: the child writes into the same file the
// Tauri build reads, so the real-token acceptance can be checked with the
// shipped app right after. Only the user token is written here; `apiKey` and
// `importedFrom` are preserved.
//
// DPAPI is user-scoped: a blob copied to another account does not decrypt, and
// a decrypt failure means "not configured", never an error dialog (PLAN §7).

use crate::config::config_dir;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;

const CRED_FILE: &str = "deepseek-credentials.json";

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct CredFile {
    v: u32,
    api_key: String,
    user_token: String,
    imported_from: Option<String>,
}

pub fn has_user_token() -> bool {
    read_raw()
        .map(|f| !f.user_token.is_empty())
        .unwrap_or(false)
}

/// Store a browser token the same way the shipped sign-in window does.
pub fn save_user_token(token: &str) -> bool {
    let existing = read_raw().unwrap_or_default();
    let file = CredFile {
        v: 1,
        api_key: existing.api_key,
        user_token: seal(token),
        imported_from: existing
            .imported_from
            .or_else(|| Some("p0-login-host".into())),
    };
    write(&file)
}

fn write(file: &CredFile) -> bool {
    let dir = config_dir();
    let _ = fs::create_dir_all(&dir);
    let Ok(json) = serde_json::to_string_pretty(file) else {
        return false;
    };
    let tmp = dir.join(format!("{CRED_FILE}.tmp"));
    if fs::write(&tmp, json).is_err() {
        return false;
    }
    fs::rename(&tmp, dir.join(CRED_FILE)).is_ok()
}

fn read_raw() -> Option<CredFile> {
    let text = fs::read_to_string(config_dir().join(CRED_FILE)).ok()?;
    serde_json::from_str(&text).ok()
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
        None => value.to_string(),
    }
}

/// User-scoped DPAPI (CryptProtectData). `CRYPTPROTECT_UI_FORBIDDEN` keeps a
/// failure silent — the app never shows a prompt or a dialog (PLAN §7).
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

    #[allow(dead_code)]
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
    fn dpapi_round_trips() {
        let blob = dpapi::protect(b"secret-token").expect("DPAPI available in this session");
        assert_ne!(blob, b"secret-token");
        assert_eq!(dpapi::unprotect(&blob).as_deref(), Some("secret-token"));
    }

    #[test]
    fn sealed_value_has_the_shipped_prefix() {
        let sealed = seal("abcdefghijklmnopqrstuvwxyz");
        assert!(sealed.starts_with("dpapi:"));
        assert!(!sealed.contains("abcdefghijklmnopqrstuvwxyz"));
    }
}
