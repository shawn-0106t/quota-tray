# HANDOFF — 不依赖 Tauri 的 Quota Tray（Slint + Skia）

> 新对话从这里接手。一句话现状：**静态视觉与 P0 交互都已用可运行 demo 证明并量化（`spike/slint-demo/REPORT.md`、`spike/p0/REPORT.md`）；三档 DPI、混合 DPI/多屏、键盘与 IME、菜单/设置窗口尚未验证；全量构建尚未开始，下一步 P1 `quota-core`。**

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
| 登录窗 | 同 exe 的 `--login-host` 模式（`tao` + `wry`），点「Sign in…」才 spawn | 空闲时零 WebView2；不开第二个 event loop |
| 托盘 | **`tray-icon` 0.26**（已实测） | Slint 1.18 的 `SystemTrayIcon` 没有 hover 事件、右键固定弹自带原生菜单且无右键回调——10 s 悬停预取和 P3 的光标处自定义菜单都做不了；tooltip 三行本身没问题 |
| keepalive | 1×1 透明、屏幕外 (-30000,-30000) 的 `Anchor` 窗口常驻 | Slint 隐藏最后一个窗口会 `quit_event_loop()`，托盘应用会在第一次失焦隐藏时退出（实测隐藏后 ~16 ms 进程结束） |
| tab 动效实现 | 4 个组合 state（tab × 显隐）+ 先放大窗口、16 ms 后翻 state | Slint states 是互斥的，第一个条件为真的 state 生效；两组独立 state 时显隐那组永远不生效 |
| DPI 测试 | 只能用真实显示缩放，**不要用 `SLINT_SCALE_FACTOR`** | 它改 Slint 逻辑缩放但 winit 仍按系统 DPI 建窗口：225% 下贴角漂 51 px |
| 发布 | 不动 v1.0.1 线；新壳暂不进五载体门禁 | 它还没成为发布产物 |

## 2. 已证明 / 未证明

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

**未证明**（后续阶段）：三档 DPI（需真改显示缩放）、混合 DPI/多屏、键盘与 IME、1–4 个模型行的动态高度（P2）、设置/skills/菜单窗口（P3）。

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
slint/                 # 新壳：ui/*.slint + src/{main,shell,panel,settings,skills,menu,tray,theme,login_host}.rs
spike/slint-demo/      # 静态视觉 demo，保留作证据与坐标基准
spike/p0/              # P0 交互 spike：壳的骨架（Anchor / tray-icon / 动效控制器 / --login-host）
```

`--login-host`：主进程 spawn `current_exe() --login-host`，子进程用 `tao` 开窗 + `wry` 加载 console，沿用现有 `initialization_script` + `navigation_handler`（取消 `quotatray://` 导航），验证后自己用 DPAPI 落盘并退出；父进程等退出后刷新。`Clear` 语义 = 杀子进程 + 清凭证 + bump epoch。

## 5. 阶段（建议顺序）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0 交互 spike（已完成 2026-11-15）** | 托盘 + 面板显示/隐藏 + tab 切换动效 + 失焦隐藏 + `--login-host` 最小闭环；结果见 `spike/p0/REPORT.md` | 底边 0 px；失焦 172–174 ms；tooltip 3 行；捕获通道 `sender=ok fetch=ok`；真实 token 已闭环（用户登录 → 现版 v1.0.1 读到真实余额） |
| P1 `quota-core` 抽取（2–3 天） | 迁移纯逻辑 + 格式化，`src-tauri` 改依赖 | 29 个 `cargo test` 全绿；`--test-fetch/--test-deepseek/--test-usage` 输出与改前逐字节一致 |
| P2 面板（1–1.5 周） | 真实数据、1–4 模型行、`ActiveTab` 持久化 | 三档 DPI 贴角不跳位；数据变化时高度正确 |
| P3 设置 / skills / menu（3–5 天） | 三窗 + 凭证状态 + 手工粘贴 + 自启动/便携/单实例/主题 | 落盘重启生效；显隐语义正确 |
| P4 登录完整链路（2–3 天） | `--login-host` + `--test-login` | probe 与 fetch 捕获双通过；Clear 不被回写 |
| P5 对齐矩阵与门禁（3–4 天） | `PARITY.md`、token 漂移门禁、截图重生成、CI job | 矩阵无 ❌；新 crate 的 fmt/clippy/test 进 CI，不阻塞现版 |

## 6. 陷阱清单（都踩过）

1. **demo 的绝对坐标不能直接搬进产品**。它证明的是「渲染器能对上」，不是「布局引擎能对上」；真实数据是 1–4 行模型，必须换成 Slint layout，再用同一套对比脚本验高度。
2. **字体 shaping 差修不掉**：同串墨迹宽度差 -4 / -11 设备像素，来自 parley/fontique vs DirectWrite，换 renderer 无效；不要用 `letter-spacing` 硬凑，会毁掉字重观感。
3. **格式化逻辑要搬到 Rust 并带测试**：`fmtYuan`（分）、`fmtMoney`（元）、`fmtTokens`、`formatReset`、`Updated M/D HH:MM`、cache<60 / 余额<¥10 阈值——现在都在 `src/common.ts`，Slint 壳没有 TS。
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

## 8. 给新对话的第一句（可直接粘贴）

> 接手 `..\quota-tray-slint`（分支 `spike/slint-demo`）。先读 `spike/HANDOFF.md`、`spike/slint-demo/REPORT.md` 与 `spike/p0/REPORT.md`。P0 交互 spike 已完成；下一步按 HANDOFF §5 做 P1：抽 `quota-core`（纯 Rust 逻辑 + 格式化 + 单测），`src-tauri` 改依赖它，29 个 `cargo test` 全绿且 `--test-fetch/--test-deepseek/--test-usage` 输出与改前逐字节一致；不要先做全量 UI 重写。

## 9. 待拍板

- 新壳目录名：`slint/` 还是 `native/`。
- 最终是否用 Slint 版替代 Tauri 版（决定 v2.0.0 的发布线怎么切）。
