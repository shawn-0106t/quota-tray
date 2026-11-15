// Quota Tray — Slint shell (P2: the real panel).
//
// The data layer is `quota-core`; this crate is only the shell: one transparent
// window, a `tray-icon` tray, the tab switch controller and the self-checks.
// Every `--test-*` mode runs before the single-instance mutex, mirroring the
// shipped Tauri app, so a check never fights a live instance.

mod app;
mod checks;
mod height;
mod shell;
mod view;

slint::include_modules!();

use std::io::Write;
use std::sync::{Mutex, OnceLock};

pub fn logf(msg: impl Into<String>) {
    let msg = msg.into();
    let line = format!("[{:>8.1}ms] {msg}", t0().elapsed().as_secs_f64() * 1000.0);
    eprintln!("{line}");
    static FILE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();
    let file = FILE.get_or_init(|| {
        let path = std::env::var("SLINT_LOG")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| quota_core::settings::config_dir().join("slint.log"));
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap_or_else(|_| {
                    std::fs::File::create(std::env::temp_dir().join("slint.log")).unwrap()
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
    // Deliberately not `QuotaTray.SingleInstance`: the shipped Tauri app owns
    // that name while both shells exist, and the spike must be able to run next
    // to it. Flip this to the canonical name when the Slint shell becomes the
    // release product (PLAN §7).
    unsafe {
        match CreateMutexW(None, true, w!("QuotaTray.Slint.SingleInstance")) {
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

fn install_panic_log() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "?".to_string());
        logf(format!("panic: {info} ({location})"));
    }));
}

fn main() {
    install_panic_log();
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Diagnostic isolation: redirect every config read (settings, credentials,
    // caches, log) before anything touches the profile.
    let config_dir = arg(&args, "--config-dir", "");
    if !config_dir.is_empty() {
        std::env::set_var("QUOTA_TRAY_APP_DATA_DIR", config_dir);
    }

    let mode = if flag(&args, "--test-heights") {
        shell::Mode::Heights
    } else if flag(&args, "--test-corner") {
        shell::Mode::Corner
    } else if flag(&args, "--test-active-tab") {
        shell::Mode::ActiveTab
    } else if flag(&args, "--test-tab-anim") {
        shell::Mode::TabAnim
    } else if flag(&args, "--test-blur") {
        shell::Mode::Blur
    } else if flag(&args, "--test-tray-click") {
        shell::Mode::TrayClick
    } else if flag(&args, "--test-tray") {
        shell::Mode::Tray
    } else if flag(&args, "--hold") {
        shell::Mode::Hold
    } else if flag(&args, "--test-clamp") {
        shell::Mode::Clamp
    } else {
        shell::Mode::Interactive
    };

    if mode == shell::Mode::ActiveTab && std::env::var("QUOTA_TRAY_APP_DATA_DIR").is_err() {
        let dir = std::env::temp_dir().join("quota-tray-slint-active-tab");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("QUOTA_TRAY_APP_DATA_DIR", &dir);
    }

    if mode == shell::Mode::Interactive && !acquire_single_instance_mutex() {
        logf("another instance is running");
        return;
    }

    let theme = arg(&args, "--theme", "");
    let tab = arg(&args, "--tab", "");
    let variant = arg(&args, "--variant", "");
    let opts = shell::Options {
        mode,
        theme: (!theme.is_empty()).then_some(theme),
        tab: (!tab.is_empty()).then_some(tab),
        variant: (!variant.is_empty()).then_some(variant),
        seconds: arg(&args, "--seconds", "0").parse().unwrap_or(0),
        out_dir: arg(&args, "--out", ""),
        demo_switch: flag(&args, "--demo-switch"),
        max_window_h: arg(&args, "--max-window-h", "").parse().ok(),
        no_poll: flag(&args, "--no-poll"),
    };
    if let Err(e) = shell::run(opts) {
        logf(format!("fatal: {e}"));
        std::process::exit(1);
    }
}
