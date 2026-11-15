// `--login-host`: the DeepSeek sign-in window as a short-lived child process
// (HANDOFF §4). The parent spawns `current_exe() --login-host`; this mode owns a
// tao event loop and exactly one WebView2 window, captures the browser token
// through a cancelled navigation, verifies it with a real `amount` request and
// writes the DPAPI-sealed credential file, then exits.
//
// The page gets no IPC surface: the only thing leaving it is a navigation to
// `quotatray://capture?…`, which the host cancels (PLAN §5.2 / §9).
//
// Probe mode (`--probe`) proves the channel without a human signing in: the
// injected hook's own sender is fired from the page, and one real `fetch` with
// a fake bearer is made so the header-interception half is exercised too.

use crate::{credentials, logf, verify};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use url::Url;
use wry::{WebContext, WebViewBuilder};

const CONSOLE_URL: &str = "https://platform.deepseek.com/";
const CAPTURE_SCHEME: &str = "quotatray";
const TITLE: &str = "DeepSeek sign-in";
const PROBE_TOKEN: &str = "probe-token-that-is-long-enough-000";

pub struct Options {
    pub probe: bool,
    pub data_dir: String,
    pub parent_pid: Option<u32>,
    pub config_dir: Option<String>,
}

enum UserEvent {
    Capture(Vec<(String, String)>),
    Verified(Result<(), String>, String),
}

pub fn run(opts: Options) -> i32 {
    if let Some(dir) = &opts.config_dir {
        std::env::set_var("P0_CONFIG_DIR", dir);
    }

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = match WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(900.0, 680.0))
        .with_min_inner_size(LogicalSize::new(640.0, 480.0))
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

    let data_dir = if opts.data_dir.is_empty() {
        default_data_dir()
    } else {
        PathBuf::from(&opts.data_dir)
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

    let probe = opts.probe;
    let parent_pid = opts.parent_pid;
    let started = Instant::now();
    let mut pending: Option<String> = None;
    let mut verifying = false;
    let mut accepted = false;
    let mut close_at: Option<Instant> = None;
    let mut probe_sent = false;
    let mut probe_sent_at: Option<Instant> = None;
    let mut probe_ok = false;
    let mut probe_fetch_ok = false;
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
                std::process::exit(if accepted { 0 } else { 3 });
            }
            Event::UserEvent(UserEvent::Capture(pairs)) => {
                let kind = pairs
                    .iter()
                    .find(|(k, _)| k == "kind")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let value = pairs
                    .iter()
                    .find(|(k, _)| k == "token")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                match kind.as_str() {
                    "probe" => {
                        probe_ok = true;
                        probe_deadline = Some(Instant::now() + Duration::from_secs(6));
                        logf("login-host: probe capture received");
                    }
                    "token" => {
                        if probe {
                            if value == PROBE_TOKEN {
                                probe_fetch_ok = true;
                                logf("login-host: probe fetch capture received");
                            }
                            if probe_ok && probe_fetch_ok {
                                println!("login-host: sender=ok fetch=ok");
                                std::process::exit(0);
                            }
                        } else if value.len() >= 20 {
                            if verifying || accepted {
                                pending = Some(value);
                            } else {
                                verifying = true;
                                let p = handler_proxy.clone();
                                std::thread::spawn(move || {
                                    let r = verify::verify_user_token(&value);
                                    let _ = p.send_event(UserEvent::Verified(r, value));
                                });
                            }
                        }
                    }
                    "request" => {
                        let url = pairs
                            .iter()
                            .find(|(k, _)| k == "url")
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default();
                        let app_version = pairs
                            .iter()
                            .find(|(k, _)| k == "appVersion")
                            .map(|(_, v)| v.clone())
                            .unwrap_or_default();
                        if !url.contains("quotatray=probe") {
                            logf(format!(
                                "login-host: usage request seen {url} (x-app-version={app_version})"
                            ));
                        }
                    }
                    other => logf(format!("login-host: unknown capture kind {other}")),
                }
            }
            Event::UserEvent(UserEvent::Verified(result, value)) => match result {
                Ok(()) => {
                    accepted = true;
                    verifying = false;
                    if credentials::save_user_token(&value) {
                        println!("login-host: saved=1 dpapi=1");
                        logf("login-host: token verified and saved");
                        window.set_title("DeepSeek sign-in — verified, saved");
                    } else {
                        println!("login-host: saved=0");
                        logf("login-host: token verified but the credential file could not be written");
                    }
                    close_at = Some(Instant::now() + Duration::from_millis(900));
                }
                Err(error) => {
                    logf(format!("login-host: candidate rejected ({error})"));
                    match pending.take() {
                        Some(next) => {
                            let p = handler_proxy.clone();
                            std::thread::spawn(move || {
                                let r = verify::verify_user_token(&next);
                                let _ = p.send_event(UserEvent::Verified(r, next));
                            });
                        }
                        None => {
                            verifying = false;
                            window.set_title(&format!(
                                "DeepSeek sign-in — {} — keep signing in",
                                explain(&error)
                            ));
                        }
                    }
                }
            },
            Event::MainEventsCleared => {
                if probe && !probe_sent && started.elapsed() > Duration::from_millis(4500) {
                    probe_sent = true;
                    probe_sent_at = Some(Instant::now());
                    logf("login-host: firing probe script");
                    if let Err(e) = webview.evaluate_script(PROBE_JS) {
                        logf(format!("login-host: probe eval failed: {e}"));
                    }
                }
                if probe {
                    if let Some(deadline) = probe_deadline {
                        if Instant::now() > deadline {
                            println!(
                                "login-host: sender={} fetch={} (timeout)",
                                if probe_ok { "ok" } else { "missing" },
                                if probe_fetch_ok { "ok" } else { "missing" }
                            );
                            std::process::exit(6);
                        }
                    }
                    // Never judge "page never loaded" from process start:
                    // building the WebView2 environment can block for a minute
                    // on a cold profile, and the probe only runs after that.
                    if probe_sent_at.is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
                        println!("login-host: sender=missing fetch=missing (probe unanswered)");
                        std::process::exit(6);
                    }
                }
                if let Some(at) = close_at {
                    if Instant::now() >= at {
                        std::process::exit(if accepted { 0 } else { 3 });
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

fn parent_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
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
            // Unknown (access denied) must not count as dead: killing a valid
            // sign-in because the query failed would be worse than an orphan.
            Err(_) => true,
        }
    }
}

fn default_data_dir() -> PathBuf {
    // WebView2's default per-executable folder. A production shell would rather
    // keep this next to the config dir; the spike only needs it to be stable.
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("QuotaTrayP0")
        .join("EBWebView")
}

fn explain(error: &str) -> &'static str {
    match error {
        "rate-limited" => "rate limited, retry in a moment",
        "network-error" => "network error",
        "server-error" => "DeepSeek server error, retry in a moment",
        "endpoint-moved" => "usage endpoint moved",
        _ => "token not accepted",
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
  //    page. P0 only logs it; persisting the learned base is P1/P4 work.
  //    Never the token.
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
