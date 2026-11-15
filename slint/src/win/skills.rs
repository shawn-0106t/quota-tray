// Skills window (SPEC 21): a fixed 404x520 sheet over a lazy scan of
// `~/.kimi-code/skills`, `~/.agents/skills` and the managed plugin roots.
//
// The list is a flat model of group titles and rows because Slint cannot run a
// prefix sum: the row tops are computed here from the same constants the Slint
// rows are painted with (measured from the shipped DOM, spike/p3/REPORT.md).

use crate::app::Msg;
use crate::shell::{self, Shell};
use crate::{SkillRow, SkillsWindow};
use quota_core::skills::SkillInfo;
use slint::{ComponentHandle, LogicalSize, ModelRc, PhysicalPosition, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

pub const SKILLS_W: f32 = 404.0;
pub const SKILLS_H: f32 = 520.0;

/// `.group-title`: a 13.5px line, 2px under it, 14px above the next group.
const GROUP_H: f32 = 13.5;
const GROUP_GAP: f32 = 2.0;
const GROUP_MARGIN: f32 = 14.0;
/// `.group-title:first-child { margin-top: 2px }`.
const LIST_TOP: f32 = 2.0;
/// `.skill`: 1px hairline + 9 + 16.5 name line + 4 + two description lines
/// (2 x 15.22) + 9. The description is a two-line clamp in the product, so a
/// row reserves two lines whether the text needs one or two (SkillsWindow's
/// header explains why).
const ROW_H: f32 = 69.94;

pub fn open(shell: &Rc<RefCell<Shell>>) {
    if shell.borrow().skills_open {
        return;
    }
    {
        let mut s = shell.borrow_mut();
        s.skills_open = true;
        s.skills_ui
            .window()
            .set_size(LogicalSize::new(SKILLS_W, SKILLS_H));
        centre(&s.skills_ui);
        if s.skills_cache.is_none() {
            s.skills_ui.set_summary("LOADING…".into());
        }
    }
    if shell.borrow().skills_cache.is_none() {
        request_scan(shell, false);
    }
    let shown = shell.borrow().skills_ui.show().is_ok();
    if !shown {
        crate::logf("skills: show failed");
        shell.borrow_mut().skills_open = false;
        return;
    }
    let hwnd = shell::hwnd_of(shell.borrow().skills_ui.window());
    if let Some(h) = hwnd {
        shell::no_round(h);
        shell::focus(h);
    }
    crate::logf(format!("skills: open hwnd={hwnd:?}"));
    if hwnd.is_none() {
        let weak = Rc::downgrade(shell);
        slint::Timer::single_shot(std::time::Duration::from_millis(120), move || {
            let Some(shell) = weak.upgrade() else { return };
            let (open, hwnd) = {
                let s = shell.borrow();
                (s.skills_open, shell::hwnd_of(s.skills_ui.window()))
            };
            if let (true, Some(h)) = (open, hwnd) {
                shell::no_round(h);
                shell::focus(h);
            }
        });
    }
}

fn centre(ui: &SkillsWindow) {
    let scale = ui.window().scale_factor() as f64;
    let (wa_left, wa_top, wa_right, wa_bottom) = shell::work_area_phys();
    let x = wa_left + ((wa_right - wa_left) - SKILLS_W as f64 * scale) / 2.0;
    let y = wa_top + ((wa_bottom - wa_top) - SKILLS_H as f64 * scale) / 2.0;
    ui.window()
        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

pub fn close(shell: &Rc<RefCell<Shell>>) {
    {
        let mut s = shell.borrow_mut();
        if !s.skills_open {
            return;
        }
        s.skills_open = false;
        let _ = s.skills_ui.hide();
    }
    crate::logf("skills: hide");
    // Same focus-loop repair as the settings sheet: the panel's blur was
    // suppressed while this window was open.
    let weak = Rc::downgrade(shell);
    slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
        let Some(shell) = weak.upgrade() else { return };
        let (visible, panel_hwnd, other_open) = {
            let s = shell.borrow();
            (
                s.visible,
                s.hwnd,
                s.settings_open || s.skills_open || s.menu_open,
            )
        };
        if other_open || !visible {
            return;
        }
        let Some(hwnd) = panel_hwnd else { return };
        if !shell::is_foreground(hwnd) {
            shell::start_hide(&shell);
        }
    });
}

/// Scan in the background; `refresh` forces a rescan (SPEC 21.2). The result
/// arrives as `Msg::Skills` so only the UI thread touches the model.
pub fn request_scan(shell: &Rc<RefCell<Shell>>, refresh: bool) {
    let (busy, tx) = {
        let s = shell.borrow();
        (s.skills_scanning, s.workers.sender())
    };
    if busy {
        crate::logf("skills: scan already running");
        return;
    }
    shell.borrow_mut().skills_scanning = true;
    crate::logf(format!("skills: scan start refresh={refresh}"));
    shell.borrow().workers.spawn(async move {
        let list = tokio::task::spawn_blocking(quota_core::skills::scan)
            .await
            .unwrap_or_default();
        let _ = tx.send(Msg::Skills(list));
    });
}

pub fn on_scanned(shell: &Rc<RefCell<Shell>>, list: Vec<SkillInfo>) {
    {
        let mut s = shell.borrow_mut();
        s.skills_scanning = false;
        s.skills_cache = Some(list);
        s.skills_ui.set_scroll_y(0.0);
    }
    render(shell);
}

/// Push the cached list into the window (also called on open, so a reopened
/// window never rescans — SPEC 21.2).
pub fn render(shell: &Rc<RefCell<Shell>>) {
    let (list, open) = {
        let s = shell.borrow();
        let Some(list) = s.skills_cache.clone() else {
            return;
        };
        (list, s.skills_open)
    };
    let (rows, content_h) = build_rows(&list);
    let summary = if list.is_empty() {
        "NO SKILLS FOUND".to_string()
    } else {
        format!("{} SKILLS", list.len())
    };
    {
        let s = shell.borrow();
        s.skills_ui.set_count(list.len() as i32);
        s.skills_ui.set_summary(summary.into());
        s.skills_ui.set_items(ModelRc::new(VecModel::from(rows)));
        s.skills_ui.set_content_h(content_h);
    }
    crate::logf(format!(
        "skills: rendered rows={} content-h={content_h:.1} open={open}",
        list.len()
    ));
}

/// Group titles and skill rows in one model, each with its own top offset.
pub fn build_rows(list: &[SkillInfo]) -> (Vec<SkillRow>, f32) {
    let mut out = Vec::new();
    let mut y = LIST_TOP;
    let mut last: Option<&str> = None;
    for skill in list {
        if last != Some(skill.source.as_str()) {
            if last.is_some() {
                y += GROUP_MARGIN;
            }
            out.push(SkillRow {
                kind: 0,
                // `.group-title` is uppercased by CSS; Slint has no
                // text-transform, so the model carries the rendered case.
                name: skill.source.to_uppercase().into(),
                desc: Default::default(),
                y,
                h: GROUP_H,
            });
            y += GROUP_H + GROUP_GAP;
            last = Some(skill.source.as_str());
        }
        out.push(SkillRow {
            kind: 1,
            name: skill.name.clone().into(),
            desc: skill.description.clone().into(),
            y,
            h: ROW_H,
        });
        y += ROW_H;
    }
    (out, y)
}
