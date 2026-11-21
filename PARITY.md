# PARITY — Tauri 壳与 Slint 壳逐项对齐

> 快照：2026-11-21，分支 `spike/slint-demo`；Tauri 参考为 `origin/main@940e789`（2026-11-20）。
> 用途：P5 的对齐矩阵。**状态只允许 ✅ / ⚠️ / N/A，不允许 ❌**；每一项都给出可重跑的证据。
> 图例：✅ 行为一致且有证据 · ⚠️ 刻意差异或已接受的偏差（§11 给理由）· N/A 架构上不适用（§9）。

这份文档比较的是**两个壳**，不是两次发布：`src-tauri` 是本分支上仍可构建的现版壳（数据层已改为依赖
`quota-core`），`slint/` 是候选壳。真正的契约仍是 `PLAN.md` §3（视觉）、§5（数据）、§7（平台行为）；
两边冲突时先改 `PLAN.md`，再改代码与本文。

新增用户可见行为时，先加一行矩阵和一条可重跑的证据，再写代码。

## 1. 数据层与轮询

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| Kimi 配额：OAuth/API key 凭证链、`amountLeft` 1e-8 元、周窗口三来源、未开通两形状 | `src-tauri/src/quota.rs` 转调 `quota-core::quota` | 同一个 `quota-core::quota` | P1 掩码后逐字节一致；`cargo test`（quota 10 项） | ✅ |
| Kimi 本机用量：turn scope、求和后再相除、隔分钟只 stat、30 天索引 | `kimi_usage.rs` 驱动 + `quota-core` 扫描 | `app.rs` 驱动同一扫描，60 s / `tick % 2` 同节奏 | P1；P2 §4；`cargo test`（kimi_usage 6 项） | ✅ |
| DeepSeek 余额/用量/模型名/端点候选/24 h 缓存 | `quota-core::deepseek` | 同一个 | P1；`--test-deepseek --verbose` | ✅ |
| 双 poller 各自 last-good、首拉 2 s、失败 30 s、间隔 1–30 min | `polling.rs` | `app.rs` 两条 task + 广播 `Notify` | P2 §4；P3/P4 回归 | ✅ |
| CLI 版本与 Update 徽章 | 启动时查一次 | 启动 + 每 6 h 复查 | `app.rs`；Tauri `lib.rs::check_and_emit` | ⚠️ §11.2 |
| 系统主题跟随（`AppsUseLightTheme`，3 s） | `theme_watch.rs` | `app.rs` 3 s，四个窗口的 `Tokens` 各自切换 | P3 `--test-settings` | ✅ |
| 托盘 tooltip 三行（每 provider 自带 fetch failed） | `quota_core::format::tray_tooltip` | 同一个 | `spike/p1/fmt_parity.py` 51 例；P0 | ✅ |

## 2. 面板

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 内容尺寸：17 个数据变体的卡片/窗口高度 | CSS 内容驱动 + `panel_height` IPC | `Metrics` 常量求和，窗口由 Rust 设置 | `--test-heights` + `spike/p2/heights_ref.py --compare`，Δ=0.00 | ✅ |
| 右下角锚定、工作区夹取、卡片内滚动 | `panel.rs` `LogicalSize`/`PhysicalPosition` | `height.rs` 同公式 | `--test-corner`（225%，469 采样漂移 0 px）；`--test-clamp` | ✅ |
| tab 切换：预放大 → cell 200/167 ms + 双 pane 167 ms 交叉淡入、窗口结束缩回 | `main.ts` MOTION + Web Animation | `panel.slint` 四个组合 state，数值逐条照抄 | `--test-tab-anim`（30 连点漂移 0 px）；真实录屏（P0/P2） | ✅ |
| `ActiveTab` 持久化与非法值归一 | `set_active_tab` + `settings.rs` | `app.rs::save_tab` + 同一 `quota-core::settings` | `--test-active-tab` 5 项；settings 单测 | ✅ |
| 表头时间戳跟随屏幕上的 pane，跨天显示 `M/D HH:MM` | `renderStamp()` 在 pane 翻转处 | `view.rs::apply_stamps` + `panel.slint` 按 `tab-ds` 选 | P2 文本带；AGENTS 契约 | ✅ |
| Kimi pane：Weekly / 5-hour / Cache 7d·local / decode / Extra Usage / CLI 版本 + 徽章 | `index.html` + `main.ts` | `kimi.slint` + `view.rs` | 17 变体 Δ=0；8 组文本带 ≤1.00 CSS px（P2） | ✅ |
| DeepSeek pane：余额、明细、1–4 模型行、月汇总、hint、无 key / 失败态 | `index.html` + `main.ts` | `ds.slint` + `view.rs` | 同上 | ✅ |
| 阈值：cache <60% 红、余额 <¥10 或不可用红 | `common.ts` | `quota-core::format::{is_low_balance,is_low_cache_hit}` | 单测 `thresholds_match_the_frontend` | ✅ |
| 颜色语义：accent 只用于用量与选中，红只用于失败 | `theme.css` | `tokens.slint` | `scripts/parity/check_tokens.py`；`demo/check_contrast.py` 18/18 | ✅ |
| 面板显隐：进场 160/220 ms、退场 130/160 ms、隐藏预算 170 ms | `main.css` transition | `panel.slint` state 动画 | `--test-blur`（180 ms 实测）；`src/main.css` 与 `panel.slint` 数值对照 | ✅ |

## 3. 窗口与平台行为

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 四个常驻窗 + 一个屏幕外 keepalive | 4 个 WebviewWindow + 隐藏不销毁 | 4 个 Slint `Window` + 1×1 `Anchor`（-30000,-30000） | `--test-windows`（4/4）；P0 隐藏最后一个窗口会退出事件循环的结论 | ✅ |
| 置顶、不占任务栏、无边框、透明、Windows 11 不自动圆角 | `tauri.conf.json` + `no_round` | 全局 winit hook + 每窗 `no-frame` + `DwmSetWindowAttribute(DONOTROUND)` | P3 截图（透明、阴影、圆角）；代码对照 | ✅ |
| 窗口标题 | `Quota Tray` / `Quota Tray Settings` / `Kimi Skills` / `Quota Tray Menu` | 四个根 `Window` 同名 | 本轮的 `title:` 补充 | ✅ |
| 失焦自动隐藏 + 隐藏后 300 ms 内托盘点击忽略 | `panel::on_main_blur` + 300 ms guard | `on_blur` + `TRAY_GUARD` | `--test-blur`；`--test-tray-click` | ✅ |
| settings/skills 打开时抑制自动隐藏；关窗后 150 ms 复核前台 | 抑制 + `collapse_after_settings_close` | `on_blur` 抑制 + 两个窗各自的 150 ms 复核 | `--test-dialogs` | ✅ |
| 系统关闭（Alt+F4 / `WM_CLOSE`）隐藏而不是销毁，状态同步 | `CloseRequested` → `prevent_close` + `hide` | `on_close_requested` → 更新 `visible`/`open` 标志后 `HideWindow` | `--test-close`（面板/设置/skills/菜单四项 PASS） | ✅ |
| 标题栏拖动；重开保留用户拖动的位置 | `start_dragging`；设置按高度变化才重新居中 | `drag_window()`；`skills_placed` 只首次居中 | `--test-close` 断言 skills 拖动后重开位置不变 | ✅ |
| DPI：尺寸 `LogicalSize`、位置 `PhysicalPosition`，每次 show 按实时缩放/工作区重算 | `panel.rs` | `height.rs` + `Shell::apply_size` | `--test-corner`（225% 漂移 0 px）；8 档 scale 单测 | ✅ §12 仅 225% 现场 |
| 工作区与缩放取主显示器（两壳相同） | `SPI_GETWORKAREA` + `primary_monitor()` | `SPI_GETWORKAREA` + `scale_factor()` | 代码对照；P2 225% 实测 | ✅ |
| 托盘菜单在光标处，贴边翻转 | `panel::show_menu` legacy 公式 | `win/menu.rs::place` 逐字移植 | `--test-menu`（与公式差 ≤0.2 物理 px） | ✅ |

## 4. 托盘与菜单

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 托盘图标（生成物，48 px 嵌入）、三行 tooltip | `tray.rs` + `icons/tray.png` | `tray-icon` + 同一 PNG | P0 tooltip 字符串；`--test-tray` | ✅ |
| 左键开合面板；右键在光标处开菜单；hover 10 s 节流预取 | `tray.rs` | `shell.rs::{toggle,on_hover}` | `--test-tray-click`；P0 hover 事件到达 | ✅ |
| 五行菜单 Open/Refresh/Settings/Skills/Exit，先关菜单再执行 | `menu.html` + `menu_action` | `menu.slint` + `win/menu.rs::on_action` | `--test-menu` 逐行动作 | ✅ |
| 菜单失焦隐藏、行点击隐藏 | `on_window_event` | `menu` 的 `Focused(false)` + `hide()` | `--test-menu` blur-hide | ✅ |
| 退出 | `app.exit(0)` | `slint::quit_event_loop()` | 代码对照；P0 | ✅ |

## 5. 设置窗

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 分区：主题 / 刷新间隔 / 自启、Kimi 只读状态、DeepSeek 状态 + Sign in / Clear + 手工粘贴 | `settings.html` + `settings.ts` | `settings.slint` + `win/settings.rs` | P3 截图 vs DOM 文本带 ≤1.00 CSS px | ✅ |
| Save 顺序：落盘 → 自启 → 主题 → 重排期 → 关窗 | `save_settings` + 前端 | `save_from_ui()` 同序 | `--test-settings`（子进程重启读回 `theme=dark interval=10 autostart=true`） | ✅ |
| 打开时回填持久化值与凭证状态 | `settings-show` 事件 | `apply_controls()` | `--test-settings` / `--test-windows` | ✅ |
| 手工粘贴：先验证后保存、失败码转文案、按钮恢复 | `save_credential` + `clears` 闸门 | `save_credentials()` + 同一 `cred_epoch`/`clears` 语义 | `--test-settings`（真实 401、不落盘、Clear 作废在途） | ✅ |
| Clear：写空文件、env 值保留、杀登录子进程、epoch 递增 | `clear_credentials` | `settings::clear` → `login::cancel` → 空文件 | P3；`--test-signin` 第 2 轮 | ✅ |
| 内容尺寸 + 居中 + 工作区 90% 上限 + 仅 pane 滚动、标题栏固定 | `apply_settings_size` | `win/settings.rs::resize` | `--test-settings`（300px 上限默认开启，断言滚动时 `title-screen-y` 不变）；P3 文本带 | ✅ |

## 6. Skills 窗

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 三个根懒扫描、按 source/name 分组、行模型、Refresh 重扫、404×520 + 滚动 | `get_skills` + `skills.ts` | `quota_core::skills::scan` + `win/skills.rs` | `--test-skills`（124 技能 / 134 行 / 滚动位可查） | ✅ |
| 重开用缓存、不重扫；Refresh 才重扫 | `get_skills(false/true)` | `on_scanned` 缓存 + `request_scan(true)` | `--test-skills` | ✅ |
| 描述统一预留两行 | `-webkit-line-clamp: 2`，单行只占一行 | 固定两行 | P3：11/124 行多 15.22 px | ⚠️ §11.3 |
| 滚动条出现时内容区是否收窄 | Chromium 收窄 8 px | 4 px 自绘滚动条覆盖，内容不收窄 | P3 §7 | ⚠️ §11.4 |
| 扫描线程 panic 的空态文案 | `Failed to load skills` | `NO SKILLS FOUND` | 代码路径；仅 `spawn_blocking` join 失败时不同 | ⚠️ §11.5 |

## 7. 登录窗与凭证

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| Sign in… → 登录窗；设置窗隐藏，退出后还原 | 进程内临时窗口 | 子进程 `--login-host`（tao + wry） | `--test-signin` 第 1 轮（WM_CLOSE → exit 3 → 设置窗回来） | ✅ |
| 捕获通道：`location.href = quotatray://capture` + 取消导航，无 IPC 面 | 注入脚本 + `on_navigation` | 同一份注入与解析 | `--test-login`（`sender=ok`） | ✅ |
| 拦截控制台自己的 `fetch` | `initialization_script` 包装 | 同一包装 | `--test-login`（`fetch=ok`） | ✅ |
| 先验证后保存：HTTP 200 + 非零 `code` 也算拒绝；候选 base 继续走；瞬时失败重试一次 | `quota-core::deepseek::client` | 同一个 | `--test-login`（`reject=ok`，文件逐字节不变）；单测 | ✅ |
| Clear 跨进程：kill → wait → 写空文件，晚到候选不能回写 | 关窗 + 清文件 | `login::cancel` + epoch 注释 | `--test-signin` 第 2 轮（epoch 0→1，1.2 s 后无回写） | ✅ |
| DPAPI、env 优先、旧目录一次性导入、清空写空壳、状态只报布尔 | `quota-core` | 同一个 | `cargo test`（credentials / deepseek credentials） | ✅ |
| 登录窗 WebView2 profile | `EBWebView` | `EBWebView-login` | 两壳可并行；P4 §3 | ⚠️ §11.1 |
| 成功登录（exit 0）后的父进程 settle | 事件 + 刷新 | 轮询退出 + 还原 | exit 3 分支已覆盖；exit 0 需真人 | ⚠️ §12.4 |

## 8. 存储、实例与日志

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 自己的状态目录 | `%APPDATA%\QuotaTray` | `%APPDATA%\QuotaTray-Slint` | P2/P3 `settings.json` 读写；AGENTS 契约 | ⚠️ §11.1 |
| 共用账号目录：DeepSeek 凭证 / endpoints / models / `kimi-usage.json` | `shared_dir()` | 同一个 `quota-core::settings::shared_dir` | P1 迁移；P3/P4 实测 | ✅ |
| 单实例互斥量 | `QuotaTray.SingleInstance` | `QuotaTray.Slint.SingleInstance` | `main.rs`；两壳可并行 | ⚠️ §11.1 |
| 自启值 | `QuotaTray` | `QuotaTray.Slint` | `settings::apply_auto_start`（quota-core） | ⚠️ §11.1 |
| 便携模式：exe 旁 `portable.dat` → 配置写 exe 目录；shared 永不共享 | `settings.rs` | 同一 `quota-core::settings` | Tauri 便携实测（STATUS）；代码共用 | ✅ |
| 窗口程序无控制台，panic 写日志 | `webview.log` | `slint.log` | `install_panic_log()` 两边都有；P4 日志 | ⚠️ §11.6 |
| 所有 IO/注册表/进程/HTTP 失败静默，只走界面文字，不弹框 | 全部壳层 | 全部壳层 | 代码对照；P3/P4 失败路径 | ✅ |

## 9. 恢复与错误

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| WebView2 `ProcessFailed` 就地重建、1/2/5 s 退避、30 s 3 次封顶、`prevent_exit` | `recovery.rs` | 无 WebView2，不适用 | P2 §7；代码结构 | N/A |
| 登录子进程的父进程存活兜底 | 不适用（进程内） | `--parent-pid` 检查；父进程退出即关闭 | P4：`--parent-pid 999999` → `parent exited, closing`，exit 3 | ✅ |
| 登录子进程崩溃/被卡住 | 关窗即止 | 退出轮询 `settle`；`WM_CLOSE` 不应答时 `kill_child` | `--test-signin` 的超时分支；P4 §4.2 | ✅ |

## 10. 门禁、工具与发布

| 行为 | Tauri（现版） | Slint | 证据 | 状态 |
|---|---|---|---|---|
| 格式化字符串（金额/百分比/tokens/倒计时/时间戳/阈值） | `src/common.ts` | `quota-core::format` | `spike/p1/fmt_parity.py` 51 例逐串相同 | ✅ |
| 设计 token 一致性 | `src/theme.css` | `slint/ui/tokens.slint` 复刻 | `scripts/parity/check_tokens.py`（15 token + 10 处 shadow） | ✅ |
| 截图与设计评审产物 | `demo/ui-demo.html` / README shots | 同一生成器 + `--shots` | 本轮重生成，与 `main` 逐字节相同 | ✅ |
| 诊断 CLI | `--test-fetch/--test-deepseek/--test-usage/--test-update/--test-ui/--test-login` | `--test-heights/-corner/-tab-anim/-blur/-tray-click/-tray/-clamp/-active-tab/-windows/-settings/-menu/-skills/-dialogs/-close/-signin/-login` | 见 §13；两套自检不重叠 | ⚠️ §11.7 |
| 发布线 | 五载体版本门禁 + 便携 exe | 尚未进发布线（本分支 `1.0.1`，crate `0.1.0`） | `check_versions.py`；`release.yml` | ⚠️ §11.8 |

## 11. 刻意差异（每条都有理由）

1. **两个应用，一个账号。** Slint 线用 `QuotaTray-Slint` 自己的状态目录、`QuotaTray.Slint.SingleInstance` 互斥量、`QuotaTray.Slint` 自启值与 `EBWebView-login` 浏览器 profile；账号级文件（DeepSeek 凭证、endpoints、模型名缓存、`kimi-usage.json`）走 `%APPDATA%\QuotaTray\shared`。理由是 P5 期间两壳要能同时运行、互不抢单实例/自启/profile。Slint 成为发布产物时，互斥量与自启值改回规范名，shared 目录不动。
2. **Update 复查频率。** 现版每次启动查一次；新壳每 6 h 再查。后者是严格的超集（长驻实例不会永远显示旧版本），不改产品文案与徽章逻辑。
3. **skills 单行描述预留两行。** Slint 量不到换行后的文本高度，统一按两行行高排。124 行里 11 行多 15.22 px；其余 113 行与 DOM 完全一致。P5 若要严格对齐，需要引入文本测量或改 `quota-core::skills` 的折叠 YAML 解析（见 §14）。
4. **skills 滚动条不收窄内容。** Slint 自绘 4 px 覆盖式滚动条；Chromium 的原生滚动条会让内容区少 8 px。只有「滚动时右侧内容是否左移 8 px」的差别，不影响文本带与行高。
5. **skills 扫描 panic 的空态。** `spawn_blocking` join 失败时现版显示 `Failed to load skills`，新壳显示 `NO SKILLS FOUND`。扫描本身从不返回错误；这是线程崩溃时的文案差别，不是状态错误。
6. **日志文件名。** `webview.log`（Tauri）与 `slint.log`（Slint）都是「永不写 token」的诊断日志；名字跟着各自的运行时。
7. **诊断 CLI 不重叠。** 现版的数据探针（`--test-fetch` 等）仍由本分支的 Tauri exe 提供，`quota-core` 是同一份；新壳的自检集中在窗口/动效/DPI/登录。两壳并存期间两种 exe 都能构建，探针不会丢；若 v2.0.0 只留 Slint exe，需要把数据探针搬过去或保留一个诊断构建（见 §12 的发布决定）。
8. **发布线。** 新壳还没进五载体门禁，分支仍是 `1.0.1`；这是 P5 的范围边界，不是对齐缺口。是否以 Slint 为 v2.0.0 主线是待拍板项（`spike/HANDOFF.md` §9）。

## 12. 验证缺口（不是产品差异）

这些是「还没在真机上证明」，不是「行为不同」；写在这里防止把 ⚠️ 和未验证混为一谈。

1. **100/150/200% 真机 DPI**：本机固定 225%，只做了 8 档 scale 的算术单测；改缩放会动用户的桌面布局，用户已选择「现场 225% + 人工清单」。
2. **混合 DPI / 多屏**：菜单用光标所在显示器的 DPI，工作区用主显示器；只有单屏 225% 现场。
3. **键盘与 IME**：两个壳都没有键盘导航/焦点环（产品本身没有）；IME 未测。
4. **成功登录（exit 0）的父进程 settle**：需要真人签出一个真 token；退出码 3 的分支（用户关窗）已覆盖，两者只差一句文案。
5. **真实网络失败文案**：`rate-limited` / `server-error` / `network-error` 只有单测级的映射把握，没有在真机上逐个打出来。
6. **父进程退出时登录子进程仍卡在 WebView2 初始化**：`--parent-pid` 检查在子进程事件循环里，冷 profile 可等 175 s 才关；期间 profile 锁会让下一次 Sign in 失败一次。这是已知时序，不是正确性缺陷，但没有现场录像。
7. **托盘 tooltip 的悬停视觉**：P0 期间本会话无法合成鼠标输入，事件链路用投递消息验证；悬停外观仍需人工看一眼。

## 13. 复现

```bash
# 构建（每次 cargo 调用都要带 Skia 镜像）
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'
(cd slint && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked)

EXE=./slint/target/debug/quota-tray-slint.exe
ABS="$(pwd -W)/.shots/parity"

# 面板
$EXE --test-heights
PYTHONUTF8=1 python spike/p2/heights_ref.py --compare
$EXE --test-corner   --variant ds-4 --tab ds --config-dir "$ABS/corner"
$EXE --test-tab-anim --variant ds-4 --tab ds --config-dir "$ABS/anim"
$EXE --test-blur     --variant kimi-full --config-dir "$ABS/blur"
$EXE --test-tray-click --variant kimi-full --config-dir "$ABS/tray"
$EXE --test-clamp --no-poll --variant ds-4 --tab ds --max-window-h 300 --out .shots/parity/clamp --config-dir "$ABS/clamp"
$EXE --test-active-tab

# 三个窗口与显隐
$EXE --test-windows  --no-poll --config-dir "$ABS/windows"
$EXE --test-settings --no-poll --config-dir "$ABS/settings"   # cap defaults to 300
$EXE --test-skills   --no-poll --config-dir "$ABS/skills"
$EXE --test-menu     --no-poll --config-dir "$ABS/menu"
$EXE --test-dialogs  --no-poll --config-dir "$ABS/dialogs"
$EXE --test-close    --no-poll --config-dir "$ABS/close"

# 登录链路
$EXE --test-login
$EXE --test-signin

# 跨壳门禁
PYTHONUTF8=1 python spike/p1/fmt_parity.py
PYTHONUTF8=1 python scripts/parity/check_tokens.py
PYTHONUTF8=1 python demo/check_contrast.py
(cd quota-core && cargo test)
(cd src-tauri && cargo test --locked)
```

`--test-*` 的判定统一看 `verdict=PASS` 或逐行 `ok`；退出码恒为 0（`--test-login` 除外）。
`--config-dir` 必须是绝对路径（相对路径会被 `quota-core` 静默忽略并落回真实 `%APPDATA%`）。

## 14. 两个壳共有的已知缺陷（不是差异）

- **折叠 YAML 的 `description: >` 显示成裸 `>`**：`quota-core::skills` 的前置解析不处理折叠块标量，两个壳都会把一个 `>` 当成描述。修它要改产品解析（两个壳同时生效），属于产品决定，不是 Slint 对齐问题。
- **托盘右键菜单无键盘导航**：两个壳都没有（产品未设计）。
- **`--test-signin` 的退出码恒为 0**：与 P2/P3 的自检一致，判定看输出。

## 15. 结论

截至 2026-11-21，矩阵没有 ❌：数据层、面板、四个窗口、托盘/菜单、设置、skills、登录与凭证、存储与实例行为全部有 ✅ 或带理由的 ⚠️。本轮矩阵抓出 4 处真实偏差，并补了 1 处覆盖缺口：

1. 面板 Console 链接缺少 `?from=kfc_overview_topbar`（两壳的 URL 不一致）。
2. 四个窗口没有设置 OS 标题（现版有）。
3. skills 窗每次打开都重新居中，丢掉用户拖动的位置。
4. 系统关闭（Alt+F4）走 Slint 默认的「隐藏」，但 `visible`/`settings_open`/`skills_open` 不更新——托盘开合与两个弹窗的重开会永久错位。现在四个窗口都接了 `on_close_requested`。
5. 上述关闭与拖动位置行为没有自检；新增 `--test-close` 逐项证明。

**2026-11-21 全量 code review** 又修了 1 个 Critical（usage 抓取把 200 错误信封当全零、覆盖 last-good）与一批 Major/Minor：设置窗标题栏移出滚动区、登录窗 4xx 文案、`--test-login` 哈希两份凭证、`--test-settings` 备份/还原与重跑可重复、token 门禁剥注释并计数、`--test-dialogs` 断言真的观察到 blur、`quota-core` 的 tie 舍入/浮点回退/原子写/索引目录等。共享数据层的 Critical 与 6 条同类问题 `main` 也有，已移植为 [PR #11](https://github.com/shawn-0106t/quota-tray/pull/11)；Slint 壳独有的问题已在本分支修掉。明细见 `CHANGELOG.md` 与 `spike/p5/REPORT.md` §9。
