// P1 cross-check (not part of the crate's contract): compute the formatting
// cases the TypeScript probe emitted, so both implementations can be diffed
// line by line. Driven by `spike/p1/fmt_parity.py`; keep the two case lists in
// step until the Tauri frontend (and `src/common.ts`) is gone.

use chrono::{DateTime, Local};
use serde_json::Value;

fn instant(value: &Value) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(value.as_str().expect("iso string"))
        .expect("rfc3339")
        .with_timezone(&Local)
}

fn main() {
    let path = std::env::args().nth(1).expect("cases json");
    let text = std::fs::read_to_string(path).expect("read cases");
    let cases: Vec<Value> = serde_json::from_str(&text).expect("cases parse");
    for case in cases {
        let label = case["label"].as_str().expect("label");
        let arg = &case["arg"];
        let result = match case["fn"].as_str().expect("fn") {
            "fmtYuan" => quota_core::format::fmt_yuan(arg.as_i64().expect("cents")),
            "fmtMoney" => {
                let pair = arg.as_array().expect("pair");
                quota_core::format::fmt_money(
                    pair[0].as_f64().expect("yuan"),
                    pair[1].as_str().expect("currency"),
                )
            }
            "fmtTokens" => quota_core::format::fmt_tokens(arg.as_i64().expect("tokens")),
            "fmtHitRate" => quota_core::format::fmt_hit_rate(arg.as_f64()),
            "fmtPct" => quota_core::format::fmt_pct(arg.as_f64()).unwrap_or("<null>".to_string()),
            "fmtCount" => quota_core::format::fmt_count(arg.as_i64().expect("count")),
            "clampPercent" => format!(
                "{}",
                quota_core::format::clamp_percent(arg.as_f64().expect("pct"))
            ),
            "isLowBalance" => {
                let pair = arg.as_array().expect("pair");
                quota_core::format::is_low_balance(
                    pair[0].as_f64().expect("total"),
                    pair[1].as_bool().expect("available"),
                )
                .to_string()
            }
            "isLowCacheHit" => quota_core::format::is_low_cache_hit(arg.as_f64()).to_string(),
            "formatReset" => {
                let pair = arg.as_array().expect("pair");
                quota_core::format::format_reset(instant(&pair[0]), instant(&pair[1]))
            }
            "fmtUpdated" => {
                let pair = arg.as_array().expect("pair");
                quota_core::format::fmt_updated(instant(&pair[0]), instant(&pair[1]))
            }
            other => panic!("unknown case {other}"),
        };
        println!("{label}\t{result}");
    }
}
