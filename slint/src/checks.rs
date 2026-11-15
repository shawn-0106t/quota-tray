// Self-check modes: the acceptance evidence for P2's panel, collected from the
// real window (rects, latency) instead of from the UI's own opinion.
//
//   --test-heights     data variant -> card/window height (vs spike/p2/heights-ref.json)
//   --test-corner      work-area corner anchoring at the live DPI
//   --test-active-tab  ActiveTab persistence round trip in an isolated config dir
//   --test-tab-anim    30 clicks, bottom-edge drift
//   --test-blur        focus loss -> hidden latency
//   --test-tray-click  posted tray callback messages
//   --hold             keep the panel on screen (screenshots)
use crate::app::Data;
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
    if visible.is_empty() {
        println!("corner: verdict=FAIL (no visible samples)");
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

    let initial = shell.borrow().data.settings.active_tab.clone();
    if initial == "kimi" {
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
                        println!(
                            "blur: hidden-after={value}ms budget={}ms blur-event={} verdict={}",
                            shell::HIDE_MS,
                            if blur_seen { "seen" } else { "MISSING" },
                            if value <= shell::HIDE_MS as u128 + 60 {
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
