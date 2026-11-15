// Tray menu window (SPEC 14 / 10.3): shown at the cursor by a right-click on
// the tray icon, hidden when it loses focus or after a row is clicked.
//
// The placement is the shipped Tauri one, ported verbatim (`panel::show_menu`):
// the legacy 150px card / 5px-margin geometry is un-done with `legacy_*` so the
// visible card lands where it always has — 5px right of the cursor and 13px
// below it (clamped so its right edge stops 13px inside the work area), or
// flipped up near the bottom edge with its bottom 13px above the cursor.
// Keeping the formula, not "cleaning it up", is what makes the two shells place
// the menu on the same pixel.

use crate::shell::{self, Shell};
use slint::{ComponentHandle, LogicalSize, PhysicalPosition};
use std::cell::RefCell;
use std::rc::Rc;

/// Window width: 140px card + the 24px shadow room `menu.css` uses.
pub const MENU_W: f32 = 188.0;
/// Window height for the five-row card: 172 + 2 x 24.
pub const MENU_H: f32 = 220.0;
/// `(window_left, window_top, flipped)` in logical px for a cursor at
/// `(cx, cy)` inside a work area whose bottom-right corner is `(wa_right,
/// wa_bottom)` — all logical.
pub fn place(cx: f64, cy: f64, wa_right: f64, wa_bottom: f64) -> (f64, f64, bool) {
    // panel.rs: the card used to be 150 wide at a 5px margin and the height
    // report grew by 2 x 19px when the shadow room became 24px.
    let legacy_w = MENU_W as f64 - 38.0; // 150
    let legacy_h = MENU_H as f64 - 38.0; // 182
    let left = cx.min(wa_right - legacy_w - 8.0) - 19.0;
    let flip = cy + legacy_h + 24.0 > wa_bottom;
    let top = if flip { cy - legacy_h - 8.0 } else { cy + 8.0 } - 19.0;
    (left, top, flip)
}

/// Show the menu at a PHYSICAL cursor position (the tray event's own units).
pub fn show_at(shell: &Rc<RefCell<Shell>>, px: f64, py: f64) {
    let scale = shell::monitor_scale_at(px, py);
    let (_, _, wa_right, wa_bottom) = shell::work_area_phys();
    let (left, top, flip) = place(px / scale, py / scale, wa_right / scale, wa_bottom / scale);
    {
        let s = shell.borrow();
        s.menu_ui
            .window()
            .set_size(LogicalSize::new(MENU_W, MENU_H));
        s.menu_ui.window().set_position(PhysicalPosition::new(
            (left * scale).round() as i32,
            (top * scale).round() as i32,
        ));
    }
    crate::logf(format!(
        "menu: show at cursor=({px:.0},{py:.0}) scale={scale:.2} pos=({:.1},{:.1}) flip={flip}",
        left * scale,
        top * scale
    ));
    show(shell);
}

pub fn show(shell: &Rc<RefCell<Shell>>) {
    let hook = {
        let mut s = shell.borrow_mut();
        if s.menu_open {
            return;
        }
        s.menu_open = true;
        let unhooked = !s.menu_hooked;
        s.menu_hooked = true;
        unhooked
    };
    let shown = shell.borrow().menu_ui.show().is_ok();
    if !shown {
        crate::logf("menu: show failed");
        shell.borrow_mut().menu_open = false;
        return;
    }
    let hwnd = shell::hwnd_of(shell.borrow().menu_ui.window());
    if let Some(h) = hwnd {
        shell::no_round(h);
        shell::focus(h);
    }
    if hook {
        // Register once: Slint keeps one winit-event handler per window.
        use slint::winit_030::WinitWindowAccessor;
        let weak = Rc::downgrade(shell);
        shell
            .borrow()
            .menu_ui
            .window()
            .on_winit_window_event(move |_w, event| {
                if let slint::winit_030::winit::event::WindowEvent::Focused(false) = event {
                    if let Some(shell) = weak.upgrade() {
                        hide(&shell);
                    }
                }
                slint::winit_030::EventResult::Propagate
            });
    }
    crate::logf(format!("menu: shown hwnd={hwnd:?}"));
    if hwnd.is_none() {
        let weak = Rc::downgrade(shell);
        slint::Timer::single_shot(std::time::Duration::from_millis(120), move || {
            let Some(shell) = weak.upgrade() else { return };
            let (open, hwnd) = {
                let s = shell.borrow();
                (s.menu_open, shell::hwnd_of(s.menu_ui.window()))
            };
            if let (true, Some(h)) = (open, hwnd) {
                shell::no_round(h);
                shell::focus(h);
            }
        });
    }
}

pub fn hide(shell: &Rc<RefCell<Shell>>) {
    let was_open = {
        let mut s = shell.borrow_mut();
        let was_open = s.menu_open;
        s.menu_open = false;
        let _ = s.menu_ui.hide();
        was_open
    };
    if was_open {
        crate::logf("menu: hide");
    }
}

/// A row was clicked: the menu closes FIRST, then the action runs (SPEC 14), so
/// an action that opens another window never leaves the menu on top.
pub fn on_action(shell: &Rc<RefCell<Shell>>, name: &str) {
    crate::logf(format!("menu: action {name}"));
    hide(shell);
    match name {
        "open" => shell::toggle(shell, false),
        "refresh" => shell.borrow().workers.refresh.notify_waiters(),
        "settings" => crate::win::settings::open(shell),
        "skills" => crate::win::skills::open(shell),
        "quit" => {
            let _ = slint::quit_event_loop();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1440p at 225%: work area 2560x1400 physical, 1137.8x622.2 logical.
    const WA_RIGHT: f64 = 1137.8;
    const WA_BOTTOM: f64 = 622.2;

    /// The visible card's top-left corner: the window origin plus the 24px
    /// margin `menu.css` reserves for the shadow.
    fn card_top_left(left: f64, top: f64) -> (f64, f64) {
        // `menu.css` #root margin (24px) — the card's inset in the window.
        (left + 24.0, top + 24.0)
    }

    #[test]
    fn menu_opens_just_inside_the_bottom_right_corner() {
        // A tray icon at the very corner: the menu must clamp inside the work
        // area and flip up.
        let (left, top, flip) = place(WA_RIGHT - 8.0, WA_BOTTOM - 8.0, WA_RIGHT, WA_BOTTOM);
        assert!(flip, "a corner click must flip the menu up");
        let (card_x, card_y) = card_top_left(left, top);
        assert!(
            card_x + 140.0 <= WA_RIGHT,
            "card right {} past {WA_RIGHT}",
            card_x + 140.0
        );
        assert!(card_y >= 0.0, "card top {card_y} above the work area");
        assert!(
            card_y + 172.0 <= WA_BOTTOM,
            "card bottom {} past {WA_BOTTOM}",
            card_y + 172.0
        );
    }

    #[test]
    fn menu_offsets_match_the_shipped_geometry() {
        // Away from the edges the card sits 5px right of and 13px below the
        // cursor; flipping up puts its bottom 13px above the cursor.
        let (left, top, flip) = place(200.0, 200.0, WA_RIGHT, WA_BOTTOM);
        assert!(!flip);
        let (card_x, card_y) = card_top_left(left, top);
        assert!((card_x - (200.0 + 5.0)).abs() < 0.01, "card x {card_x}");
        assert!((card_y - (200.0 + 13.0)).abs() < 0.01, "card y {card_y}");

        let (left, top, flip) = place(200.0, WA_BOTTOM - 20.0, WA_RIGHT, WA_BOTTOM);
        assert!(flip);
        let (_, card_y) = card_top_left(left, top);
        assert!(
            (card_y + 172.0 - (WA_BOTTOM - 20.0 - 13.0)).abs() < 0.01,
            "flipped card bottom {}",
            card_y + 172.0
        );
    }
}
