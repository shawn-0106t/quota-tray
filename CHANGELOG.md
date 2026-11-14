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
- Phase 3 DeepSeek backend, ported from `deepseek-planbar`'s WPF service:
  balance on `api.deepseek.com`, console usage on `platform.deepseek.com`,
  the `data.biz_data` object/array tolerance, `REQUEST` excluded from tokens
  and money, today-only `days[]` aggregation, all-zero placeholder filtering
  and the top-4-by-month-cost selection. Nothing renders yet — the data stops
  at the `get_deepseek` command and the `deepseek-updated` event.
- DeepSeek credentials in `config_dir()/deepseek-credentials.json`: key names
  readable, values DPAPI-sealed behind a `dpapi:` prefix. Resolution is
  `DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` → that file → a one-time import of
  the parent app's plaintext `~/.deepseek-planbar/credentials.json` (read-only;
  the parent keeps working). A decrypt failure means "not configured".
- DeepSeek display names follow the vendor: the official `name` from
  `GET api.deepseek.com/models` wins over the built-in table, so a rename
  reaches the panel within the 24 h cache TTL without a new release. Aggregate
  console rows (`A & B`) and non-`deepseek-*` ids are shown verbatim.
- Usage endpoint candidate chain (`/api/v0/usage` → `/api/v1/usage`): a 404/410
  walks to the next candidate and the answer is remembered.
- `--test-deepseek [--verbose]` self-check, and 13 new Rust tests covering the
  DeepSeek parser (four fixtures ported from the parent repo) plus the
  credential prefix handling and a DPAPI round trip.
- A separate poller for DeepSeek with its own schedule and last-good slot, so
  one provider's failure never blanks or retimes the other. Refresh and hover
  prefetch now cover both providers.

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
- The failure hint reads `Fetch failed` (panel header) and ` (fetch failed)`
  (tray tooltip) instead of `Update failed`, which read like the CLI version
  check had failed — that is a separate path.
- Closing the settings window now collapses the panel when it lost focus:
  the suppressed blur never arrives on its own, so the panel used to linger
  until the next click elsewhere.

### 中文

- **新增**：Phase 1 基线导入——`kimi-planbar-tray` 的 rust 版（Tauri 2 +
  vanilla TS/HTML/CSS）落到仓库根目录；`docs/SPEC.md` 与 `docs/SPEC_EN.md`
  作为继承行为规格一并拷入，并在首页标注各章在合并版的效力。
- **新增**：Phase 3 DeepSeek 后端（移植自 `deepseek-planbar` 的 WPF 服务）——
  `api.deepseek.com` 余额、`platform.deepseek.com` 控制台用量、`biz_data`
  对象/数组两种形态、`REQUEST` 既不计 token 也不计金额、`days[]` 只取本地今天、
  过滤全零占位模型、按本月消费取前 4。尚未接 UI：数据停在 `get_deepseek`
  命令与 `deepseek-updated` 事件。
- **新增**：DeepSeek 凭证存放在 `config_dir()/deepseek-credentials.json`，
  键名明文、值以 `dpapi:` 前缀经 DPAPI 加密；读取顺序为环境变量
  `DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` → 该文件 → 一次性导入旧版明文的
  `~/.deepseek-planbar/credentials.json`（只读，旧版照常可用）；解密失败按
  「未配置」处理。
- **新增**：展示名以厂商上游为准——`GET api.deepseek.com/models` 的官方
  `name` 压过内置表，改名后最长 24 小时（缓存 TTL）即生效，无需发版；
  控制台的组合行（`A & B`）与非 `deepseek-*` id 原样显示。
- **新增**：用量端点候选链（`/api/v0/usage` → `/api/v1/usage`），遇 404/410
  换下一个并记住可用的那个。
- **新增**：`--test-deepseek [--verbose]` 自检；13 个 Rust 测试覆盖 DeepSeek
  解析（4 份 fixtures 从父仓库移植）、凭证前缀处理与 DPAPI 往返。
- **新增**：DeepSeek 独立 poller，自带调度与 last-good 槽位——任一 provider
  失败都不会清空或改写另一个；Refresh 与托盘悬停预取现在覆盖两个 provider。
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
- **变更**：失败文案改为 `Fetch failed`（面板 header）与 ` (fetch failed)`
  （托盘 tooltip），原来的 `Update failed` 读起来像 CLI 版本检查失败，
  而那是另一条独立的路。
- **变更**：关掉设置窗时若面板已失去焦点就顺手收起——被抑制的那次 blur 不会
  自己补来，之前面板要等到下一次点击别处才收。
