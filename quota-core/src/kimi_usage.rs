// Kimi prompt-cache and decode-speed statistics from the local CLI session logs
// (PLAN §5.1).
//
// The quota API (`api.kimi.com/coding/v1/usages`) reports window percentages and
// the Extra Usage wallet only — it has no token numbers. The CLI's own session
// records do: every `usage.record` carries `inputCacheRead` / `inputCacheCreation`
// / `inputOther` / `output`, and every timed `event` carries the per-call
// `llmServerDecodeMs` / `llmServerFirstTokenMs` that turn output tokens into a
// speed. This module reads only those counters — never message content — and
// aggregates them into a 7-day window and today.
//
// The scan is incremental: each file keeps its byte offset, so a poll only reads
// what was appended since last time. A file that shrank was rewritten, and its
// contribution is dropped before rescanning it. The index lives in
// `config_dir()/kimi-usage.json` so a restart does not repay the first full scan.

use chrono::{DateTime, Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const INDEX_FILE: &str = "kimi-usage.json";
/// Days kept per file. Long enough for the 7-day window plus a diagnosis, short
/// enough that the index cannot grow without bound.
const KEEP_DAYS: i32 = 30;
const WEEK_DAYS: i32 = 7;

/// One day's accumulation for one file.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
struct Bucket {
    /// `inputCacheRead` — prompt tokens served from cache
    hit: u64,
    /// `inputCacheCreation` — tokens written into the cache (always 0 for Kimi
    /// today, kept so the formula survives a provider that starts reporting it)
    creation: u64,
    /// `inputOther` — uncached prompt tokens
    other: u64,
    /// `output` tokens
    out: u64,
    /// counted `usage.record` rows
    requests: u64,
    /// output tokens over timed calls (rate numerator)
    rate_out: u64,
    /// summed `llmServerDecodeMs` (rate denominator, milliseconds)
    decode_ms: f64,
    /// summed `llmServerFirstTokenMs`
    first_ms: f64,
    /// timed calls behind `decode_ms`
    calls: u64,
    /// timed calls that also reported a first-token time
    first_n: u64,
}

impl Bucket {
    fn add(&mut self, other: &Bucket) {
        self.hit += other.hit;
        self.creation += other.creation;
        self.other += other.other;
        self.out += other.out;
        self.requests += other.requests;
        self.rate_out += other.rate_out;
        self.decode_ms += other.decode_ms;
        self.first_ms += other.first_ms;
        self.calls += other.calls;
        self.first_n += other.first_n;
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct FileState {
    /// Bytes consumed so far; the next scan resumes here
    len: u64,
    /// Modification time in ms, for cheap "did anything change" checks
    mtime_ms: i64,
    /// day key (`NaiveDate::num_days_from_ce`) -> bucket
    days: BTreeMap<i32, Bucket>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Index {
    v: u32,
    files: BTreeMap<String, FileState>,
}

/// One window's worth of numbers, shaped for the pane (PLAN §5.1).
#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheWindow {
    pub requests: u64,
    /// Percent, 0-100; `None` when no prompt tokens were seen
    pub hit_rate: Option<f64>,
    /// Output tokens per second of server decode time
    pub decode_tps: Option<f64>,
    /// Mean time to first token, milliseconds
    pub first_token_ms: Option<u64>,
    /// Calls behind the two speed figures
    pub calls: u64,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiUsage {
    pub week: CacheWindow,
    pub today: CacheWindow,
    pub scanned_at: DateTime<Local>,
    /// Diagnostics: session files seen and how long the last scan took
    pub files: usize,
    pub scan_ms: u64,
}

impl KimiUsage {
    /// Cheap change detector for the emit gate: the pane only needs an event
    /// when a number moved.
    pub fn signature(&self) -> (u64, u64, u64, u64) {
        (
            self.week.requests,
            self.today.requests,
            self.week.calls,
            self.today.calls,
        )
    }
}

/// One parsed line, reduced to the fields this module is allowed to see.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Line {
    #[serde(default)]
    time: Option<i64>,
    #[serde(rename = "type", default)]
    kind: Option<String>,
    #[serde(default)]
    usage_scope: Option<String>,
    #[serde(default)]
    usage: Option<Counters>,
    #[serde(default)]
    event: Option<TimedEvent>,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct Counters {
    #[serde(default)]
    input_cache_read: u64,
    #[serde(default)]
    input_cache_creation: u64,
    #[serde(default)]
    input_other: u64,
    #[serde(default)]
    output: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimedEvent {
    #[serde(default)]
    usage: Option<Counters>,
    #[serde(default)]
    llm_server_decode_ms: Option<f64>,
    #[serde(default)]
    llm_server_first_token_ms: Option<f64>,
}

/// Fold one JSON line into the bucket for its own local day. One parse per line,
/// and the counting rules stay testable without touching the filesystem.
fn absorb(days: &mut BTreeMap<i32, Bucket>, bytes: &[u8]) -> bool {
    // Cheap gate before the JSON parser: most lines are conversation content.
    if !contains(bytes, b"\"usage\"") {
        return false;
    }
    let Ok(line) = serde_json::from_slice::<Line>(bytes) else {
        return false;
    };
    let Some(day) = line.time.and_then(day_key) else {
        return false;
    };
    let mut touched = false;
    let bucket = days.entry(day).or_default();
    // Per-request counters. `usageScope == "session"` rows are session totals and
    // would double count, and `message.meta.usage` copies the same numbers, so
    // only `usage.record` / `turn` is counted.
    if line.kind.as_deref() == Some("usage.record") && line.usage_scope.as_deref() == Some("turn") {
        if let Some(c) = line.usage {
            bucket.hit += c.input_cache_read;
            bucket.creation += c.input_cache_creation;
            bucket.other += c.input_other;
            bucket.out += c.output;
            bucket.requests += 1;
            touched = true;
        }
    }
    // Speed figures: the call timed its own decode step.
    if let Some(event) = line.event {
        if let (Some(c), Some(decode)) = (event.usage, event.llm_server_decode_ms) {
            if decode > 0.0 {
                bucket.rate_out += c.output;
                bucket.decode_ms += decode;
                bucket.calls += 1;
                if let Some(first) = event.llm_server_first_token_ms {
                    if first > 0.0 {
                        bucket.first_ms += first;
                        bucket.first_n += 1;
                    }
                }
                touched = true;
            }
        }
    }
    touched
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

/// Local calendar day index, so "today" follows the user's clock, not UTC.
fn day_key(ms: i64) -> Option<i32> {
    let dt = Local.timestamp_millis_opt(ms).single()?;
    Some(chrono::Datelike::num_days_from_ce(&dt.date_naive()))
}

fn index_path() -> PathBuf {
    crate::settings::shared_dir().join(INDEX_FILE)
}

fn sessions_root() -> Option<PathBuf> {
    let home = crate::credentials::home_dir()?;
    Some(crate::credentials::kimi_home(&home).join("sessions"))
}

fn load_index() -> Index {
    fs::read_to_string(index_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_index(index: &Index, today: i32) {
    let dir = crate::settings::config_dir();
    let _ = fs::create_dir_all(&dir);
    let mut pruned = Index {
        v: 1,
        files: BTreeMap::new(),
    };
    for (path, state) in &index.files {
        let days: BTreeMap<i32, Bucket> = state
            .days
            .iter()
            .filter(|(day, _)| **day >= today - KEEP_DAYS)
            .map(|(day, bucket)| (*day, *bucket))
            .collect();
        if days.is_empty() && state.len == 0 {
            continue;
        }
        pruned.files.insert(
            path.clone(),
            FileState {
                len: state.len,
                mtime_ms: state.mtime_ms,
                days,
            },
        );
    }
    let Ok(json) = serde_json::to_string(&pruned) else {
        return;
    };
    // Temp + rename: a crash must not leave a truncated index behind.
    let tmp = dir.join(format!("{INDEX_FILE}.tmp"));
    if fs::write(&tmp, json).is_ok() {
        let _ = fs::rename(&tmp, index_path());
    }
}

/// Read whatever was appended to `path` since `state.len`, folding complete
/// lines only (a line still being written is left for the next pass).
fn scan_file(path: &Path, state: &mut FileState) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let len = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if len == state.len && mtime == state.mtime_ms {
        return false;
    }
    let mut reset = false;
    if len <= state.len {
        // Truncated, or rewritten in place with the same length: the old
        // contribution no longer describes the file.
        state.days.clear();
        state.len = 0;
        reset = true;
    }
    let Ok(mut file) = File::open(path) else {
        return reset;
    };
    if file.seek(SeekFrom::Start(state.len)).is_err() {
        return reset;
    }
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut pos = state.len;
    loop {
        line.clear();
        match reader.read_until(b'\n', &mut line) {
            Ok(0) => break,
            Ok(n) => {
                if line.last() != Some(&b'\n') {
                    // Partial trailing line: leave it for the next scan.
                    break;
                }
                absorb(&mut state.days, &line);
                pos += n as u64;
            }
            Err(_) => break,
        }
    }
    let changed = pos != state.len || reset;
    state.len = pos;
    state.mtime_ms = mtime;
    changed
}

/// Walk the documented layout: `sessions/<workspace>/<session>/agents/<agent>/wire.jsonl`
/// (data-locations docs). Pruning to the three directories that can hold a log
/// keeps the walk off the thousands of `tasks/` / `logs/` entries, which is what
/// made a full pass cost a second on a large history.
fn collect_sessions(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(workspaces) = fs::read_dir(root) else {
        return;
    };
    for workspace in workspaces.flatten() {
        let Ok(sessions) = fs::read_dir(workspace.path()) else {
            continue;
        };
        for session in sessions.flatten() {
            let Ok(agents) = fs::read_dir(session.path().join("agents")) else {
                continue;
            };
            for agent in agents.flatten() {
                let wire = agent.path().join("wire.jsonl");
                if wire.is_file() {
                    out.push(wire);
                }
            }
        }
    }
    // Layout drift guard: if the documented shape ever stops matching, fall back
    // to a recursive search rather than reporting zero usage (PLAN §6 mindset).
    if out.is_empty() {
        collect_sessions_recursive(root, out);
    }
}

fn collect_sessions_recursive(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sessions_recursive(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("wire.jsonl") {
            out.push(path);
        }
    }
}

/// One pass. `walk` decides whether the session tree is rediscovered: walking
/// costs a second on a large history (thousands of directories), while statting
/// the files we already know costs milliseconds, so the loop walks every other
/// tick and stats in between.
fn scan_inner(walk: bool) -> KimiUsage {
    let started = std::time::Instant::now();
    let mut index = load_index();
    let today = day_key(now_ms()).unwrap_or(0);
    let mut dirty = false;

    if walk {
        if let Some(root) = sessions_root() {
            if root.is_dir() {
                let mut files = Vec::new();
                collect_sessions(&root, &mut files);
                for path in &files {
                    let key = path.to_string_lossy().to_string();
                    let state = index.files.entry(key).or_default();
                    if scan_file(path, state) {
                        dirty = true;
                    }
                }
                // A deleted session must stop counting: keep live files only.
                // Guarded by the directory check above, so an unreadable root
                // never empties the index.
                let live: std::collections::BTreeSet<String> = files
                    .iter()
                    .map(|path| path.to_string_lossy().to_string())
                    .collect();
                let before = index.files.len();
                index.files.retain(|path, _| live.contains(path));
                dirty |= index.files.len() != before;
            }
        }
    } else {
        let paths: Vec<String> = index.files.keys().cloned().collect();
        for key in paths {
            let path = PathBuf::from(&key);
            if let Some(state) = index.files.get_mut(&key) {
                if scan_file(&path, state) {
                    dirty = true;
                }
            }
        }
    }
    // Writing 100 KB every minute for nothing is waste; only persist movement.
    if dirty || walk {
        save_index(&index, today);
    }

    let mut week = Bucket::default();
    let mut day = Bucket::default();
    for state in index.files.values() {
        for (key, bucket) in &state.days {
            if *key >= today - (WEEK_DAYS - 1) && *key <= today {
                week.add(bucket);
            }
            if *key == today {
                day.add(bucket);
            }
        }
    }

    KimiUsage {
        week: window_of(&week),
        today: window_of(&day),
        scanned_at: Local::now(),
        files: index.files.len(),
        scan_ms: started.elapsed().as_millis() as u64,
    }
}

/// Full pass: rediscover the session tree. Used at startup, by `--test-usage`,
/// and every other tick of the background loop.
pub fn scan() -> KimiUsage {
    scan_inner(true)
}

/// Cheap pass: re-stat the files already known, read only what was appended.
pub fn scan_known() -> KimiUsage {
    scan_inner(false)
}

fn window_of(bucket: &Bucket) -> CacheWindow {
    let prompt = bucket.hit + bucket.creation + bucket.other;
    let hit_rate = if prompt > 0 {
        Some(bucket.hit as f64 / prompt as f64 * 100.0)
    } else {
        None
    };
    let decode_tps = if bucket.decode_ms > 0.0 {
        Some(bucket.rate_out as f64 / (bucket.decode_ms / 1000.0))
    } else {
        None
    };
    let first_token_ms = if bucket.first_n > 0 {
        Some((bucket.first_ms / bucket.first_n as f64).round() as u64)
    } else {
        None
    };
    CacheWindow {
        requests: bucket.requests,
        hit_rate,
        decode_tps,
        first_token_ms,
        calls: bucket.calls,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(json: &str) -> Vec<u8> {
        let mut bytes = json.as_bytes().to_vec();
        bytes.push(b'\n');
        bytes
    }

    fn usage_line(ms: i64, scope: &str, hit: u64, other: u64, out: u64) -> Vec<u8> {
        line(&format!(
            r#"{{"type":"usage.record","usageScope":"{scope}","time":{ms},"model":"k2","usage":{{"inputCacheRead":{hit},"inputCacheCreation":0,"inputOther":{other},"output":{out}}}}}"#
        ))
    }

    fn event_line(ms: i64, out: u64, decode: f64, first: f64) -> Vec<u8> {
        line(&format!(
            r#"{{"time":{ms},"event":{{"usage":{{"output":{out}}},"llmServerDecodeMs":{decode},"llmServerFirstTokenMs":{first}}}}}"#
        ))
    }

    fn fold(bytes: &[u8]) -> Bucket {
        let mut days: BTreeMap<i32, Bucket> = BTreeMap::new();
        absorb(&mut days, bytes);
        let mut total = Bucket::default();
        for bucket in days.values() {
            total.add(bucket);
        }
        total
    }

    // Only `usage.record` + `usageScope=turn` may count: session totals and
    // message copies repeat the same numbers (PLAN §5.1).
    #[test]
    fn only_turn_scoped_records_count() {
        let bucket = fold(&usage_line(1000, "turn", 90, 10, 7));
        assert_eq!(bucket.hit, 90);
        assert_eq!(bucket.other, 10);
        assert_eq!(bucket.out, 7);
        assert_eq!(bucket.requests, 1);

        assert_eq!(fold(&usage_line(1000, "session", 90, 10, 7)).requests, 0);
        assert_eq!(
            fold(&line(
                r#"{"time":1000,"type":"message","message":{"meta":{"usage":{"inputCacheRead":90}}}}"#
            ))
            .hit,
            0
        );
    }

    #[test]
    fn timed_events_feed_the_speed_figures() {
        let bucket = fold(&event_line(1000, 100, 500.0, 250.0));
        assert_eq!(bucket.rate_out, 100);
        assert_eq!(bucket.decode_ms, 500.0);
        assert_eq!(bucket.calls, 1);
        assert_eq!(bucket.first_n, 1);
        let window = window_of(&bucket);
        assert_eq!(window.decode_tps, Some(200.0));
        assert_eq!(window.first_token_ms, Some(250));
        assert_eq!(window.calls, 1);
        assert_eq!(window.requests, 0, "an event is not a request");
    }

    // The rate is the ratio of sums, not a mean of per-call ratios.
    #[test]
    fn speed_and_hit_rate_aggregate_as_ratios_of_sums() {
        let mut bucket = Bucket::default();
        bucket.add(&fold(&usage_line(1000, "turn", 990, 10, 5)));
        bucket.add(&fold(&event_line(1000, 50, 1000.0, 100.0)));
        bucket.add(&fold(&usage_line(2000, "turn", 900, 100, 5)));
        bucket.add(&fold(&event_line(2000, 150, 2000.0, 300.0)));

        let window = window_of(&bucket);
        assert_eq!(window.requests, 2);
        assert_eq!(window.calls, 2);
        // 1890 / (1890 + 110) = 94.5% — the ratio of sums, not a mean of ratios.
        assert_eq!(window.hit_rate.map(|v| v.round()), Some(95.0));
        // 200 output over 3 s of decode.
        assert_eq!(window.decode_tps.map(|v| v.round()), Some(67.0));
        assert_eq!(window.first_token_ms, Some(200));
    }

    // Two different days must land in two buckets, and the index must round-trip.
    #[test]
    fn days_are_bucketed_separately_and_the_index_round_trips() {
        let mut days: BTreeMap<i32, Bucket> = BTreeMap::new();
        absorb(&mut days, &usage_line(now_ms(), "turn", 100, 0, 1));
        absorb(
            &mut days,
            &usage_line(now_ms() - 86_400_000, "turn", 50, 0, 1),
        );
        assert_eq!(days.len(), 2);

        let state = FileState {
            len: 42,
            mtime_ms: 1_700_000_000_000,
            days,
        };
        let json = serde_json::to_string(&state).expect("index serialises");
        let back: FileState = serde_json::from_str(&json).expect("index parses");
        assert_eq!(back.len, 42);
        assert_eq!(back.days.len(), 2);
        let total: u64 = back.days.values().map(|b| b.hit).sum();
        assert_eq!(total, 150);
    }

    #[test]
    fn a_line_without_usage_costs_no_parsing() {
        let mut days: BTreeMap<i32, Bucket> = BTreeMap::new();
        assert!(!absorb(&mut days, &line(r#"{"text":"hello"}"#)));
        assert!(!absorb(&mut days, &line(r#"{"usage":"not-an-object"}"#)));
        assert!(!absorb(&mut days, &line("not json at all")));
        assert!(days.is_empty());
    }

    #[test]
    fn day_keys_are_local_and_ordered() {
        let ms = now_ms();
        let today = day_key(ms).expect("today has a key");
        let yesterday = day_key(ms - 86_400_000).expect("yesterday has a key");
        assert!(yesterday < today);
    }
}
