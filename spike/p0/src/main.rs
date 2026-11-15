// P0 interaction spike — dispatch.
//
// The interactive shell (tray + panel + motion + blur-hide) is the default mode.
// `--login-host` is the same executable re-entered as a short-lived child that
// owns one WebView2 console window (HANDOFF §4). Every `--test-*` mode runs
// before the single-instance mutex, mirroring the shipped app.

mod config;
mod credentials;
mod login_host;
mod shell;
mod verify;

slint::include_modules!();

use std::io::Write;
use std::sync::{Mutex, OnceLock};

pub fn logf(msg: impl Into<String>) {
    let msg = msg.into();
    let line = format!("[{:>8.1}ms] {msg}", t0().elapsed().as_secs_f64() * 1000.0);
    eprintln!("{line}");
    static FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
    let file = FILE.get_or_init(|| {
        let path = std::env::var("P0_LOG")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("quota-tray-p0.log"));
        Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap_or_else(|_| {
                    std::fs::File::create(std::env::temp_dir().join("p0.log")).unwrap()
                }),
        )
    });
    if let Ok(mut f) = file.lock() {
        let _ = writeln!(f, "{line}");
    }
}

fn t0() -> &'static std::time::Instant {
    static T0: OnceLock<std::time::Instant> = OnceLock::new();
    T0.get_or_init(std::time::Instant::now)
}

fn arg(args: &[String], name: &str, default: &str) -> String {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn acquire_single_instance_mutex() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        match CreateMutexW(None, true, w!("QuotaTray.P0.SingleInstance")) {
            Ok(h) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    return false;
                }
                let _keep_alive: &'static _ = Box::leak(Box::new(h));
                true
            }
            Err(_) => true,
        }
    }
}

fn run_login_probe(args: &[String]) -> i32 {
    let probe_dir = std::env::temp_dir().join("quota-tray-p0-login-probe");
    let _ = std::fs::remove_dir_all(&probe_dir);
    let _ = std::fs::create_dir_all(&probe_dir);
    let Ok(exe) = std::env::current_exe() else {
        println!("test-login: FAIL (no current_exe)");
        return 1;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--login-host").arg("--probe");
    cmd.arg("--config-dir").arg(&probe_dir);
    let data_dir = arg(args, "--data-dir", "");
    if !data_dir.is_empty() {
        cmd.arg("--data-dir").arg(data_dir);
    } else {
        // Never share a WebView2 profile with another live instance (the shipped
        // app holds the Tauri one): the probe only needs a loadable console.
        let dir =
            std::env::temp_dir().join(format!("quota-tray-p0-webview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        cmd.arg("--data-dir").arg(dir);
    }
    logf(format!(
        "test-login: spawning probe with config-dir {}",
        probe_dir.display()
    ));
    match cmd.status() {
        Ok(status) => {
            let code = status.code().unwrap_or(-1);
            println!(
                "test-login: exit={code} verdict={}",
                if code == 0 { "PASS" } else { "FAIL" }
            );
            if code == 0 {
                0
            } else {
                1
            }
        }
        Err(e) => {
            println!("test-login: FAIL (spawn: {e})");
            1
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Route every config read (this process and the sign-in child) before the
    // shell creates anything: P0_CONFIG_DIR is what keeps a probe out of the
    // shipped app's credential file.
    let config_dir = arg(&args, "--config-dir", "");
    if !config_dir.is_empty() {
        std::env::set_var("P0_CONFIG_DIR", config_dir);
    }
    let mut data_dir = arg(&args, "--data-dir", "");
    if data_dir.is_empty() && arg(&args, "--profile", "") == "tauri" {
        // Reuse the shipped app's WebView2 profile: if the console session is
        // still live there, the child captures a real token without a human
        // signing in again.
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if !local.is_empty() {
                data_dir = format!(r"{local}\com.shawnqi.quotatray\EBWebView");
            }
        }
    }
    if !data_dir.is_empty() {
        std::env::set_var("P0_DATA_DIR", data_dir);
    }

    // The sign-in child owns its own tao event loop and exits when it is done;
    // it must not touch the single-instance mutex or create a Slint backend.
    if flag(&args, "--login-host") {
        let code = login_host::run(login_host::Options {
            probe: flag(&args, "--probe"),
            data_dir: arg(&args, "--data-dir", ""),
            parent_pid: arg(&args, "--parent-pid", "").parse().ok(),
            config_dir: std::env::var("P0_CONFIG_DIR").ok(),
        });
        std::process::exit(code);
    }

    // Headless sign-in channel check: re-enter this exe as the child with
    // `--probe`, wait for its verdict, exit with it.
    if flag(&args, "--test-login") {
        let code = run_login_probe(&args);
        std::process::exit(code);
    }

    let mode = if flag(&args, "--test-tab-anim") {
        shell::Mode::TabAnim
    } else if flag(&args, "--test-blur") {
        shell::Mode::Blur
    } else if flag(&args, "--test-tray") {
        shell::Mode::Tray
    } else if flag(&args, "--test-toggle") {
        shell::Mode::Toggle
    } else if flag(&args, "--hold") {
        shell::Mode::Hold
    } else if flag(&args, "--test-tray-click") {
        shell::Mode::TrayClick
    } else if flag(&args, "--tray-probe") {
        shell::Mode::SlintTrayProbe
    } else {
        shell::Mode::Interactive
    };

    if mode == shell::Mode::Interactive && !acquire_single_instance_mutex() {
        return;
    }

    let opts = shell::Options {
        mode,
        dark: arg(&args, "--theme", "light") == "dark",
        start_ds: arg(&args, "--tab", "kimi") == "ds",
        scale: arg(&args, "--scale", "").parse().ok(),
        seconds: arg(&args, "--seconds", "0").parse().unwrap_or(0),
        out_dir: arg(&args, "--out", ""),
        demo_switch: flag(&args, "--demo-switch"),
    };
    if let Err(e) = shell::run(opts) {
        logf(format!("fatal: {e}"));
        std::process::exit(1);
    }
}
