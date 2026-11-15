// Window placement math (PLAN §4 / §7): the panel is content-sized, bottom-right
// anchored to the work-area corner with a 12px card gap, and clamped to the work
// area's height.
//
// Sizes stay LOGICAL (the window manager re-scales physical size from a logical
// size on a DPI change), positions must be PHYSICAL. The two live here as pure
// functions so the 100/150/200/225% corner cases are unit-tested instead of only
// being observed on whatever DPI a machine happens to run at.

/// Panel width in logical px, shadow room included.
pub const PANEL_W: f32 = 424.0;
/// Transparent shadow fade room per side (matches panel.slint / tauri.conf.json).
pub const MARGIN: f32 = 28.0;
/// Card edge to work-area corner.
pub const GAP: f32 = 12.0;
/// Shadow room on both sides (window height = card + this).
pub const SHADOW_ROOM: f32 = 2.0 * MARGIN;
/// Smallest window the shell will ever apply (an empty pane still shows chrome).
pub const MIN_WINDOW_H: f32 = 80.0;

/// Physical top-left for a window of `window_h` logical px whose card's visible
/// bottom-right corner must sit `GAP` inside the work-area corner.
///
/// The window's own corner is `MARGIN - GAP` outside the card, so the physical
/// window rect ends `MARGIN - GAP` past the work area (accepted, PLAN §4: that
/// strip is transparent but hit-tested).
pub fn window_pos(window_h: f32, wa_right: f64, wa_bottom: f64, scale: f64) -> (i32, i32) {
    let x = wa_right - f64::from(PANEL_W + GAP - MARGIN) * scale;
    let y = wa_bottom - f64::from(window_h + GAP - MARGIN) * scale;
    (x.round() as i32, y.round() as i32)
}

/// Tallest window that still fits the work area with a `GAP` margin on both
/// vertical sides — the content beyond this scrolls inside the card.
pub fn max_window_h(wa_top: f64, wa_bottom: f64, scale: f64) -> f32 {
    let logical = (wa_bottom - wa_top) / scale;
    (logical as f32 - 2.0 * GAP).max(MIN_WINDOW_H)
}

/// Clamp a content-derived window height to the work-area cap.
pub fn clamp_window(natural: f32, max_h: f32) -> f32 {
    natural.clamp(MIN_WINDOW_H, max_h.max(MIN_WINDOW_H))
}

/// The card's scroll cap in logical px, pushed into the UI (`max-card-h`).
pub fn max_card_h(max_window_h: f32) -> f32 {
    (max_window_h - SHADOW_ROOM).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALES: [f64; 8] = [1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 3.0];
    // Work areas seen in practice: 1080p, 1440p at 225%, 768p at 125%.
    const CORNERS: [(f64, f64); 3] = [(2560.0, 1400.0), (3840.0, 2160.0), (1366.0, 728.0)];

    /// The card's visible bottom-right corner, in physical px, derived from the
    /// window rect the way the renderer places it.
    fn card_corner(h: f32, wa_right: f64, wa_bottom: f64, scale: f64) -> (f64, f64) {
        let (x, y) = window_pos(h, wa_right, wa_bottom, scale);
        (
            x as f64 + f64::from(PANEL_W - MARGIN) * scale,
            y as f64 + f64::from(h - MARGIN) * scale,
        )
    }

    #[test]
    fn card_stays_one_gap_inside_the_work_area_at_every_scale() {
        for scale in SCALES {
            for (wa_right, wa_bottom) in CORNERS {
                for h in [331.5_f32, 504.0, 545.0, 601.0] {
                    let (x, y) = card_corner(h, wa_right, wa_bottom, scale);
                    let want_x = wa_right - GAP as f64 * scale;
                    let want_y = wa_bottom - GAP as f64 * scale;
                    assert!(
                        (x - want_x).abs() <= 0.5,
                        "scale {scale}: card right {x} vs {want_x}"
                    );
                    assert!(
                        (y - want_y).abs() <= 0.5,
                        "scale {scale} h {h}: card bottom {y} vs {want_y}"
                    );
                }
            }
        }
    }

    #[test]
    fn resizing_never_moves_the_visible_bottom_edge() {
        // The property the tab switch depends on: pre-sizing the window to the
        // taller tab must not move the card's bottom edge by even one pixel.
        for scale in SCALES {
            for (wa_right, wa_bottom) in CORNERS {
                let (_, y_short) = window_pos(504.0, wa_right, wa_bottom, scale);
                let (_, y_tall) = window_pos(601.0, wa_right, wa_bottom, scale);
                let bottom_short = y_short as f64 + f64::from(504.0 - MARGIN) * scale;
                let bottom_tall = y_tall as f64 + f64::from(601.0 - MARGIN) * scale;
                assert!(
                    (bottom_short - bottom_tall).abs() <= 1.0,
                    "scale {scale}: bottom edge moved {} px",
                    (bottom_short - bottom_tall).abs()
                );
            }
        }
    }

    #[test]
    fn window_height_is_clamped_to_the_work_area() {
        // 728 / 1.25 = 582.4 logical work area, minus the 12px gap on both sides.
        let max_h = max_window_h(0.0, 728.0, 1.25);
        assert!((max_h - 558.4).abs() < 0.1, "max_h {max_h}");
        assert_eq!(clamp_window(601.0, max_h), 558.4);
        assert_eq!(clamp_window(331.5, max_h), 331.5);
        // A work area shorter than the minimum still yields a usable window.
        let tiny = max_window_h(0.0, 80.0, 1.0);
        assert_eq!(tiny, MIN_WINDOW_H);
        assert_eq!(clamp_window(300.0, tiny), MIN_WINDOW_H);
    }

    #[test]
    fn card_cap_leaves_the_shadow_room_free() {
        assert_eq!(max_card_h(560.0), 504.0);
    }
}
