// The three P3 windows: settings sheet, skills list and tray menu.
//
// Each module owns one window's behaviour and its data -> property mapping;
// `shell.rs` owns the windows themselves (they live in `Shell` so the panel and
// the tray can reach them) and the shared win32 helpers (work area, focus,
// blur hooks).

pub mod menu;
pub mod settings;
pub mod skills;
