// Tray icon, 1:1 port of TrayManager (SPEC section 14):
// static icon, left-click toggles the panel, right-click opens the menu window
// at the cursor, hover (Enter/Move) prefetch-throttled at 10s, tooltip text only.

use crate::panel;
use crate::polling;
use crate::state::AppState;
use std::time::{Duration, Instant};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

const ICON_PNG: &[u8] = include_bytes!("../icons/tray.png");

/// Programmatic 48x48 copy of the tray mark, used only if the PNG above fails to
/// decode: the same two accent bars on the same 24-unit grid as
/// `demo/make_icon.py`, supersampled 4x so the corners stay clean.
fn fallback_icon() -> tauri::image::Image<'static> {
    const S: usize = 48;
    const SS: usize = 4;
    const GRID: f64 = 24.0;
    const RADIUS: f64 = 1.6;
    const ACCENT: [u8; 3] = [0x8c, 0x74, 0xb1];
    const BARS: [(f64, f64, f64, f64); 2] = [(3.5, 4.5, 10.5, 19.5), (12.5, 10.0, 19.5, 19.5)];

    /// Rounded-rectangle coverage: clamp into the inset rect, then measure.
    fn inside(x: f64, y: f64, bar: (f64, f64, f64, f64)) -> bool {
        let (x0, y0, x1, y1) = bar;
        let cx = x.clamp(x0 + RADIUS, x1 - RADIUS);
        let cy = y.clamp(y0 + RADIUS, y1 - RADIUS);
        let dx = x - cx;
        let dy = y - cy;
        dx * dx + dy * dy <= RADIUS * RADIUS
    }

    let mut px = vec![0u8; S * S * 4];
    for sy in 0..S {
        for sx in 0..S {
            let mut hits = 0u32;
            for oy in 0..SS {
                for ox in 0..SS {
                    let gx = (sx as f64 + (ox as f64 + 0.5) / SS as f64) * GRID / S as f64;
                    let gy = (sy as f64 + (oy as f64 + 0.5) / SS as f64) * GRID / S as f64;
                    if BARS.iter().any(|bar| inside(gx, gy, *bar)) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let i = (sy * S + sx) * 4;
                px[i] = ACCENT[0];
                px[i + 1] = ACCENT[1];
                px[i + 2] = ACCENT[2];
                px[i + 3] = (hits as f64 / (SS * SS) as f64 * 255.0).round() as u8;
            }
        }
    }
    tauri::image::Image::new_owned(px, S as u32, S as u32)
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let icon = tauri::image::Image::from_bytes(ICON_PNG).unwrap_or_else(|_| fallback_icon());
    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .tooltip(tooltip_text(app))
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                // MouseUp semantics (not click): more reliable for tray icons
                TrayIconEvent::Click {
                    button,
                    button_state: MouseButtonState::Up,
                    position,
                    ..
                } => match button {
                    MouseButton::Left => panel::toggle_panel(app, true),
                    MouseButton::Right => panel::show_menu(app, position.x, position.y),
                    _ => {}
                },
                // Hover-to-refresh, throttled at 10s (SPEC 14)
                TrayIconEvent::Enter { .. } | TrayIconEvent::Move { .. } => on_hover(app),
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

fn on_hover(app: &AppHandle) {
    let st = app.state::<AppState>();
    {
        let mut last = st.last_hover.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_secs(10) {
                return;
            }
        }
        *last = Some(Instant::now());
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        polling::safe_refresh(&app2).await;
    });
    // Hover prefetch covers both providers under the same 10s throttle
    // (PLAN §5.3), so the pane is warm by the time the user opens it.
    let app3 = app.clone();
    tauri::async_runtime::spawn(async move {
        polling::safe_refresh_deepseek(&app3).await;
    });
}

/// Tooltip: product name plus one provider line each (PLAN §5.3). The text is
/// built in `quota_core::format` so the Slint shell's tray shows the identical
/// string; each line carries its own failure suffix, so a dead Kimi fetch never
/// hides a healthy DeepSeek balance.
pub fn update_tooltip(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("tray") {
        let _ = tray.set_tooltip(Some(tooltip_text(app)));
    }
}

fn tooltip_text(app: &AppHandle) -> String {
    let st = app.state::<AppState>();
    let kimi = st.last_quota.read().unwrap().clone();
    let deepseek = st.last_deepseek.read().unwrap().clone();
    quota_core::format::tray_tooltip(kimi.as_ref(), deepseek.as_ref())
}
