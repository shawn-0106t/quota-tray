# HANDOFF — 不依赖 Tauri 的 Quota Tray（Slint + Skia）

> 新对话从这里接手。一句话现状：**静态视觉、P0 交互、P1 `quota-core` 抽取、P2 真实面板与 P3 三个窗口都已完成并量化（`spike/slint-demo/REPORT.md`、`spike/p0/REPORT.md`、`spike/p1/REPORT.md`、`spike/p2/REPORT.md`、`spike/p3/REPORT.md`）；新壳代码在 `slint/`，四个窗口全部可用、设置落盘重启生效、skills 懒加载、托盘菜单在光标处；下一步 P4 登录链路。**

## 0. 位置与前提

- worktree：与本仓库同级的 `..\quota-tray-slint`，分支 `spike/slint-demo`，基点 `3350300`（v1.0.1 已发布）。
- 主仓库 `...\kimi-deepseek-quota-dashboard` 的 `main` 未被本线改动。
- 证据（截图、对比页、rect 基准）在 `.shots/slint-demo/`、`.shots/p0/`，**gitignored**；源码在 `spike/`。
- `spike/` 与 `demo/slint-compare.py` 已提交；对比页与 16 张关键截图在 `spike/evidence/`，P0 的 4 张在 `spike/p0/evidence/`（`.shots/` 仍只是本地缓存，gitignored）。
- P0 spike 在 `spike/p0/`（crate `quota-tray-p0`），是未来 `slint/` 壳的骨架；构建前必须设 `SKIA_BINARIES_URL`（见 §6.4），否则 build script 回落源码构建并因缺 LLVM 失败。

## 1. 已定决策

| 项 | 决定 | 依据 |
|---|---|---|
| 宿主 | Slint `1.18.x`，**renderer = Skia** | 只有 Skia 的 `drop-shadow-blur: 18` 与 CSS 1:1 对齐；femtovg 要 42 且仍短 4.5 px |
| 视觉契约 | `PLAN.md` §3 继续是唯一来源；实现写在 `.slint` | 颜色/字号/几何逐条照抄已验过 |
| 像素门禁 | **重定基线**：文本带 Δy = 0 + 整窗像素差阈值（Skia ≈ 2.5% / 3.5%） | 字形 shaping 差不可消除，截图字节一致不再适用 |
| 数据层 | 抽 `quota-core`（0 处 `tauri` 引用），两个壳共用 | 避免 DeepSeek 解析/金额取整两份实现漂移 |
| 数据层落位 | 仓库根 `quota-core/`，独立 package + `src-tauri` path 依赖；**不建 workspace** | workspace 成员必须在根目录之下（试过 `../quota-core`，cargo 直接报错）；根 workspace 会把 target 目录与 `Cargo.lock` 挪到仓库根，而 `check_versions.py`、release 脚本、CI 都读 `src-tauri/` 的那一份。代价是两个 crate 各跑一次 `cargo test`（26 个迁走 + 3 个留在 src-tauri），见 `spike/p1/REPORT.md` |
| Rust 侧格式化 | `quota-core/src/format.rs` 承载 `fmtYuan/fmtMoney/fmtTokens/formatReset/fmtUpdated/阈值/tray_tooltip` | Slint 壳没有 TS；`spike/p1/fmt_parity.py` 把 Rust 与 `src/common.ts` 逐串比对（45 例全等），两壳并存期间改一边必须跑一次 |
| 登录窗 | 同 exe 的 `--login-host` 模式（`tao` + `wry`），点「Sign in…」才 spawn | 空闲时零 WebView2；不开第二个 event loop |
| 托盘 | **`tray-icon` 0.26**（已实测） | Slint 1.18 的 `SystemTrayIcon` 没有 hover 事件、右键固定弹自带原生菜单且无右键回调——10 s 悬停预取和 P3 的光标处自定义菜单都做不了；tooltip 三行本身没问题 |
| keepalive | 1×1 透明、屏幕外 (-30000,-30000) 的 `Anchor` 窗口常驻 | Slint 隐藏最后一个窗口会 `quit_event_loop()`，托盘应用会在第一次失焦隐藏时退出（实测隐藏后 ~16 ms 进程结束） |
| tab 动效实现 | 4 个组合 state（tab × 显隐）+ 先放大窗口、16 ms 后翻 state | Slint states 是互斥的，第一个条件为真的 state 生效；两组独立 state 时显隐那组永远不生效 |
| DPI 测试 | 只能用真实显示缩放，**不要用 `SLINT_SCALE_FACTOR`** | 它改 Slint 逻辑缩放但 winit 仍按系统 DPI 建窗口：225% 下贴角漂 51 px |
| 发布 | 不动 v1.0.1 线；新壳暂不进五载体门禁 | 它还没成为发布产物 |
| 新壳目录 | **`slint/`**（crate `quota-tray-slint`，exe `quota-tray-slint.exe`） | 2026-11-15 用户拍板；`spike/p0` 保留为冻结证据 |
| 面板布局 | **绝对堆叠**：`Metrics` 常量给每个块算 y，`panel.slint` 用同一批常量求和定窗口高度；**panes 不用 layout** | Slint 的 layout 在"兄弟组件里有 repeater"时会静默丢掉后面的兄弟（`spike/p2/REPORT.md` §5.1） |
| P3 多窗口 | **一个事件循环里 4 个 `Window` 组件 + 屏幕外 `Anchor`**；`win/{settings,skills,menu}.rs` 各管一个窗口 | Slint 的 `Component::show()/hide()` 本来就按组件实例建窗；Tauri 版同样是 4 个常驻窗 + 一个临时登录窗 |
| 主题实例 | 切主题要**逐个窗口**设 `Tokens.dark` | Slint 的 global 是 per-component，不是进程单例（踩过：只设 panel，其余窗口不变） |
| skills 行模型 | **Rust 算前缀和**，模型里带 `y`/`h`，Slint 只按 `y` 画 | `Flickable` 不会自己量内容，`content-height` 必须外部给；repeater 里也没有累计变量 |
| skills 描述行高 | **统一预留两行（30.44 px）** | Slint 量不到换行后的文本高度；124 个技能里 11 个单行描述会多 15.22 px（`spike/p3/REPORT.md` §5.7） |
| 菜单定位 | **逐字移植 `panel.rs::show_menu` 的 legacy 公式** | 两个壳要落在同一个像素；`win/menu.rs` 的单测钉住「贴角翻转 / 光标右下 5×13 / 翻转后底边在光标上方 13」 |
| 设置窗滚动 | 卡片内 `Flickable` + 右侧 4 px 自适应滚动条 | 与 `settings.css` 的 8px 轨道 / 4px 滑块一致；内容宽度不随滚动条收窄是已知偏差 |

## 2. 已证明 / 未证明

**P1 已证明**（`spike/p1/REPORT.md` 有全部数字与复现命令）：

- 结构：`quota-core` 12 个源文件（lib + credentials / quota / settings / skills / update / kimi_usage / format + deepseek 4 件），`src-tauri` 侧 `pub use quota_core::{...}` + 只留 `kimi_usage::run` 一个 shim；迁走的 10 个文件里 7 个与改前**逐字节相同**，`quota.rs` 只多了 `Debug` 与 5 个测试，`parse.rs` 只改了一行注释。
- 测试：**43 个全绿**（`src-tauri` 3 个 recovery + `quota-core` 40 = 原 29 个全部保留 + 9 个 format + 5 个 quota）；`cargo fmt --check` / `cargo clippy --all-targets -D warnings` 两个 crate 都干净。
- 输出：同一台机器、前后相隔 1 秒跑改前/改后 exe，`--test-fetch`、`--test-deepseek`、`--test-deepseek --verbose`、`--test-usage` 的 stdout/stderr 在掩掉 `fetchedAt` / `scannedAt` / `scanMs` / `ms=` 后**逐字节一致**（4 组，`spike/p1/compare_test_output.py`）；`--test-update` 输出也相同。
- 格式化：`spike/p1/fmt_parity.py` 用 node 直接跑 `src/common.ts`（剥掉 `@tauri-apps` import），与 `quota-core::format` 比对 45 个用例，**全部逐串相同**（含负数金额、`1.33M/284K`、`3,097`、`Resets in 2d 3h`、`Updated 11/14 23:59`）。

**P0 已证明**（`spike/p0/REPORT.md` 有全部数字）：

- 连点 tab 30 次（间隔 40 ms）：窗口底边/右边漂移 **0 px**，卡片底边 0 px、卡片顶边往返 91 px（41 逻辑）；预放大到 601 逻辑后不再变，settle 才回 560。
- 失焦 → 隐藏 **171–174 ms**（预算 170 ms，多次）；`Focused(false)` 在强制切前台后 <1 ms 到达。
- 托盘：三行 tooltip 字符串正确；hover 事件到达（spike 只记日志）、左键开合经 Explorer 同款回调消息验证通过；右键菜单已接线但本会话无法用真实鼠标点开，悬停视觉待人工。
- `--login-host`：注入钩子 + 取消导航捕获通道 `sender=ok fetch=ok`（正常 5–6 s；WebView2 冷启动 build 可能阻塞 70 s+，超时从探针发出时算）；**真实 token 已闭环**（2026-11-15 17:49 用户登录 → `verified and saved` → 现版 v1.0.1 `--test-deepseek` 读同一文件拉到真实余额，格式兼容）；DPAPI 往返有单测。
- 透明窗：白底可见完整投影与 12 px 卡片间距（Skia 与 femtovg 都成立）。

**静态视觉已证明**（`spike/slint-demo/REPORT.md` 有全部数字）：

- 布局：18（Kimi）/ 22（DeepSeek）条文本带的 y 与 Chromium **0 设备像素**偏差；所有 1 px 分隔线、进度条、卡片几何一致。
- 像素：整窗 2.51%–3.44%（Skia）/ 2.53%–3.55%（femtovg）的像素差 >8/255，全部来自字形栅格化与 shaping。
- 透明窗：`--stage none` 快照里 13.8% 像素 alpha=0、8.2% 半透明（阴影衰减），逐像素 alpha 是真的。
- 阴影：Skia blur 18 → 采样带平均差 0.52 灰阶（最大 1）；暗色下末带 Δy 从 3 降到 0。

**P2 已证明**（`spike/p2/REPORT.md` 有全部数字与复现命令）：

- 高度：17 个数据变体（Kimi full/no-cache/cache-only/extra-monthly/not-activated/no-badge、DS 4/3/2/1 行、4 种 hint、无详情、无 key、失败）的卡片与窗口高度与 Chromium DOM **Δ = 0.00**（45–545 逻辑 px 全覆盖）。
- 文本带：8 个布局变体（含深色 ds-4）带数一致、位置差 **≤ 1.00 CSS px**；残差是 2.25×/2× 两套字形栅格化的取整差，不是布局漂移。
- 贴角（真实 225%）：窗口 954 = 424 × 2.25、卡片 gap 27.0 = 12 × 2.25、10 次切换 469 采样 **drift 0 px**；`height.rs` 的 4 个单测覆盖 8 档 scale × 3 工作区 × 4 高度。
- 连点 30 次：底边/右边 drift **0 px**、窗口高度恒 1352；失焦隐藏 **180 ms**；托盘左键开合 PASS。
- `ActiveTab`：无文件读 kimi、切 ds/kimi 各落盘、旧文件无键读 kimi、`ActiveTab=deepseek` 回读，5 项全过。
- 夹取：强制 cap 300 时窗口 300、卡片 244、内容 543（可滚 299），滚动 120 px 后帧内容变化 PASS。

**P3 已证明**（`spike/p3/REPORT.md` 有全部数字与复现命令）：

- 三个窗口：设置（404 宽、内容高、居中、90% 上限 + 卡片内滚动）、skills（404×520 固定、按需扫描、124 技能 / 134 行 / 内容 8955.6、滚动位可查）、托盘菜单（188×220，光标处定位与贴边翻转逐 0.2 物理像素对上 `panel.rs` 公式）。
- 设置：落盘 → **子进程重启**读回 `theme=dark interval=10 autostart=true`；主题立即切到四个窗口的 `Tokens`；`--max-settings-h 300` 下窗口 300 / 卡片 244 / 内容 604 → 卡片滚动；Clear 把两个凭证状态复位并保留说明，凭证文件只剩空壳。
- 显隐语义：设置/skills 打开时面板失焦**不**隐藏；关设置后面板若不在前台则收起；关 skills 后焦点被 Windows 交还面板，面板保持「可见且前台」。
- 手工粘贴：无效 key 打真实接口 → `not saved`、文件无值、按钮恢复；验证在途时 Clear 会丢弃晚到结果。只带主题变化的消息也会让四个窗口重绘。
- 视觉：8 组截图 vs DOM 文本带（设置默认/展开 × 深浅、skills × 深浅、菜单 × 深浅）全部 PASS，文本带最差 1.00 CSS px；唯一 1.56 px 是按钮下边框的抗锯齿起点差。
- 回归：P2 的 17 个高度变体仍 Δ=0.00，`--test-corner/-tab-anim/-blur/-tray-click/-clamp/-active-tab` 全 PASS。

**未证明**（后续阶段）：三档 DPI 的**真机**验证（只有 225% 现场；100/150/200% 仅算术证明）、混合 DPI/多屏、键盘与 IME、登录链路（P4）、手工粘贴的真实网络往返（P4 的登录自检一并覆盖）。

**P0 期间实测到的环境限制**：本会话的合成鼠标输入（`mouse_event`/`SetInput`）不生效（连开始菜单都点不开），托盘在 CU 里也不可观察；因此托盘的 tooltip 视觉与悬停截图需要人工看一眼，事件链路用投递 `WM_USER_TRAYICON`(6002) 验证。

## 3. 写进验收的关键数字

- renderer：**Skia**；`drop-shadow-blur` 用 **18**（等于 CSS 的 `0 8px 18px`）。femtovg 的 42 是另一套值，别混用。
- 参考图采样：**rect dump 与截图必须同为 DPR=2**。DPR 不一致时 Chrome 对 `line-height: normal` 的取整不同，每块文本差 0.5–1 px，叠起来 3 px。
- 目标：文本带 Δy = 0；整窗像素差 Kimi ≈ 2.5%、DeepSeek ≈ 3.5% 是当前下界。
- P0：底边/右边漂移 **0 px**（30 次连点、186 采样）；失焦隐藏 **171–174 ms**；最后一次 shrink 的 cell 到位 **185–195 ms**（167 ms 曲线 + 16 ms 预放大一帧）；面板 424×560 / 424×601，cell 361.5 / 402.5（差 41）；225% DPI 下窗口 954×1260 物理。
- 体积：femtovg 14.6 MB / Skia 21.1 MB；依赖树 751 crate（Tauri 版 7.2 MB / 498）。
- 版本与 features：`slint = "=1.18.1"`、`slint-build = "=1.18.1"`，features = `renderer-skia` + `renderer-femtovg-wgpu`（后备）+ `unstable-winit-030`（`WinitWindowAccessor` 拿 `skip_taskbar` / 焦点事件）+ 默认的 `system-tray`。

## 4. 目标架构

```
quota-core/            # 纯 Rust：credentials / quota / deepseek/* / kimi_usage / settings / skills / update
                       #  + 格式化（fmtYuan / fmtMoney / fmtTokens / formatReset / 阈值）+ 它们的单测
src-tauri/             # 现版壳，改为依赖 quota-core（pub use 保住 crate::xxx 路径）
slint/                 # 新壳：ui/*.slint + src/{main,shell,app,view,height,checks}.rs + src/win/{settings,skills,menu}.rs
spike/slint-demo/      # 静态视觉 demo，保留作证据与坐标基准
spike/p0/              # P0 交互 spike：壳的骨架（Anchor / tray-icon / 动效控制器 / --login-host）
```

`--login-host`：主进程 spawn `current_exe() --login-host`，子进程用 `tao` 开窗 + `wry` 加载 console，沿用现有 `initialization_script` + `navigation_handler`（取消 `quotatray://` 导航），验证后自己用 DPAPI 落盘并退出；父进程等退出后刷新。`Clear` 语义 = 杀子进程 + 清凭证 + bump epoch。

## 5. 阶段（建议顺序）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0 交互 spike（已完成 2026-11-15）** | 托盘 + 面板显示/隐藏 + tab 切换动效 + 失焦隐藏 + `--login-host` 最小闭环；结果见 `spike/p0/REPORT.md` | 底边 0 px；失焦 172–174 ms；tooltip 3 行；捕获通道 `sender=ok fetch=ok`；真实 token 已闭环（用户登录 → 现版 v1.0.1 读到真实余额） |
| **P1 `quota-core` 抽取（已完成 2026-11-15）** | 纯逻辑 + 格式化迁到仓库根 `quota-core/`，`src-tauri` 改 path 依赖并 `pub use` 保住 `crate::xxx` 路径；结果见 `spike/p1/REPORT.md` | 43 测试全绿（原 29 全保留）；`--test-fetch/--test-deepseek/--test-usage` 与改前 exe 在掩掉 4 个 live 字段后逐字节一致（4 组）；格式化 45 例 TS==Rust |
| **P2 面板（已完成 2026-11-15）** | 新壳 `slint/`：`quota-core` 真实数据、1–4 模型行、`ActiveTab` 持久化、夹取滚动；结果见 `spike/p2/REPORT.md` | 17 个变体高度 Δ=0；8 个变体文本带 ≤1.00 css px；225% 下漂移 0 px（10 次切换 469 采样）；连点 30 次 drift 0；`ActiveTab` 5 项断言；夹取滚动 PASS |
| **P3 设置 / skills / menu（已完成 2026-11-15）** | 三窗 + 凭证状态 + 手工粘贴 + 自启动/便携/单实例/主题；结果见 `spike/p3/REPORT.md` | 落盘 → 子进程重启读回 PASS；8 组文本带 ≤1.00 css px；显隐语义 `--test-dialogs` PASS |
| P4 登录完整链路（2–3 天） | `--login-host` + `--test-login`；接上设置窗的「Sign in…」与 Clear 的 epoch | probe 与 fetch 捕获双通过；Clear 不被回写 |
| P5 对齐矩阵与门禁（3–4 天） | `PARITY.md`、token 漂移门禁、截图重生成、CI job | 矩阵无 ❌（`quota-core` 的 fmt/clippy/test 已在 P1 接入 CI，且不阻塞现版） |

## 6. 陷阱清单（都踩过）

1. **demo 的绝对坐标不能直接搬进产品**。它证明的是「渲染器能对上」，不是「布局引擎能对上」；真实数据是 1–4 行模型，必须换成 Slint layout，再用同一套对比脚本验高度。
2. **字体 shaping 差修不掉**：同串墨迹宽度差 -4 / -11 设备像素，来自 parley/fontique vs DirectWrite，换 renderer 无效；不要用 `letter-spacing` 硬凑，会毁掉字重观感。
3. **格式化逻辑要搬到 Rust 并带测试**：`fmtYuan`（分）、`fmtMoney`（元）、`fmtTokens`、`formatReset`、`Updated M/D HH:MM`、cache<60 / 余额<¥10 阈值——P1 已落到 `quota-core/src/format.rs`（Slint 壳没有 TS）；Tauri 前端仍用 `src/common.ts`，两边由 `spike/p1/fmt_parity.py` 对拍 45 例，改任一侧都要跑它。
4. **Skia 预编译包**：首次构建从 `github.com` 下载（本机曾被 hosts 指到 127.0.0.1 而失败）。直连不通时用镜像：`SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'`。
5. **Skia 启动会打印** `Failed to initialize Skia wgpu surface … Falling back…`：它先试 wgpu/Vulkan，回落到自己的 GL/D3D surface，出图正常，忽略即可。
6. **DPR 一致性**：rect dump 与截图同 DPR（`--force-device-scale-factor=2`），否则出现 3 px 累积漂移。
7. `.shots/` 是 gitignored；要长期保留对比页就得挪进 `spike/`。
8. `SystemTrayIcon` 是 1.17 新增，先验证 tooltip 与 hover 事件，再决定要不要换 `tray-icon`。**结论：换 `tray-icon`**（无 hover、右键无回调）。
9. **Slint states 是互斥的**：只有第一个为真的 state 生效，不会叠加。tab 与显隐必须写成 4 个组合 state（`kimi-shown`/`ds-shown`/`kimi-hidden`/`ds-hidden`）。1.18 已移除 `transitions` 块，动画写在 state 的 `in { animate … }` 里。写成两组独立 state 的症状：`shown=true` 但 `body-opacity` 恒为 0，窗口全黑。
10. **隐藏最后一个窗口会退出事件循环**：Slint 的 keepalive 计数归零后调用 `quit_event_loop()`。托盘应用必须常驻一个可见窗口（本 spike 用 1×1 透明 `Anchor`，屏幕外），否则第一次失焦隐藏就退出进程。
11. **`SLINT_SCALE_FACTOR` 只改逻辑缩放**：winit 仍按系统 DPI 建窗口，`window.scale_factor()` 与物理尺寸不一致，贴角计算会漂（225% 下 51 px）。DPI 测试必须真改显示缩放。
12. **`take_snapshot()` 冻结动画时钟**：动画中调用只会得到最后一次绘制的那一帧（本机看到两个 pane 各 50% 叠加）；静态截图正常。动效证据用真实截屏。
13. **WebView2 数据目录不可共享**：另一个进程持有同一 profile 时环境创建返回 `0x800700AA`。测试用 `%TEMP%` 下按 pid 全新目录；要复用 Tauri 的会话必须先退出 Tauri 版。probe 偶发秒退（143），重跑即可。
14. **透明窗本身没问题**：白底截图证明 alpha 与投影都在；“黑底”是背后的黑色终端透出来。
15. **Skia 预编译包必须走镜像**：`SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'`。不设时每次 build 都会重试 GitHub（curl 35）并回落源码构建，本机无 LLVM 直接失败。
16. **cargo workspace 的成员必须在根目录之下**：`members = ["../quota-core"]` 直接被拒（`is not hierarchically below the workspace root`）。把 workspace 根放到仓库根则会把 `target/` 与 `Cargo.lock` 从 `src-tauri/` 挪走，而 `scripts/release/check_versions.py`、release 工作流、CI 的 rust-cache 全都读 `src-tauri/` 那一份。所以 `quota-core` 是**独立 package + path 依赖**：`quota-core/Cargo.lock` 不提交（library 约定，已进 `.gitignore`），CI 对它只跑 `cargo test`（不带 `--locked`），测试因此是两条命令。
17. **「输出逐字节一致」只能对 live 字段取掩码**：`--test-fetch` / `--test-deepseek` / `--test-usage` 打的是实时数据（`fetchedAt`、`scannedAt`、`scanMs`、`ms=`，以及 DeepSeek 账户与 Kimi 会话的计数），跨进程、跨分钟不可能相同。验法是保留改前 exe，在同一分钟里前后各跑一次两个 exe，再用 `spike/p1/compare_test_output.py` 掩掉这 4 处比对；直接 diff 两次隔几分钟的输出会得出假阴性（真实见过 4958→4972 requests、¥16.98→¥16.91，全是账户在动，不是代码）。
18. **Slint 的 layout 会被 repeater 打断**：`VerticalLayout` 的某个兄弟组件内部有 `for … in model` 时，**它后面的所有兄弟都不渲染**——而高度表达式照旧把它们算进去（症状：窗口比内容高、末尾一行永远不出现）。定位手法：给可疑块加 `background: #ff000060`、在布局首尾各放一个绿色探针，逐次挪位置；只有 repeater 之后的兄弟消失。修法：panes 改成按 `Metrics` 常量算 y 的绝对堆叠（Kimi 5 块 / DS 4 块），不依赖 layout。`spike/p2/REPORT.md` §5.1。
19. **Slint 会居中"没有显式位置"的子元素**：`Field` 只写了 `x`，整列内容低了 5 px = `(115 − 105) / 2`。位置由父级决定的元素，x/y 都要显式写。
20. **`Window::size()` 是物理像素**（`PhysicalSize`）：225% 下 300 逻辑的窗口报 675。凡"逻辑高度"一律除以 `scale_factor()`；`take_snapshot()` 的 buffer 同理（按逻辑宽度存图会切成斜条）。
21. **`Flickable.content-y` 滚动时是负值**，设正值会被夹回 0；1.18 把 `viewport-width/height` 改名 `content-width/height`。`scroll-y <=> card-scroll.content-y` 两向绑定后 Rust 可直接驱动滚动做测试。
22. **Chromium 在分数 DPR 下布局会变**：同一份 DOM 在 DPR 2 量到 504、2.25 量到 501（`line-height: normal` 按设备像素取整）。参考图固定 DPR 2，对比时两边各自换算成 CSS px；另外参考页必须 `html, body { height: 100% }`，否则 `overflow: hidden` 的零高 body 会把绝对定位的卡片裁成空白（DOM 仍报 504，只有截图能暴露）。
23. **Rust 格式化必须对着 TS 跑一遍再交付**：`toFixed` 的舍入、`Math.floor` 对负数的处理、`toLocaleString('en-US')` 的分组都不是 Rust 默认行为。`spike/p1/fmt_parity.py` 用 node 直接 import `src/common.ts`（node 24 原生剥类型，只需删掉开头两行 `@tauri-apps` import），与 `quota-core::format` 对拍 45 个用例。两壳并存期间，改任一侧都要先跑它。
24. **别把模块叫 `windows`**：`mod windows;` 会让 `use windows::Win32::…`（windows crate）解析成自己的模块，整个 win32 层报 `could not find Win32`。P3 的窗口模块因此叫 `win`。
25. **Slint 的 global 是 per-component**：同一个 `tokens.slint` 里的 `Tokens`，在四个窗口里是四个实例。只设面板那一个是 P3 的第一个真 bug（其余窗口不换主题）。切主题要逐个 `window.global::<Tokens>().set_dark(...)`。
26. **`callback close()` 撞 `Window` 内建 `close()`**：构建报 warning 并被遮蔽。窗口级回调改名 `dismiss`。
27. **`TextInput` 没有 placeholder**：占位符要另叠一个 `visible: text == ""` 的 `Text`。
28. **U+2715（✕）在 Skia + Segoe UI 下是豆腐块**：WebView2 靠字体回退，Slint 不会。关闭按钮改成两条 `Path` 线（`M0 0 L7 7 M7 0 L0 7`）。
29. **`Flickable` 不自量内容**：`content-width/height` 默认 0，必须外部给；所以 skills 的行 y 与总高只能在 Rust 里算前缀和，repeater 里没有累计变量。
30. **`--config-dir` 要给绝对路径**：`quota-core` 只在 `QUOTA_TRAY_APP_DATA_DIR` 是绝对路径时采用它，相对路径会被静默忽略并落回真实 `%APPDATA%`（现在会打一行提示）。自检一律 `$(pwd -W)`。
31. **首次 `show()` 会阻塞 100–200 ms**（winit 建窗 + Skia surface），同一个 tick 里排好的后续单次定时器整体顺延。自检时间轴要按「每个动作后留 400 ms 以上」排，否则「关窗」会抢在「开窗」前面。
32. **`AtomicU64::fetch_add` 返回的是加之前的值**：credential epoch 的守卫如果拿返回值当「本次世代」，那么 `load() == epoch` 恒为 false —— 表现是手动粘贴永远「被更新的一次请求取代」、一次都不保存、而且 `verifying` 永远为 true（按钮永久禁用、重启才好）。正确写法是 `fetch_add(1, …) + 1`（审查抓到的 Critical，`--test-settings` 现在覆盖失败路径与 Clear 作废在途）。
33. **只带一个字段的消息批次也要置 `changed`**：主题 watcher 只在变化那一刻发一条 `Msg::Theme`，泵里若不置 `changed`，`apply_theme` 会被跳过——面板可能在下一次轮询时顺带换色，设置/skills/菜单永远不跟随（审查抓到的 Major）。
34. **关子窗后的焦点不一定回面板**：Windows 通常把焦点还给上一个前台窗口——面板原本在前台时，关 skills 后面板就是「可见且前台」；否则保持不可见。判定写成「不可见 或 前台」，不是「必须不可见」（`--test-dialogs` 两种分支都覆盖）。

## 7. 复现命令

```bash
cd ../quota-tray-slint    # 从主仓库目录进入 worktree
PYTHONUTF8=1 python spike/slint-demo/ref/build_ref.py      # 参考图 + rect 基准（DPR 2）
cd spike/slint-demo && cargo build --release
for t in light dark; do for tab in kimi ds; do
  ./target/release/slint-demo.exe --theme $t --tab $tab --renderer skia --scale 2 --blur 18 \
    --out ../../.shots/slint-demo/skia-$t-$tab.png
done; done
cd ../.. && PYTHONUTF8=1 python demo/slint-compare.py       # 三栏对比 + Δy + 探针表
explorer.exe ".shots\slint-demo\compare.html"
```

P0：

```bash
cd spike/p0
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'
cargo build
./target/debug/quota-tray-p0.exe --test-tab-anim     # 底边漂移
./target/debug/quota-tray-p0.exe --test-blur         # 失焦 -> 隐藏
./target/debug/quota-tray-p0.exe --test-tray-click   # 托盘消息投递
./target/debug/quota-tray-p0.exe --test-login        # 子进程 probe
./target/debug/quota-tray-p0.exe                     # 交互模式（托盘左键开面板）
./target/debug/quota-tray-p0.exe --hold --seconds 20 --demo-switch --out DIR   # 动效截图/目视
```

P1：

```bash
cd ../quota-tray-slint
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked)
(cd quota-core && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
PYTHONUTF8=1 python spike/p1/fmt_parity.py      # 45 个格式化用例，TS == Rust
```

P2（新壳；**每次 cargo 调用都要带 `SKIA_BINARIES_URL`**）：

```bash
cd ../quota-tray-slint
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'
(cd slint && cargo build && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
PYTHONUTF8=1 python spike/p2/heights_ref.py --compare          # 17 个变体的高度 vs DOM
./slint/target/debug/quota-tray-slint.exe --hold --no-poll --variant ds-4 --tab ds     --theme light --out .shots/p2/ds-4
PYTHONUTF8=1 python spike/p2/compare_bands.py --shot .shots/p2/ds-4/panel.png --variant ds-4
EXE=./slint/target/debug/quota-tray-slint.exe
$EXE --test-active-tab
$EXE --test-corner    --variant ds-4 --tab ds --config-dir .shots/p2/cfg-corner
$EXE --test-tab-anim  --variant ds-4 --tab ds --config-dir .shots/p2/cfg-anim
$EXE --test-blur      --variant kimi-full --config-dir .shots/p2/cfg-blur
$EXE --test-tray-click --variant kimi-full --config-dir .shots/p2/cfg-tray
$EXE --test-clamp --no-poll --variant ds-4 --tab ds --max-window-h 300     --out .shots/p2/clamp --config-dir .shots/p2/cfg-clamp
$EXE --hold --seconds 9 --out .shots/p2/live                    # 真实数据
```

P3（三个窗口；同样每次都带 `SKIA_BINARIES_URL`）：

```bash
cd ../quota-tray-slint
ABS="$(pwd -W)/.shots/p3"
EXE=./slint/target/debug/quota-tray-slint.exe
$EXE --test-windows  --no-poll --config-dir "$ABS/cfg-windows"
$EXE --test-settings --no-poll --config-dir "$ABS/cfg-settings" --max-settings-h 300
$EXE --test-skills   --no-poll --config-dir "$ABS/cfg-skills"
$EXE --test-menu     --no-poll --config-dir "$ABS/cfg-menu"
$EXE --test-dialogs  --no-poll --config-dir "$ABS/cfg-dialogs"
$EXE --shots --no-poll --theme light --out .shots/p3/light --config-dir "$ABS/cfg-shots"   # 入库证据（skills 固定列表）
$EXE --shots --no-poll --theme dark  --out .shots/p3/dark  --config-dir "$ABS/cfg-shots"
$EXE --shots --no-poll --variant skills-real --theme light --out .shots/p3/real-light --config-dir "$ABS/cfg-shots"   # 对比用（真实列表）
$EXE --shots --no-poll --variant skills-real --theme dark  --out .shots/p3/real-dark  --config-dir "$ABS/cfg-shots"
PYTHONUTF8=1 python spike/p3/ref_windows.py          # 参考页 + rect 基准（注入真实 skills 列表）
PYTHONUTF8=1 python spike/p3/compare_windows.py --shot .shots/p3/real-light/settings.png --window settings --theme light
```

`--test-*` 输出一致性要两个 exe 才比得出来：先 `git stash -u` 回到改前构建一份 exe 存到 `.shots/p1/quota-tray-before.exe`，`git stash pop` 后再构建改后的，然后在同一分钟里对每种模式前后各跑一次，`spike/p1/compare_test_output.py BEFORE_DIR AFTER_DIR` 掩掉 4 个 live 字段比对（目录里要有 `fetch.* / deepseek.* / deepseek-verbose.* / usage.*` 共 8 个文件）。

## 8. 给新对话的第一句（可直接粘贴）

> 接手 `..\quota-tray-slint`（分支 `spike/slint-demo`）。先读 `spike/HANDOFF.md`，再读 `spike/slint-demo/REPORT.md`、`spike/p0/REPORT.md`、`spike/p1/REPORT.md`、`spike/p2/REPORT.md` 与 `spike/p3/REPORT.md`。P0–P3 都已完成，新壳代码在 `slint/`（crate `quota-tray-slint`）：面板接 `quota-core` 真实数据、1–4 行模型、`ActiveTab` 持久化、夹取滚动；设置窗（主题/间隔/自启/凭证状态/手工粘贴）落盘重启生效；skills 懒加载 + 刷新；托盘右键在光标处开自定义菜单。下一步按 HANDOFF §5 做 P4：`--login-host` + `--test-login`，把设置窗的「Sign in…」接上真实登录窗，并验证 Clear 的 epoch 在真实链路里不被回写。

## 9. 待拍板

- ~~新壳目录名~~（**已定 2026-11-15：`slint/`**）。
- 最终是否用 Slint 版替代 Tauri 版（决定 v2.0.0 的发布线怎么切）。
- skills 的单行描述：本实现统一预留两行（124 行里 11 行多 15.22 px）。P5 若要严格对齐，要么引入文本测量，要么顺手修 `quota-core::skills` 的折叠 YAML 解析（`description: >` 现在显示成裸 `>`，两个壳都一样）——后者会改现版行为，需要单独决定。
- 三档 DPI 的真机验证：本机固定 225%，改缩放会动用户的桌面布局；用户已选「算术证明 + 现场 225%，100/150/200% 留人工检查单」。
