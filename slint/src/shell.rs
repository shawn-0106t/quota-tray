// The shell: one transparent Slint window, a tray-icon crate tray, the tab
// switch controller, the work-area-anchored sizing and the data pump.
//
// Window model (PLAN §4 / §3.6): the window is sized to (content + 2x28 shadow
// room) and pinned so its bottom-right corner is (12px card gap - 28px margin)
// from the work-area corner. The card is anchored to the window's BOTTOM edge
// from inside (panel.slint), so the content growing or the tab switching never
// moves the visible card edge. When the content is taller than the work area the
// window stops at the cap and the card scrolls instead (the header and the
// provider tab must stay reachable — PLAN §3.6).

use crate::app::{self, Data, Msg};
use crate::checks;
use crate::height::{self, PANEL_W};
use crate::view;
use crate::{Anchor, Panel};
use slint::{ComponentHandle, LogicalSize, PhysicalPosition, Timer, TimerMode};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// Tray left-clicks this soon after an auto-hide are ignored (SPEC 14).
pub const TRAY_GUARD: Duration = Duration::from_millis(300);
/// Blur -> window hidden budget (main.ts playHide waits 170ms).
pub const HIDE_MS: u64 = 170;
/// A show steals focus; a blur inside this window is the tray click's own
/// mouse-up handing focus back to the taskbar, not the user clicking away.
const FOCUS_GRACE: Duration = Duration::from_millis(250);
const HOVER_PREFETCH: Duration = Duration::from_secs(10);
/// How long a content shrink waits for the cell animation to land.
const SHRINK_SETTLE_MS: u64 = 220;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Interactive,
    /// Print the card/window height the Slint layout derives for every data
    /// variant, then exit (spike/p2/heights_ref.py compares it with the DOM).
    Heights,
    /// Live corner-anchor measurement: window/card edges vs work area, plus the
    /// drift across a burst of tab switches.
    Corner,
    /// Persist -> reload round trip for `ActiveTab`, in an isolated config dir.
    ActiveTab,
    /// 30 tab clicks at 40ms, sampling the real window rect for edge drift.
    TabAnim,
    /// Focus loss -> hidden latency.
    Blur,
    /// Post the tray callback messages Explorer would send.
    TrayClick,
    /// Tray only; ends after `--seconds`.
    Tray,
    /// Show the panel and hold it (screenshots / visual inspection).
    Hold,
    /// Force a tiny work-area cap and prove the card scrolls instead of
    /// clipping the header and the provider tab.
    Clamp,
}

pub struct Options {
    pub mode: Mode,
    pub theme: Option<String>,
    pub tab: Option<String>,
    pub variant: Option<String>,
    pub seconds: u64,
    pub out_dir: String,
    pub demo_switch: bool,
    /// Test-only work-area cap (logical px), so the clamp path can be exercised
    /// on a screen that is tall enough not to need it.
    pub max_window_h: Option<f32>,
    /// Skip the background pollers: a `--variant` screenshot must show the
    /// variant, not whatever the live account returned two seconds later.
    pub no_poll: bool,
}

/// Instrumentation for the self-check modes. Empty in normal runs.
#[derive(Default)]
pub struct Checks {
    pub samples: Vec<Sample>,
    pub clicks_left: u32,
    pub last_click_at: Option<Instant>,
    pub reported: bool,
    pub blur_at: Option<Instant>,
    pub timers: Vec<Rc<Timer>>,
}

#[derive(Clone, Copy)]
pub struct Sample {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub visible: bool,
}

pub struct Shell {
    pub ui: Panel,
    pub data: Data,
    pub mode: Mode,
    pub visible: bool,
    pub hiding: bool,
    pub hide_run: u64,
    pub shown_at: Instant,
    pub last_hide: Option<Instant>,
    pub switching: bool,
    pub run: u64,
    pub shrink_run: u64,
    pub win_h: f32,
    pub max_win_h: f32,
    pub hwnd: Option<isize>,
    pub tray: Option<tray_icon::TrayIcon>,
    pub tray_timer: Option<Timer>,
    pub poll_timer: Option<Timer>,
    pub refresh: Arc<Notify>,
    pub hover_at: Option<Instant>,
    pub out_dir: String,
    pub demo_switch: bool,
    pub max_override: Option<f32>,
    /// Pollers + their runtime. `None` under `--no-poll`; the field also keeps
    /// the runtime alive for the process lifetime.
    pub workers: Option<app::Workers>,
    pub checks: Checks,
}

impl Shell {
    fn apply_size(&mut self, h: f32) {
        let scale = self.ui.window().scale_factor() as f64;
        let (_, wa_top, wa_right, wa_bottom) = work_area_phys();
        let mut max_h = height::max_window_h(wa_top, wa_bottom, scale);
        if let Some(cap) = self.max_override {
            max_h = max_h.min(cap);
        }
        self.max_win_h = max_h;
        let h = height::clamp_window(h, self.max_win_h);
        self.ui.set_max_card_h(height::max_card_h(self.max_win_h));
        let (x, y) = height::window_pos(h, wa_right, wa_bottom, scale);
        self.ui.window().set_size(LogicalSize::new(PANEL_W, h));
        self.ui.window().set_position(PhysicalPosition::new(x, y));
        self.ui.set_win_h(h);
        self.win_h = h;
    }

    fn natural_for(&self, ds: bool) -> f32 {
        if ds {
            self.ui.get_natural_ds()
        } else {
            self.ui.get_natural_kimi()
        }
    }
}

pub fn run(opts: Options) -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("SLINT_BACKEND").is_err() {
        std::env::set_var("SLINT_BACKEND", "winit-skia");
    }
    select_backend()?;

    let ui = Panel::new()?;
    let mut data = Data::load();
    if let Some(theme) = &opts.theme {
        data.theme = theme.clone();
    }
    if let Some(tab) = &opts.tab {
        data.settings.active_tab = quota_core::settings::normalize_tab(tab).to_string();
    }
    if let Some(variant) = &opts.variant {
        checks::apply_variant(&mut data, variant);
    }
    view::apply(&ui, &data);
    let start_ds = data.settings.active_tab == "deepseek";
    ui.set_tab_ds(start_ds);
    ui.set_outgoing(0);

    // Pure read-out mode: no window, no event loop, one line per variant.
    if opts.mode == Mode::Heights {
        return checks::print_heights(&ui);
    }

    let workers = if opts.no_poll {
        None
    } else {
        Some(app::spawn(data.settings.refresh_minutes))
    };
    let refresh = workers
        .as_ref()
        .map(|w| w.refresh.clone())
        .unwrap_or_else(|| Arc::new(Notify::new()));
    let shell = Rc::new(RefCell::new(Shell {
        ui,
        data,
        mode: opts.mode,
        visible: false,
        hiding: false,
        hide_run: 0,
        shown_at: Instant::now(),
        last_hide: None,
        switching: false,
        run: 0,
        shrink_run: 0,
        win_h: 560.0,
        max_win_h: 10_000.0,
        hwnd: None,
        tray: None,
        tray_timer: None,
        poll_timer: None,
        refresh,
        hover_at: None,
        out_dir: opts.out_dir.clone(),
        demo_switch: opts.demo_switch,
        max_override: opts.max_window_h,
        workers,
        checks: Checks::default(),
    }));

    // Park the keepalive window off-screen before anything can hide the panel.
    let anchor = Anchor::new()?;
    if anchor.show().is_err() {
        crate::logf("anchor: show failed; the event loop will quit on the first panel hide");
    }
    anchor
        .window()
        .set_position(PhysicalPosition::new(-30000, -30000));

    wire_callbacks(&shell);
    build_tray(&shell);
    spawn_tray_poll(&shell);
    spawn_data_poll(&shell);

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
            crate::logf(format!("panel: action {name}"));
            match name.as_str() {
                "console" => {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", "https://www.kimi.com/code/console"])
                        .spawn();
                }
                "refresh" => shell_action.borrow().refresh.notify_waiters(),
                "settings" => crate::logf("panel: settings window is P3, not built"),
                "exit" => {
                    let _ = slint::quit_event_loop();
                }
                _ => {}
            }
        });
    }
    let shell_releases = shell.clone();
    {
        let s = shell.borrow();
        s.ui.on_open_releases(move || {
            crate::logf("panel: open releases");
            let _ = shell_releases;
            let _ = std::process::Command::new("cmd")
                .args([
                    "/C",
                    "start",
                    "",
                    "https://github.com/MoonshotAI/kimi-code/releases",
                ])
                .spawn();
        });
    }
}

// ---------------------------------------------------------------------------
// data pump
// ---------------------------------------------------------------------------

fn spawn_data_poll(shell: &Rc<RefCell<Shell>>) {
    let weak = Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let Some(shell) = weak.upgrade() else { return };
        let mut changed = false;
        let mut theme = None;
        let mut drained = Vec::new();
        {
            let s = shell.borrow();
            let Some(workers) = &s.workers else { return };
            while let Ok(msg) = workers.rx.try_recv() {
                drained.push(msg);
            }
        }
        if !drained.is_empty() {
            let mut s = shell.borrow_mut();
            for msg in drained {
                match msg {
                    Msg::Quota(quota) => s.data.quota = Some(quota),
                    Msg::Deepseek(deepseek) => s.data.deepseek = Some(deepseek),
                    Msg::Usage(usage) => s.data.usage = Some(usage),
                    Msg::Update(update) => s.data.update = update,
                    Msg::Theme(next) => theme = Some(next),
                }
                changed = true;
            }
            if let Some(next) = theme {
                s.data.theme = next;
            }
        }
        if !changed {
            return;
        }
        {
            let s = shell.borrow();
            view::apply(&s.ui, &s.data);
            update_tray_tooltip(&s);
        }
        sync_height(&shell);
    });
    shell.borrow_mut().poll_timer = Some(timer);
}

/// Size the window to the content of the tab on screen.
///
/// Growing is immediate: the card is bottom-anchored, so the extra room opens
/// above it and the visible edge never moves. Shrinking waits for the cell
/// animation (the card is still tall while Slint eases the cell down, and a
/// window that shrank first would clip the card's top).
fn sync_height(shell: &Rc<RefCell<Shell>>) {
    let (natural, win_h, switching, visible) = {
        let s = shell.borrow();
        (
            s.natural_for(s.ui.get_tab_ds()),
            s.win_h,
            s.switching,
            s.visible,
        )
    };
    if switching {
        // A poll tick during the switch may only grow the pre-sized window;
        // the settle applies the final height.
        if natural > win_h + 0.5 {
            shell.borrow_mut().apply_size(natural);
        }
        return;
    }
    if !visible || natural > win_h + 0.5 {
        shell.borrow_mut().apply_size(natural);
        return;
    }
    if natural < win_h - 0.5 {
        let run = {
            let mut s = shell.borrow_mut();
            s.shrink_run += 1;
            s.shrink_run
        };
        let weak = Rc::downgrade(shell);
        Timer::single_shot(Duration::from_millis(SHRINK_SETTLE_MS), move || {
            let Some(shell) = weak.upgrade() else { return };
            let target = {
                let s = shell.borrow();
                if s.shrink_run != run || s.switching {
                    return;
                }
                s.natural_for(s.ui.get_tab_ds())
            };
            shell.borrow_mut().apply_size(target);
        });
    }
}

// ---------------------------------------------------------------------------
// window + motion
// ---------------------------------------------------------------------------

pub fn show_panel(shell: &Rc<RefCell<Shell>>) {
    let (visible, h, ds) = {
        let s = shell.borrow();
        (
            s.visible,
            s.natural_for(s.ui.get_tab_ds()),
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
        crate::logf("panel: show failed");
        return;
    }
    install_window_hooks(shell);
    let (hwnd, scale) = {
        let s = shell.borrow();
        (s.hwnd, s.ui.window().scale_factor())
    };
    if let Some(h) = hwnd {
        focus(h);
    }
    {
        let mut s = shell.borrow_mut();
        s.ui.set_shown(true);
        s.visible = true;
    }
    crate::logf(format!("panel: show tab-ds={ds} h={h:.1} scale={scale:.2}"));
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

pub fn start_hide(shell: &Rc<RefCell<Shell>>) {
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
    crate::logf("panel: hide start");
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
        crate::logf(format!("panel: hidden ({}ms budget)", HIDE_MS));
    });
}

fn toggle(shell: &Rc<RefCell<Shell>>, from_tray: bool) {
    if from_tray {
        let last = shell.borrow().last_hide;
        if let Some(t) = last {
            if t.elapsed() < TRAY_GUARD {
                crate::logf(format!(
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

pub fn on_blur(shell: &Rc<RefCell<Shell>>) {
    let act = {
        let s = shell.borrow();
        if !s.visible || s.hiding {
            false
        } else if s.shown_at.elapsed() < FOCUS_GRACE {
            crate::logf("focus: blur inside the show grace window, ignored");
            false
        } else {
            true
        }
    };
    if !act {
        return;
    }
    shell.borrow_mut().checks.blur_at = Some(Instant::now());
    crate::logf("focus: lost, hiding");
    start_hide(shell);
}

/// Tab switch (PLAN §3.6). The window is pre-sized to the taller of the two
/// tabs BEFORE the first animated frame; the card grows into that room from the
/// bottom, so the visible bottom edge never moves. The tab itself flips one
/// frame later, which is what starts the Slint state transition.
pub fn request_tab(shell: &Rc<RefCell<Shell>>, ds: bool) {
    let (pre, from_natural, grow) = {
        let s = shell.borrow();
        if s.ui.get_tab_ds() == ds {
            return;
        }
        let target = s.natural_for(ds);
        let pre = target.max(s.win_h);
        (
            pre,
            s.natural_for(s.ui.get_tab_ds()),
            target > s.natural_for(s.ui.get_tab_ds()) + 0.5,
        )
    };
    let duration = if grow { 200 } else { 167 };
    let run = {
        let mut s = shell.borrow_mut();
        s.run += 1;
        s.shrink_run += 1; // a pending shrink belongs to the tab being left
        s.switching = true;
        s.ui.set_outgoing(if ds { 1 } else { 2 });
        s.apply_size(pre);
        // Persist the choice (PLAN §4: only ActiveTab is touched).
        s.data.save_tab(if ds { "deepseek" } else { "kimi" });
        if s.mode == Mode::TabAnim {
            s.checks.last_click_at = Some(Instant::now());
        }
        s.run
    };
    crate::logf(format!(
        "tab: start ds={ds} from-natural={from_natural:.1} pre={pre:.1} grow={grow}"
    ));
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(16), move || {
        let Some(shell) = weak.upgrade() else { return };
        if shell.borrow().run != run {
            return;
        }
        shell.borrow().ui.set_tab_ds(ds);
        schedule_settle(&shell, run, duration);
    });
}

fn schedule_settle(shell: &Rc<RefCell<Shell>>, run: u64, dur_ms: u64) {
    let weak = Rc::downgrade(shell);
    Timer::single_shot(Duration::from_millis(dur_ms + 80), move || {
        let Some(shell) = weak.upgrade() else { return };
        let size = {
            let mut s = shell.borrow_mut();
            if s.run != run {
                return;
            }
            s.switching = false;
            s.ui.set_outgoing(0);
            // The content may have moved during the animation (a poll tick);
            // settle on the live natural height, not the one read at click time.
            let live = s.natural_for(s.ui.get_tab_ds());
            s.apply_size(live);
            (s.ui.window().size().width, s.ui.window().size().height)
        };
        crate::logf(format!("tab: settled at {}x{}", size.0, size.1));
    });
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
    let weak = Rc::downgrade(shell);
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
    crate::logf(format!("window: hooks installed hwnd={hwnd:?}"));
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

pub fn focus(hwnd: isize) {
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

/// Primary-monitor work area in physical px (SPI_GETWORKAREA).
pub fn work_area_phys() -> (f64, f64, f64, f64) {
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

pub fn window_rect(hwnd: isize) -> (i32, i32, i32, i32) {
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(HWND(hwnd as _), &mut r);
    }
    (r.left, r.top, r.right, r.bottom)
}

pub fn is_visible(hwnd: isize) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
    unsafe { IsWindowVisible(HWND(hwnd as _)).as_bool() }
}

// ---------------------------------------------------------------------------
// tray
// ---------------------------------------------------------------------------

fn tray_icon_image() -> Option<tray_icon::Icon> {
    let bytes = include_bytes!("../../src-tauri/icons/tray.png");
    let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = decoded.dimensions();
    tray_icon::Icon::from_rgba(decoded.into_raw(), w, h).ok()
}

fn tooltip_text(s: &Shell) -> String {
    quota_core::format::tray_tooltip(s.data.quota.as_ref(), s.data.deepseek.as_ref())
}

fn update_tray_tooltip(s: &Shell) {
    let Some(tray) = &s.tray else { return };
    let text = tooltip_text(s);
    let _ = tray.set_tooltip(Some(text));
}

fn build_tray(shell: &Rc<RefCell<Shell>>) {
    use tray_icon::menu::{Menu, MenuId, MenuItem};

    let Some(icon) = tray_icon_image() else {
        crate::logf("tray: icon decode failed");
        return;
    };
    let menu = Menu::new();
    let open = MenuItem::with_id(MenuId::new("p2-open"), "Open Quota Tray", true, None);
    let refresh = MenuItem::with_id(MenuId::new("p2-refresh"), "Refresh now", true, None);
    let exit = MenuItem::with_id(MenuId::new("p2-exit"), "Exit", true, None);
    for item in [&open, &refresh, &exit] {
        let _ = menu.append(item);
    }
    let tooltip = tooltip_text(&shell.borrow());
    let built = tray_icon::TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip(tooltip)
        .with_menu(Box::new(menu))
        // Left click toggles the panel ourselves; right click shows the native
        // menu (the custom menu window is P3 and needs the right-click event,
        // which this keeps delivering as well).
        .with_menu_on_left_click(false)
        .with_menu_on_right_click(true)
        .build();
    match built {
        Ok(icon) => {
            crate::logf(format!(
                "tray: built, tooltip={:?}",
                tooltip_text(&shell.borrow())
            ));
            shell.borrow_mut().tray = Some(icon);
        }
        Err(e) => crate::logf(format!("tray: build failed: {e}")),
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
                        crate::logf("tray: left click");
                        toggle(&shell, true);
                    }
                    MouseButton::Right => crate::logf("tray: right click (menu)"),
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
            crate::logf(format!("tray: menu {id}"));
            match id {
                "p2-open" => toggle(&shell, false),
                "p2-refresh" => shell.borrow().refresh.notify_waiters(),
                "p2-exit" => {
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
        // Hover prefetch (PLAN §5.3): fire both providers, throttled 10s.
        crate::logf("tray: hover prefetch (throttled 10s)");
        shell.borrow().refresh.notify_waiters();
    }
}

// ---------------------------------------------------------------------------
// startup per mode
// ---------------------------------------------------------------------------

fn startup_mode(shell: &Rc<RefCell<Shell>>, seconds: u64) {
    let mode = shell.borrow().mode;
    crate::logf(format!("mode: {mode:?}"));
    match mode {
        Mode::Interactive => crate::logf("ready: left-click the tray icon to open the panel"),
        Mode::Heights => {}
        Mode::ActiveTab => checks::start_active_tab(shell),
        Mode::TabAnim => checks::start_tab_anim(shell),
        Mode::Blur => checks::start_blur(shell),
        Mode::TrayClick => checks::start_tray_click(shell),
        Mode::Corner => checks::start_corner(shell),
        Mode::Tray => {
            checks::report_tray(shell);
            Timer::single_shot(Duration::from_secs(seconds.max(20)), || {
                let _ = slint::quit_event_loop();
            });
        }
        Mode::Hold => checks::start_hold(shell, seconds),
        Mode::Clamp => checks::start_clamp(shell),
    }
}

/// Sample the real window rect on a 5ms timer (the drift measurements read the
/// window, not the UI's idea of it).
pub fn spawn_sampler(shell: &Rc<RefCell<Shell>>) {
    let weak = Rc::downgrade(shell);
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(5), move || {
        let Some(shell) = weak.upgrade() else { return };
        let mut s = shell.borrow_mut();
        let Some(hwnd) = s.hwnd else { return };
        let (left, top, right, bottom) = window_rect(hwnd);
        s.checks.samples.push(Sample {
            left,
            top,
            right,
            bottom,
            visible: is_visible(hwnd),
        });
    });
    shell.borrow_mut().checks.timers.push(Rc::new(timer));
}

pub fn save_png(
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

pub fn snapshot(shell: &Rc<RefCell<Shell>>, path: &std::path::Path) {
    let s = shell.borrow();
    match s.ui.window().take_snapshot() {
        Ok(buffer) => match save_png(&buffer, path) {
            Ok(()) => crate::logf(format!("snapshot: {}", path.display())),
            Err(e) => crate::logf(format!("snapshot failed: {e}")),
        },
        Err(e) => crate::logf(format!("snapshot failed: {e}")),
    }
}
