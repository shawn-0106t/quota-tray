// `--login-host`: the DeepSeek sign-in window as a short-lived CHILD process
// (HANDOFF §4). The parent spawns `current_exe() --login-host`; this mode owns a
// tao event loop and exactly one WebView2 window, captures the browser token
// through a cancelled navigation, verifies it with a real `amount` request,
// writes the DPAPI-sealed credential file and exits.
//
// The page gets no IPC surface: the only thing leaving it is a navigation to
// `quotatray://capture?…`, which this host cancels (PLAN §5.2 / §9). The window
// is temporary by design — a real `Window`, built on demand, destroyed when it
// closes, so the next sign-in starts from a clean console session.
//
// stdout is the machine-readable contract the parent reads (`--test-login`):
//
//   login-host: window ready
//   login-host: sender=ok|missing      the hook reached the host
//   login-host: fetch=ok|missing       the wrapper saw a bearer on a fetch
//   login-host: reject=ok|missing      a bad candidate was refused, file intact
//   login-host: verdict=PASS|FAIL
//   login-host: saved=1|0              real sign-in path only
//
// Everything else goes to the shared log (`slint.log`, `crate::logf`) and
// stderr. Exit codes: 0 accepted / probe PASS, 2 window or webview build
// failed, 3 closed or cancelled, 4 verified but the credential file could not
// be written, 6 the probe timed out.

use crate::logf;
use quota_core::deepseek::{client, credentials};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use url::Url;
use wry::{WebContext, WebViewBuilder};

const CONSOLE_URL: &str = "https://platform.deepseek.com/";
const CAPTURE_SCHEME: &str = "quotatray";
const TITLE: &str = "DeepSeek sign-in";
const PROBE_TOKEN: &str = "probe-token-that-is-long-enough-000";
/// How long after start the probe script is fired: the console page has to be
/// up, or `window.fetch` is not the wrapper's yet.
const PROBE_FIRE_MS: u64 = 4500;
/// Budgets, measured from the event that starts them — never from process start
/// (building the WebView2 environment can block for a minute on a cold profile).
const PROBE_CAPTURE_TIMEOUT: Duration = Duration::from_secs(30);
const PROBE_REJECT_TIMEOUT: Duration = Duration::from_secs(25);

pub struct Options {
    pub probe: bool,
    pub data_dir: Option<String>,
    pub parent_pid: Option<u32>,
}

enum UserEvent {
    Capture(Vec<(String, String)>),
    Verified(Result<(), String>, String),
}

pub fn run(opts: Options) -> i32 {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = match WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(900.0, 680.0))
        .with_min_inner_size(LogicalSize::new(640.0, 480.0))
        // Same flag as every other window here: the console must open ON TOP of
        // the tray overlays, or the sheet the user just clicked "Sign in…" in
        // keeps covering it (PLAN §4).
        .with_always_on_top(true)
        // Centre first, show after: building visible flashes the window at the
        // default corner before the position lands.
        .with_visible(false)
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            logf(format!("login-host: window build failed: {e}"));
            return 2;
        }
    };
    center(&window);
    window.set_visible(true);

    let data_dir = match &opts.data_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => default_data_dir(),
    };
    logf(format!(
        "login-host: start probe={} data-dir={}",
        opts.probe,
        data_dir.display()
    ));
    // The WebContext owns the WebView2 environment; leak it so it outlives the
    // webview inside the blocking event loop.
    let ctx: &'static mut WebContext = Box::leak(Box::new(WebContext::new(Some(data_dir))));

    let nav_proxy = proxy.clone();
    let builder = WebViewBuilder::new_with_web_context(ctx)
        .with_url(CONSOLE_URL)
        .with_initialization_script(HOOK_JS)
        .with_navigation_handler(move |url| {
            if !url.starts_with(&format!("{CAPTURE_SCHEME}:")) {
                return true;
            }
            let pairs = Url::parse(&url)
                .map(|u| {
                    u.query_pairs()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let _ = nav_proxy.send_event(UserEvent::Capture(pairs));
            false
        });
    let webview = match builder.build(&window) {
        Ok(w) => w,
        Err(e) => {
            logf(format!("login-host: webview build failed: {e}"));
            return 2;
        }
    };
    line("login-host: window ready");

    let probe = opts.probe;
    let parent_pid = opts.parent_pid;
    let started = Instant::now();
    let baseline = digest();
    let mut pending: Option<String> = None;
    let mut verifying = false;
    // Number of token candidates the page has reported. A transient retry
    // captures this before its pause and drops itself if a newer candidate
    // arrived meanwhile (the page never resends a token it already reported).
    let candidates = Arc::new(AtomicU64::new(0));
    let mut accepted = false;
    let mut refused = false;
    // The candidate a transient failure already re-tried once. Keyed by value
    // so a queued candidate starts with a fresh budget without any reset.
    let mut retried: Option<String> = None;
    let mut close_at: Option<Instant> = None;
    let mut probe_sent = false;
    let mut probe_sent_at: Option<Instant> = None;
    let mut probe_sender = false;
    let mut probe_fetch = false;
    let mut probe_reject = false;
    let mut probe_deadline: Option<Instant> = None;

    let handler_proxy = proxy.clone();
    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(120));
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                logf("login-host: window closed by user");
                std::process::exit(if accepted {
                    0
                } else if refused {
                    4
                } else {
                    3
                });
            }
            Event::UserEvent(UserEvent::Capture(pairs)) => {
                let kind = field(&pairs, "kind");
                let value = field(&pairs, "token");
                match kind.as_str() {
                    "probe" => {
                        probe_sender = true;
                        probe_deadline = Some(Instant::now() + PROBE_CAPTURE_TIMEOUT);
                        logf("login-host: probe capture received");
                    }
                    "token" => {
                        if probe && value != PROBE_TOKEN {
                            // A reused WebView2 profile can hand us the real
                            // console token on page load. Probe mode never
                            // writes: it is a diagnostic, not a sign-in.
                            logf("login-host: probe saw a real console token, not saving");
                            return;
                        }
                        if value.len() < 20 {
                            return;
                        }
                        if probe {
                            if value == PROBE_TOKEN {
                                probe_fetch = true;
                            }
                        } else if accepted {
                            return;
                        }
                        candidates.fetch_add(1, Ordering::SeqCst);
                        if verifying || accepted {
                            pending = Some(value);
                            return;
                        }
                        verifying = true;
                        spawn_verify(handler_proxy.clone(), value);
                    }
                    "request" => {
                        let url = field(&pairs, "url");
                        let version = field(&pairs, "appVersion");
                        // The probe's own fetch carries a marker: a diagnostic
                        // run must not teach the app an endpoint the console
                        // never used (PLAN §6.3).
                        if !url.contains("quotatray=probe") {
                            match client::learn_request(&url, &version) {
                                Some(learned) => logf(format!("login-host: learned {learned}")),
                                None => logf(format!(
                                    "login-host: usage request seen {}",
                                    safe_log_text(&url, 160)
                                )),
                            }
                        }
                    }
                    other => logf(format!(
                        "login-host: unknown capture kind {}",
                        safe_log_text(other, 32)
                    )),
                }
            }
            Event::UserEvent(UserEvent::Verified(result, value)) => {
                verifying = false;
                match result {
                    Ok(()) if probe => {
                        // Only reachable with a real token on a reused profile;
                        // reported, never written.
                        logf("login-host: probe mode refused to save a verified token");
                        rearm(&handler_proxy, &mut pending, &mut verifying);
                    }
                    Ok(()) => {
                        // The write reports its own success: inferring it from
                        // `status()` would read an OLD token (shared file, env
                        // or the private fallback) and claim a save that never
                        // happened.
                        let stored = credentials::save(None, Some(&value), None);
                        line(format!("login-host: saved={}", u8::from(stored)));
                        if stored {
                            accepted = true;
                            logf("login-host: token verified and saved");
                            window.set_title("DeepSeek sign-in — verified, saved");
                        } else {
                            // Verified but not written (disk/permission): exit 4
                            // so the parent does not claim "Signed in".
                            refused = true;
                            logf("login-host: token verified but the credential file could not be written");
                            window.set_title("DeepSeek sign-in — verified, could not save");
                        }
                        close_at = Some(Instant::now() + Duration::from_millis(900));
                    }
                    Err(error) => {
                        logf(format!("login-host: candidate rejected ({error})"));
                        if probe {
                            // Verify-before-save, proven without a human: the
                            // SERVER refused the candidate AND the stored
                            // credential is byte-identical to what it was before
                            // the run. A network error proves nothing — an
                            // offline machine must not print a passing probe.
                            // Only an explicit token ruling counts. 404/410
                            // mean the walk exhausted the endpoint candidates
                            // (PLAN §6.2) and a 403 can be a WAF page; neither
                            // proves verify-before-save ran.
                            let rejected_by_server = matches!(
                                error.as_str(),
                                "token-expired" | "http-401" | "http-403"
                            );
                            if !rejected_by_server {
                                logf(format!(
                                    "login-host: probe verification ended in {error}, not a rejection"
                                ));
                            } else if digest() == baseline {
                                probe_reject = true;
                            } else {
                                logf("login-host: probe reject check FAILED, the file changed");
                            }
                        } else if pending.is_some() {
                            // A newer candidate landed while this one was in
                            // flight (the console finishes its own sequence
                            // after the first few). Try that one before
                            // declaring failure.
                            rearm(&handler_proxy, &mut pending, &mut verifying);
                        } else if retried.as_deref() != Some(value.as_str())
                            && transient(&error)
                        {
                            // The page never resends a token it already
                            // reported (its hook dedupes by `lastToken`), so a
                            // blip on the finished token would cost the whole
                            // sign-in. Same candidate, once, after a pause —
                            // unless a newer candidate arrives meanwhile.
                            retried = Some(value.clone());
                            verifying = true;
                            let proxy = handler_proxy.clone();
                            let candidates = candidates.clone();
                            let generation = candidates.load(Ordering::SeqCst);
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(800));
                                let result = if candidates.load(Ordering::SeqCst) == generation {
                                    runtime().block_on(client::verify_user_token(&value))
                                } else {
                                    Err("superseded".to_string())
                                };
                                let _ = proxy.send_event(UserEvent::Verified(result, value));
                            });
                            logf("login-host: transient failure, re-trying the same candidate");
                        } else {
                            window.set_title(&format!(
                                "DeepSeek sign-in — {} — keep signing in",
                                explain(&error)
                            ));
                        }
                    }
                }
            }
            Event::MainEventsCleared => {
                if probe && !probe_sent && started.elapsed() > Duration::from_millis(PROBE_FIRE_MS)
                {
                    probe_sent = true;
                    probe_sent_at = Some(Instant::now());
                    logf("login-host: firing probe script");
                    if let Err(e) = webview.evaluate_script(PROBE_JS) {
                        logf(format!("login-host: probe eval failed: {e}"));
                    }
                }
                if probe && probe_sent {
                    if probe_sender && probe_fetch && probe_reject {
                        print_verdict(probe_sender, probe_fetch, probe_reject, true);
                        std::process::exit(0);
                    }
                    let expired = probe_deadline.is_some_and(|d| Instant::now() > d)
                        || probe_sent_at.is_some_and(|t| {
                            t.elapsed() > PROBE_REJECT_TIMEOUT + PROBE_CAPTURE_TIMEOUT
                        });
                    if expired {
                        logf(format!(
                            "login-host: probe timed out ({})",
                            probe_verdict(probe_sender, probe_fetch, probe_reject)
                        ));
                        print_verdict(probe_sender, probe_fetch, probe_reject, false);
                        std::process::exit(6);
                    }
                }
                if let Some(at) = close_at {
                    if Instant::now() >= at {
                        // 0 saved, 4 verified but not written, 3 anything else.
                        std::process::exit(if accepted {
                            0
                        } else if refused {
                            4
                        } else {
                            3
                        });
                    }
                }
                if let Some(pid) = parent_pid {
                    if !parent_alive(pid) {
                        logf("login-host: parent exited, closing");
                        std::process::exit(3);
                    }
                }
            }
            _ => {}
        }
    });
}

/// A failure that says nothing about the token itself: the same candidate is
/// worth one more try, where `token-expired` is not.
fn transient(error: &str) -> bool {
    // A 5xx arrives as `server-error` (usage_error maps it), so a raw `http-5…`
    // code cannot come out of verify_user_token; listing it would be dead code.
    matches!(error, "network-error" | "rate-limited" | "server-error")
}

fn rearm(proxy: &EventLoopProxy<UserEvent>, pending: &mut Option<String>, verifying: &mut bool) {
    match pending.take() {
        Some(next) => {
            *verifying = true;
            spawn_verify(proxy.clone(), next);
        }
        None => *verifying = false,
    }
}

fn spawn_verify(proxy: EventLoopProxy<UserEvent>, token: String) {
    std::thread::spawn(move || {
        let result = runtime().block_on(client::verify_user_token(&token));
        let _ = proxy.send_event(UserEvent::Verified(result, token));
    });
}

/// One worker thread is enough: verification is single-flight, and this keeps
/// the EventLoopProxy deliveries in a fixed order.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("login-host runtime")
    })
}

fn field(pairs: &[(String, String)], key: &str) -> String {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn probe_verdict(sender: bool, fetch: bool, reject: bool) -> String {
    format!(
        "sender={} fetch={} reject={}",
        ok_str(sender),
        ok_str(fetch),
        ok_str(reject)
    )
}

fn print_verdict(sender: bool, fetch: bool, reject: bool, pass: bool) {
    line(format!("login-host: sender={}", ok_str(sender)));
    line(format!("login-host: fetch={}", ok_str(fetch)));
    line(format!("login-host: reject={}", ok_str(reject)));
    line(format!(
        "login-host: verdict={}",
        if pass { "PASS" } else { "FAIL" }
    ));
}

fn ok_str(value: bool) -> &'static str {
    if value {
        "ok"
    } else {
        "missing"
    }
}

/// stdout is line-buffered, but the parent closes the pipe the moment it has the
/// verdict; flush so a line can never sit in the buffer while the process exits.
/// Page-controlled text (capture kind, fetched URL) lands in `slint.log`;
/// strip control characters and cap the length so a hostile page cannot forge
/// a log line with `%0A`. Same idea as the Tauri shell's sanitiser.
fn safe_log_text(text: &str, max: usize) -> String {
    text.chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .take(max)
        .collect()
}

fn line(text: impl Into<String>) {
    println!("{}", text.into());
    let _ = std::io::stdout().flush();
}

/// Byte digest of every credential file (shared first, then the private
/// fallback). "missing" for a path that does not exist — the probe's proof is
/// that the bytes did not change, so it has to read the files the save path
/// actually writes.
fn digest() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for path in quota_core::deepseek::credentials::files() {
        path.hash(&mut hasher);
        match std::fs::read(&path) {
            Ok(bytes) => bytes.hash(&mut hasher),
            Err(_) => "missing".hash(&mut hasher),
        }
    }
    format!("{:016x}", hasher.finish())
}

fn center(window: &tao::window::Window) {
    if let Some(monitor) = window.primary_monitor() {
        let screen = monitor.size();
        // Outer size, not the inner one: the title bar is part of the window and
        // the position API addresses the outer rectangle.
        let outer = window.outer_size();
        let x = (screen.width as i32 - outer.width as i32) / 2;
        let y = (screen.height as i32 - outer.height as i32) / 2;
        window.set_outer_position(tao::dpi::PhysicalPosition::new(x.max(0), y.max(0)));
    }
}

/// Is the process that spawned us still alive? The parent exits before we do on
/// every shutdown path, so this check is the whole "no orphan console" guarantee.
fn parent_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, STILL_ACTIVE,
    };
    use windows::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let mut code = 0u32;
                let ok = GetExitCodeProcess(handle, &mut code).is_ok();
                let _ = CloseHandle(handle);
                ok && code == STILL_ACTIVE.0 as u32
            }
            // A pid that no longer exists fails with ERROR_INVALID_PARAMETER —
            // that IS the dead case, and treating it as alive (the old blanket
            // `Err(_) => true`) meant the console outlived the app on every
            // quit. Access denied is the only genuinely unknown answer, and it
            // must not count as dead: killing a valid sign-in because the query
            // failed would be worse than an orphan.
            Err(_) => GetLastError() != ERROR_INVALID_PARAMETER,
        }
    }
}

/// WebView2 profile for the sign-in window. Mirrors the shipped app's
/// `%LOCALAPPDATA%\com.shawnqi.quotatray\EBWebView` convention (it is a cache,
/// not user data, so it does not live in the config dir) but keeps its own copy:
/// a profile cannot be opened by two browsers at once (HANDOFF §6.13), and the
/// shipped app may be running.
fn default_data_dir() -> PathBuf {
    if let Some(dir) = quota_core::app_data_dir_override() {
        // An isolated `--config-dir` run must isolate the profile too, or two
        // self-checks (and the live app) fight over one profile.
        return dir.join("EBWebView-login");
    }
    std::env::var("LOCALAPPDATA")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("com.shawnqi.quotatray")
        .join("EBWebView-login")
}

fn explain(error: &str) -> &'static str {
    match error {
        "rate-limited" => "rate limited, retry in a moment",
        "network-error" => "network error",
        "server-error" => "DeepSeek server error, retry in a moment",
        "token-expired" => "token not accepted",
        // Every candidate answering 404/410 is a moved endpoint (PLAN §6.2),
        // not a bad token — telling the user to sign in again would be a dead
        // end. Same mapping as the Tauri shell (src-tauri/src/login.rs).
        _ => "verification failed",
    }
}

/// Fired through the injected hook itself, then one real `fetch` the wrapper
/// must intercept. Keep the marker in the URL: the host refuses to learn an
/// endpoint from it.
const PROBE_JS: &str = "try { \
   window.__quotaTrayCapture && window.__quotaTrayCapture({ kind: 'probe' }); \
   window.fetch('https://platform.deepseek.com/api/v0/usage/amount?month=1&year=2000&quotatray=probe', { \
     headers: { Authorization: 'Bearer probe-token-that-is-long-enough-000' } \
   }).catch(function () {}); \
 } catch (e) {}";

/// Injected into every document of the sign-in window. Everything is wrapped:
/// a console page change must never turn into a broken page because of us.
const HOOK_JS: &str = r#"
(function () {
  if (window.__quotaTrayCapture) return;

  // The only way out of this page: a navigation to a private scheme that the
  // host cancels (PLAN §5.2). Top-level on purpose — a frame would be subject
  // to the console's own frame-src policy.
  window.__quotaTrayCapture = function (payload) {
    try {
      var query = Object.keys(payload).map(function (key) {
        return key + '=' + encodeURIComponent(payload[key]);
      }).join('&');
      window.location.href = 'quotatray://capture?' + query;
    } catch (e) {}
  };
  var send = window.__quotaTrayCapture;

  var lastToken = '';
  var deliver = function (value) {
    try {
      // Only the console's own document may hand us a token. The script runs in
      // EVERY document this window loads, and an SSO / help / ad page carries
      // its own `Authorization: Bearer …`: sending that to DeepSeek would leak
      // a third party's credential and, if it verified, replace the user's.
      if (location.origin !== 'https://platform.deepseek.com') return;
      var token = value == null ? '' : String(value);
      if (token.length < 20 || token === lastToken) return;
      lastToken = token;
      send({ kind: 'token', token: token });
    } catch (e) {}
  };
  var fromHeader = function (value) {
    if (!value) return;
    var match = /Bearer\s+(\S+)/i.exec(String(value));
    if (match) deliver(match[1]);
  };

  // 1) The token rides on the console's own requests.
  var fetchOriginal = window.fetch;
  if (typeof fetchOriginal === 'function') {
    window.fetch = function (input, init) {
      try {
        var headers = (init && init.headers) || (input && input.headers) || null;
        var target = typeof input === 'string' ? input : (input && input.url) || '';
        if (headers) {
          if (typeof headers.get === 'function') fromHeader(headers.get('Authorization'));
          else fromHeader(headers.Authorization || headers.authorization);
        }
        learnUsage(target, headers);
      } catch (e) {}
      return fetchOriginal.apply(this, arguments);
    };
  }

  // 2) Fallback the parent app also used: the console persists the token here.
  var fromStorage = function () {
    try {
      var raw = localStorage.getItem('userToken');
      if (!raw) return;
      var parsed = JSON.parse(raw);
      if (parsed && parsed.value) deliver(parsed.value);
    } catch (e) {}
  };
  fromStorage();
  [800, 2500, 6000].forEach(function (ms) { setTimeout(fromStorage, ms); });

  var setHeader = XMLHttpRequest.prototype.setRequestHeader;
  XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
    try {
      if (String(name).toLowerCase() === 'authorization') fromHeader(value);
    } catch (e) {}
    return setHeader.apply(this, arguments);
  };

  // 3) PLAN §6.3: report the shape of the console's usage request once per
  //    page, so a moved endpoint heals without a release. Never the token.
  var lastRequest = '';
  function learnUsage(target, headers) {
    try {
      var text = String(target || '');
      if (text.indexOf('/usage') < 0) return;
      var version = '';
      if (headers) {
        if (typeof headers.get === 'function') version = headers.get('x-app-version') || '';
        else version = headers['x-app-version'] || headers['X-App-Version'] || '';
      }
      var absolute = new URL(text, location.href).href;
      if (absolute.indexOf('https://platform.deepseek.com/') !== 0) return;
      var signature = absolute + '|' + version;
      if (signature === lastRequest) return;
      lastRequest = signature;
      send({ kind: 'request', url: absolute, appVersion: String(version) });
    } catch (e) {}
  }
})();
"#;
