// DeepSeek sign-in window (PLAN §4 / §5.2 / §6.3).
//
// The window IS the DeepSeek console: the user signs in normally and an
// injected hook reports the browser token the console already puts on its own
// requests (`Authorization: Bearer …`), with `localStorage.userToken` as the
// fallback the parent app also used. Nothing reads a password field or a
// cookie.
//
// The report channel is a navigation attempt to a private scheme that the host
// cancels: the remote page never gets a Tauri IPC command surface, so no
// `remote` capability and no ACL entry are involved (PLAN §9). The hook also
// forwards the URL shape of the console's own usage request so the endpoint can
// heal itself (PLAN §6.3) — never the Authorization header.
//
// A candidate is verified with one real `amount` request before it replaces the
// stored token (PLAN §5.2); the console hands out several intermediate tokens
// while signing in and only one of them is finished. Status is carried by the
// window title and the settings section, never by a dialog (PLAN §7).

use crate::deepseek::{client, credentials};
use crate::polling;
use crate::recovery;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// Temporary window label. Not part of the recovery set: it is created on
/// demand and destroyed when it closes (PLAN §4).
pub const LABEL: &str = "login";
const CONSOLE_URL: &str = "https://platform.deepseek.com/";
/// Capture scheme the hook navigates to; the host cancels every one of these.
const CAPTURE_SCHEME: &str = "quotatray";
const TITLE: &str = "DeepSeek sign-in";
const STATUS_VERIFYING: &str = "DeepSeek sign-in — verifying…";
const STATUS_SAVED: &str = "DeepSeek sign-in — verified, saved";

/// What the settings window calls (PLAN §4).
pub fn open(app: &AppHandle) {
    {
        let st = app.state::<AppState>();
        st.login.reset();
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        // Reopening: keep it where it is, but bring it forward again — a stale
        // hidden window would otherwise stay behind the topmost overlays.
        let _ = w.show();
        let _ = w.set_focus();
        recovery::log("login: sign-in window refocused");
        return;
    }
    let Ok(url) = CONSOLE_URL.parse::<tauri::Url>() else {
        recovery::log("login: console URL is not parseable");
        return;
    };
    let handler_app = app.clone();
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::External(url))
        .title(TITLE)
        .inner_size(900.0, 680.0)
        .min_inner_size(640.0, 480.0)
        .center()
        // Every other window in this app is always-on-top (the panel and the
        // settings sheet are tray overlays). Without the same flag the console
        // opens *underneath* the sheet the user just clicked "Sign in…" in,
        // which reads as "the window is not in front" — and the sheet, being
        // topmost, keeps covering the page. Created last, so it sits on top of
        // the rest of the topmost band.
        .always_on_top(true)
        .focused(true)
        // Runs before every document in this webview, including the console's
        // own navigations, so a reload never loses the capture.
        .initialization_script(HOOK_JS)
        .on_navigation(move |url| on_navigation(&handler_app, url));
    match builder.build() {
        Ok(_) => {
            // No explicit `set_focus()` here: `.focused(true)` above already
            // asked for it, and a post-build focus call is one more blocking
            // hop onto the main thread right after the webview was created —
            // the hop that used to wedge the window blank and unclosable.
            recovery::log("login: sign-in window opened");
        }
        Err(e) => recovery::log(&format!("login: window build failed: {e}")),
    }
    // The console is a real browser window; the settings sheet is a topmost
    // overlay. Leaving it up either covers the page or hides under it, so it
    // is hidden here and restored when the sign-in window closes (lib.rs).
    //
    // Deliberately *after* `build()` and off the calling thread: the sheet's
    // hide path schedules a settle timer that hops onto the main thread ~150ms
    // later, which is exactly when the new window is being created — nesting
    // the two dispatches is how a "sign in" click could hang the app with a
    // blank, unclosable window. If the build itself goes wrong, the user keeps
    // the sheet instead of losing it. A `--test-*` run is left alone: it shows
    // every window on purpose.
    if crate::app_data_dir_override().is_none() {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            crate::close_settings(handle);
        });
    }
}

/// Close the sign-in window for real: it is recreated on the next open, so a
/// stale console session cannot outlive a sign-in (PLAN §4).
pub fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}

/// Returns true when the navigation may proceed. Our scheme is always
/// cancelled: it exists only to carry one report out of the page.
fn on_navigation(app: &AppHandle, url: &tauri::Url) -> bool {
    if url.scheme() != CAPTURE_SCHEME {
        return true;
    }
    let mut kind = String::new();
    let mut token = String::new();
    let mut request_url = String::new();
    let mut app_version = String::new();
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "kind" => kind = value.into_owned(),
            "token" => token = value.into_owned(),
            "url" => request_url = value.into_owned(),
            "appVersion" => app_version = value.into_owned(),
            _ => {}
        }
    }
    match kind.as_str() {
        "token" => submit_token(app, &token),
        "request" => {
            // The probe's own fetch carries a marker: a diagnostic run must not
            // teach the app an endpoint the console never used (PLAN §6.3).
            if !request_url.contains("quotatray=probe") {
                if let Some(learned) = client::learn_request(&request_url, &app_version) {
                    recovery::log(&format!("login: learned {learned}"));
                }
            }
        }
        // `--test-login` fires this through the injected hook itself, proving
        // the page -> host channel without needing a real sign-in.
        "probe" => app
            .state::<AppState>()
            .login
            .probe
            .store(true, Ordering::SeqCst),
        other => recovery::log(&format!("login: unknown capture kind {other}")),
    }
    false
}

fn submit_token(app: &AppHandle, token: &str) {
    if token.len() < 20 {
        return;
    }
    let st = app.state::<AppState>();
    st.login.candidate.store(true, Ordering::SeqCst);
    if st.login.accepted.load(Ordering::SeqCst) {
        return;
    }
    // One verification at a time: the console emits a burst of candidates and
    // only the last one is finished. The newcomer replaces any queued token and
    // is retried when the current verification ends — dropping it would cost a
    // whole sign-in, and the page never resends a token it already reported.
    if st.login.verifying.swap(true, Ordering::SeqCst) {
        if let Ok(mut pending) = st.login.pending.lock() {
            *pending = Some(token.to_string());
        }
        return;
    }
    let owned = token.to_string();
    let epoch = st.login.epoch.load(Ordering::SeqCst);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        verify_and_save(&app, owned, epoch).await;
    });
}

/// A failure that says nothing about the token itself: the same candidate is
/// worth one more try, where `token-expired` is not.
fn transient(error: &str) -> bool {
    // A 5xx arrives as `server-error` (usage_error maps it), so a raw `http-5…`
    // code cannot come out of verify_user_token; listing it would be dead code.
    matches!(error, "network-error" | "rate-limited" | "server-error")
}

async fn verify_and_save(app: &AppHandle, mut token: String, epoch: u64) {
    // One re-try per candidate, and only for a transient failure. The page never
    // resends a token it already reported (its hook dedupes by `lastToken`), so
    // a blip on the finished token used to cost the whole sign-in.
    let mut retried = false;
    loop {
        set_title(app, STATUS_VERIFYING);
        let result = client::verify_user_token(&token).await;
        let st = app.state::<AppState>();
        match result {
            Ok(()) => {
                // Clear ran while this verification was in flight: the user
                // asked for no credential at all, so do not put one back.
                if st.login.epoch.load(Ordering::SeqCst) != epoch {
                    st.login.verifying.store(false, Ordering::SeqCst);
                    recovery::log("login: cleared while verifying, candidate dropped");
                    return;
                }
                st.login.accepted.store(true, Ordering::SeqCst);
                credentials::save(None, Some(&token), None);
                set_title(app, STATUS_SAVED);
                recovery::log("login: candidate verified and saved");
                // The settings section re-reads its status; the panel asks for
                // new numbers so the user sees the effect of signing in.
                let _ = app.emit("deepseek-credentials-changed", ());
                st.login.verifying.store(false, Ordering::SeqCst);
                polling::safe_refresh_deepseek(app).await;
                tokio::time::sleep(Duration::from_millis(900)).await;
                close(app);
                return;
            }
            Err(error) => {
                recovery::log(&format!("login: candidate rejected ({error})"));
                // A newer candidate may have landed while this one was in
                // flight (the console finishes its own sequence after the first
                // few). Try that one before declaring failure.
                let next = st.login.pending.lock().ok().and_then(|mut p| p.take());
                if let Some(next) = next {
                    token = next;
                    retried = false;
                    continue;
                }
                if !retried && transient(&error) {
                    retried = true;
                    recovery::log("login: transient failure, re-trying the same candidate");
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    // A newer candidate that arrived during the pause is the
                    // better one to try: it is the console's later, more
                    // finished token, and the page never resends anything.
                    if let Some(next) = st.login.pending.lock().ok().and_then(|mut p| p.take()) {
                        token = next;
                        retried = false;
                    }
                    continue;
                }
                // Closing and reopening the window bumps the epoch: a future
                // that outlived its window must not write a title into the new
                // one.
                if st.login.epoch.load(Ordering::SeqCst) != epoch {
                    st.login.verifying.store(false, Ordering::SeqCst);
                    recovery::log("login: window replaced while verifying, result dropped");
                    return;
                }
                st.login.verifying.store(false, Ordering::SeqCst);
                set_title(
                    app,
                    &format!("DeepSeek sign-in — {} — keep signing in", explain(&error)),
                );
                return;
            }
        }
    }
}

fn set_title(app: &AppHandle, text: &str) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.set_title(text);
    }
}

/// Error code -> short window-title text (PLAN §5.2 codes).
fn explain(error: &str) -> &'static str {
    match error {
        "rate-limited" => "rate limited, retry in a moment",
        "network-error" => "network error",
        "server-error" => "DeepSeek server error, retry in a moment",
        _ => "token not accepted",
    }
}

// ---- --test-login ----

/// Fire the hook's own sender with a probe payload, then make one `fetch` call
/// the wrapper cannot miss. `--test-login` evals this in the live page and then
/// checks both flags: the channel and the fetch interception are the two halves
/// that a console rewrite would break silently.
pub fn probe_script() -> &'static str {
    "try { \
       window.__quotaTrayCapture && window.__quotaTrayCapture({ kind: 'probe' }); \
       window.fetch('https://platform.deepseek.com/api/v0/usage/amount?month=1&year=2000&quotatray=probe', { \
         headers: { Authorization: 'Bearer probe-token-that-is-long-enough-000' } \
       }).catch(function () {}); \
     } catch (e) {}"
}

pub fn probe_seen(app: &AppHandle) -> bool {
    app.state::<AppState>().login.probe.load(Ordering::SeqCst)
}

pub fn candidate_seen(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .login
        .candidate
        .load(Ordering::SeqCst)
}

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

#[cfg(test)]
mod tests {
    use super::transient;

    // The list is the whole retry budget: a real refusal must never be retried,
    // and a transport blip always is. `usage_error` maps every 5xx to
    // `server-error`, so a raw `http-5…` never reaches this predicate.
    #[test]
    fn only_transport_failures_are_retried() {
        for retry in ["network-error", "rate-limited", "server-error"] {
            assert!(transient(retry), "{retry}");
        }
        for final_answer in ["token-expired", "http-404", "http-400", "http-410"] {
            assert!(!transient(final_answer), "{final_answer}");
        }
    }
}
