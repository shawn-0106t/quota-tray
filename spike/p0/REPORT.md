# P0 交互 spike — 结果

> 结论先行：**P0 的四项验收都已在真机上跑通或证伪到具体原因**。tab 连点 30 次底边漂移 **0 px**；失焦到窗口隐藏 **171–174 ms**（预算 170 ms）；托盘三行 tooltip 与左右键/悬停链路打通（用 Explorer 同款回调消息验证，因为本会话的合成鼠标输入被系统拒绝）；`--login-host` 子进程的注入 + 取消导航捕获通道 **sender=ok / fetch=ok**。**发现 3 个会让托盘应用直接不可用的 Slint 事实**（states 互斥、隐藏最后窗口会退出事件循环、`SLINT_SCALE_FACTOR` 与 winit 窗口缩放不一致），都已在 spike 里绕开并记录在 HANDOFF §6。

## 1. 验收

| 验收项 | 结果 | 证据 |
|---|---|---|
| 连点 tab 30 次，底边不动 | **PASS**：窗口底边 1728 px 恒定、漂移 **0 px**；右边 0 px；卡片底边（由 win-h + cell 高度换算）也恒为 1665 px、**0 px**，卡片顶边在 440..531 之间移动（**91 px ≈ 41 逻辑**，正好是两 tab 的差值）——即卡片向上长、底边不动 | `--test-tab-anim`，约 120–190 个 5 ms 采样 |
| 失焦 170 ms 内收起 | **PASS**：171 / 172 / 172 / 174 ms（多次），`focus: lost` 在 SetForegroundWindow 后 <1 ms | `--test-blur` |
| 托盘三行 tooltip 正确 | **字符串正确**（`Quota Tray\nKimi 5h 31% · week 12%\nDeepSeek ¥45.44`，3 行，交给 `Shell_NotifyIcon` 的 `szTip`）。**悬停视觉未能在本会话验证**：系统拒绝合成鼠标输入（连点开始菜单都无效），托盘在 CU 里也不可观察 | `--test-tray` 日志 + `evidence/tray-row.png` |
| 左键开合 / 悬停 | **PASS（链路）**：向 tray-icon 的回调窗口投递 Explorer 同款消息后，hover → 预取、左键 → 显示、再左键 → 隐藏全部成立。右键菜单已接线（tray-icon 显示 muda 菜单），本会话无法用真实鼠标点开验证 | `--test-tray-click` |
| 子进程捕获并落盘真实 token | **PASS（2026-11-15 17:49）**：用户在托盘菜单登录后，子进程日志 `token verified and saved`、父进程 `child exited code=0 stored-token=true`；凭证文件 userToken 为 398 字符 DPAPI blob；再用现版 v1.0.1 `quota-tray.exe --test-deepseek` 读同一文件成功拉到真实余额（¥17.59），证明落盘格式与现版兼容。probe 另有 `sender=ok fetch=ok`（5–6 s；WebView2 冷启动 build 可能阻塞 70 s+，超时从探针发出时算） | 共享日志 + `%APPDATA%\QuotaTray\deepseek-credentials.json` mtime + 现版 `--test-deepseek` |
| 透明窗 + 卡片贴角 | **PASS**：白底截图可见完整投影与 12px 卡片间距；225% DPI 下窗口 954×1260 物理、卡片 828×1134 物理 | `evidence/panel-kimi.png` |

## 2. 关键数字

- 面板：424×560 / 424×601 逻辑；卡片 368×504 / 545；cell 361.5 / 402.5（差值 41 = 两 tab 的高度差）。
- tab 动效：grow 200 ms / shrink 167 ms；从 cell 高度采样，最后一次 shrink **185–195 ms** 到位（167 ms 曲线 + 16 ms 的“先放大窗口、后翻状态”一帧 + 采样粒度）。
- 预放大：连点期间窗口高度到 1352（601 逻辑）后**不再变**，settle 才回到 1260（560 逻辑）；窗口底边/右边、卡片底边都不动，卡片顶边往返 91 px——“窗口先放大、卡片向上长、底边不动”成立。窗口底边恒定的公式本身是构造性的（`y = wa_bottom - (h-16)·scale`），这个测试证明的是它在 30 次 pre-size/settle 里没有被破坏（它抓到过 51 px 的 scale bug），以及卡片顶边的真实位移。
- 失焦：Windows 的 `Focused(false)` 在 `SetForegroundWindow(Shell_TrayWnd)` 后 <1 ms 到达；隐藏发生在 170 ms 之后（`main.ts playHide` 同值）。测试必须先真正拿到前台，否则面板从头到尾没焦点、不会失焦——用 `AttachThreadInput` 取一次前台后才稳定 3/3。
- 托盘：图标 72×108 物理；tooltip 3 行；Enter/Move 事件按 10 s 节流到达（spike 里预取钩子只记日志）。
- 构建：debug 55 MB；首次冷构建 ~6 min（Skia 预编译包 10 MB，经 ghproxy 镜像）；之后增量 ~2 s。

## 3. 证据文件

- `evidence/panel-kimi.png` — 面板快照（954×1260，含 28px 透明留白与投影）。
- `evidence/tab-crossfade.png` — 切换中的一帧：Kimi/DeepSeek 两个 pane 同时可见、DeepSeek 单选已选中、footer 已到 DS 位置；outgoing 在上层（文字压住 incoming）。
- `evidence/tray-row.png` — 任务栏里的双竖条图标（托盘区，非 overflow）。
- 悬停 tooltip 与右键原生菜单两张图**缺**：本会话合成鼠标输入不生效（见 §4.6），需要人工在真机上悬停/右键各看一眼。
- `evidence/login-host.png` — `--login-host` 打开的 DeepSeek 控制台（登录页；说明复用会话已登出）。

## 4. 发现（按严重度）

1. **Slint 的 states 是互斥的**：只有第一个条件为真的 state 生效，属性不会叠加。把“tab”和“显隐”写成两组独立 state 时，第二组永远不生效——表现为面板 `shown=true` 但 `body-opacity` 一直 0、窗口全黑。修复：4 个组合 state（`kimi-shown` / `ds-shown` / `kimi-hidden` / `ds-hidden`），每个 state 里用 `in { animate … }` 写自己的曲线（1.18 已移除 `transitions` 块）。
2. **隐藏最后一个窗口会退出事件循环**：`ui.hide()` 之后 `release_keepalive()` 把计数减到 0，`quit_event_loop()` 被调用，进程在隐藏后 ~16 ms 退出——托盘应用会在第一次失焦隐藏时自杀。修复：一个 1×1 透明、屏幕外 (-30000,-30000) 的 `Anchor` 窗口常驻可见，顶住 keepalive 计数。
3. **`SLINT_SCALE_FACTOR` 不是可信的 DPI 模拟**：它改 Slint 的逻辑缩放，但 winit 仍按系统 225% 建 954×1260 的物理窗口；拿 `window.scale_factor()`（=1.0）算贴角，底边会漂 51 px。P0 因此**改用真实系统 DPI**（本机 225%）。三档 DPI 验证要在显示设置里真改缩放，不能靠这个环境变量。
4. **`take_snapshot()` 会冻结动画时钟**：动画进行中调用，得到的是“最后一次绘制”的状态（本轮里两个 pane 各 50% 叠加的一帧）；静态调用正常。而且一次 snapshot 会阻塞事件循环数百毫秒，四个 40/100/160/260 ms 的“逐帧”实际落在 ~380 ms 间隔上。不要用它做逐帧动效证据，动效证据用真实截屏/目视；`--out` 目录已改为自动创建。
5. **WebView2 环境首次 build 可能阻塞**（冷 profile 见过 76 s），探针超时要从发出时算；数据目录不可共享：另一个进程（含本机正在运行的 `quota-tray.exe`）持有同一 profile 时 `CreateCoreWebView2Environment` 返回 `0x800700AA`，子进程在 1 s 内秒退。**退出码是 143（崩溃式退出，不是本程序的 2/3/6 正常路径），更像 WebView2 在锁冲突下被终止**；同一目录重跑一次即恢复。`--test-login` 因此每次用 `%TEMP%\quota-tray-p0-webview-<pid>` 全新目录；真实 token 测试要复用的 Tauri profile 需先退出 Tauri 版。
6. **本会话不接受合成鼠标输入**：`mouse_event` 移动、点击开始菜单均无效（CU 持有真实输入通道；托盘/任务栏也不在 CU 的可用目标里）。托盘的 tooltip 视觉、悬停截图需要人工看一眼；事件链路用投递 `WM_USER_TRAYICON`（6002）+ `WM_LBUTTONUP` / `WM_MOUSEMOVE` 证明。
7. **Skia 预编译包**：GitHub 直连 `curl error 35`，必须 `SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'`；不设它时 build script 每次重试并回落到源码构建（本机无 LLVM，直接失败）。启动时 `Failed to initialize Skia wgpu surface … vulkan` 是噪声，忽略。
8. **透明本身没问题**：两套 renderer 都在白底上给出真实 alpha 与投影；之前看到的“黑底”是背后的黑色终端透出来的，不是窗口不透明。

## 5. 托盘选型结论

**用 `tray-icon` crate，不用 Slint 1.18 的 `SystemTrayIcon`。** 源码与实机都确认 Slint 托盘：

- 右键固定弹它自己的原生菜单，**没有右键回调**——P3 的“光标处自定义菜单窗口”做不了；
- **没有任何 hover 事件**——10 s 节流的悬停预取做不了；
- 左键 `clicked` 与 tooltip 可用（tooltip 3 行没问题）。

`tray-icon` 0.26 给左键/右键/Enter/Move/Leave 全套事件，菜单交给 `muda`（`tray_icon::menu` 已重导出），与 Tauri 版同一底子。`--tray-probe` 保留了 Slint 托盘的最小对照实现，可随时复测。

## 6. 本期没证明的

- 真实 token 的“捕获→校验→落盘”整链已由用户 2026-11-15 17:49 登录闭环；只在本机单账号、单 profile 下验证过。
- **三档 DPI**：本机固定 225%，需要在显示设置里切 100/150/200% 各看一次贴角（P2）。
- **跨屏 / 混合 DPI**、键盘与 IME、P3 的菜单/设置/skills 窗口。
- 已知的 spike 级偏差（不是产品决定）：`Shell` 里的回调持有强 `Rc`，进程退出前不会析构（spike 不在意）；`show` 后 250 ms 内的失焦被忽略（防托盘点击把面板瞬间关掉，真机点击路径未验证）；托盘的“预取”只写日志，没有真实刷新。
- 失焦测试存在环境抖动：连续快速跑多个 case 时 `SetForegroundWindow(Shell_TrayWnd)` 偶尔不生效（单跑 3/3 通过）；已在测试里先强制面板前台并加桌面窗口兜底。

## 7. 复现

```bash
cd ../quota-tray-slint/spike/p0
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'
cargo build

./target/debug/quota-tray-p0.exe --test-tab-anim        # 30 clicks, bottom-edge drift
./target/debug/quota-tray-p0.exe --test-blur            # blur -> hidden latency
./target/debug/quota-tray-p0.exe --test-tray-click      # posted tray messages
./target/debug/quota-tray-p0.exe --test-tray --seconds 30   # tray + native menu, manual hover
./target/debug/quota-tray-p0.exe --test-login           # child probe: sender + fetch
./target/debug/quota-tray-p0.exe --tray-probe --seconds 30  # Slint SystemTrayIcon comparison

# interactive: tray only; left-click the icon
./target/debug/quota-tray-p0.exe
# visual hold, optional mid-switch snapshot
./target/debug/quota-tray-p0.exe --hold --seconds 20 [--tab ds] [--out DIR] [--demo-switch]
```

日志同时写 stderr 与 `%TEMP%\quota-tray-p0.log`（`P0_LOG` 可改路径）。运行时自检参数在单实例互斥量之前分派，可与已运行实例并存。

## 8. 文件

```
spike/p0/Cargo.toml / build.rs
spike/p0/src/main.rs         dispatch、日志、单实例、--test-login
spike/p0/src/shell.rs        透明窗/定位/显隐/动效控制器/托盘/自检
spike/p0/src/login_host.rs   --login-host：tao + wry + 注入钩子 + 捕获
spike/p0/src/credentials.rs  DPAPI + 与现版一致的凭证文件格式
spike/p0/src/verify.rs       真实 amount 校验
spike/p0/src/config.rs       配置目录（P0_CONFIG_DIR / portable / APPDATA）
spike/p0/ui/panel.slint      面板（demo 布局的动效化 fork）
spike/p0/ui/primitives.slint / tokens.slint / anchor.slint / slint-tray.slint / app.slint
spike/p0/evidence/           本文引用的 4 张图
```

面板 UI 是 `spike/slint-demo/ui/panel.slint` 的 fork：坐标与 token 全部保留，只把“整卡绝对定位”改成“header + panes cell + footer”，cell 可裁切、可动。P1 抽 `quota-core` 后，P2 再把绝对坐标换成 Slint layout（HANDOFF §6.1 的坑仍在）。
