// The P0 shell: one transparent Slint window, a tray-icon crate tray, the tab
// switch controller and the automated self-checks.
//
// Window model (PLAN §4 / §3.6): the window is sized to (content + 2x28 shadow
// room) and pinned so its bottom-right corner is (12px card gap - 28px margin)
// from the work-area corner. The card is anchored to the window's BOTTOM edge
// from inside (panel.slint), so resizing never moves the visible card edge.

use crate::{credentials, logf, Anchor, ModelRow, Panel, SlintTray, Tokens};
use slint::{ComponentHandle, LogicalSize, PhysicalPosition, Timer, TimerMode};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PANEL_W: f32 = 424.0;
/// Transparent shadow room per side (matches panel.slint / tauri.conf.json).
const MARGIN: f32 = 28.0;
/// Card edge to work-area corner.
const GAP: f32 = 12.0;
/// Tray left-clicks this soon after an auto-hide are ignored (SPEC 14).
const TRAY_GUARD: Duration = Duration::from_millis(300);
/// Blur -> window hidden budget (main.ts playHide waits 170ms).
const HIDE_MS: u64 = 170;
/// A show steals focus; a blur inside this window is the tray click's own
/// mouse-up handing focus back to the taskbar, not the user clicking away.
const FOCUS_GRACE: Duration = Duration::from_millis(250);
const HOVER_PREFETCH: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Interactive,
    TabAnim,
    Blur,
    Tray,
    Toggle,
    SlintTrayProbe,
    /// Show the panel and hold it for `--seconds`: visual inspection / screenshots.
    Hold,
    /// Post the tray callback messages (the same ones Explorer sends) and assert
    /// that click/hover reach the app and toggle the panel.
    TrayClick,
}

pub struct Options {
    pub mode: Mode,
    pub dark: bool,
    pub start_ds: bool,
    pub scale: Option<f32>,
    pub seconds: u64,
    pub out_dir: String,
    pub demo_switch: bool,
}

struct Shell {
    ui: Panel,
    mode: Mode,
    natural_kimi: f32,
    natural_ds: f32,
    win_h: f32,
    visible: bool,
    hiding: bool,
    hide_run: u64,
    shown_at: Instant,
    last_hide: Option<Instant>,
    switching: bool,
    run: u64,
    hwnd: Option<isize>,
    tray_icon: Option<tray_icon::TrayIcon>,
    tray_timer: Option<Timer>,
    slint_tray: Option<SlintTray>,
    login_child: Option<std::process::Child>,
    login_running: bool,
    hover_at: Option<Instant>,
    out_dir: String,
    demo_switch: bool,
    refresh_count: u32,
    // test instrumentation
    samples: Vec<Sample>,
    clicks_left: u32,
    last_click_at: Option<Instant>,
    last_click_ms: f64,
    sampler_start: Option<Instant>,
    first_switch_at: Option<Instant>,
    blur_at: Option<Instant>,
    finished_hide_at: Option<Instant>,
    test_timers: Vec<Rc<Timer>>,
    login_timer: Option<Rc<Timer>>,
    reported: bool,
}

#[derive(Clone, Copy)]
struct Sample {
    t_ms: f64,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    visible: bool,
    cell: f32,
    card_bottom: i32,
    card_top: i32,
}

impl Shell {
    /// Method form so callers holding a `RefMut<Shell>` can use it directly.
    fn apply_size(&mut self, h: f32) {
        apply_size(self, h);
    }
}

pub fn run(opts: Options) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(scale) = opts.scale {
        std::env::set_var("SLINT_SCALE_FACTOR", scale.to_string());
    }
    if std::env::var("SLINT_BACKEND").is_err() {
        std::env::set_var("SLINT_BACKEND", "winit-skia");
    }
    select_backend()?;

    let ui = Panel::new()?;
    ui.global::<Tokens>().set_dark(opts.dark);
    ui.set_dark(opts.dark);
    apply_sample_data(&ui);
    ui.set_tab_ds(opts.start_ds);
    ui.set_outgoing(0);

    let natural_kimi = ui.get_natural_kimi();
    let natural_ds = ui.get_natural_ds();
    let shell = Rc::new(RefCell::new(Shell {
        ui,
        mode: opts.mode,
        natural_kimi,
        natural_ds,
        win_h: natural_kimi,
        visible: false,
        hiding: false,
        hide_run: 0,
        shown_at: Instant::now(),
        last_hide: None,
        switching: false,
        run: 0,
        hwnd: None,
        tray_icon: None,
        tray_timer: None,
        slint_tray: None,
        login_child: None,
        login_running: false,
        hover_at: None,
        out_dir: opts.out_dir.clone(),
        demo_switch: opts.demo_switch,
        refresh_count: 0,
        samples: Vec::new(),
        clicks_left: 0,
        last_click_at: None,
        last_click_ms: 0.0,
        sampler_start: None,
        first_switch_at: None,
        blur_at: None,
        finished_hide_at: None,
        test_timers: Vec::new(),
        login_timer: None,
        reported: false,
    }));

    // Park the keepalive window off-screen before anything can hide the panel.
    let anchor = Anchor::new()?;
    if anchor.show().is_err() {
        logf("anchor: show failed; the event loop will quit on the first panel hide");
    }
    anchor
        .window()
        .set_position(PhysicalPosition::new(-30000, -30000));

    wire_callbacks(&shell);
    if opts.mode != Mode::SlintTrayProbe {
        build_tray(&shell);
        spawn_tray_poll(&shell);
    }

    // Everything that needs a live event loop (winit window, tray rect) runs
    // from the first tick, not before it.
    let startup = shell.clone();
    let seconds = opts.seconds;
    Timer::single_shot(Duration::from_millis(30), move || {
        startup_mode(&startup, seconds);
    });

    slint::run_event_loop()?;
    let _ = &anchor;
    Ok(())
}

fn select_backend() -> Result<(), Box<dyn std::error::Error>> {
    use slint::winit_030::winit::platform::windows::WindowAttributesExtWindows;
    use slint::winit_030::winit::window::WindowLevel;
    slint::BackendSelector::new()
        .with_winit_window_attributes_hook(|attrs| {
            attrs
                .with_skip_taskbar(true)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_resizable(false)
                .with_active(false)
        })
        .select()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// callbacks
// ---------------------------------------------------------------------------

fn wire_callbacks(shell: &Rc<RefCell<Shell>>) {
    // The closure needs to own the Rc while the Panel itself stays inside the
    // RefCell (the generated component handle is not Clone). Clone the Rc into a
    // separate binding first, then register through a shared borrow.
    let shell_tab = shell.clone();
    {
        let s = shell.borrow();
        s.ui.on_request_tab(move |ds| {
            request_tab(&shell_tab, ds);
        });
    }
    let shell_action = shell.clone();
    {
        let s = shell.borrow();
        s.ui.on_action(move |name| {
            let name = name.to_string();
            logf(format!("panel: action {name}"));
            match name.as_str() {
                "console" => {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", "https://www.kimi.com/code/console"])
                        .spawn();
                }
                "refresh" => {
                    let mut s = shell_action.borrow_mut();
                    s.refresh_count += 1;
                    let now = chrono::Local::now().format("%H:%M").to_string();
                    s.ui.set_stamp_kimi(format!("Updated {now}").into());
                    s.ui.set_stamp_ds(format!("Updated {now}").into());
                }
                "settings" => logf("panel: settings window is P3, not built"),
                "exit" => {
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            }
        });
    }
}

// ---------------------------------------------------------------------------
// window + motion
// ---------------------------------------------------------------------------

fn scale_of(s: &Shell) -> f64 {
    s.ui.window().scale_factor() as f64
}

fn apply_size(s: &mut Shell, h: f32) {
    let scale = scale_of(s);
    let (_, _, wa_right, wa_bottom) = work_area_phys();
    let x = wa_right - ((PANEL_W + GAP - MARGIN) as f64) * scale;
    let y = wa_bottom - ((h + GAP - MARGIN) as f64) * scale;
    s.ui.window().set_size(LogicalSize::new(PANEL_W, h));
    s.ui.window()
        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
    s.ui.set_win_h(h);
    s.win_h = h;
}

fn natural_for(s: &Shell, ds: bool) -> f32 {
    if ds {
        s.natural_ds
    } else {
        s.natural_kimi
    }
}

fn show_panel(shell: &Rc<RefCell<Shell>>) {
    let (visible, h, ds) = {
        let s = shell.borrow();
        (
            s.visible,
            natural_for(&s, s.ui.get_tab_ds()),
            s.ui.get_tab_ds(),
        )
    };
    if visible {
        return;
    }
    {
        let mut s = shell.borrow_mut();
        s.apply_size(h);
        s.hiding = false;
        s.hide_run += 1;
        s.shown_at = Instant::now();
    }
    let shown = {
        let s = shell.borrow();
        s.ui.show().is_ok()
    };
    if !shown {
        logf("panel: show failed");
        return;
    }
    install_window_hooks(shell);
    let hwnd = shell.borrow().hwnd;
    if let Some(h) = hwnd {
        focus(h);
    }
    {
        let mut s = shell.borrow_mut();
        s.ui.set_shown(true);
        s.visible = true;
    }
    logf(format!(
        "panel: show tab-ds={ds} h={h:.1} scale={:.2}",
        shell.borrow().ui.window().scale_factor()
    ));
    // The winit window may only exist after the first event-loop tick; retry the
    // HWND-dependent setup once the loop has had a chance to create it.
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(120), move || {
        let Some(shell) = weak.upgrade() else { return };
        install_window_hooks(&shell);
        let (hwnd, visible) = {
            let s = shell.borrow();
            (s.hwnd, s.visible)
        };
        if let (Some(h), true) = (hwnd, visible) {
            focus(h);
        }
    });
}

fn start_hide(shell: &Rc<RefCell<Shell>>) {
    let run = {
        let mut s = shell.borrow_mut();
        if !s.visible || s.hiding {
            return;
        }
        s.hiding = true;
        s.hide_run += 1;
        // `shown=false` is the fade-out state; the window is hidden after 170ms.
        s.ui.set_shown(false);
        s.hide_run
    };
    logf("panel: hide start");
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(HIDE_MS), move || {
        let Some(shell) = weak.upgrade() else { return };
        let mut s = shell.borrow_mut();
        if !s.hiding || s.hide_run != run {
            return;
        }
        let _ = s.ui.hide();
        s.visible = false;
        s.hiding = false;
        s.last_hide = Some(Instant::now());
        s.finished_hide_at = Some(Instant::now());
        logf(format!("panel: hidden ({}ms budget)", HIDE_MS));
    });
}

fn toggle(shell: &Rc<RefCell<Shell>>, from_tray: bool) {
    if from_tray {
        let last = shell.borrow().last_hide;
        if let Some(t) = last {
            if t.elapsed() < TRAY_GUARD {
                logf(format!(
                    "tray: toggle ignored, {}ms since hide",
                    t.elapsed().as_millis()
                ));
                return;
            }
        }
    }
    if shell.borrow().visible {
        start_hide(shell);
    } else {
        show_panel(shell);
    }
}

fn on_blur(shell: &Rc<RefCell<Shell>>) {
    let act = {
        let s = shell.borrow();
        if !s.visible || s.hiding {
            false
        } else if s.shown_at.elapsed() < FOCUS_GRACE {
            logf("focus: blur inside the show grace window, ignored");
            false
        } else if s.login_running {
            logf("focus: blur while the sign-in child is up, ignored");
            false
        } else {
            true
        }
    };
    if !act {
        return;
    }
    shell.borrow_mut().blur_at = Some(Instant::now());
    logf("focus: lost, hiding");
    start_hide(shell);
}

/// Tab switch (PLAN §3.6). The window is pre-sized to the taller of the two
/// tabs BEFORE the first animated frame; the card grows into that room from the
/// bottom, so the visible bottom edge never moves. The tab itself flips one
/// frame later, which is what starts the Slint state transition.
fn request_tab(shell: &Rc<RefCell<Shell>>, ds: bool) {
    let (target, from, pre) = {
        let s = shell.borrow();
        if s.ui.get_tab_ds() == ds {
            return;
        }
        let target = natural_for(&s, ds);
        let pre = target.max(s.win_h);
        (target, s.ui.get_cell_h(), pre)
    };
    let to = if ds { 402.5 } else { 361.5 };
    let grow = to > from;
    let (run, snap) = {
        let mut s = shell.borrow_mut();
        s.run += 1;
        s.switching = true;
        s.ui.set_outgoing(if ds { 1 } else { 2 });
        s.apply_size(pre);
        let mut snap = false;
        if s.mode == Mode::TabAnim {
            if s.first_switch_at.is_none() {
                s.first_switch_at = Some(Instant::now());
                snap = true;
            }
            s.last_click_at = Some(Instant::now());
            s.last_click_ms = s
                .sampler_start
                .map(|t| t.elapsed().as_secs_f64() * 1000.0)
                .unwrap_or(0.0);
        }
        (s.run, snap)
    };
    if snap {
        snapshot_frames(shell);
    }
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(16), move || {
        let Some(shell) = weak.upgrade() else { return };
        let size = {
            let s = shell.borrow_mut();
            if s.run != run {
                return;
            }
            s.ui.set_tab_ds(ds);
            (s.ui.window().size().width, s.ui.window().size().height)
        };
        logf(format!(
            "tab: start ds={ds} from-cell={from:.1} pre={pre:.1} window={}x{} grow={grow}",
            size.0, size.1
        ));
        schedule_settle(&shell, run, target, if grow { 200 } else { 167 });
    });
}

fn schedule_settle(shell: &Rc<RefCell<Shell>>, run: u64, target: f32, dur_ms: u64) {
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(dur_ms + 80), move || {
        let Some(shell) = weak.upgrade() else { return };
        let size = {
            let mut s = shell.borrow_mut();
            if s.run != run {
                return;
            }
            s.switching = false;
            s.apply_size(target);
            s.ui.set_outgoing(0);
            (s.ui.window().size().width, s.ui.window().size().height)
        };
        logf(format!(
            "tab: settled at {target:.1} (window {}x{})",
            size.0, size.1
        ));
    });
}

fn snapshot_frames(shell: &Rc<RefCell<Shell>>) {
    let dir = shell.borrow().out_dir.clone();
    if dir.is_empty() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&dir) {
        logf(format!("tab: cannot create {}: {e}", dir));
        return;
    }
    for (delay, name) in [
        (40u64, "tab-040.png"),
        (100, "tab-100.png"),
        (160, "tab-160.png"),
        (260, "tab-260.png"),
    ] {
        let weak = Rc::downgrade(shell);
        let path = std::path::Path::new(&dir).join(name);
        Timer::single_shot(Duration::from_millis(delay), move || {
            let Some(shell) = weak.upgrade() else { return };
            let s = shell.borrow();
            match s.ui.window().take_snapshot() {
                Ok(buf) => match save_png(&buf, &path) {
                    Ok(()) => logf(format!("tab: snapshot {}", path.display())),
                    Err(e) => logf(format!("tab: snapshot failed: {e}")),
                },
                Err(e) => logf(format!("tab: snapshot failed: {e}")),
            }
        });
    }
}

fn save_png(
    buffer: &slint::SharedPixelBuffer<slint::Rgba8Pixel>,
    path: &std::path::Path,
) -> Result<(), String> {
    let mut bytes = Vec::with_capacity((buffer.width() * buffer.height() * 4) as usize);
    for pixel in buffer.as_slice() {
        bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
    }
    let image = image::RgbaImage::from_raw(buffer.width(), buffer.height(), bytes)
        .ok_or_else(|| "pixel buffer does not match its size".to_string())?;
    image.save(path).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// window hooks
// ---------------------------------------------------------------------------

fn install_window_hooks(shell: &Rc<RefCell<Shell>>) {
    use slint::winit_030::WinitWindowAccessor;
    let hwnd = hwnd_of(&shell.borrow().ui);
    {
        let mut s = shell.borrow_mut();
        if hwnd.is_none() || s.hwnd == hwnd {
            return;
        }
        s.hwnd = hwnd;
    }
    if let Some(h) = hwnd {
        unsafe {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::Graphics::Dwm::{
                DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
            };
            let pref = DWMWCP_DONOTROUND;
            let _ = DwmSetWindowAttribute(
                HWND(h as _),
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const core::ffi::c_void,
                std::mem::size_of_val(&pref) as u32,
            );
        }
    }
    let weak: Weak<RefCell<Shell>> = Rc::downgrade(shell);
    shell
        .borrow()
        .ui
        .window()
        .on_winit_window_event(move |_w, event| {
            if let slint::winit_030::winit::event::WindowEvent::Focused(false) = event {
                if let Some(shell) = weak.upgrade() {
                    on_blur(&shell);
                }
            }
            slint::winit_030::EventResult::Propagate
        });
    logf(format!("window: hooks installed hwnd={hwnd:?}"));
}

fn hwnd_of(ui: &Panel) -> Option<isize> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::WinitWindowAccessor;
    let mut out = None;
    ui.window().with_winit_window(|w| {
        if let Ok(handle) = w.window_handle() {
            if let RawWindowHandle::Win32(win32) = handle.as_raw() {
                out = Some(win32.hwnd.get());
            }
        }
    });
    out
}

fn focus(hwnd: isize) {
    // A process started from a terminal is not the foreground process, so a
    // plain SetForegroundWindow is refused (the taskbar just flashes). Attach to
    // the foreground thread's input queue for the duration of the call — the
    // same trick a shell uses — then detach.
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Threading::AttachThreadInput;
    use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
    use windows::Win32::UI::WindowsAndMessaging::{
        BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };
    unsafe {
        let h = HWND(hwnd as *mut core::ffi::c_void);
        let foreground = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(foreground, None);
        let our_thread = windows::Win32::System::Threading::GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != our_thread;
        if attached {
            let _ = AttachThreadInput(our_thread, fg_thread, true);
        }
        let _ = SetForegroundWindow(h);
        let _ = BringWindowToTop(h);
        let _ = SetFocus(Some(h));
        if attached {
            let _ = AttachThreadInput(our_thread, fg_thread, false);
        }
    }
}

fn work_area_phys() -> (f64, f64, f64, f64) {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut rect = RECT::default();
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rect as *mut _ as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    (
        rect.left as f64,
        rect.top as f64,
        rect.right as f64,
        rect.bottom as f64,
    )
}

fn window_rect(hwnd: isize) -> (i32, i32, i32, i32) {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(HWND(hwnd as _), &mut r);
    }
    (r.left, r.top, r.right, r.bottom)
}

fn is_visible(hwnd: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    unsafe { IsWindowVisible(HWND(hwnd as _)).as_bool() }
}

// ---------------------------------------------------------------------------
// tray
// ---------------------------------------------------------------------------

fn tooltip_text() -> String {
    // Same shape as tray.rs in the shipped app: product name, then one line per
    // provider; each provider carries its own failure suffix.
    "Quota Tray\nKimi 5h 31% · week 12%\nDeepSeek ¥45.44".to_string()
}

fn tray_icon_image() -> Option<tray_icon::Icon> {
    let bytes = include_bytes!("../../../src-tauri/icons/tray.png");
    let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = decoded.dimensions();
    tray_icon::Icon::from_rgba(decoded.into_raw(), w, h).ok()
}

fn build_tray(shell: &Rc<RefCell<Shell>>) {
    use tray_icon::menu::{Menu, MenuId, MenuItem};

    let Some(icon) = tray_icon_image() else {
        logf("tray: icon decode failed");
        return;
    };
    let menu = Menu::new();
    let open = MenuItem::with_id(MenuId::new("p0-open"), "Open Quota Tray", true, None);
    let refresh = MenuItem::with_id(MenuId::new("p0-refresh"), "Refresh now", true, None);
    let signin = MenuItem::with_id(MenuId::new("p0-signin"), "Sign in…", true, None);
    let exit = MenuItem::with_id(MenuId::new("p0-exit"), "Exit", true, None);
    for item in [&open, &refresh, &signin, &exit] {
        let _ = menu.append(item);
    }
    let built = tray_icon::TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip(tooltip_text())
        .with_menu(Box::new(menu))
        // Left click toggles the panel ourselves; right click shows the native
        // menu (the custom menu window is P3 and needs the right-click event,
        // which this keeps delivering as well).
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .build();
    match built {
        Ok(icon) => {
            let tooltip = tooltip_text();
            logf(format!(
                "tray: built, tooltip lines={} text={:?}",
                tooltip.lines().count(),
                tooltip
            ));
            shell.borrow_mut().tray_icon = Some(icon);
        }
        Err(e) => logf(format!("tray: build failed: {e}")),
    }
}

fn spawn_tray_poll(shell: &Rc<RefCell<Shell>>) {
    use tray_icon::menu::MenuEvent;
    use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

    let weak = Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(40), move || {
        let Some(shell) = weak.upgrade() else { return };
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click {
                    button,
                    button_state: MouseButtonState::Up,
                    ..
                } => match button {
                    MouseButton::Left => {
                        logf("tray: left click");
                        toggle(&shell, true);
                    }
                    MouseButton::Right => logf("tray: right click (menu)"),
                    _ => {}
                },
                TrayIconEvent::Enter { .. } | TrayIconEvent::Move { .. } => on_hover(&shell),
                TrayIconEvent::Leave { .. } => {}
                TrayIconEvent::DoubleClick { .. } => {}
                _ => {}
            }
        }
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id().0.as_str();
            logf(format!("tray: menu {id}"));
            match id {
                "p0-open" => toggle(&shell, false),
                "p0-refresh" => {
                    let mut s = shell.borrow_mut();
                    s.refresh_count += 1;
                    let now = chrono::Local::now().format("%H:%M").to_string();
                    s.ui.set_stamp_kimi(format!("Updated {now}").into());
                    s.ui.set_stamp_ds(format!("Updated {now}").into());
                }
                "p0-signin" => start_login(&shell),
                "p0-exit" => {
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            }
        }
    });
    shell.borrow_mut().tray_timer = Some(timer);
}

fn on_hover(shell: &Rc<RefCell<Shell>>) {
    let fire = {
        let mut s = shell.borrow_mut();
        match s.hover_at {
            Some(t) if t.elapsed() < HOVER_PREFETCH => false,
            _ => {
                s.hover_at = Some(Instant::now());
                true
            }
        }
    };
    if fire {
        logf("tray: hover prefetch (throttled 10s)");
    }
}

// ---------------------------------------------------------------------------
// sign-in child
// ---------------------------------------------------------------------------

fn start_login(shell: &Rc<RefCell<Shell>>) {
    if shell.borrow().login_running {
        logf("login: already running");
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        logf("login: cannot resolve current_exe");
        return;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--login-host")
        .arg("--parent-pid")
        .arg(std::process::id().to_string());
    if let Ok(dir) = std::env::var("P0_CONFIG_DIR") {
        cmd.env("P0_CONFIG_DIR", dir);
    }
    if let Ok(dir) = std::env::var("P0_DATA_DIR") {
        if !dir.is_empty() {
            cmd.arg("--data-dir").arg(dir);
        }
    }
    logf("login: spawning --login-host");
    match cmd.spawn() {
        Ok(child) => {
            {
                let mut s = shell.borrow_mut();
                s.login_child = Some(child);
                s.login_running = true;
            }
            let visible = shell.borrow().visible;
            if visible {
                start_hide(shell);
            }
            watch_login(shell);
        }
        Err(e) => logf(format!("login: spawn failed: {e}")),
    }
}

fn watch_login(shell: &Rc<RefCell<Shell>>) {
    let Some(mut child) = shell.borrow_mut().login_child.take() else {
        return;
    };
    // The wait happens off-thread; the exit code travels through a Send slot
    // because the UI thread's Shell is Rc<RefCell<..>> and cannot cross threads.
    let slot: Arc<Mutex<Option<i32>>> = Arc::new(Mutex::new(None));
    let slot_thread = slot.clone();
    std::thread::spawn(move || {
        let status = child.wait();
        let code = status.ok().and_then(|s| s.code()).unwrap_or(-1);
        *slot_thread.lock().unwrap() = Some(code);
    });
    let weak = Rc::downgrade(shell);
    let timer = Rc::new(Timer::default());
    let ticker = timer.clone();
    let stopper = timer.clone();
    ticker.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        let Some(shell) = weak.upgrade() else { return };
        let code = {
            let mut guard = slot.lock().unwrap();
            guard.take()
        };
        let Some(code) = code else { return };
        stopper.stop();
        finish_login(&shell, code);
    });
    shell.borrow_mut().login_timer = Some(timer);
}

fn finish_login(shell: &Rc<RefCell<Shell>>, code: i32) {
    let saved = credentials::has_user_token();
    {
        let mut s = shell.borrow_mut();
        s.login_running = false;
        s.login_timer = None;
        logf(format!(
            "login: child exited code={code} stored-token={saved}"
        ));
        let text = if code == 0 && saved {
            "DeepSeek sign-in: saved"
        } else {
            "DeepSeek sign-in: not saved"
        };
        s.ui.set_stamp_ds(text.into());
    }
    let visible = shell.borrow().visible;
    if !visible {
        show_panel(shell);
    }
}

// ---------------------------------------------------------------------------
// startup per mode
// ---------------------------------------------------------------------------

fn startup_mode(shell: &Rc<RefCell<Shell>>, seconds: u64) {
    let mode = shell.borrow().mode;
    logf(format!("mode: {mode:?}"));
    match mode {
        Mode::Interactive => {
            logf("ready: left-click the tray icon to open the panel");
        }
        Mode::TabAnim => {
            show_panel(shell);
            spawn_sampler(shell);
            start_tab_anim(shell);
        }
        Mode::Blur => {
            show_panel(shell);
            spawn_sampler(shell);
            start_blur_test(shell);
        }
        Mode::Toggle => {
            show_panel(shell);
            spawn_sampler(shell);
            start_toggle_test(shell);
        }
        Mode::Tray => {
            report_tray(shell);
            Timer::single_shot(Duration::from_secs(seconds.max(20)), || {
                let _ = slint::quit_event_loop();
            });
        }
        Mode::SlintTrayProbe => start_slint_tray(shell, seconds.max(20)),
        Mode::TrayClick => start_tray_click_test(shell),
        Mode::Hold => {
            show_panel(shell);
            let (out, demo) = {
                let s = shell.borrow();
                (s.out_dir.clone(), s.demo_switch)
            };
            if demo {
                let weak = Rc::downgrade(shell);
                Timer::single_shot(Duration::from_millis(1000), move || {
                    let Some(shell) = weak.upgrade() else { return };
                    let ds = !shell.borrow().ui.get_tab_ds();
                    request_tab(&shell, ds);
                });
                if !out.is_empty() {
                    let weak = Rc::downgrade(shell);
                    let path = std::path::Path::new(&out).join("switch-mid.png");
                    Timer::single_shot(Duration::from_millis(1100), move || {
                        let Some(shell) = weak.upgrade() else { return };
                        let s = shell.borrow();
                        match s.ui.window().take_snapshot() {
                            Ok(buf) => match save_png(&buf, &path) {
                                Ok(()) => logf(format!("demo: snapshot {}", path.display())),
                                Err(e) => logf(format!("demo: snapshot failed: {e}")),
                            },
                            Err(e) => logf(format!("demo: snapshot failed: {e}")),
                        }
                    });
                }
            } else if !out.is_empty() {
                let weak = Rc::downgrade(shell);
                Timer::single_shot(Duration::from_millis(1200), move || {
                    let Some(shell) = weak.upgrade() else { return };
                    snapshot_frames(&shell);
                });
            }
            let weak = Rc::downgrade(shell);
            Timer::single_shot(Duration::from_millis(900), move || {
                let Some(shell) = weak.upgrade() else { return };
                let s = shell.borrow();
                logf(format!(
                    "hold: shown={} body-opacity={:.2} body-lift={:.1} cell-h={:.1} win-h={:.1} size={}x{}",
                    s.ui.get_shown(),
                    s.ui.get_body_opacity(),
                    s.ui.get_body_lift(),
                    s.ui.get_cell_h(),
                    s.ui.get_win_h(),
                    s.ui.window().size().width,
                    s.ui.window().size().height
                ));
                if let Some(h) = s.hwnd {
                    use windows::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_EXSTYLE};
                    let ex = unsafe {
                        GetWindowLongPtrW(windows::Win32::Foundation::HWND(h as _), GWL_EXSTYLE)
                    };
                    logf(format!(
                        "hold: exstyle=0x{ex:x} layered={} transparent={}",
                        (ex & 0x80000) != 0,
                        (ex & 0x20) != 0
                    ));
                }
            });
            Timer::single_shot(Duration::from_secs(seconds.max(10)), || {
                let _ = slint::quit_event_loop();
            });
        }
    }
}

/// Post the tray window messages Explorer would send. This exercises the same
/// wndproc -> channel -> poll-timer path as a real mouse, without depending on
/// this session accepting synthetic mouse input (it does not; even the Start
/// button ignores SendInput here).
fn tray_hwnd(shell: &Rc<RefCell<Shell>>) -> Option<isize> {
    shell
        .borrow()
        .tray_icon
        .as_ref()
        .map(|t| t.window_handle() as isize)
}

fn post_tray_msg(hwnd: isize, msg: u32) {
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::PostMessageW;
    const WM_USER_TRAYICON: u32 = 6002;
    let h = HWND(hwnd as *mut core::ffi::c_void);
    let _ = unsafe { PostMessageW(Some(h), WM_USER_TRAYICON, WPARAM(0), LPARAM(msg as isize)) };
}

fn start_tray_click_test(shell: &Rc<RefCell<Shell>>) {
    use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONUP, WM_MOUSEMOVE};

    let hwnd = tray_hwnd(shell).unwrap_or(0);
    for (delay, msg, label) in [
        (600u64, WM_MOUSEMOVE, "hover"),
        (1000, WM_LBUTTONUP, "left click"),
    ] {
        Timer::single_shot(Duration::from_millis(delay), move || {
            if hwnd != 0 {
                post_tray_msg(hwnd, msg);
                logf(format!("tray-test: posted {label}"));
            }
        });
    }
    let weak = Rc::downgrade(shell);
    let h = hwnd;
    Timer::single_shot(Duration::from_millis(1800), move || {
        let Some(shell) = weak.upgrade() else { return };
        let visible = shell.borrow().visible;
        println!(
            "tray-click: panel-visible-after-click={visible} verdict={}",
            if visible { "PASS" } else { "FAIL" }
        );
        if h != 0 {
            post_tray_msg(h, WM_LBUTTONUP);
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

fn report_tray(shell: &Rc<RefCell<Shell>>) {
    let s = shell.borrow();
    match &s.tray_icon {
        Some(icon) => {
            let rect = icon.rect();
            logf(format!(
                "tray: registered rect={:?} tooltip-lines={}",
                rect,
                tooltip_text().lines().count()
            ));
        }
        None => logf("tray: NOT registered"),
    }
}

fn start_slint_tray(shell: &Rc<RefCell<Shell>>, seconds: u64) {
    let tray = match SlintTray::new() {
        Ok(t) => t,
        Err(e) => {
            logf(format!("slint-tray: build failed: {e}"));
            let _ = slint::quit_event_loop();
            return;
        }
    };
    let tip = tooltip_text();
    tray.set_tip(tip.clone().into());
    logf(format!(
        "slint-tray: tooltip lines={} text={:?}",
        tip.lines().count(),
        tip
    ));
    tray.on_open_requested(|| logf("slint-tray: left click"));
    if let Err(e) = tray.show() {
        logf(format!("slint-tray: show failed: {e}"));
    }
    shell.borrow_mut().slint_tray = Some(tray);
    Timer::single_shot(Duration::from_secs(seconds), || {
        let _ = slint::quit_event_loop();
    });
}

// ---------------------------------------------------------------------------
// self-checks
// ---------------------------------------------------------------------------

fn spawn_sampler(shell: &Rc<RefCell<Shell>>) {
    let weak = Rc::downgrade(shell);
    let start = Instant::now();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(5), move || {
        let Some(shell) = weak.upgrade() else { return };
        let mut s = shell.borrow_mut();
        let Some(hwnd) = s.hwnd else { return };
        let (left, top, right, bottom) = window_rect(hwnd);
        let cell = s.ui.get_cell_h();
        let scale = s.ui.window().scale_factor();
        let win_h = s.ui.get_win_h();
        // The card is anchored to the window's bottom edge, 28px in. Derive its
        // screen edges from the real window rect + the UI's own sizes.
        let card_bottom = top + ((win_h - 28.0) * scale).round() as i32;
        let card_h = 88.5 + cell + 54.0;
        let card_top = top + ((win_h - 28.0 - card_h) * scale).round() as i32;
        s.samples.push(Sample {
            t_ms: start.elapsed().as_secs_f64() * 1000.0,
            left,
            top,
            right,
            bottom,
            visible: is_visible(hwnd),
            cell,
            card_bottom,
            card_top,
        });
    });
    let mut s = shell.borrow_mut();
    s.sampler_start = Some(start);
    s.test_timers.push(Rc::new(timer));
}

fn start_tab_anim(shell: &Rc<RefCell<Shell>>) {
    let clicks = Rc::new(Cell::new(0u32));
    let weak = Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(40), move || {
        let Some(shell) = weak.upgrade() else { return };
        let n = clicks.get();
        if n >= 30 {
            return;
        }
        clicks.set(n + 1);
        {
            let mut s = shell.borrow_mut();
            s.clicks_left = 30 - n - 1;
            s.last_click_at = Some(Instant::now());
        }
        let ds = !shell.borrow().ui.get_tab_ds();
        request_tab(&shell, ds);
    });
    {
        let mut s = shell.borrow_mut();
        s.clicks_left = 30;
        s.test_timers.push(Rc::new(timer));
    }

    // Finish when the last click has settled.
    let weak = Rc::downgrade(shell);
    let watch = Timer::default();
    watch.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let Some(shell) = weak.upgrade() else { return };
        let done = {
            let s = shell.borrow();
            s.clicks_left == 0
                && !s.switching
                && !s.reported
                && s.last_click_at
                    .map(|t| t.elapsed() > Duration::from_millis(400))
                    .unwrap_or(false)
        };
        if done {
            shell.borrow_mut().reported = true;
            finish_tab_anim(&shell);
        }
    });
    shell.borrow_mut().test_timers.push(Rc::new(watch));
}

fn finish_tab_anim(shell: &Rc<RefCell<Shell>>) {
    let s = shell.borrow();
    let visible: Vec<&Sample> = s.samples.iter().filter(|x| x.visible).collect();
    if visible.is_empty() {
        logf("tab-anim: no samples");
        let _ = slint::quit_event_loop();
        return;
    }
    let bottom_min = visible.iter().map(|x| x.bottom).min().unwrap();
    let bottom_max = visible.iter().map(|x| x.bottom).max().unwrap();
    let right_min = visible.iter().map(|x| x.right).min().unwrap();
    let right_max = visible.iter().map(|x| x.right).max().unwrap();
    let card_bottom_min = visible.iter().map(|x| x.card_bottom).min().unwrap();
    let card_bottom_max = visible.iter().map(|x| x.card_bottom).max().unwrap();
    let card_top_min = visible.iter().map(|x| x.card_top).min().unwrap();
    let card_top_max = visible.iter().map(|x| x.card_top).max().unwrap();
    let top_min = visible.iter().map(|x| x.top).min().unwrap();
    let top_max = visible.iter().map(|x| x.top).max().unwrap();
    let left_min = visible.iter().map(|x| x.left).min().unwrap();
    let left_max = visible.iter().map(|x| x.left).max().unwrap();
    let height_min = visible.iter().map(|x| x.bottom - x.top).min().unwrap();
    let height_max = visible.iter().map(|x| x.bottom - x.top).max().unwrap();
    let drift = bottom_max - bottom_min;
    println!(
        "tab-anim: clicks=30 samples={} bottom=[{bottom_min}..{bottom_max}] drift={drift}px \
         top=[{top_min}..{top_max}] left=[{left_min}..{left_max}] height=[{height_min}..{height_max}]",
        visible.len()
    );
    println!(
        "tab-anim: verdict {} (bottom edge drift <= 1px)",
        if drift <= 1 { "PASS" } else { "FAIL" }
    );
    println!(
        "tab-anim: card bottom=[{card_bottom_min}..{card_bottom_max}] drift={}px          card top=[{card_top_min}..{card_top_max}] travel={}px",
        card_bottom_max - card_bottom_min,
        card_top_max - card_top_min
    );
    logf(format!(
        "tab-anim: drift={drift}px right-drift={}px height=[{height_min}..{height_max}]",
        right_max - right_min
    ));
    // How long the final switch took to reach its end value: sampled from the
    // animated cell height, not from the scheduled timer.
    let target_cell = if s.ui.get_tab_ds() { 402.5 } else { 361.5 };
    let reached = s
        .samples
        .iter()
        .find(|x| x.t_ms >= s.last_click_ms && (x.cell - target_cell).abs() < 0.3)
        .map(|x| x.t_ms);
    if let Some(reached) = reached {
        println!(
            "tab-anim: last-switch cell {target_cell:.1}px reached after {:.0}ms (sampled)",
            reached - s.last_click_ms
        );
    }
    let _ = slint::quit_event_loop();
}

fn start_blur_test(shell: &Rc<RefCell<Shell>>) {
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(700), move || {
        let Some(shell) = weak.upgrade() else { return };
        let hwnd = shell.borrow().hwnd;
        let Some(hwnd) = hwnd else {
            logf("blur: no hwnd");
            let _ = slint::quit_event_loop();
            return;
        };
        // Make sure the panel actually owns the foreground before taking it
        // away: the blur event can only fire if the panel had focus.
        focus(hwnd);
        std::thread::sleep(Duration::from_millis(200));
        let slot: Arc<Mutex<Option<u128>>> = Arc::new(Mutex::new(None));
        let slot_thread = slot.clone();
        std::thread::spawn(move || {
            let t0 = Instant::now();
            focus_taskbar();
            let mut hidden_after = None;
            while t0.elapsed() < Duration::from_secs(2) {
                if !is_visible(hwnd) {
                    hidden_after = Some(t0.elapsed().as_millis());
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            logf(format!("blur: thread measured {:?}", hidden_after));
            *slot_thread.lock().unwrap() = Some(hidden_after.unwrap_or(u128::MAX));
        });
        let weak = Rc::downgrade(&shell);
        let timer = Rc::new(Timer::default());
        let ticker = timer.clone();
        let stopper = timer.clone();
        ticker.start(TimerMode::Repeated, Duration::from_millis(30), move || {
            let Some(shell) = weak.upgrade() else { return };
            let value = {
                let mut guard = slot.lock().unwrap();
                guard.take()
            };
            let Some(value) = value else { return };
            stopper.stop();
            let s = shell.borrow();
            let blur_seen = s.blur_at.is_some();
            drop(s);
            if value == u128::MAX {
                println!("blur: verdict=FAIL (still visible after 2000ms)");
            } else {
                println!(
                    "blur: hidden-after={value}ms budget={HIDE_MS}ms blur-event={} verdict={}",
                    if blur_seen { "seen" } else { "MISSING" },
                    if value <= HIDE_MS as u128 + 60 {
                        "PASS"
                    } else {
                        "FAIL"
                    }
                );
            }
            logf("blur: done");
            let _ = slint::quit_event_loop();
        });
        shell.borrow_mut().test_timers.push(timer);
    });
}

fn focus_taskbar() {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
    unsafe {
        if let Ok(h) = FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) {
            let _ = SetForegroundWindow(h);
            logf("blur: focused Shell_TrayWnd");
        }
    }
}

fn start_toggle_test(shell: &Rc<RefCell<Shell>>) {
    let steps: [(u64, bool); 4] = [(300, true), (800, false), (1500, true), (2100, false)];
    for (delay, show) in steps {
        let weak = Rc::downgrade(shell);
        Timer::single_shot(Duration::from_millis(delay), move || {
            let Some(shell) = weak.upgrade() else { return };
            if show {
                show_panel(&shell);
            } else {
                start_hide(&shell);
            }
        });
    }
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(2600), move || {
        let Some(shell) = weak.upgrade() else { return };
        let s = shell.borrow();
        let mut transitions = Vec::new();
        let mut last: Option<bool> = None;
        for sample in &s.samples {
            if last != Some(sample.visible) {
                transitions.push(format!("{:.0}ms={}", sample.t_ms, sample.visible));
                last = Some(sample.visible);
            }
        }
        println!("toggle: transitions {}", transitions.join(" "));
        let ok = transitions.len() >= 4;
        println!("toggle: verdict {}", if ok { "PASS" } else { "FAIL" });
        drop(s);
        let _ = slint::quit_event_loop();
    });
}

// ---------------------------------------------------------------------------
// sample data
// ---------------------------------------------------------------------------

fn apply_sample_data(ui: &Panel) {
    ui.set_week(12);
    ui.set_five(31);
    ui.set_week_reset("Resets in 2d 18h".into());
    ui.set_five_reset("Resets in 3h 33m".into());
    ui.set_cache_week("99.5".into());
    ui.set_cache_week_pct(99.5);
    ui.set_cache_week_note("3,169 requests".into());
    ui.set_cache_today("99.7".into());
    ui.set_cache_today_pct(99.7);
    ui.set_cache_today_note("807 requests".into());
    ui.set_speed_value("135.8 tok/s".into());
    ui.set_speed_note("First token 1.04 s · 3,166 calls".into());
    ui.set_extra_balance("No data".into());
    ui.set_cli_version("0.39.1".into());
    ui.set_ds_status("Available".into());
    ui.set_ds_total("¥45.44".into());
    ui.set_ds_detail("Granted ¥0.00 · Topped up ¥45.44".into());
    ui.set_ds_sum("This month ¥44.40 · 897M tok".into());
    ui.set_ds_sum_today("today ¥6.85".into());
    ui.set_stamp_kimi("Updated 16:42".into());
    ui.set_stamp_ds("Updated 16:31".into());
    ui.set_models(slint::ModelRc::new(slint::VecModel::from(models())));
    let _ = &ui;
}

fn models() -> Vec<ModelRow> {
    vec![
        ModelRow {
            name: "DeepSeek-V4.1-Flash".into(),
            cost: "¥28.50".into(),
            share: 1.0,
            tokens: "665.8M".into(),
            hit: "cache 99.6%".into(),
            hit_bad: false,
            today: "today ¥4.53".into(),
        },
        ModelRow {
            name: "DeepSeek-V4.1-Pro".into(),
            cost: "¥12.88".into(),
            share: 0.2766,
            tokens: "184.2M".into(),
            hit: "cache 71.3%".into(),
            hit_bad: false,
            today: "today ¥1.90".into(),
        },
        ModelRow {
            name: "DeepSeek-V3.2".into(),
            cost: "¥2.13".into(),
            share: 0.0619,
            tokens: "41.2M".into(),
            hit: "cache 58.4%".into(),
            hit_bad: true,
            today: "today ¥0.30".into(),
        },
        ModelRow {
            name: "DeepSeek-V4.1-Reasoner-Pro-Max-Extended".into(),
            cost: "¥0.88".into(),
            share: 0.0136,
            tokens: "9.03M".into(),
            hit: "cache --".into(),
            hit_bad: false,
            today: "today ¥0.11".into(),
        },
    ]
}
