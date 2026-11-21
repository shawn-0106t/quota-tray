// DeepSeek sign-in: the parent half (PLAN §4 / §5.2).
//
// The console window is a CHILD process (`--login-host`, `login_host.rs`): it
// owns its own tao event loop and WebView2 window, so the Slint loop never
// nests a second event loop and a wedged console cannot freeze the tray. The
// parent only spawns it, hides the settings sheet while it runs, and reads the
// child's exit code when it closes.
//
// Clear is the one interaction that crosses both processes, and it is ordered,
// not sequenced by an epoch: `settings::clear` calls `login::cancel` (kill AND
// wait, so the process is gone) and only then writes the empty credential file,
// so a captured token can never land after the wipe. The parent deliberately has
// no epoch check of its own here — `cred_epoch` is also bumped by a pasted
// credential, so "the epoch moved" would wipe what the user just saved by hand.
//
// `--test-login` lives here too: it is the shipped command, re-pointed at the
// child process — the credential source, one real `amount` request against the
// stored token, then a sign-in window whose capture channel is fired from
// inside the page and reports its verdict on stdout.

use crate::logf;
use crate::shell::Shell;
use crate::win::settings as settings_win;
use quota_core::deepseek::{self, credentials};
use slint::{ComponentHandle, Timer};
use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// The child mode's flag, dispatched in `main.rs` before any Slint window.
pub const HOST_FLAG: &str = "--login-host";
/// How often the parent looks for the child's exit.
const POLL_MS: u64 = 250;
/// Probe-mode stdout line the child prints once the WebView2 window exists.
const LINE_READY: &str = "login-host: window ready";

const NOTE_SIGNED_IN: &str = "Signed in — the usage token the console already has is stored.";
const NOTE_CANCELLED: &str = "Sign-in cancelled.";
const NOTE_FAILED: &str = "The sign-in window could not open.";
const NOTE_TIMEOUT: &str = "The sign-in window timed out.";
const NOTE_NOT_SAVED: &str = "Verified, but the credential file could not be written — try again.";

/// An open sign-in window (one at a time: two children would fight over the
/// WebView2 profile, HANDOFF §6.13).
#[derive(Default)]
pub struct Login {
    pub child: Option<Child>,
    /// The child was terminated by the parent instead of exiting on its own
    /// (`kill_child`, used when a console cannot answer `WM_CLOSE`). Its exit
    /// status is `1` in that case, which must read as "cancelled", not "failed".
    pub killed: bool,
    /// The sheet was open when Sign in was pressed: it comes back afterwards.
    pub reopen_sheet: bool,
    /// The parent's verdict on the last sign-in, for the status line.
    pub last_code: Option<Option<i32>>,
}

// ---------------------------------------------------------------------------
// open / cancel / settle
// ---------------------------------------------------------------------------

/// "Sign in…": spawn the console window and get the sheet out of its way.
pub fn open(shell: &Rc<RefCell<Shell>>) {
    if is_running(shell) {
        // A second click used to be a dead button. The console is a real window
        // and can sit behind everything; bring it forward instead (the shipped
        // app refocuses its existing window the same way).
        logf("login: a sign-in window is already open");
        focus_console(shell);
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        logf("login: current_exe() is unavailable");
        note(shell, NOTE_FAILED, true);
        return;
    };
    let mut cmd = Command::new(exe);
    cmd.arg(HOST_FLAG)
        .arg("--parent-pid")
        .arg(std::process::id().to_string());
    match cmd.spawn() {
        Ok(child) => {
            let reopened = {
                let mut s = shell.borrow_mut();
                let was_open = s.settings_open;
                if was_open {
                    // The sheet is a topmost overlay and the console is a real
                    // window: leaving the sheet up either covers the page or
                    // hides under it (PLAN §4). It comes back on exit.
                    s.settings_open = false;
                    let _ = s.settings_ui.hide();
                }
                s.login.child = Some(child);
                s.login.killed = false;
                s.login.reopen_sheet = was_open;
                s.login.last_code = None;
                was_open
            };
            logf(format!(
                "login: sign-in window spawned pid={:?} sheet-hidden={reopened}",
                shell.borrow().login.child.as_ref().map(Child::id)
            ));
            schedule_exit_check(shell);
        }
        Err(e) => {
            logf(format!("login: spawn failed: {e}"));
            note(shell, NOTE_FAILED, true);
        }
    }
}

/// Kill an open sign-in window (Clear, PLAN §4). Returns whether one was open.
pub fn cancel(shell: &Rc<RefCell<Shell>>) -> bool {
    let taken = shell.borrow_mut().login.child.take();
    let Some(mut child) = taken else { return false };
    let _ = child.kill();
    let _ = child.wait();
    shell.borrow_mut().login.reopen_sheet = false;
    logf(format!("login: sign-in window pid={} killed", child.id()));
    true
}

/// Is the child process still running? (`try_wait` also reaps a zombie.)
pub fn is_running(shell: &Rc<RefCell<Shell>>) -> bool {
    let mut s = shell.borrow_mut();
    let Some(child) = s.login.child.as_mut() else {
        return false;
    };
    matches!(child.try_wait(), Ok(None))
}

/// Kill the child WITHOUT the Clear semantics: it stays in its slot, so the
/// exit poll sees the exit and settles as usual. Used when a console cannot
/// answer `WM_CLOSE` because its event loop is still inside WebView2's cold
/// start (`--test-signin`), which is a real state a wedged console can reach.
pub fn kill_child(shell: &Rc<RefCell<Shell>>) -> bool {
    let mut s = shell.borrow_mut();
    let Some(child) = s.login.child.as_mut() else {
        return false;
    };
    let pid = child.id();
    let killed = child.kill().is_ok();
    s.login.killed = killed;
    crate::logf(format!("login: sign-in window pid={pid} killed"));
    killed
}

pub fn pid(shell: &Rc<RefCell<Shell>>) -> Option<u32> {
    shell.borrow().login.child.as_ref().map(Child::id)
}

/// The console window `pid` owns, if it exists yet.
///
/// Enumerated instead of looked up by title: `FindWindowW` returns the first
/// title match in z-order, so a console left behind by an earlier run would hide
/// the live one (and the `--test-signin` poll would never see its own window).
pub fn console_hwnd(pid: u32) -> Option<isize> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId};

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut (u32, Option<isize>)) };
        let mut owner = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner));
        }
        if owner == state.0 {
            state.1 = Some(hwnd.0 as isize);
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut state: (u32, Option<isize>) = (pid, None);
    unsafe {
        let _ = EnumWindows(
            Some(callback),
            LPARAM(&mut state as *mut (u32, Option<isize>) as isize),
        );
    }
    state.1
}

fn focus_console(shell: &Rc<RefCell<Shell>>) {
    let Some(pid) = pid(shell) else { return };
    match console_hwnd(pid) {
        Some(hwnd) => {
            crate::shell::focus(hwnd);
            logf("login: existing console brought to the front");
        }
        None => logf("login: the console window is not created yet"),
    }
}

/// Poll for the exit on the UI thread. One chained single-shot instead of a
/// repeating timer: the chain simply stops re-arming once the child is gone,
/// including when Clear took it.
fn schedule_exit_check(shell: &Rc<RefCell<Shell>>) {
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(POLL_MS), move || {
        let Some(shell) = weak.upgrade() else { return };
        if shell.borrow().login.child.is_none() {
            return;
        }
        let Some(code) = take_exit(&shell) else {
            schedule_exit_check(&shell);
            return;
        };
        settle(&shell, code);
    });
}

/// The child's exit code, once it has one (`None` while it still runs).
fn take_exit(shell: &Rc<RefCell<Shell>>) -> Option<Option<i32>> {
    let mut s = shell.borrow_mut();
    let child = s.login.child.as_mut()?;
    match child.try_wait() {
        Ok(Some(status)) => {
            let code = status.code();
            s.login.child = None;
            Some(code)
        }
        Ok(None) => None,
        Err(e) => {
            logf(format!("login: try_wait failed: {e}"));
            s.login.child = None;
            Some(None)
        }
    }
}

/// The sign-in window closed: write the status line, refresh the numbers and
/// bring the sheet back.
///
/// Nothing is cleared here on purpose — see the module header. `Clear` already
/// guarantees its own ordering (kill, wait, then wipe), and a parent-side epoch
/// check would also fire for a credential the user pasted by hand while the
/// console was open.
fn settle(shell: &Rc<RefCell<Shell>>, code: Option<i32>) {
    let (reopen, killed) = {
        let mut s = shell.borrow_mut();
        let killed = std::mem::take(&mut s.login.killed);
        let reopen = std::mem::take(&mut s.login.reopen_sheet);
        s.login.last_code = Some(code);
        (reopen, killed)
    };
    let text = match code {
        Some(0) => NOTE_SIGNED_IN,
        Some(4) => NOTE_NOT_SAVED,
        Some(2) => NOTE_FAILED,
        Some(6) => NOTE_TIMEOUT,
        Some(3) => NOTE_CANCELLED,
        // `Child::kill` terminates with status 1: the console did not exit on
        // its own, but from the user's side it was cancelled all the same.
        _ if killed => NOTE_CANCELLED,
        _ => NOTE_FAILED,
    };
    let bad = !killed && !matches!(code, Some(0) | Some(3));
    logf(format!(
        "login: exited code={code:?} killed={killed} reopen={reopen}"
    ));
    if reopen && !shell.borrow().settings_open {
        settings_win::open(shell);
    }
    note(shell, text, bad);
    // The balance/usage pollers hold the old token (or a failure): ask again.
    // `notify_one`, not `notify_waiters`: that one only wakes a waiter that is
    // registered *right now*, and a poller in the middle of `fetch().await` has
    // not registered yet. `notify_one` leaves a permit behind, so the next wait
    // returns immediately instead of sleeping a whole interval.
    shell.borrow().workers.refresh();
}

/// Write the sheet's DeepSeek note. `apply_status` regenerates the env note,
/// so a transient one has to land after it.
fn note(shell: &Rc<RefCell<Shell>>, text: &str, bad: bool) {
    let s = shell.borrow();
    if !s.settings_open {
        // Nothing on screen; the next open re-reads the status anyway.
        return;
    }
    settings_win::apply_status(&s);
    s.settings_ui.set_ds_note(text.into());
    s.settings_ui.set_ds_note_bad(bad);
    drop(s);
    settings_win::manual_toggled(shell);
}

// ---------------------------------------------------------------------------
// --test-login
// ---------------------------------------------------------------------------

/// `--test-login` (PLAN §8). Same three checks as the shipped command, in the
/// same order: credential source, one real `amount` request against the stored
/// token, then a sign-in window driven by the injected hook itself. Prints
/// `LoginWindow …` lines and returns the process exit code.
pub fn run_test(verbose: bool) -> i32 {
    let status = credentials::status();
    println!("credentialSource={}", status.source.as_str());
    println!("apiKeyConfigured={}", status.api_key);
    println!("userTokenConfigured={}", status.user_token);
    if verbose {
        match deepseek::client::load_usage_base() {
            Some(base) => println!("usageBase={base}"),
            None => println!("usageBase=(not learned yet)"),
        }
    }
    let stored = credentials::load().user_token;
    match stored {
        Some(token) => match block_on(deepseek::client::verify_user_token(&token)) {
            Ok(()) => println!("verify=ok"),
            Err(error) => println!("verify={error}"),
        },
        None => println!("verify=skipped(no-user-token)"),
    }
    println!("test-login: (verify is informational; the verdict below is the capture channel)");
    run_probe()
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("test-login runtime")
        .block_on(fut)
}

/// Spawn `--login-host --probe` and translate its stdout into the shipped
/// `LoginWindow …` lines.
///
/// The probe reuses one WebView2 profile in `%TEMP%`, because a cold start here
/// measured 175 s and a warm one a few seconds. A profile can be left locked by
/// a browser process from an interrupted run (`HRESULT(0x800700AA)`, HANDOFF
/// §6.13); the child then exits before its window is ready and the run is
/// retried once on a wiped profile.
fn run_probe() -> i32 {
    let shared = std::env::temp_dir().join("quota-tray-slint-login-probe");
    let mut probe = probe_once(&shared);
    if !probe.ready {
        // The shared profile can be held by a browser process left over from an
        // interrupted run (`0x800700AA`) or by a concurrent self-check. A private
        // directory always works; it costs a cold WebView2 start, so it is only
        // the fallback. Never wipe the shared one: another process may be using
        // it right now.
        logf("test-login: no window on the shared probe profile, retrying on a private one");
        let private = std::env::temp_dir().join(format!(
            "quota-tray-slint-login-probe-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&private);
        probe = probe_once(&private);
        let _ = std::fs::remove_dir_all(&private);
    }
    if probe.ready {
        println!("LoginWindow OK");
    }
    if probe.sender {
        println!("LoginWindow capture channel OK");
    }
    if probe.fetch {
        println!("LoginWindow fetch capture OK");
    }
    if probe.reject {
        println!("LoginWindow rejects a bad candidate OK");
    }
    for (ok, what) in [
        (probe.ready, "window"),
        (probe.sender, "capture channel"),
        (probe.fetch, "fetch capture"),
        (probe.reject, "bad-candidate rejection"),
    ] {
        if !ok {
            println!("LOGIN-FAIL: LoginWindow: {what} missing");
        }
    }
    if probe.verdict.is_none() {
        println!("LOGIN-FAIL: LoginWindow: the probe never reached a verdict");
    }
    let pass = probe.passed();
    println!(
        "test-login: window={} sender={} fetch={} reject={} verdict={}",
        ok(probe.ready),
        ok(probe.sender),
        ok(probe.fetch),
        ok(probe.reject),
        if pass { "PASS" } else { "FAIL" }
    );
    if pass {
        0
    } else {
        1
    }
}

/// What one probe run managed to prove.
#[derive(Default)]
struct Probe {
    ready: bool,
    sender: bool,
    fetch: bool,
    reject: bool,
    verdict: Option<bool>,
}

impl Probe {
    fn passed(&self) -> bool {
        self.verdict == Some(true) && self.ready && self.sender && self.fetch && self.reject
    }
}

fn probe_once(data_dir: &std::path::Path) -> Probe {
    let mut probe = Probe::default();
    let Ok(exe) = std::env::current_exe() else {
        println!("LOGIN-FAIL: LoginWindow: current_exe() is unavailable");
        return probe;
    };
    let mut cmd = Command::new(exe);
    cmd.arg(HOST_FLAG)
        .arg("--probe")
        .arg("--data-dir")
        .arg(data_dir)
        // The probe child must not outlive an interrupted --test-login: without
        // this it stays inside WebView2's cold start and keeps the profile
        // locked for the next run.
        .arg("--parent-pid")
        .arg(std::process::id().to_string())
        .stdout(Stdio::piped())
        // The child's own log lines stay visible: a failed build is diagnosed
        // from them, and its file copy goes to the shared slint.log anyway.
        .stderr(Stdio::inherit());
    logf(format!(
        "test-login: spawning probe data-dir={}",
        data_dir.display()
    ));
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            println!("LOGIN-FAIL: LoginWindow: spawn failed ({e})");
            return probe;
        }
    };
    let stdout = child.stdout.take();
    let (tx, rx) = mpsc::channel::<String>();
    if let Some(stdout) = stdout {
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
    }

    let deadline = Instant::now() + Duration::from_secs(240);
    while probe.verdict.is_none() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let line = match rx.recv_timeout(left) {
            Ok(line) => line,
            // A closed pipe means the child died without a verdict; waiting out
            // the rest of the budget would make a failure look like a hang.
            Err(_) => break,
        };
        match line.as_str() {
            LINE_READY => probe.ready = true,
            "login-host: sender=ok" => probe.sender = true,
            "login-host: fetch=ok" => probe.fetch = true,
            "login-host: reject=ok" => probe.reject = true,
            "login-host: verdict=PASS" => probe.verdict = Some(true),
            "login-host: verdict=FAIL" => probe.verdict = Some(false),
            // `sender=missing` and friends: the verdict line that follows (or
            // the summary) carries the failure.
            other if other.starts_with("login-host:") => logf(format!("test-login: {other}")),
            other => logf(format!("test-login: probe stdout: {other}")),
        }
    }
    // A leaked console window would keep the profile and its WebView2 processes
    // alive: a self-check must clean up after itself.
    if !matches!(child.try_wait(), Ok(Some(_))) {
        let _ = child.kill();
    }
    let _ = child.wait();
    probe
}

fn ok(value: bool) -> &'static str {
    if value {
        "ok"
    } else {
        "missing"
    }
}
