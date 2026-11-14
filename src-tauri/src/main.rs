// Entry point. Self-check args (--test-fetch / --test-update / --test-deepseek
// --test-ui / --test-login / --test-usage) run BEFORE any GUI or single-instance logic,
// mirroring App.OnStartup (SPEC 19).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Named mutex (PLAN 7): a second Quota Tray launch exits silently. Deliberately
// a new name, so the merged app can run alongside the two parent apps it was
// built from during the migration. The tauri-plugin-single-instance mutex
// alone uses a different name and would not cover this case.
fn acquire_single_instance_mutex() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        match CreateMutexW(None, true, w!("QuotaTray.SingleInstance")) {
            Ok(h) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    return false;
                }
                // Leak the handle on purpose: it must stay open until process
                // exit, or the mutex would be released early. (HANDLE is Copy
                // with no Drop, so this is about expressing intent.)
                let _keep_alive: &'static _ = Box::leak(Box::new(h));
                true
            }
            Err(_) => true, // mutex creation failure must not block startup
        }
    }
}

fn dotnet_bool(b: bool) -> &'static str {
    // C# string interpolation prints True/False
    if b {
        "True"
    } else {
        "False"
    }
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    rt.block_on(fut)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Headless quota self-check: fetch once, print indented JSON, exit
    if args.iter().any(|a| a == "--test-fetch") {
        let r = block_on(quota_tray_lib::quota::fetch());
        match serde_json::to_string_pretty(&r) {
            Ok(j) => println!("{j}"),
            Err(e) => println!("serialize error: {e}"),
        }
        return;
    }

    // Headless DeepSeek self-check: one fetch, JSON to stdout. `--verbose`
    // adds the PLAN §6.1 diagnostics (credential source, endpoints, response
    // shape, per-model aggregation, /models hits) on stderr so stdout stays
    // pipeable. Neither stream ever carries a token or an API key.
    if args.iter().any(|a| a == "--test-deepseek") {
        let verbose = args.iter().any(|a| a == "--verbose");
        let (r, diag) = block_on(quota_tray_lib::deepseek::fetch_diag(verbose));
        for line in diag {
            eprintln!("{line}");
        }
        match serde_json::to_string_pretty(&r) {
            Ok(j) => println!("{j}"),
            Err(e) => println!("serialize error: {e}"),
        }
        return;
    }

    // Headless update-check self-check: single-line summary, exit
    if args.iter().any(|a| a == "--test-update") {
        let st = block_on(quota_tray_lib::update::check());
        println!(
            "local={} latest={} updateAvailable={} checkFailed={}",
            st.local_version.as_deref().unwrap_or(""),
            st.latest_version.as_deref().unwrap_or(""),
            dotnet_bool(st.update_available),
            dotnet_bool(st.check_failed),
        );
        return;
    }

    // Headless Kimi cache/usage self-check: scan the local session logs twice
    if args.iter().any(|a| a == "--test-usage") {
        quota_tray_lib::run_usage_test();
        return;
    }

    // Headless UI self-check: construct the four windows, print OK lines, exit
    if args.iter().any(|a| a == "--test-ui") {
        quota_tray_lib::run_ui_test();
        return;
    }

    // Headless DeepSeek sign-in self-check (PLAN §8): verify the configured
    // token, then build the sign-in window and prove its capture channel
    if args.iter().any(|a| a == "--test-login") {
        quota_tray_lib::run_login_test(args.iter().any(|a| a == "--verbose"));
        return;
    }

    if !acquire_single_instance_mutex() {
        return; // another Quota Tray instance is already running
    }

    quota_tray_lib::run();
}
