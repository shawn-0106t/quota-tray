// Kimi prompt-cache / decode-speed statistics (PLAN §5.1). The scan itself
// lives in `quota-core::kimi_usage` (HANDOFF §4) so the Slint shell can share
// it; this module keeps only the Tauri loop that drives it and emits the event.

pub use quota_core::kimi_usage::*;

use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};

/// Scan at startup, then keep up with the CLI: a tick only reads the bytes
/// appended since the last one, so this can run often without touching the disk
/// volume again. Emits `kimi-usage-updated` only when a counter moved.
pub async fn run(app: AppHandle) {
    let mut previous: Option<(u64, u64, u64, u64)> = None;
    let mut tick: u64 = 0;
    loop {
        let scanned = if tick % 2 == 0 {
            tauri::async_runtime::spawn_blocking(scan).await
        } else {
            tauri::async_runtime::spawn_blocking(scan_known).await
        }
        .unwrap_or_default();
        tick += 1;
        let signature = scanned.signature();
        {
            let st = app.state::<AppState>();
            *st.kimi_usage.write().unwrap() = Some(scanned.clone());
        }
        if previous != Some(signature) {
            previous = Some(signature);
            let _ = app.emit("kimi-usage-updated", &scanned);
        }
        crate::recovery::log(&format!(
            "kimi-usage: files={} requests(7d)={} scan={}ms",
            scanned.files, scanned.week.requests, scanned.scan_ms
        ));
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
