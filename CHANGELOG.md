# Changelog

All notable changes to Quota Tray are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each section carries
the English notes first, then the Chinese mirror.

## [0.1.0] — Unreleased

### Added

- Phase 1 baseline: the `kimi-planbar-tray` rust edition (Tauri 2 + vanilla
  TS/HTML/CSS) is imported at the repository root as the starting point for
  Quota Tray.
- `docs/SPEC.md` / `docs/SPEC_EN.md`: the inherited behaviour specification,
  with a scope table marking which chapters still govern the merged app.

### Changed

- All four windows now share the frozen D design language: warm paper cards,
  hairline rows, uppercase micro-labels, 30/600 tabular numerals, one blue for
  usage and one red for failures. The settings and skills windows drop their
  old flat WPF skin, and the tray menu becomes a plain ghost list.
- The main panel and the settings window are now content-sized. The frontend
  reports `panel_height` / `settings_height`; the backend resizes with
  `LogicalSize`, re-pins the panel's bottom-right corner `PhysicalPosition`
  and re-centres the settings sheet.
- The settings sheet swaps the radio buttons and pills for the shared
  segmented control, and its window height follows the content (404x347 today
  instead of a fixed 464).
- Product identity: `Quota Tray` / `com.shawnqi.quotatray` / `quota-tray.exe`,
  version reset to 0.1.0.
- Single-instance mutex renamed to `QuotaTray.SingleInstance`, so the app can
  coexist with the two parent apps during the migration.
- Config directory `%APPDATA%\QuotaTray` (portable mode unchanged: an empty
  `portable.dat` next to the exe); autostart value name `QuotaTray`; the
  diagnostic override variable is now `QUOTA_TRAY_APP_DATA_DIR`.
- Tray tooltip, window titles, fallback quit menu and the GitHub `User-Agent`
  now say Quota Tray.

### 中文

- **新增**：Phase 1 基线导入——`kimi-planbar-tray` 的 rust 版（Tauri 2 +
  vanilla TS/HTML/CSS）落到仓库根目录；`docs/SPEC.md` 与 `docs/SPEC_EN.md`
  作为继承行为规格一并拷入，并在首页标注各章在合并版的效力。
- **变更**：四个窗口统一到定稿 D 版视觉——暖纸底、发丝线分行、大写微标签、
  30/600 等宽数字、单一蓝表示用量、单一红表示故障；设置窗与技能窗不再是旧的
  扁平皮肤，托盘菜单改为 ghost 列表。
- **变更**：主面板与设置窗改为内容自适应高度（`panel_height` /
  `settings_height`）——主面板右下角钉在工作区角 12px，设置窗改高后重新居中；
  设置窗用分段控件替代 radio / pill，当前为 404×347。
- **变更**：产品标识改为 `Quota Tray` / `com.shawnqi.quotatray` /
  `quota-tray.exe`，版本归零到 0.1.0；单实例互斥锁改名
  `QuotaTray.SingleInstance`（迁移期可与两个旧版并存）；配置目录
  `%APPDATA%\QuotaTray`，自启值名 `QuotaTray`，诊断覆盖变量改为
  `QUOTA_TRAY_APP_DATA_DIR`；托盘 tooltip、窗口标题、兜底退出菜单与
  GitHub `User-Agent` 统一为 Quota Tray。
