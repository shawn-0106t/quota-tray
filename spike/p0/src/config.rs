// Where the spike keeps its state. Deliberately the same chain as the shipped
// app (settings::config_dir): P0_CONFIG_DIR -> portable.dat next to the exe ->
// %APPDATA%\QuotaTray. Tests pass P0_CONFIG_DIR so a probe never touches the
// real credential file.

use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("P0_CONFIG_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = &exe_dir {
        if dir.join("portable.dat").exists() {
            return dir.clone();
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("QuotaTray");
        }
    }
    exe_dir.unwrap_or_else(|| PathBuf::from("."))
}
