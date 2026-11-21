// Self-check modes: the acceptance evidence for P2's panel, collected from the
// real window (rects, latency) instead of from the UI's own opinion.
//
//   --test-heights     data variant -> card/window height (vs spike/p2/heights-ref.json)
//   --test-corner      work-area corner anchoring at the live DPI
//   --test-active-tab  ActiveTab persistence round trip in an isolated config dir
//   --test-tab-anim    30 clicks, bottom-edge drift
//   --test-blur        focus loss -> hidden latency
//   --test-tray-click  posted tray callback messages
//   --test-close       WM_CLOSE (Alt+F4) hides, flags stay in step, windows reopen
//   --hold             keep the panel on screen (screenshots)
use crate::app::{Data, Msg};
use crate::height::{GAP, MARGIN, PANEL_W};
use crate::shell::{self, Shell};
use crate::view;
use crate::Panel;
use chrono::{Duration as ChronoDuration, Local};
use quota_core::deepseek::{BalanceInfo, DeepseekResult, ModelUsage, UsageInfo};
use quota_core::kimi_usage::{CacheWindow, KimiUsage};
use quota_core::quota::{ExtraInfo, ExtraState, QuotaResult, QuotaSegment};
use quota_core::settings;
use quota_core::update::UpdateStatus;
use slint::{ComponentHandle, Timer};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

/// Variant names are the contract with `spike/p2/heights_ref.py`: the same name
/// paints the same pane state in the browser reference and in the shell.
pub const VARIANTS: [&str; 17] = [
    "kimi-full",
    "kimi-nocache",
    "kimi-cache-only",
    "kimi-extra-monthly",
    "kimi-not-activated",
    "kimi-no-badge",
    "ds-4",
    "ds-3",
    "ds-2",
    "ds-1",
    "ds-hint",
    "ds-hint-short",
    "ds-hint-rate",
    "ds-hint-empty",
    "ds-balance-only",
    "ds-nokey",
    "ds-failed",
];

pub fn is_ds_variant(name: &str) -> bool {
    name.starts_with("ds-")
}

/// Paint one variant's data into `data` (the panel's real view mapper then turns
/// it into the strings the user sees).
pub fn apply_variant(data: &mut Data, name: &str) {
    // Window-only variants: `skills-demo` feeds the skills window a synthetic
    // list (see start_shots) and must not touch the panel's data.
    if name == "skills-demo" {
        return;
    }
    let now = Local::now();
    data.settings.active_tab = if is_ds_variant(name) {
        "deepseek"
    } else {
        "kimi"
    }
    .to_string();
    // Every variant starts from a clean slate: no stale half of a previous one.
    data.quota = None;
    data.deepseek = None;
    data.usage = None;
    data.update = UpdateStatus {
        local_version: Some("0.39.1".to_string()),
        latest_version: Some("0.40.0".to_string()),
        update_available: true,
        check_failed: false,
    };
    let mut extra = ExtraState::NoData;
    let mut monthly = false;
    let mut cache = true;
    let mut speed = true;

    match name {
        "kimi-nocache" => {
            cache = false;
            speed = false;
        }
        "kimi-cache-only" => speed = false,
        "kimi-extra-monthly" => extra = ExtraState::Ready,
        "kimi-not-activated" => extra = ExtraState::NotActivated,
        "kimi-no-badge" => data.update.update_available = false,
        _ => {}
    }
    if name.starts_with("kimi-") {
        let extra = match extra {
            ExtraState::Ready => {
                monthly = true;
                ExtraInfo {
                    state: ExtraState::Ready,
                    balance_cents: Some(4544),
                    monthly_enabled: true,
                    monthly_used_cents: Some(45),
                    monthly_limit_cents: Some(100),
                }
            }
            ExtraState::NotActivated => ExtraInfo {
                state: ExtraState::NotActivated,
                balance_cents: None,
                monthly_enabled: false,
                monthly_used_cents: None,
                monthly_limit_cents: None,
            },
            ExtraState::NoData => ExtraInfo {
                state: ExtraState::NoData,
                balance_cents: None,
                monthly_enabled: false,
                monthly_used_cents: None,
                monthly_limit_cents: None,
            },
        };
        data.quota = Some(QuotaResult {
            five_hour: Some(QuotaSegment {
                percent: 31.0,
                reset_at: Some(now + ChronoDuration::minutes(213)),
            }),
            week: Some(QuotaSegment {
                percent: 12.0,
                reset_at: Some(now + ChronoDuration::minutes(2 * 24 * 60 + 18 * 60)),
            }),
            extra: Some(extra),
            fetched_at: now,
            error: None,
        });
        let _ = monthly;
        if cache {
            data.usage = Some(KimiUsage {
                week: CacheWindow {
                    requests: 3169,
                    hit_rate: Some(99.5),
                    decode_tps: if speed { Some(135.8) } else { None },
                    first_token_ms: if speed { Some(1040) } else { None },
                    calls: 3166,
                },
                today: CacheWindow {
                    requests: 807,
                    hit_rate: Some(99.7),
                    decode_tps: None,
                    first_token_ms: None,
                    calls: 807,
                },
                scanned_at: now,
                files: 6579,
                scan_ms: 98,
            });
        }
        return;
    }

    let balance = Some(BalanceInfo {
        is_available: true,
        currency: "CNY".to_string(),
        total: 45.44,
        granted: 0.0,
        topped_up: 45.44,
    });
    let models = |n: usize| -> Vec<ModelUsage> {
        let all = [
            (
                "DeepSeek-V4.1-Flash",
                "DeepSeek-V4.1-Flash",
                665_797_039_i64,
                28.503_168_64_f64,
                Some(99.6),
                4.531_104,
            ),
            (
                "deepseek-v4.1-pro",
                "DeepSeek-V4.1-Pro",
                184_200_311,
                12.882_041_17,
                Some(71.3),
                1.902_244,
            ),
            (
                "deepseek-v3.2",
                "DeepSeek-V3.2",
                41_203_311,
                2.134_099_22,
                Some(58.4),
                0.301_188,
            ),
            (
                "deepseek-v4.1-reasoner-pro-max",
                "DeepSeek-V4.1-Reasoner-Pro-Max-Extended",
                9_033_112,
                0.882_011_33,
                None,
                0.112_004,
            ),
        ];
        all.iter()
            .take(n)
            .map(|(model, name, tokens, cost, hit, today)| ModelUsage {
                model: (*model).to_string(),
                display_name: (*name).to_string(),
                month_tokens: *tokens,
                month_requests: 0,
                month_cost: *cost,
                cache_hit_rate: *hit,
                today_tokens: 0,
                today_cost: *today,
            })
            .collect()
    };
    let usage = |rows: Vec<ModelUsage>| UsageInfo {
        month_tokens: 897_000_000,
        month_requests: 5975,
        month_cost: 44.4,
        cache_hit_rate: Some(97.0),
        today_tokens: 0,
        today_cost: 6.85,
        by_model: rows,
    };

    let mut result = DeepseekResult {
        balance,
        usage: None,
        balance_error: None,
        usage_error: None,
        fetched_at: now,
    };
    match name {
        "ds-1" => result.usage = Some(usage(models(1))),
        "ds-2" => result.usage = Some(usage(models(2))),
        "ds-3" => result.usage = Some(usage(models(3))),
        "ds-4" => result.usage = Some(usage(models(4))),
        "ds-hint" => result.usage_error = Some("token-expired".to_string()),
        "ds-hint-short" => result.usage_error = Some("no-user-token".to_string()),
        "ds-hint-rate" => result.usage_error = Some("rate-limited".to_string()),
        "ds-hint-empty" => result.usage = Some(usage(Vec::new())),
        "ds-balance-only" => {}
        "ds-nokey" => {
            result.balance = None;
            result.usage_error = Some("no-user-token".to_string());
            result.balance_error = Some("no-api-key".to_string());
        }
        "ds-failed" => {
            result.balance = None;
            result.balance_error = Some("http-500".to_string());
            result.usage_error = Some("network-error".to_string());
        }
        _ => {}
    }
    data.deepseek = Some(result);
}

/// `--test-heights`: print what the Slint layout derived for every variant.
/// `spike/p2/heights_ref.py --compare` diffs it against the DOM.
pub fn print_heights(ui: &Panel) -> Result<(), Box<dyn std::error::Error>> {
    let mut data = Data::load();
    for name in VARIANTS {
        apply_variant(&mut data, name);
        view::apply(ui, &data);
        let win = if is_ds_variant(name) {
            ui.get_natural_ds()
        } else {
            ui.get_natural_kimi()
        };
        println!("height: {name} card={:.2} win={:.2}", win - 56.0, win);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// --test-corner
// ---------------------------------------------------------------------------

pub fn start_corner(shell: &Rc<RefCell<Shell>>) {
    shell::show_panel(shell);
    shell::spawn_sampler(shell);
    // Ten switches 120ms apart: the pre-size/settle pair runs on every one.
    let clicks = Rc::new(std::cell::Cell::new(0u32));
    let weak = std::rc::Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(120),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            let n = clicks.get();
            if n >= 10 {
                return;
            }
            clicks.set(n + 1);
            if shell.borrow().visible {
                let ds = !shell.borrow().ui.get_tab_ds();
                shell::request_tab(&shell, ds);
            }
        },
    );
    shell.borrow_mut().checks.timers.push(Rc::new(timer));

    let weak = std::rc::Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(3200), move || {
        let Some(shell) = weak.upgrade() else { return };
        report_corner(&shell);
        let _ = slint::quit_event_loop();
    });
}

fn report_corner(shell: &Rc<RefCell<Shell>>) {
    let s = shell.borrow();
    let scale = s.ui.window().scale_factor() as f64;
    let (_, _, wa_right, wa_bottom) = shell::work_area_phys();
    let visible: Vec<&shell::Sample> = s.checks.samples.iter().filter(|x| x.visible).collect();
    if visible.len() < 50 {
        println!(
            "corner: verdict=FAIL (only {} visible samples; the tab switches did not run)",
            visible.len()
        );
        return;
    }
    let card_right = |x: &shell::Sample| x.right as f64 - f64::from(MARGIN) * scale;
    let card_bottom = |x: &shell::Sample| x.bottom as f64 - f64::from(MARGIN) * scale;
    let right_gap = wa_right - card_right(visible[0]);
    let bottom_gap = wa_bottom - card_bottom(visible[0]);
    let right_min = visible
        .iter()
        .map(|x| card_right(x))
        .fold(f64::MAX, f64::min);
    let right_max = visible
        .iter()
        .map(|x| card_right(x))
        .fold(f64::MIN, f64::max);
    let bottom_min = visible
        .iter()
        .map(|x| card_bottom(x))
        .fold(f64::MAX, f64::min);
    let bottom_max = visible
        .iter()
        .map(|x| card_bottom(x))
        .fold(f64::MIN, f64::max);
    let want = f64::from(GAP) * scale;
    let want_w = f64::from(PANEL_W) * scale;
    let window_w = visible[0].right as f64 - visible[0].left as f64;
    let ok = (right_gap - want).abs() <= 1.0
        && (bottom_gap - want).abs() <= 1.0
        && (right_max - right_min) <= 1.0
        && (bottom_max - bottom_min) <= 1.0;
    println!(
        "corner: scale={scale:.2} window-w={window_w:.0} want-w={want_w:.0} \
         card-right-gap={right_gap:.1} card-bottom-gap={bottom_gap:.1} want-gap={want:.1} \
         right-drift={:.0} bottom-drift={:.0} samples={} verdict={}",
        right_max - right_min,
        bottom_max - bottom_min,
        visible.len(),
        if ok { "PASS" } else { "FAIL" }
    );
    crate::logf(format!(
        "corner: right=[{right_min:.1}..{right_max:.1}] bottom=[{bottom_min:.1}..{bottom_max:.1}]"
    ));
}

// ---------------------------------------------------------------------------
// --test-active-tab
// ---------------------------------------------------------------------------

pub fn start_active_tab(shell: &Rc<RefCell<Shell>>) {
    let dir = std::path::PathBuf::from(std::env::var("QUOTA_TRAY_APP_DATA_DIR").unwrap_or_else(
        |_| {
            std::env::temp_dir()
                .join("quota-tray-slint-active-tab")
                .to_string_lossy()
                .into_owned()
        },
    ));
    let file = dir.join("settings.json");
    let failures = Rc::new(std::cell::Cell::new(0u32));
    // The "fresh start" assertion only means something when the check owns a
    // new profile: a rerun in the same --config-dir starts with the previous
    // run's tab. Back the file up and restore it at the end.
    let settings_backup = std::fs::read(&file).ok();
    let fresh = settings_backup.is_none();

    let initial = shell.borrow().data.settings.active_tab.clone();
    if !fresh {
        println!(
            "active-tab: existing settings.json (ActiveTab={initial:?}) — fresh-start check skipped"
        );
    } else if initial == "kimi" {
        println!("active-tab: fresh start reads kimi ok");
    } else {
        failures.set(failures.get() + 1);
        println!("active-tab: fresh start read {initial:?}, want kimi FAIL");
    }

    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(50), move || {
        let Some(shell) = weak.upgrade() else { return };
        shell::request_tab(&shell, true);
    });

    let weak = Rc::downgrade(shell);
    let f1 = failures.clone();
    let file_ds = file.clone();
    Timer::single_shot(Duration::from_millis(700), move || {
        let Some(shell) = weak.upgrade() else { return };
        f1.set(f1.get() + check_persisted(&file_ds, "deepseek", "switch to deepseek"));
        shell::request_tab(&shell, false);
    });

    let weak = Rc::downgrade(shell);
    let f2 = failures.clone();
    let file_kimi = file.clone();
    let dir_check = dir.clone();
    Timer::single_shot(Duration::from_millis(1400), move || {
        let Some(shell) = weak.upgrade() else { return };
        let _ = &shell;
        f2.set(f2.get() + check_persisted(&file_kimi, "kimi", "switch back to kimi"));
        // A settings.json written before ActiveTab existed must still load, and
        // a saved tab must survive the round trip (PLAN §7).
        let legacy = r#"{"Theme":"dark","RefreshMinutes":5,"AutoStart":true}"#;
        if std::fs::write(dir_check.join("settings.json"), legacy).is_ok() {
            let loaded = settings::load();
            if loaded.active_tab == "kimi" && loaded.theme == "dark" {
                println!("active-tab: legacy file without ActiveTab loads kimi ok");
            } else {
                f2.set(f2.get() + 1);
                println!(
                    "active-tab: legacy file loaded {:?} FAIL",
                    loaded.active_tab
                );
            }
        }
        let saved =
            r#"{"Theme":"light","RefreshMinutes":30,"AutoStart":false,"ActiveTab":"deepseek"}"#;
        if std::fs::write(dir_check.join("settings.json"), saved).is_ok() {
            let loaded = settings::load();
            if loaded.active_tab == "deepseek" {
                println!("active-tab: saved deepseek tab round-trips ok");
            } else {
                f2.set(f2.get() + 1);
                println!("active-tab: saved tab loaded {:?} FAIL", loaded.active_tab);
            }
        }
        println!(
            "active-tab: verdict={}",
            if f2.get() == 0 { "PASS" } else { "FAIL" }
        );
        match settings_backup {
            Some(bytes) => {
                let _ = std::fs::write(&file, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&file);
            }
        }
        let _ = slint::quit_event_loop();
    });
}

fn check_persisted(file: &std::path::Path, want: &str, label: &str) -> u32 {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let ok = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| {
            v.get("ActiveTab")
                .and_then(|t| t.as_str())
                .map(str::to_string)
        })
        .is_some_and(|tab| tab == want);
    if ok {
        println!("active-tab: {label} persisted ok");
        0
    } else {
        println!(
            "active-tab: {label} persisted {:?} FAIL (want {want})",
            text.trim()
        );
        1
    }
}

// ---------------------------------------------------------------------------
// --test-tab-anim (P0 harness, still the drift proof)
// ---------------------------------------------------------------------------

pub fn start_tab_anim(shell: &Rc<RefCell<Shell>>) {
    shell::show_panel(shell);
    shell::spawn_sampler(shell);
    let clicks = Rc::new(std::cell::Cell::new(0u32));
    let weak = Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            let n = clicks.get();
            if n >= 30 {
                return;
            }
            clicks.set(n + 1);
            {
                let mut s = shell.borrow_mut();
                s.checks.clicks_left = 30 - n - 1;
                s.checks.last_click_at = Some(Instant::now());
            }
            let ds = !shell.borrow().ui.get_tab_ds();
            shell::request_tab(&shell, ds);
        },
    );
    {
        let mut s = shell.borrow_mut();
        s.checks.clicks_left = 30;
        s.checks.timers.push(Rc::new(timer));
    }
    let weak = Rc::downgrade(shell);
    let watch = Timer::default();
    watch.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            let done = {
                let s = shell.borrow();
                s.checks.clicks_left == 0
                    && !s.switching
                    && !s.checks.reported
                    && s.checks
                        .last_click_at
                        .map(|t| t.elapsed() > Duration::from_millis(400))
                        .unwrap_or(false)
            };
            if done {
                shell.borrow_mut().checks.reported = true;
                finish_tab_anim(&shell);
            }
        },
    );
    shell.borrow_mut().checks.timers.push(Rc::new(watch));

    // A stuck `switching` flag is exactly what this check exists to catch; the
    // watcher above would wait forever for it, so bound the whole run.
    let weak = Rc::downgrade(shell);
    let guard = Timer::default();
    guard.start(
        slint::TimerMode::SingleShot,
        Duration::from_secs(12),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            if !shell.borrow().checks.reported {
                println!("tab-anim: verdict=FAIL (timed out waiting for the settle)");
                let _ = slint::quit_event_loop();
            }
        },
    );
    shell.borrow_mut().checks.timers.push(Rc::new(guard));
}

fn finish_tab_anim(shell: &Rc<RefCell<Shell>>) {
    let s = shell.borrow();
    let visible: Vec<&shell::Sample> = s.checks.samples.iter().filter(|x| x.visible).collect();
    if visible.is_empty() {
        println!("tab-anim: verdict=FAIL (no visible samples)");
        let _ = slint::quit_event_loop();
        return;
    }
    let bottom_min = visible.iter().map(|x| x.bottom).min().unwrap();
    let bottom_max = visible.iter().map(|x| x.bottom).max().unwrap();
    let right_min = visible.iter().map(|x| x.right).min().unwrap();
    let right_max = visible.iter().map(|x| x.right).max().unwrap();
    let height_min = visible.iter().map(|x| x.bottom - x.top).min().unwrap();
    let height_max = visible.iter().map(|x| x.bottom - x.top).max().unwrap();
    let drift = bottom_max - bottom_min;
    println!(
        "tab-anim: clicks=30 samples={} bottom=[{bottom_min}..{bottom_max}] drift={drift}px \
         right=[{right_min}..{right_max}] height=[{height_min}..{height_max}]",
        visible.len()
    );
    println!(
        "tab-anim: verdict {} (bottom edge drift <= 1px)",
        if drift <= 1 { "PASS" } else { "FAIL" }
    );
    let _ = slint::quit_event_loop();
}

// ---------------------------------------------------------------------------
// --test-blur
// ---------------------------------------------------------------------------

pub fn start_blur(shell: &Rc<RefCell<Shell>>) {
    shell::show_panel(shell);
    shell::spawn_sampler(shell);
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(700), move || {
        let Some(shell) = weak.upgrade() else { return };
        let hwnd = shell.borrow().hwnd;
        let Some(hwnd) = hwnd else {
            println!("blur: verdict=FAIL (no hwnd)");
            let _ = slint::quit_event_loop();
            return;
        };
        // Make sure the panel actually owns the foreground before taking it
        // away: the blur event can only fire if the panel had focus.
        shell::focus(hwnd);
        std::thread::sleep(Duration::from_millis(200));
        let start = Instant::now();
        focus_taskbar();
        let weak = Rc::downgrade(&shell);
        let seen_slot: Rc<std::cell::Cell<u128>> = Rc::new(std::cell::Cell::new(u128::MAX));
        let slot = seen_slot.clone();
        let ticks = Rc::new(std::cell::Cell::new(0u32));
        let timer = Rc::new(Timer::default());
        let stopper = timer.clone();
        let counter = ticks.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(10),
            move || {
                let Some(shell) = weak.upgrade() else { return };
                counter.set(counter.get() + 1);
                if !shell::is_visible(hwnd) && slot.get() == u128::MAX {
                    slot.set(start.elapsed().as_millis());
                }
                if counter.get() > 200 || (slot.get() != u128::MAX && counter.get() > 30) {
                    stopper.stop();
                    let value = slot.get();
                    let blur_seen = shell.borrow().checks.blur_at.is_some();
                    if value == u128::MAX {
                        println!("blur: verdict=FAIL (still visible after 2000ms)");
                    } else {
                        let budget = shell::HIDE_MS as u128 + 60;
                        println!(
                            "blur: hidden-after={value}ms budget={budget}ms blur-event={} verdict={}",
                            if blur_seen { "seen" } else { "MISSING" },
                            if blur_seen && value <= budget {
                                "PASS"
                            } else {
                                "FAIL"
                            }
                        );
                    }
                    let _ = slint::quit_event_loop();
                }
            },
        );
        shell.borrow_mut().checks.timers.push(timer);
    });
}

fn focus_taskbar() {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
    unsafe {
        if let Ok(h) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            let _ = SetForegroundWindow(h);
            crate::logf("blur: focused Shell_TrayWnd");
        }
    }
}

// ---------------------------------------------------------------------------
// --test-tray-click
// ---------------------------------------------------------------------------

pub fn start_tray_click(shell: &Rc<RefCell<Shell>>) {
    use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONUP, WM_MOUSEMOVE};

    let hwnd = shell
        .borrow()
        .tray
        .as_ref()
        .map(|t| t.window_handle() as isize)
        .unwrap_or(0);
    for (delay, msg, label) in [
        (600u64, WM_MOUSEMOVE, "hover"),
        (1000, WM_LBUTTONUP, "left click"),
    ] {
        Timer::single_shot(Duration::from_millis(delay), move || {
            if hwnd != 0 {
                post_tray_msg(hwnd, msg);
                crate::logf(format!("tray-test: posted {label}"));
            }
        });
    }
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(1800), move || {
        let Some(shell) = weak.upgrade() else { return };
        let visible = shell.borrow().visible;
        println!(
            "tray-click: panel-visible-after-click={visible} verdict={}",
            if visible { "PASS" } else { "FAIL" }
        );
        if hwnd != 0 {
            post_tray_msg(hwnd, WM_LBUTTONUP);
        }
    });
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(2600), move || {
        let Some(shell) = weak.upgrade() else { return };
        let visible = shell.borrow().visible;
        println!(
            "tray-click: panel-hidden-after-second-click={} verdict={}",
            !visible,
            if !visible { "PASS" } else { "FAIL" }
        );
        let _ = slint::quit_event_loop();
    });
}

fn post_tray_msg(hwnd: isize, msg: u32) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    const WM_USER_TRAYICON: u32 = 6002;
    let h = HWND(hwnd as *mut core::ffi::c_void);
    let _ = unsafe { PostMessageW(Some(h), WM_USER_TRAYICON, WPARAM(0), LPARAM(msg as isize)) };
}

// ---------------------------------------------------------------------------
// --test-clamp
// ---------------------------------------------------------------------------

/// Work-area cap forced low (`--max-window-h`), then the card must stay inside
/// the window and scroll: a bottom-anchored card that gets clipped instead would
/// lose the header and the provider tab first, locking the user on one tab
/// (PLAN §3.6).
pub fn start_clamp(shell: &Rc<RefCell<Shell>>) {
    shell::show_panel(shell);
    let out = shell.borrow().out_dir.clone();
    if !out.is_empty() {
        let _ = std::fs::create_dir_all(&out);
    }
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(700), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (win_h, card, content, cap, top) = {
            let s = shell.borrow();
            let scale = s.ui.window().scale_factor();
            // Slint's Window::size() is PHYSICAL px; every height the shell
            // reasons about is logical.
            let physical = s.ui.window().size().height as f32;
            let top =
                s.ui.window()
                    .take_snapshot()
                    .ok()
                    .map(|b| snapshot_parts(&b));
            (
                physical / scale,
                physical / scale - 56.0,
                s.ui.get_content_h(),
                s.max_win_h,
                top,
            )
        };
        let mut failures = 0;
        if (win_h - cap).abs() > 0.5 {
            failures += 1;
            println!("clamp: window-h={win_h:.1} want cap {cap:.1} FAIL");
        } else {
            println!("clamp: window-h={win_h:.1} == work-area cap");
        }
        if content <= card + 0.5 {
            failures += 1;
            println!("clamp: content-h={content:.1} not taller than card {card:.1} FAIL");
        } else {
            println!(
                "clamp: card={card:.1} content-h={content:.1} scrollable={:.1}",
                content - card
            );
        }
        if let Some((pixels, w, h)) = &top {
            let path = std::path::Path::new(&out).join("clamp-top.png");
            save_buffer(pixels, *w, *h, &path);
        }
        // Scroll the interior through the same property the wheel writes.
        // `content-y` is the content's offset, so it is negative when scrolled.
        shell.borrow().ui.set_scroll_y(-120.0);
        let weak = Rc::downgrade(&shell);
        let out = out.clone();
        Timer::single_shot(Duration::from_millis(250), move || {
            let Some(shell) = weak.upgrade() else { return };
            let (scrolled, scroll_y) = {
                let s = shell.borrow();
                (
                    s.ui.window()
                        .take_snapshot()
                        .ok()
                        .map(|b| snapshot_parts(&b)),
                    s.ui.get_scroll_y(),
                )
            };
            if let Some((pixels, w, h)) = &scrolled {
                let path = std::path::Path::new(&out).join("clamp-scrolled.png");
                save_buffer(pixels, *w, *h, &path);
            }
            let differs = match (&top, &scrolled) {
                (Some((a, _, _)), Some((b, _, _))) => {
                    a.len() == b.len() && a.iter().zip(b).any(|(x, y)| x != y)
                }
                _ => false,
            };
            if (scroll_y + 120.0).abs() > 0.5 || !differs {
                println!(
                    "clamp: scroll-y={scroll_y:.1} frame-differs={differs} FAIL                      (the card did not scroll)"
                );
                println!("clamp: verdict=FAIL");
            } else if failures > 0 {
                println!("clamp: verdict=FAIL");
            } else {
                println!("clamp: scroll-y={scroll_y:.1} frame-differs={differs} verdict=PASS");
            }
            let _ = slint::quit_event_loop();
        });
    });
}

/// Pixels as u32 plus the snapshot's own (physical) size — `take_snapshot`
/// hands back the window buffer, not a logical-size image.
fn snapshot_parts(buffer: &slint::SharedPixelBuffer<slint::Rgba8Pixel>) -> (Vec<u32>, u32, u32) {
    let pixels = buffer
        .as_slice()
        .iter()
        .map(|p| u32::from_le_bytes([p.r, p.g, p.b, p.a]))
        .collect();
    (pixels, buffer.width(), buffer.height())
}

fn save_buffer(pixels: &[u32], width: u32, height: u32, path: &std::path::Path) {
    let mut bytes = Vec::with_capacity(pixels.len() * 4);
    for px in pixels {
        bytes.extend_from_slice(&px.to_le_bytes());
    }
    match image::RgbaImage::from_raw(width, height, bytes) {
        Some(image) => match image.save(path) {
            Ok(()) => crate::logf(format!("clamp: snapshot {}", path.display())),
            Err(e) => crate::logf(format!("clamp: snapshot failed: {e}")),
        },
        None => crate::logf("clamp: snapshot buffer size mismatch"),
    }
}

// ---------------------------------------------------------------------------
// P3 window checks: --test-windows / --test-settings / --test-skills / --test-menu
// ---------------------------------------------------------------------------

/// `--test-windows`: construct and show all four windows (the Tauri `--test-ui`
/// equivalent). Proves resource resolution for every window in one run.
pub fn start_windows(shell: &Rc<RefCell<Shell>>) {
    shell::show_panel(shell);
    crate::win::settings::open(shell);
    crate::win::skills::open(shell);
    crate::win::menu::show(shell);
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(900), move || {
        let Some(shell) = weak.upgrade() else { return };
        let s = shell.borrow();
        let mut ok = 0;
        for (name, window) in [
            ("Panel", s.ui.window()),
            ("SettingsWindow", s.settings_ui.window()),
            ("SkillsWindow", s.skills_ui.window()),
            ("MenuWindow", s.menu_ui.window()),
        ] {
            match shell::hwnd_of(window) {
                Some(hwnd) => {
                    let (l, t, r, b) = shell::window_rect(hwnd);
                    println!(
                        "{name} OK hwnd={hwnd} rect={l},{t},{r},{b} visible={}",
                        shell::is_visible(hwnd)
                    );
                    ok += 1;
                }
                None => println!("UI-FAIL: {name}: window was not created"),
            }
        }
        println!(
            "windows: created={ok}/4 settings-open={} skills-open={} menu-open={}",
            s.settings_open, s.skills_open, s.menu_open
        );
        drop(s);
        // The panel's own Settings footer button must reach the same sheet the
        // tray menu opens (it is the other entry point into `settings::open`).
        crate::win::settings::close_from_ui(&shell);
        let ui = shell.borrow().ui.clone_strong();
        ui.invoke_action("settings".into());
    });

    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(1400), move || {
        let Some(shell) = weak.upgrade() else { return };
        let open = shell.borrow().settings_open;
        println!(
            "windows: panel action Settings reopened the sheet={open} verdict={}",
            if open { "PASS" } else { "FAIL" }
        );
        let _ = slint::quit_event_loop();
    });
}

/// `--test-settings`: the settings sheet's acceptance proof.
///
///   * defaults round trip through a REAL restart (`--print-settings` child);
///   * open backfills the controls;
///   * Save persists, applies the theme immediately, restarts the poll period
///     and closes the sheet — then a second child reads the new values back;
///   * the sheet is content-sized, centred, and scrolls inside the card when a
///     `--max-settings-h` cap forces it.
pub fn start_settings(shell: &Rc<RefCell<Shell>>) {
    // Always exercise the 90% cap: without a cap the scroll branch never runs
    // and the check used to pass while the cap was unverified.
    if shell.borrow().settings_max_override.is_none() {
        shell.borrow_mut().settings_max_override = Some(300.0);
    }
    let dir = quota_core::settings::config_dir();
    let failures = Rc::new(std::cell::Cell::new(0u32));
    let settings_file = dir.join("settings.json");
    let fresh = std::fs::read(&settings_file).is_err();
    SETTINGS_CREDENTIAL_BACKUP.with(|slot| *slot.borrow_mut() = Some(snapshot_credentials()));
    SETTINGS_JSON_BACKUP.with(|slot| *slot.borrow_mut() = Some(std::fs::read(&settings_file).ok()));

    let before = read_settings_child(&dir);
    println!("settings: restart-before {before}");
    if fresh {
        check_line(
            &failures,
            &before,
            "theme=system",
            "default theme is system",
        );
        check_line(&failures, &before, "interval=5", "default interval is 5");
        check_line(
            &failures,
            &before,
            "autostart=false",
            "default autostart is off",
        );
    } else {
        println!("settings: existing settings.json — default-value checks skipped");
    }

    // A seeded credential pair (the documented plaintext escape hatch) so the
    // DeepSeek statuses and Clear have something real to act on.
    let cred_file = dir.join("deepseek-credentials.json");
    let _ = std::fs::write(
        &cred_file,
        r#"{"v":1,"apiKey":"sk-test","userToken":"ut-test"}"#,
    );

    // 1) open: the controls carry the persisted values.
    crate::win::settings::open(shell);
    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(500), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (theme_i, interval_i, autostart, natural, content) = {
            let s = shell.borrow();
            (
                s.settings_ui.get_theme_index(),
                s.settings_ui.get_interval_index(),
                s.settings_ui.get_autostart(),
                s.settings_ui.get_natural_h(),
                s.settings_ui.get_content_h(),
            )
        };
        let loaded = quota_core::settings::load();
        let want_theme = crate::win::settings::theme_index(&loaded.theme);
        let want_interval = crate::win::settings::interval_index(loaded.refresh_minutes);
        if theme_i == want_theme && interval_i == want_interval && autostart == loaded.auto_start {
            println!("settings: backfill theme/interval/autostart ok");
        } else {
            f.set(f.get() + 1);
            println!(
                "settings: backfill theme={theme_i} interval={interval_i} autostart={autostart} want=({want_theme},{want_interval},{}) FAIL",
                loaded.auto_start
            );
        }
        println!("settings: content-h={content:.1} natural-h={natural:.1}");
        check_settings_geometry(&shell, &f);
        let (key, token) = {
            let s = shell.borrow();
            (
                s.settings_ui.get_ds_key().to_string(),
                s.settings_ui.get_ds_token().to_string(),
            )
        };
        if key == "Configured" && token == "Configured" {
            println!("settings: credential statuses read Configured ok");
        } else {
            f.set(f.get() + 1);
            println!("settings: status api={key:?} token={token:?} FAIL");
        }

        // 2) change every control, then Save.
        {
            let s = shell.borrow();
            s.settings_ui.set_theme_index(2);
            s.settings_ui.set_interval_index(2);
            s.settings_ui.set_autostart(true);
        }
        crate::win::settings::save_from_ui(&shell);
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    let dir = dir.clone();
    Timer::single_shot(Duration::from_millis(1000), move || {
        let Some(shell) = weak.upgrade() else { return };
        let loaded = quota_core::settings::load();
        if loaded.theme == "dark" && loaded.refresh_minutes == 10 && loaded.auto_start {
            println!(
                "settings: saved theme={} interval={} autostart={} ok",
                loaded.theme, loaded.refresh_minutes, loaded.auto_start
            );
        } else {
            f.set(f.get() + 1);
            println!(
                "settings: saved theme={} interval={} autostart={} FAIL",
                loaded.theme, loaded.refresh_minutes, loaded.auto_start
            );
        }
        let (open, dark, data_theme, dark_all) = {
            let s = shell.borrow();
            (
                s.settings_open,
                s.ui.global::<crate::Tokens>().get_dark(),
                s.data.theme.clone(),
                s.ui.global::<crate::Tokens>().get_dark()
                    && s.settings_ui.global::<crate::Tokens>().get_dark()
                    && s.skills_ui.global::<crate::Tokens>().get_dark()
                    && s.menu_ui.global::<crate::Tokens>().get_dark(),
            )
        };
        if open {
            f.set(f.get() + 1);
            println!("settings: window still open after Save FAIL");
        } else {
            println!("settings: closed after Save ok");
        }
        let period = shell.borrow().workers.period_seconds();
        if period == 600 {
            println!("settings: poll schedule restarted at 600s (10 min) ok");
        } else {
            f.set(f.get() + 1);
            println!("settings: poll period {period}s, want 600 FAIL");
        }
        if dark && data_theme == "dark" && dark_all {
            // Slint globals are per-component: each window owns a Tokens
            // instance, so "applied" means all four were switched.
            println!("settings: theme applied to all four windows immediately ok");
        } else {
            f.set(f.get() + 1);
            println!("settings: theme applied dark={dark} data={data_theme} FAIL");
        }
        // A theme-only message must repaint every window (the pump used to skip
        // `apply_theme` for a batch that carried nothing else).
        shell.borrow().workers.post(Msg::Theme("light".into()));
        let weak = Rc::downgrade(&shell);
        let f_theme = f.clone();
        Timer::single_shot(Duration::from_millis(300), move || {
            let Some(shell) = weak.upgrade() else { return };
            let (theme, dark_all) = {
                let s = shell.borrow();
                (
                    s.data.theme.clone(),
                    s.ui.global::<crate::Tokens>().get_dark()
                        || s.settings_ui.global::<crate::Tokens>().get_dark()
                        || s.skills_ui.global::<crate::Tokens>().get_dark()
                        || s.menu_ui.global::<crate::Tokens>().get_dark(),
                )
            };
            if theme == "light" && !dark_all {
                println!("settings: theme-only message repaints all four windows ok");
            } else {
                f_theme.set(f_theme.get() + 1);
                println!("settings: theme message theme={theme:?} any-dark={dark_all} FAIL");
            }
        });
        let after = read_settings_child(&dir);
        println!("settings: restart-after {after}");
        check_line(&f, &after, "theme=dark", "saved theme survives a restart");
        check_line(
            &f,
            &after,
            "interval=10",
            "saved interval survives a restart",
        );
        check_line(
            &f,
            &after,
            "autostart=true",
            "saved autostart survives a restart",
        );

        // 3) a forced cap: the window stops at the cap and the card scrolls.
        crate::win::settings::open(&shell);
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(1500), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (win_h, content, cap, card) = {
            let s = shell.borrow();
            let scale = s.settings_ui.window().scale_factor();
            let physical = s.settings_ui.window().size().height as f32;
            (
                physical / scale,
                s.settings_ui.get_content_h(),
                s.settings_ui.get_max_card_h(),
                (physical / scale - 56.0).max(1.0),
            )
        };
        println!("settings: capped window-h={win_h:.1} content-h={content:.1} card={card:.1} cap={cap:.1}");
        if content > card + 0.5 {
            println!("settings: card scrolls (content {content:.1} > card {card:.1}) ok");
            // Prove the scroll actually paints: drive `scroll-y` (the same
            // property the wheel writes) and compare frames.
            let top = shell
                .borrow()
                .settings_ui
                .window()
                .take_snapshot()
                .ok()
                .map(|b| frame_hash(&b));
            let title_top = shell.borrow().settings_ui.get_title_screen_y();
            shell.borrow().settings_ui.set_scroll_y(-120.0);
            let weak = Rc::downgrade(&shell);
            let f = f.clone();
            Timer::single_shot(Duration::from_millis(250), move || {
                let Some(shell) = weak.upgrade() else { return };
                let (scroll_y, title_bottom, bottom) = {
                    let s = shell.borrow();
                    (
                        s.settings_ui.get_scroll_y(),
                        s.settings_ui.get_title_screen_y(),
                        s.settings_ui
                            .window()
                            .take_snapshot()
                            .ok()
                            .map(|b| frame_hash(&b)),
                    )
                };
                let differs = matches!((top, bottom), (Some(a), Some(b)) if a != b);
                let title_fixed = (title_bottom - title_top).abs() <= 0.01;
                if (scroll_y + 120.0).abs() <= 0.5 && differs && title_fixed {
                    println!(
                        "settings: scrolled to {scroll_y:.1}, frame changed, title fixed (y={title_bottom:.1}) ok"
                    );
                } else {
                    f.set(f.get() + 1);
                    println!(
                        "settings: scroll-y={scroll_y:.1} frame-changed={differs} title-fixed={title_fixed} (y {title_top:.1}->{title_bottom:.1}) FAIL"
                    );
                }
                crate::win::settings::clear(&shell);
            });
        } else {
            println!("settings: not clamped — pass --max-settings-h 300 to exercise the cap");
            crate::win::settings::clear(&shell);
        }
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    let cred_file = cred_file.clone();
    Timer::single_shot(Duration::from_millis(2100), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (key, token, note, open) = {
            let s = shell.borrow();
            (
                s.settings_ui.get_ds_key().to_string(),
                s.settings_ui.get_ds_token().to_string(),
                s.settings_ui.get_ds_note().to_string(),
                s.settings_open,
            )
        };
        if key == "Not configured" && token == "Not configured" && note.contains("Cleared") && open
        {
            println!("settings: Clear resets both statuses and explains itself ok");
        } else {
            f.set(f.get() + 1);
            println!(
                "settings: after Clear api={key:?} token={token:?} note={note:?} open={open} FAIL"
            );
        }
        let raw = std::fs::read_to_string(&cred_file).unwrap_or_default();
        if raw.contains("\"apiKey\": \"\"") || !raw.contains("sk-test") {
            println!("settings: credential file keeps its shape but no values ok");
        } else {
            f.set(f.get() + 1);
            println!("settings: credential file still carries a value FAIL ({raw})");
        }
        // 4) the manual fallback: a wrong value must fail without being saved,
        //    and the button must recover from "Verifying…".
        crate::win::settings::save_credentials(&shell, "sk-invalid-check-key".into(), "".into());
        let verifying = shell.borrow().settings_ui.get_verifying();
        println!("settings: verification started, verifying={verifying}");
        let weak = Rc::downgrade(&shell);
        let f = f.clone();
        let cred_file = cred_file.clone();
        Timer::single_shot(Duration::from_millis(2500), move || {
            let Some(shell) = weak.upgrade() else { return };
            let (verifying, note, key) = {
                let s = shell.borrow();
                (
                    s.settings_ui.get_verifying(),
                    s.settings_ui.get_cred_note().to_string(),
                    s.settings_ui.get_ds_key().to_string(),
                )
            };
            let raw = std::fs::read_to_string(&cred_file).unwrap_or_default();
            if !verifying
                && (note.contains("not saved") || note.contains("rate limited"))
                && key == "Not configured"
                && !raw.contains("sk-invalid")
            {
                println!("settings: rejected paste stays unsaved and the button recovers ok (note={note:?})");
            } else {
                f.set(f.get() + 1);
                println!(
                    "settings: after bad paste verifying={verifying} note={note:?} key={key:?} FAIL"
                );
            }

            // 5) Clear during an in-flight verification must win: the late
            //    result is dropped by the epoch and never written.
            crate::win::settings::save_credentials(
                &shell,
                "sk-invalid-check-key".into(),
                "".into(),
            );
            crate::win::settings::clear(&shell);
            let weak = Rc::downgrade(&shell);
            let f = f.clone();
            let cred_file = cred_file.clone();
            Timer::single_shot(Duration::from_millis(2500), move || {
                let Some(shell) = weak.upgrade() else { return };
                let (verifying, note) = {
                    let s = shell.borrow();
                    (
                        s.settings_ui.get_verifying(),
                        s.settings_ui.get_ds_note().to_string(),
                    )
                };
                let raw = std::fs::read_to_string(&cred_file).unwrap_or_default();
                if !verifying && note.contains("Cleared") && !raw.contains("sk-invalid") {
                    println!("settings: Clear supersedes an in-flight verification ok");
                } else {
                    f.set(f.get() + 1);
                    println!(
                        "settings: after clear-during-verify verifying={verifying} note={note:?} FAIL"
                    );
                }
                println!(
                    "settings: verdict={}",
                    if f.get() == 0 { "PASS" } else { "FAIL" }
                );
                restore_settings_files();
                let _ = slint::quit_event_loop();
            });
        });
    });
}

/// FNV-1a over the frame's pixels: the snapshot itself cannot be moved into a
/// nested timer closure, a hash can.
fn frame_hash(buffer: &slint::SharedPixelBuffer<slint::Rgba8Pixel>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for px in buffer.as_slice() {
        for byte in [px.r, px.g, px.b, px.a] {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn read_settings_child(dir: &std::path::Path) -> String {
    let Ok(exe) = std::env::current_exe() else {
        return "child: exe path unavailable".to_string();
    };
    match std::process::Command::new(exe)
        .arg("--print-settings")
        .arg("--config-dir")
        .arg(dir)
        .output()
    {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(e) => format!("child: {e}"),
    }
}

fn check_line(failures: &Rc<std::cell::Cell<u32>>, line: &str, needle: &str, label: &str) {
    if line.contains(needle) {
        println!("settings: {label} ok");
    } else {
        failures.set(failures.get() + 1);
        println!("settings: {label} FAIL ({line})");
    }
}

/// Content-sized + centred: the applied height is the Slint-derived one (or the
/// 90% cap) and the window's centre is the work-area centre.
fn check_settings_geometry(shell: &Rc<RefCell<Shell>>, failures: &Rc<std::cell::Cell<u32>>) {
    let s = shell.borrow();
    let scale = s.settings_ui.window().scale_factor() as f64;
    let Some(hwnd) = shell::hwnd_of(s.settings_ui.window()) else {
        failures.set(failures.get() + 1);
        println!("settings: geometry FAIL (no hwnd)");
        return;
    };
    let (left, top, right, bottom) = shell::window_rect(hwnd);
    let (wa_left, wa_top, wa_right, wa_bottom) = shell::work_area_phys();
    let win_w = (right - left) as f64 / scale;
    let win_h = (bottom - top) as f64 / scale;
    let cx = (left + right) as f64 / 2.0;
    let cy = (top + bottom) as f64 / 2.0;
    let want_cx = (wa_left + wa_right) / 2.0;
    let want_cy = (wa_top + wa_bottom) / 2.0;
    let size_ok = (win_w - 404.0).abs() <= 1.0;
    let centre_ok = (cx - want_cx).abs() <= 2.0 * scale && (cy - want_cy).abs() <= 2.0 * scale;
    println!(
        "settings: window={win_w:.1}x{win_h:.1} centre=({cx:.0},{cy:.0}) want=({want_cx:.0},{want_cy:.0})"
    );
    if size_ok && centre_ok {
        println!("settings: 404 wide and centred ok");
    } else {
        failures.set(failures.get() + 1);
        println!("settings: geometry FAIL size-ok={size_ok} centre-ok={centre_ok}");
    }
}

/// `--test-menu`: placement + the show/hide contract of the five rows.
pub fn start_menu(shell: &Rc<RefCell<Shell>>) {
    let failures = Rc::new(std::cell::Cell::new(0u32));
    let scale = shell.borrow().menu_ui.window().scale_factor() as f64;
    let (_, _, wa_right, wa_bottom) = shell::work_area_phys();
    // A point near the bottom-right corner, where a tray icon lives: the flip-up
    // branch must fire.
    let (px, py) = (wa_right - 40.0 * scale, wa_bottom - 12.0 * scale);
    crate::win::menu::show_at(shell, px, py);

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(400), move || {
        let Some(shell) = weak.upgrade() else { return };
        let s = shell.borrow();
        let Some(hwnd) = shell::hwnd_of(s.menu_ui.window()) else {
            f.set(f.get() + 1);
            println!("menu: placement FAIL (no hwnd)");
            return;
        };
        let (left, top, right, bottom) = shell::window_rect(hwnd);
        let (want_left, want_top, flip) =
            crate::win::menu::place(px / scale, py / scale, wa_right / scale, wa_bottom / scale);
        let dx = left as f64 - want_left * scale;
        let dy = top as f64 - want_top * scale;
        println!(
            "menu: rect={left},{top},{right},{bottom} want=({:.0},{:.0}) flip={flip} d=({dx:.1},{dy:.1})",
            want_left * scale,
            want_top * scale
        );
        // The card's top-left corner, derived from the window rect, must sit
        // 14px left of / 13px above the cursor (flip) or 13px below it.
        // panel.rs' geometry, expressed as the visible card: 5px right of the
        // cursor (clamped so the card stops 13px inside the work area's right
        // edge), 13px below it, or (flipped up) with its bottom 13px above it.
        let (cx, cy) = (px / scale, py / scale);
        let wa_right_logical = wa_right / scale;
        let card_x = left as f64 / scale + 24.0;
        let card_y = top as f64 / scale + 24.0;
        let want_card_x = (cx + 5.0).min(wa_right_logical - 153.0);
        let want_card_y = if flip { cy - 13.0 - 172.0 } else { cy + 13.0 };
        let card_ok = (card_x - want_card_x).abs() <= 1.0 && (card_y - want_card_y).abs() <= 1.0;
        let pos_ok = dx.abs() <= 1.0 && dy.abs() <= 1.0;
        let size_ok = ((right - left) as f64 / scale - 188.0).abs() <= 1.0
            && ((bottom - top) as f64 / scale - 220.0).abs() <= 1.0;
        if card_ok && pos_ok && size_ok {
            println!(
                "menu: card=( {card_x:.1},{card_y:.1}) cursor-derived=({want_card_x:.1},{want_card_y:.1}) ok"
            );
        } else {
            f.set(f.get() + 1);
            println!("menu: placement FAIL card-ok={card_ok} pos-ok={pos_ok} size-ok={size_ok}");
        }
    });

    // Row actions, one dialog at a time: each row must close the menu first,
    // then act (SPEC 14). The closes run after each dialog finished opening
    // (window creation takes ~100-200ms and blocks the loop).
    for (delay, action) in [
        (600u64, "settings"),
        (1300, "skills"),
        (2000, "open"),
        (2300, "refresh"),
    ] {
        let weak = Rc::downgrade(shell);
        let action = action.to_string();
        Timer::single_shot(Duration::from_millis(delay), move || {
            let Some(shell) = weak.upgrade() else { return };
            crate::win::menu::on_action(&shell, &action);
        });
    }
    for (delay, which) in [(1000u64, 0), (1700, 1)] {
        let weak = Rc::downgrade(shell);
        Timer::single_shot(Duration::from_millis(delay), move || {
            let Some(shell) = weak.upgrade() else { return };
            if which == 0 {
                crate::win::settings::close_from_ui(&shell);
            } else {
                crate::win::skills::close(&shell);
            }
        });
    }

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(2700), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (menu_open, settings_open, skills_open, panel_visible) = {
            let s = shell.borrow();
            (s.menu_open, s.settings_open, s.skills_open, s.visible)
        };
        // After the rows: the menu is closed, both dialogs were opened and
        // closed again, and "open" toggled the panel on (hidden before).
        let ok = !menu_open && !settings_open && !skills_open && panel_visible;
        println!(
            "menu: after actions menu-open={menu_open} settings-open={settings_open} \
             skills-open={skills_open} panel-visible={panel_visible}"
        );
        if !ok {
            f.set(f.get() + 1);
        }
        println!("menu: row actions {}", if ok { "ok" } else { "FAIL" });

        // Blur closes the menu (SPEC 10.3).
        crate::win::menu::show(&shell);
        let hwnd = shell::hwnd_of(shell.borrow().menu_ui.window());
        if let Some(h) = hwnd {
            shell::focus(h);
        }
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(3100), move || {
        let Some(shell) = weak.upgrade() else { return };
        focus_taskbar();
        let start = Instant::now();
        let weak = Rc::downgrade(&shell);
        let f = f.clone();
        let slot: Rc<std::cell::Cell<u128>> = Rc::new(std::cell::Cell::new(u128::MAX));
        let timer = Rc::new(Timer::default());
        let stopper = timer.clone();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(20),
            move || {
                let Some(shell) = weak.upgrade() else { return };
                let open = shell.borrow().menu_open;
                if !open && slot.get() == u128::MAX {
                    slot.set(start.elapsed().as_millis());
                }
                if slot.get() != u128::MAX || start.elapsed() > Duration::from_millis(2000) {
                    stopper.stop();
                    let ms = slot.get();
                    if ms == u128::MAX {
                        f.set(f.get() + 1);
                        println!("menu: blur-hide FAIL (still open after 2000ms)");
                    } else {
                        println!("menu: blur-hide after={ms}ms ok");
                    }
                    println!(
                        "menu: verdict={}",
                        if f.get() == 0 { "PASS" } else { "FAIL" }
                    );
                    let _ = slint::quit_event_loop();
                }
            },
        );
        shell.borrow_mut().checks.timers.push(timer);
    });
}

/// `--test-skills`: lazy scan, row model, fixed size and scrolling.
pub fn start_skills(shell: &Rc<RefCell<Shell>>) {
    use slint::Model;
    let failures = Rc::new(std::cell::Cell::new(0u32));
    crate::win::skills::open(shell);

    // Poll until the scan lands (it runs on the worker runtime).
    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    let start = Instant::now();
    let timer = Rc::new(Timer::default());
    let stopper = timer.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            let cached = {
                let s = shell.borrow();
                s.skills_cache.is_some()
            };
            if !cached && start.elapsed() < Duration::from_secs(8) {
                return;
            }
            stopper.stop();
            if !cached {
                f.set(f.get() + 1);
                println!("skills: scan FAIL (not finished in 8s)");
                println!("skills: verdict=FAIL");
                let _ = slint::quit_event_loop();
                return;
            }
            let expected = quota_core::skills::scan().len();
            let (count, summary, rows, content, win_w, win_h) = {
                let s = shell.borrow();
                let scale = s.skills_ui.window().scale_factor();
                let size = s.skills_ui.window().size();
                (
                    s.skills_ui.get_count(),
                    s.skills_ui.get_summary().to_string(),
                    s.skills_ui.get_items().row_count(),
                    s.skills_ui.get_content_h(),
                    size.width as f32 / scale,
                    size.height as f32 / scale,
                )
            };
            if count as usize == expected && expected > 0 {
                println!("skills: count={count} matches the scan ok");
            } else {
                f.set(f.get() + 1);
                println!("skills: count={count} want {expected} FAIL");
            }
            let want_summary = format!("{} SKILLS", expected);
            if summary == want_summary {
                println!("skills: summary={summary:?} ok");
            } else {
                f.set(f.get() + 1);
                println!("skills: summary={summary:?} want {want_summary:?} FAIL");
            }
            if rows > expected {
                println!("skills: rows={rows} (group titles included) ok");
            } else {
                f.set(f.get() + 1);
                println!("skills: rows={rows} expected more than {expected} FAIL");
            }
            if (win_w - 404.0).abs() <= 1.0 && (win_h - 520.0).abs() <= 1.0 {
                println!("skills: window={win_w:.1}x{win_h:.1} ok");
            } else {
                f.set(f.get() + 1);
                println!("skills: window={win_w:.1}x{win_h:.1} want 404x520 FAIL");
            }
            println!("skills: content-h={content:.1} viewport=388");

            // First two rows: a group title then a skill, 13.5 + 2 + 2 below.
            let first = shell.borrow().skills_ui.get_items().row_data(0);
            let second = shell.borrow().skills_ui.get_items().row_data(1);
            match (first, second) {
                (Some(a), Some(b)) if a.kind == 0 && b.kind == 1 && (b.y - 17.5).abs() < 0.6 => {
                    println!("skills: first row group={:?} then skill y={:.1} ok", a.name, b.y);
                }
                (Some(a), Some(b)) => {
                    f.set(f.get() + 1);
                    println!(
                        "skills: first rows kind={}/{} y={}/{} FAIL",
                        a.kind, b.kind, a.y, b.y
                    );
                }
                _ => {
                    f.set(f.get() + 1);
                    println!("skills: first rows missing FAIL");
                }
            }

            // Scroll to the bottom through the same property the wheel writes.
            let top_hash = shell
                .borrow()
                .skills_ui
                .window()
                .take_snapshot()
                .ok()
                .map(|b| frame_hash(&b));
            let target = -((content - 388.0).max(0.0));
            shell.borrow().skills_ui.set_scroll_y(target);
            let weak = Rc::downgrade(&shell);
            let f = f.clone();
            Timer::single_shot(Duration::from_millis(250), move || {
                let Some(shell) = weak.upgrade() else { return };
                let (scroll_y, bottom_hash) = {
                    let s = shell.borrow();
                    (
                        s.skills_ui.get_scroll_y(),
                        s.skills_ui
                            .window()
                            .take_snapshot()
                            .ok()
                            .map(|b| frame_hash(&b)),
                    )
                };
                let differs = matches!((top_hash, bottom_hash), (Some(a), Some(b)) if a != b);
                let scrolled = (scroll_y - target).abs() <= 0.5;
                if scrolled && differs {
                    println!(
                        "skills: scrolled to {scroll_y:.1} (target {target:.1}) frame-changed={differs} ok"
                    );
                } else {
                    f.set(f.get() + 1);
                    println!(
                        "skills: scroll FAIL y={scroll_y:.1} target={target:.1} frame-changed={differs}"
                    );
                }
                // Refresh rescans on demand; the list must come back intact.
                crate::win::skills::request_scan(&shell, true);
                let weak = Rc::downgrade(&shell);
                let f = f.clone();
                let expected = expected;
                Timer::single_shot(Duration::from_millis(900), move || {
                    let Some(shell) = weak.upgrade() else { return };
                    let (scanning, count) = {
                        let s = shell.borrow();
                        (s.skills_scanning, s.skills_ui.get_count())
                    };
                    if !scanning && count as usize == expected {
                        println!("skills: refresh rescan kept {count} skills ok");
                    } else {
                        f.set(f.get() + 1);
                        println!("skills: refresh scanning={scanning} count={count} FAIL");
                    }
                    println!(
                        "skills: verdict={}",
                        if f.get() == 0 { "PASS" } else { "FAIL" }
                    );
                    let _ = slint::quit_event_loop();
                });
            });
        },
    );
    shell.borrow_mut().checks.timers.push(timer);
}

/// `--test-dialogs`: the panel <-> dialog focus contract.
///
/// While the settings or skills window is open the panel must NOT auto-hide
/// (its blur is the dialog's own focus move), and closing the dialog must
/// collapse the panel if focus did not come back to it (SPEC 20 / 21). Both
/// halves are driven with real window focus, not by calling the handler.
pub fn start_dialogs(shell: &Rc<RefCell<Shell>>) {
    let failures = Rc::new(std::cell::Cell::new(0u32));
    let blurs_before_skills = Rc::new(std::cell::Cell::new(0u32));
    shell::show_panel(shell);

    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(600), move || {
        let Some(shell) = weak.upgrade() else { return };
        println!("dialogs: panel-visible-before={}", shell.borrow().visible);
        crate::win::settings::open(&shell);
        let hwnd = shell::hwnd_of(shell.borrow().settings_ui.window());
        if let Some(h) = hwnd {
            shell::focus(h);
        }
        // Steal the focus away from our own windows while the sheet is open:
        // the panel's blur must be suppressed by `settings_open`.
        focus_taskbar();
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(1200), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, settings_open, blurs) = {
            let s = shell.borrow();
            (s.visible, s.settings_open, s.checks.blurs)
        };
        // The panel must actually have lost focus at least once: a no-op
        // SetForegroundWindow would otherwise let this pass without exercising
        // the suppression branch.
        if visible && settings_open && blurs > 0 {
            println!("dialogs: settings open keeps the panel visible (blurs={blurs}) ok");
        } else {
            f.set(f.get() + 1);
            println!(
                "dialogs: panel-visible={visible} settings-open={settings_open} blurs={blurs} FAIL"
            );
        }
        crate::win::settings::close_from_ui(&shell);
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    let before_open = blurs_before_skills.clone();
    Timer::single_shot(Duration::from_millis(1750), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, settings_open) = {
            let s = shell.borrow();
            (s.visible, s.settings_open)
        };
        // After a dialog closes the panel is either hidden (focus went
        // elsewhere) or it holds the foreground again (Windows handed focus
        // back to it). A visible-but-unfocused panel is the bug.
        let focused = {
            let s = shell.borrow();
            s.hwnd.map(shell::is_foreground).unwrap_or(false)
        };
        if !settings_open && (!visible || focused) {
            println!(
                "dialogs: closing settings leaves the panel {} ok",
                if visible { "focused" } else { "hidden" }
            );
        } else {
            f.set(f.get() + 1);
            println!(
                "dialogs: panel-visible={visible} focused={focused} settings-open={settings_open} FAIL"
            );
        }
        // Same pair for the skills window.
        shell::show_panel(&shell);
        before_open.set(shell.borrow().checks.blurs);
        crate::win::skills::open(&shell);
        focus_taskbar();
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    let before_skills = blurs_before_skills.clone();
    Timer::single_shot(Duration::from_millis(2400), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, skills_open, blurs) = {
            let s = shell.borrow();
            (s.visible, s.skills_open, s.checks.blurs)
        };
        let new_blur = blurs > before_skills.get();
        if visible && skills_open && new_blur {
            println!("dialogs: skills open keeps the panel visible (blurs={blurs}) ok");
        } else {
            f.set(f.get() + 1);
            println!(
                "dialogs: panel-visible={visible} skills-open={skills_open} new-blur={new_blur} FAIL"
            );
        }
        crate::win::skills::close(&shell);
    });

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(2950), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, skills_open, focused) = {
            let s = shell.borrow();
            (
                s.visible,
                s.skills_open,
                s.hwnd.map(shell::is_foreground).unwrap_or(false),
            )
        };
        if !skills_open && (!visible || focused) {
            println!(
                "dialogs: closing skills leaves the panel {} ok",
                if visible { "focused" } else { "hidden" }
            );
        } else {
            f.set(f.get() + 1);
            println!(
                "dialogs: panel-visible={visible} focused={focused} skills-open={skills_open} FAIL"
            );
        }
        println!(
            "dialogs: verdict={}",
            if f.get() == 0 { "PASS" } else { "FAIL" }
        );
        let _ = slint::quit_event_loop();
    });
}

/// `--test-close`: a system close (Alt+F4 arrives as `WM_CLOSE`) hides a
/// window instead of destroying it, and the shell's flags follow. Without that
/// the tray toggle fought a stale `visible`, and the two dialogs could never
/// reopen because `settings_open`/`skills_open` stayed true.
pub fn start_close(shell: &Rc<RefCell<Shell>>) {
    shell::show_panel(shell);
    let failures = Rc::new(std::cell::Cell::new(0u32));
    let state = Rc::new(std::cell::Cell::new(0u8));
    let wait = Rc::new(std::cell::Cell::new(0u8));
    let moved_pos = Rc::new(std::cell::Cell::new((0i32, 0i32)));
    let weak = Rc::downgrade(shell);
    let timer = Rc::new(Timer::default());
    let stopper = timer.clone();
    let f = failures.clone();
    let s = state.clone();
    let w = wait.clone();
    let m = moved_pos.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            w.set(w.get() + 1);
            let ready = match s.get() {
                0 | 1 => w.get() >= 5,
                2 => w.get() >= 6,
                3 | 5 => w.get() >= 7,
                4 => w.get() >= 6,
                6 => w.get() >= 9,
                7 => w.get() >= 7,
                8 => w.get() >= 8,
                9 => w.get() >= 7,
                _ => w.get() >= 6,
            };
            if !ready {
                return;
            }
            w.set(0);
            match s.get() {
                0 => {
                    match shell.borrow().hwnd {
                        Some(hwnd) => post_close(hwnd),
                        None => {
                            f.set(f.get() + 1);
                            println!("close: no panel hwnd FAIL");
                        }
                    }
                    s.set(1);
                }
                1 => {
                    let visible = shell.borrow().visible;
                    if visible {
                        f.set(f.get() + 1);
                    }
                    println!("close: panel hidden after WM_CLOSE ok={}", ok_str(!visible));
                    shell::show_panel(&shell);
                    s.set(2);
                }
                2 => {
                    let visible = shell.borrow().visible;
                    let on_screen = shell.borrow().hwnd.map(shell::is_visible).unwrap_or(false);
                    let ok = visible && on_screen;
                    if !ok {
                        f.set(f.get() + 1);
                    }
                    println!("close: panel shows again ok={}", ok_str(ok));
                    crate::win::settings::open(&shell);
                    s.set(3);
                }
                3 => {
                    match shell::hwnd_of(shell.borrow().settings_ui.window()) {
                        Some(hwnd) => post_close(hwnd),
                        None => {
                            f.set(f.get() + 1);
                            println!("close: no settings hwnd FAIL");
                        }
                    }
                    s.set(4);
                }
                4 => {
                    let open = shell.borrow().settings_open;
                    if open {
                        f.set(f.get() + 1);
                    }
                    println!("close: settings closed by WM_CLOSE ok={}", ok_str(!open));
                    crate::win::settings::open(&shell);
                    s.set(5);
                }
                5 => {
                    let open = shell.borrow().settings_open;
                    if !open {
                        f.set(f.get() + 1);
                    }
                    println!("close: settings reopens ok={}", ok_str(open));
                    crate::win::skills::open(&shell);
                    s.set(6);
                }
                6 => {
                    match shell::hwnd_of(shell.borrow().skills_ui.window()) {
                        Some(hwnd) => post_close(hwnd),
                        None => {
                            f.set(f.get() + 1);
                            println!("close: no skills hwnd FAIL");
                        }
                    }
                    s.set(7);
                }
                7 => {
                    let open = shell.borrow().skills_open;
                    if open {
                        f.set(f.get() + 1);
                    }
                    println!("close: skills closed by WM_CLOSE ok={}", ok_str(!open));
                    // A dragged position must survive the reopen: move the
                    // window, open it again and compare. `skills_placed` stops
                    // the second open from re-centring (Tauri's rule).
                    let target = {
                        let s = shell.borrow();
                        let pos = s.skills_ui.window().position();
                        let target = (pos.x + 40, pos.y + 40);
                        s.skills_ui
                            .window()
                            .set_position(slint::PhysicalPosition::new(target.0, target.1));
                        target
                    };
                    m.set(target);
                    crate::win::skills::open(&shell);
                    s.set(8);
                }
                8 => {
                    let open = shell.borrow().skills_open;
                    let pos = shell.borrow().skills_ui.window().position();
                    let target = m.get();
                    let kept =
                        (pos.x - target.0).abs() <= 2 && (pos.y - target.1).abs() <= 2;
                    let ok = open && kept;
                    if !ok {
                        f.set(f.get() + 1);
                    }
                    println!(
                        "close: skills reopens at the dragged position ok={} (pos={pos:?} want={target:?})",
                        ok_str(ok)
                    );
                    let (_, _, wa_right, wa_bottom) = shell::work_area_phys();
                    crate::win::menu::show_at(&shell, wa_right - 40.0, wa_bottom - 40.0);
                    s.set(9);
                }
                9 => {
                    match shell::hwnd_of(shell.borrow().menu_ui.window()) {
                        Some(hwnd) => post_close(hwnd),
                        None => {
                            f.set(f.get() + 1);
                            println!("close: no menu hwnd FAIL");
                        }
                    }
                    s.set(10);
                }
                _ => {
                    let open = shell.borrow().menu_open;
                    if open {
                        f.set(f.get() + 1);
                    }
                    println!("close: menu closed by WM_CLOSE ok={}", ok_str(!open));
                    println!(
                        "close: verdict={}",
                        if f.get() == 0 { "PASS" } else { "FAIL" }
                    );
                    stopper.stop();
                    let _ = slint::quit_event_loop();
                }
            }
        },
    );
}

/// `--test-signin` (P4): the settings sheet's "Sign in…" and Clear wiring,
/// driven against the real child process instead of a stub.
///
/// Round 1 — the user closes the console. The sheet opens, the button is pressed
/// through the window's own callback, the child appears with a real top-level
/// window, and closing that window makes the child exit by itself. The parent's
/// exit poll must then bring the sheet back with a "cancelled" note: that is the
/// round trip every real sign-in ends with.
///
/// Round 2 — Clear while the console is up. It must kill the child, bump the
/// credential epoch and leave no usage token behind, and nothing may come back.
///
/// Neither round authenticates: the capture channel is `--test-login`'s job and
/// a human sign-in is the only way to a real token (`spike/p4/REPORT.md`).
pub fn start_signin(shell: &Rc<RefCell<Shell>>) {
    let failures = Rc::new(std::cell::Cell::new(0u32));
    let dir = settings::config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let cred_file = dir.join("deepseek-credentials.json");
    // This check seeds a marker credential and then clears it, so whatever was
    // there has to come back afterwards: `--config-dir` can point at a real
    // profile, and a token destroyed that way is not recoverable.
    SIGNIN_CREDENTIAL_BACKUP.with(|slot| *slot.borrow_mut() = Some(snapshot_credentials()));
    // A NON-empty marker token: round 1 asserts that the cancelled sign-in left
    // it alone, and round 2 asserts Clear removed it. An empty seed made both
    // checks true before and after, so a no-op Clear still passed.
    let _ = std::fs::write(
        &cred_file,
        r#"{"v":1,"apiKey":"sk-test","userToken":"ut-test"}"#,
    );

    let weak = Rc::downgrade(shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(300), move || {
        let Some(shell) = weak.upgrade() else { return };
        let Some(pid) = press_sign_in(&shell, &f, "round 1") else {
            finish_signin(&f);
            return;
        };
        wait_for_window(shell, f, pid, 0, Rc::new(close_console));
    });
}

thread_local! {
    /// The credential files `--test-signin` found before it seeded its marker.
    /// Both the shared file and the private fallback are restored in
    /// `finish_signin`, whatever path the check took.
    static SIGNIN_CREDENTIAL_BACKUP: RefCell<Option<CredentialBackup>> = const { RefCell::new(None) };
}

type CredentialBackup = Vec<(std::path::PathBuf, Option<Vec<u8>>)>;

thread_local! {
    /// `--test-settings` seeds credentials and clears them; both files are
    /// restored at the end. The settings file is restored too, so a real
    /// `--config-dir` survives the check.
    static SETTINGS_CREDENTIAL_BACKUP: RefCell<Option<CredentialBackup>> = const { RefCell::new(None) };
    static SETTINGS_JSON_BACKUP: RefCell<Option<Option<Vec<u8>>>> = const { RefCell::new(None) };
}

fn restore_settings_files() {
    let creds = SETTINGS_CREDENTIAL_BACKUP.with(|slot| slot.borrow_mut().take());
    if let Some(creds) = creds {
        restore_credentials(&creds);
    }
    let file = settings::config_dir().join("settings.json");
    let json = SETTINGS_JSON_BACKUP.with(|slot| slot.borrow_mut().take());
    if let Some(json) = json {
        match json {
            Some(bytes) => {
                let _ = std::fs::write(&file, bytes);
            }
            None => {
                let _ = std::fs::remove_file(&file);
            }
        }
    }
}

fn snapshot_credentials() -> CredentialBackup {
    quota_core::deepseek::credentials::files()
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(&path).ok();
            (path, bytes)
        })
        .collect()
}

fn restore_credentials(backup: &CredentialBackup) {
    for (path, bytes) in backup {
        match bytes {
            Some(bytes) => {
                let _ = std::fs::write(path, bytes);
            }
            None => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

type SigninStep = Rc<dyn Fn(Rc<RefCell<Shell>>, Rc<std::cell::Cell<u32>>, isize)>;

/// Open the sheet if needed, then press the button through the callback the
/// window wires it to — not by calling `login::open` directly.
fn press_sign_in(
    shell: &Rc<RefCell<Shell>>,
    failures: &Rc<std::cell::Cell<u32>>,
    label: &str,
) -> Option<u32> {
    if !shell.borrow().settings_open {
        crate::win::settings::open(shell);
    }
    let open_before = shell.borrow().settings_open;
    let ui = shell.borrow().settings_ui.clone_strong();
    ui.invoke_sign_in();
    let running = crate::login::is_running(shell);
    let pid = crate::login::pid(shell);
    let open_after = shell.borrow().settings_open;
    let ok = open_before && running && pid.is_some() && !open_after;
    println!(
        "signin: {label}: sheet-before={open_before} running={running} pid={pid:?} sheet-after={open_after} {}",
        ok_str(ok)
    );
    if !ok {
        failures.set(failures.get() + 1);
    }
    pid.filter(|_| ok)
}

/// Poll for the child's console window (by owning process, never by title: a
/// leftover console from an earlier run would hide the live one). The tao window
/// exists before WebView2 initialises, so this is fast even on a cold profile.
fn wait_for_window(
    shell: Rc<RefCell<Shell>>,
    failures: Rc<std::cell::Cell<u32>>,
    pid: u32,
    attempt: u32,
    next: SigninStep,
) {
    if let Some(hwnd) = crate::login::console_hwnd(pid) {
        println!("signin: console window present (pid={pid}, attempt {attempt}) ok");
        next(shell, failures, hwnd);
        return;
    }
    if attempt >= 120 {
        failures.set(failures.get() + 1);
        println!("signin: console window never appeared (pid={pid}) FAIL");
        crate::login::cancel(&shell);
        finish_signin(&failures);
        return;
    }
    let weak = Rc::downgrade(&shell);
    Timer::single_shot(Duration::from_millis(400), move || {
        let Some(shell) = weak.upgrade() else { return };
        wait_for_window(shell, failures, pid, attempt + 1, next);
    });
}

/// Round 1: close the console like a user would (`WM_CLOSE`). A child still
/// inside WebView2's cold start cannot answer that message, so the fallback
/// kills it — the parent's exit poll runs the same code either way, and which
/// one happened is logged.
fn close_console(shell: Rc<RefCell<Shell>>, failures: Rc<std::cell::Cell<u32>>, hwnd: isize) {
    post_close(hwnd);
    wait_for_exit(shell, failures, 0);
}

fn wait_for_exit(shell: Rc<RefCell<Shell>>, failures: Rc<std::cell::Cell<u32>>, attempt: u32) {
    if !crate::login::is_running(&shell) {
        if attempt > 0 {
            println!(
                "signin: round 1: the console exited on its own (WM_CLOSE, {attempt} polls) ok"
            );
        }
        settle_round(&shell, &failures);
        return;
    }
    if attempt >= 40 {
        println!("signin: round 1: the console did not answer WM_CLOSE, killing it");
        crate::login::kill_child(&shell);
        let weak = Rc::downgrade(&shell);
        let f = failures.clone();
        Timer::single_shot(Duration::from_millis(600), move || {
            let Some(shell) = weak.upgrade() else { return };
            settle_round(&shell, &f);
        });
        return;
    }
    let weak = Rc::downgrade(&shell);
    Timer::single_shot(Duration::from_millis(200), move || {
        let Some(shell) = weak.upgrade() else { return };
        wait_for_exit(shell, failures, attempt + 1);
    });
}

/// The parent must have settled: child gone, sheet back, note explains why, and
/// still no usage token in the credential file. Settling runs from the exit poll
/// (250 ms), so give it a few ticks before judging.
fn settle_round(shell: &Rc<RefCell<Shell>>, failures: &Rc<std::cell::Cell<u32>>) {
    wait_for_settle(shell.clone(), failures.clone(), 0);
}

fn wait_for_settle(shell: Rc<RefCell<Shell>>, failures: Rc<std::cell::Cell<u32>>, attempt: u32) {
    if shell.borrow().settings_open || attempt >= 15 {
        assert_settled(&shell, &failures);
        return;
    }
    let weak = Rc::downgrade(&shell);
    Timer::single_shot(Duration::from_millis(200), move || {
        let Some(shell) = weak.upgrade() else { return };
        wait_for_settle(shell, failures, attempt + 1);
    });
}

fn assert_settled(shell: &Rc<RefCell<Shell>>, failures: &Rc<std::cell::Cell<u32>>) {
    let running = crate::login::is_running(shell);
    let (open, note, code) = {
        let s = shell.borrow();
        (
            s.settings_open,
            s.settings_ui.get_ds_note().to_string(),
            s.login.last_code,
        )
    };
    let status = quota_core::deepseek::credentials::status();
    let token = status.user_token;
    // Exit 3 is "closed by the user" (login_host), which is what WM_CLOSE is.
    // Exit 1 is the fallback `kill_child`: the parent terminated it, and
    // `settle` reports that as a cancellation too. The seeded token must still
    // be there: a cancelled sign-in changed nothing.
    let ok = !running
        && open
        && note.to_lowercase().contains("cancelled")
        && token
        && matches!(code, Some(Some(3)) | Some(Some(1)));
    println!(
        "signin: round 1: running={running} sheet-open={open} code={code:?} note={note:?} token={token} {}",
        ok_str(ok)
    );
    if !ok {
        failures.set(failures.get() + 1);
    }

    let Some(pid) = press_sign_in(shell, failures, "round 2") else {
        finish_signin(failures);
        return;
    };
    wait_for_window(
        shell.clone(),
        failures.clone(),
        pid,
        0,
        Rc::new(clear_console),
    );
}

/// Round 2: Clear must take the child away with it.
fn clear_console(shell: Rc<RefCell<Shell>>, failures: Rc<std::cell::Cell<u32>>, _hwnd: isize) {
    // The console is up and the sheet is hidden; a user gets to Clear by
    // reopening the sheet from the tray.
    if !shell.borrow().settings_open {
        crate::win::settings::open(&shell);
    }
    let before = shell.borrow().cred_epoch.load(Ordering::SeqCst);
    crate::win::settings::clear(&shell);
    let running = crate::login::is_running(&shell);
    let epoch = shell.borrow().cred_epoch.load(Ordering::SeqCst);
    let status = quota_core::deepseek::credentials::status();
    let token = status.user_token;
    let api = status.api_key;
    let note = shell.borrow().settings_ui.get_ds_note().to_string();
    let ok = !running && epoch == before + 1 && !token && !api && !note.is_empty();
    println!(
        "signin: round 2: running={running} epoch={before}->{epoch} token={token} api={api} note={note:?} {}",
        ok_str(ok)
    );
    if !ok {
        failures.set(failures.get() + 1);
    }
    let weak = Rc::downgrade(&shell);
    let f = failures.clone();
    Timer::single_shot(Duration::from_millis(1200), move || {
        let Some(shell) = weak.upgrade() else { return };
        let running = crate::login::is_running(&shell);
        let status = quota_core::deepseek::credentials::status();
        let token = status.user_token;
        let api = status.api_key;
        let open = shell.borrow().settings_open;
        let ok = !running && !token && !api && open;
        println!(
            "signin: round 2: running={running} token={token} api={api} sheet-open={open} {}",
            ok_str(ok)
        );
        if !ok {
            f.set(f.get() + 1);
        }
        crate::login::cancel(&shell);
        finish_signin(&f);
    });
}

fn finish_signin(failures: &Rc<std::cell::Cell<u32>>) {
    restore_signin_credentials();
    println!(
        "signin: verdict={}",
        if failures.get() == 0 { "PASS" } else { "FAIL" }
    );
    let _ = slint::quit_event_loop();
}

/// Put back what `start_signin` found: the original bytes, or no file at all.
fn restore_signin_credentials() {
    let backup = SIGNIN_CREDENTIAL_BACKUP.with(|slot| slot.borrow_mut().take());
    let Some(backup) = backup else { return };
    restore_credentials(&backup);
    crate::logf("signin: credential files restored");
}

fn post_close(hwnd: isize) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    unsafe {
        let _ = PostMessageW(Some(HWND(hwnd as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

fn ok_str(ok: bool) -> &'static str {
    if ok {
        "ok"
    } else {
        "FAIL"
    }
}

/// `--shots`: open each P3 window in turn and save a PNG (`.shots` review).
/// Runs in the theme `--theme` selects, so both passes are one command each.
pub fn start_shots(shell: &Rc<RefCell<Shell>>) {
    let out = shell.borrow().out_dir.clone();
    if out.is_empty() {
        crate::logf("shots: --out is required");
        let _ = slint::quit_event_loop();
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&out) {
        crate::logf(format!("shots: cannot create {out}: {e}"));
    }
    // The default is a fixed list: the real scan is this machine's skill
    // inventory and must not end up in a committed screenshot. `--variant
    // skills-real` opts into the live scan for the DOM band comparison.
    if shell.borrow().variant.as_deref() != Some("skills-real") {
        shell.borrow_mut().skills_cache = Some(demo_skills());
        // Setting the cache alone does not paint it: `render` builds the row
        // model (and would otherwise be skipped because no scan runs).
        crate::win::skills::render(shell);
    }
    let step = Rc::new(std::cell::Cell::new(0u32));
    let weak = Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(600),
        move || {
            let Some(shell) = weak.upgrade() else { return };
            let n = step.get();
            step.set(n + 1);
            let dir = std::path::Path::new(&out);
            match n {
                0 => crate::win::settings::open(&shell),
                1 => {
                    save_window(
                        shell.borrow().settings_ui.window(),
                        &dir.join("settings.png"),
                    );
                    {
                        let s = shell.borrow();
                        s.settings_ui.set_manual_open(true);
                    }
                    crate::win::settings::manual_toggled(&shell);
                }
                2 => {
                    save_window(
                        shell.borrow().settings_ui.window(),
                        &dir.join("settings-manual.png"),
                    );
                    crate::win::settings::close_from_ui(&shell);
                }
                3 => crate::win::skills::open(&shell),
                4 => {
                    save_window(shell.borrow().skills_ui.window(), &dir.join("skills.png"));
                    crate::win::skills::close(&shell);
                }
                5 => {
                    let (_, _, wa_right, wa_bottom) = shell::work_area_phys();
                    let scale = shell.borrow().menu_ui.window().scale_factor() as f64;
                    crate::win::menu::show_at(
                        &shell,
                        wa_right - 300.0 * scale,
                        wa_bottom - 200.0 * scale,
                    );
                }
                6 => {
                    save_window(shell.borrow().menu_ui.window(), &dir.join("menu.png"));
                    crate::win::menu::hide(&shell);
                }
                7 => {
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            }
        },
    );
    shell.borrow_mut().checks.timers.push(Rc::new(timer));
}

/// Publishable stand-in for the local skill inventory (same shapes the real
/// scan produces: 2 groups + a plugin, one short description, long ones).
fn demo_skills() -> Vec<quota_core::skills::SkillInfo> {
    let row = |name: &str, description: &str, source: &str| quota_core::skills::SkillInfo {
        id: name.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        source: source.to_string(),
    };
    vec![
        row("code-review", "Review code changes for security, performance and correctness.", "Kimi Code"),
        row(
            "docx",
            "Create, read, edit and manipulate Word documents, including templates, headers, footers and tracked changes, and export a finished document as a PDF when the task asks for one.",
            "Kimi Code",
        ),
        row(
            "skill-creator",
            "Create new skills, modify and improve existing skills, and measure skill performance with evals and benchmark runs.",
            "Kimi Code",
        ),
        row("frontend-design", "Guidance for distinctive, intentional visual design when building new UI.", "Agents"),
        row(
            "theme-factory",
            "Apply a consistent theme to slides, documents, reports, landing pages and other artifacts: ten preset themes plus custom fonts and colour pairs.",
            "Agents",
        ),
        row(
            "doc-lookup",
            "Look up an internal document by title or token and read its sections.",
            "Plugin: doc-tools",
        ),
    ]
}

fn save_window(window: &slint::Window, path: &std::path::Path) {
    match window.take_snapshot() {
        Ok(buffer) => match shell::save_png(&buffer, path) {
            Ok(()) => crate::logf(format!("shots: {}", path.display())),
            Err(e) => crate::logf(format!("shots: save failed: {e}")),
        },
        Err(e) => crate::logf(format!("shots: snapshot failed: {e}")),
    }
}

// ---------------------------------------------------------------------------
// --hold / --test-tray
// ---------------------------------------------------------------------------

pub fn start_hold(shell: &Rc<RefCell<Shell>>, seconds: u64) {
    shell::show_panel(shell);
    let (out, demo) = {
        let s = shell.borrow();
        (s.out_dir.clone(), s.demo_switch)
    };
    if demo {
        let weak = Rc::downgrade(shell);
        Timer::single_shot(Duration::from_millis(1000), move || {
            let Some(shell) = weak.upgrade() else { return };
            let ds = !shell.borrow().ui.get_tab_ds();
            shell::request_tab(&shell, ds);
        });
    }
    if !out.is_empty() {
        if let Err(e) = std::fs::create_dir_all(&out) {
            crate::logf(format!("hold: cannot create {out}: {e}"));
        }
        let weak = Rc::downgrade(shell);
        let path =
            std::path::Path::new(&out).join(if demo { "switch-mid.png" } else { "panel.png" });
        let delay = if demo { 1100 } else { 1200 };
        Timer::single_shot(Duration::from_millis(delay), move || {
            let Some(shell) = weak.upgrade() else { return };
            shell::snapshot(&shell, &path);
        });
    }
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(900), move || {
        let Some(shell) = weak.upgrade() else { return };
        let s = shell.borrow();
        crate::logf(format!(
            "hold: ds has-summary={} sum={:?} sum-today={:?}",
            s.ui.get_ds_has_summary(),
            s.ui.get_ds_sum(),
            s.ui.get_ds_sum_today()
        ));
        crate::logf(format!(
            "hold: shown={} cell-h={:.1} win-h={:.1} natural-kimi={:.1} natural-ds={:.1} size={}x{}",
            s.ui.get_shown(),
            s.ui.get_cell_h(),
            s.ui.get_win_h(),
            s.ui.get_natural_kimi(),
            s.ui.get_natural_ds(),
            s.ui.window().size().width,
            s.ui.window().size().height
        ));
    });
    Timer::single_shot(Duration::from_secs(seconds.max(10)), || {
        let _ = slint::quit_event_loop();
    });
}

pub fn report_tray(shell: &Rc<RefCell<Shell>>) {
    let s = shell.borrow();
    match &s.tray {
        Some(icon) => {
            let rect = icon.rect();
            let tip =
                quota_core::format::tray_tooltip(s.data.quota.as_ref(), s.data.deepseek.as_ref());
            crate::logf(format!(
                "tray: registered rect={rect:?} tooltip-lines={} text={tip:?}",
                tip.lines().count()
            ));
        }
        None => crate::logf("tray: NOT registered"),
    }
}
