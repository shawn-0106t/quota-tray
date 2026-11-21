# P5 对齐矩阵与门禁 — 结果

> 结论先行：**P5 的四项都落地了**——`PARITY.md`（66 行矩阵，0 ❌）、token 漂移门禁（`scripts/parity/check_tokens.py` + CI）、截图重生成（与 `main` 逐字节一致）、`ci-slint.yml`（路径过滤的壳门禁）。矩阵写完后抓出并修掉 4 处真实偏差，另补了 1 个关闭路径自检 `--test-close`；P2/P3 的检查全部重跑通过（P4 的登录代码本轮未动，沿用 P4 REPORT 的结果）。

## 1. 验收

| P5 项 | 结果 | 证据 |
|---|---|---|
| `PARITY.md` | **完成**：10 张表、66 行，覆盖数据/面板/窗口/托盘/设置/skills/登录/存储/恢复/门禁；每行带证据，状态只有 ✅ / ⚠️ / N/A，**没有 ❌** | `PARITY.md` §1–§10；⚠️ 的理由在 §11；未现场证明的 7 条在 §12 |
| token 漂移门禁 | **完成**：比对 `src/theme.css` 两主题的 15 个颜色 token 与 `slint/ui/tokens.slint`，外加 10 处 `drop-shadow-*` 的 `0/8/18/0` 几何；缺/多/重复/单边改动都失败 | `scripts/parity/check_tokens.py`；`ci.yml` 的 `Design tokens agree` 步骤 |
| 截图重生成 | **完成**：`demo/ui-demo.html`、`demo/icon-demo.html`、`spike/p2/ref-variants.html` 重新生成；README 两张截图重生成后**字节未变** | `git hash-object` 与 `origin/main` 对比：`src/theme.css`、三个 demo 脚本、`ui-demo.html` 全部相同 |
| CI job | **完成**：`ci-slint.yml` 按路径触发，跑 token 门禁 + `fmt` + `clippy --locked -D warnings` + `test --locked` | `.github/workflows/ci-slint.yml`；本地已用同样的 `--locked` 命令验证；**尚未在 GitHub runner 上跑过**（下一次 push 才会触发） |

## 2. 关键数字

**镜像 `main@940e789`（PR #8，2026-11-20）**

- `--ink-3`：`#00000070` / `#ffffff5c`（浅/深；悬停滚动条拇指 ≥3:1，WCAG 1.4.11）。
- `expires_at`：毫秒戳先除 1000（`quota-core::credentials::unix_seconds` + 单测），否则「有效到 5138 年」。
- 5 小时窗口：`parse_five_hour()` 固定取 `limits[0]`，非对象 `detail` 返回 `None`，单测钉住。
- demo 工具：`heights.py` 默认量发布窗口（`#m-product`，`--demos` 保留冻结历史）；`check_contrast.py` 直接解析 `theme.css`；`shot.sh` 容忍未设置的 `LOCALAPPDATA`。
- 同源文件：`src/theme.css`、`demo/{heights,check_contrast,build_ui_demo,shot,make_icon}.py`、`demo/ui-demo.html` 与 `origin/main` **逐字节相同**。

**token 门禁**

```
15 tokens, 10 drop-shadow declarations, 0 mismatch(es) (theme.css vs tokens.slint)
```

- 抓旧的 `--ink-3`：`--ink-3  #00000070  #ffffff5c  ink3  DIFFERS: slint #0000004d / #ffffff42`（exit 1）。
- 抓投影漂移：把 `primitives.slint` 的 blur 改 20px → `primitives.slint:132: drop-shadow-blur: 20px != 18px`（exit 1）。
- 同时删掉 `Tokens.stage`：Slint 侧无引用（舞台色只属于冻结 demo），门禁把多余 token 记为失败。

**矩阵修掉的 4 处偏差**

1. 面板 Console 链接补 `?from=kfc_overview_topbar`，与 `lib.rs::CONSOLE_URL` 一致。
2. 四个窗口补 OS 标题：`Quota Tray` / `Quota Tray Settings` / `Kimi Skills` / `Quota Tray Menu`。
3. skills 窗只在首次打开居中（`Shell::skills_placed`），用户拖动的位置跨重开保留。
4. 系统关闭（Alt+F4 / `WM_CLOSE`）：Slint 默认隐藏窗口但不更新 Rust 状态。原来 `visible` 不更新会让托盘开合「第一次点只触发隐藏」，`settings_open`/`skills_open` 不更新会让两个弹窗整场会话拒绝重开。四个窗口现在都接 `on_close_requested`。

**`--test-close`（证明第 4 项）**

```
close: panel hidden after WM_CLOSE ok=ok
close: panel shows again ok=ok
close: settings closed by WM_CLOSE ok=ok
close: settings reopens ok=ok
close: skills closed by WM_CLOSE ok=ok
close: skills reopens at the dragged position ok=ok (pos=PhysicalPosition { x: 1026, y: 301 } want=(1026, 301))
close: menu closed by WM_CLOSE ok=ok
close: verdict=PASS
```

## 3. 结构

```
PARITY.md                        # 66 行矩阵（中文，每行带证据/理由）
scripts/parity/check_tokens.py   # theme.css ↔ slint/ui/tokens.slint + 投影几何
.github/workflows/ci-slint.yml   # 壳的路径过滤门禁（fmt / clippy --locked / test --locked / tokens）
slint/src/shell.rs               # wire_close_requests(), skills_placed, CONSOLE_URL 对齐
slint/src/checks.rs              # --test-close（WM_CLOSE 四个窗口 + 拖动位置）
slint/src/win/skills.rs          # 首次打开才居中
slint/ui/{panel,menu,settings,skills}.slint   # OS 标题
```

## 4. 发现

### 4.1 分支比 `main` 少一个提交，而且已经漂移

`origin/main` 的 `940e789`（PR #8）改了 `--ink-3`、`expires_at` 的毫秒归一、`limits[0]` 的单测和三件 demo 工具。本分支的 `--ink-3` 还停在旧值，`quota-core` 也没有毫秒归一。镜像后，两边的 `src/theme.css`、demo 脚本与 `ui-demo.html` 逐字节相同。

### 4.2 `--ink-3` 就是 token 门禁要拦的漂移

`--ink-3` 的唯一消费者是悬停时的滚动条拇指（`slint/ui/settings.slint:495`）。main 为了 WCAG 1.4.11 把它从 30%/26% 提到 44%/36%，Slint 的复刻没有跟上——肉眼几乎看不见，但对比度门禁（`check_contrast.py`）只读 `theme.css`，不会发现 Slint 侧。`check_tokens.py` 就是把这个「两个文件必须一起改」变成 CI 失败。

### 4.3 Slint 的 `CloseRequested` 只隐藏窗口，不碰 Rust 状态

这是矩阵里最严重的一处。Slint 的默认行为是隐藏（不销毁），但它不知道 shell 里的 `visible`/`settings_open`/`skills_open`。症状：

- 面板 Alt+F4 后，托盘左键第一次只会走「隐藏」分支（窗口已经是隐藏的），第二次才能打开；
- 设置/skills Alt+F4 后，`settings_open`/`skills_open` 永远是 `true`，两个弹窗再也打不开，而且面板的失焦抑制永远生效。

Tauri 在 `on_window_event` 里显式处理。Slint 1.18 的 `Window::on_close_requested` 是唯一的钩子；四个窗口都接上，返回 `HideWindow`，在回调里更新标志。`--test-close` 用 `PostMessageW(WM_CLOSE)` 走真实的系统路径（不是直接调 `hide()`）。

### 4.4 skills 的重开位置

Slint 没有「只在创建时居中」的声明式开关；P3 的实现每次 `open` 都调用 `centre()`，把用户拖动的位置丢掉。现版 Tauri 的 `show_skills_window` 只重设尺寸，不重新居中。修法是 `Shell::skills_placed`，与设置窗「高度变化才重新居中」的规则一致。

### 4.5 两处刻意差异被写进文档而不是「修掉」

- 现版每次启动查一次 CLI 版本；新壳每 6 h 复查（长驻实例不会永远显示旧徽章）。这是超集，不影响文案与阈值。
- skills 单行描述统一预留两行（124 行里 11 行多 15.22 px），因为 Slint 量不到换行后的文本高度。严格对齐要改产品端的折叠 YAML 解析，属于产品决定（`HANDOFF.md` §9）。

## 5. 陷阱（给后面接手的人）

1. **`CloseRequested` 必须接**：Slint 默认只隐藏；不接 `on_close_requested`，任何窗口的 Alt+F4 都会让 Rust 状态与窗口状态不一致。新增窗口照抄 `wire_close_requests`。
2. **设计 token 有两份**：`src/theme.css` 与 `slint/ui/tokens.slint`。Slint 全局是 per-component，读不到 CSS 变量；改一边必须改另一边，`check_tokens.py` 会拦。
3. **`--test-close` 用 `PostMessageW(WM_CLOSE)`**，不要直接调 `hide()`：后者绕过 close-requested 路径，证明不了任何东西。
4. **CI 的 Skia 走直连**：runner 访问 GitHub releases 正常，`SKIA_BINARIES_URL` 镜像只是本机网络问题的办法，不要写进 workflow。
5. **`ci-slint.yml` 按路径过滤**：`slint/**`、`quota-core/**`、`src/theme.css`、`src-tauri/icons/**`、`scripts/parity/**`。`spike/` 的文档/证据不是构建输入，不触发。
6. **中文 skills 描述能正常渲染**：Skia + fontdb 会回退到系统 CJK 字体（`--shots --variant skills-real` 的截图里中文完整，无豆腐块）。

## 6. 回归（本轮重跑）

| 检查 | 结果 |
|---|---|
| `--test-heights` + `heights_ref.py --compare` | 17 个变体 Δ=0.00，PASS |
| `--test-corner`（225%） | 485 采样，right/bottom drift **0 px**，PASS |
| `--test-tab-anim` | 30 连点 / 182 采样，drift **0 px**，PASS |
| `--test-blur` | 180 ms（预算 170），PASS |
| `--test-tray-click` | PASS |
| `--test-clamp` | 滚动 -120，帧变化，PASS |
| `--test-active-tab` | PASS |
| `--test-windows` / `--test-dialogs` / `--test-menu` / `--test-skills` / `--test-settings` | 全 PASS |
| `--test-close` | 7 行 `ok`，verdict PASS |
| `slint` `cargo fmt --check` / `clippy --locked -D warnings` / `test --locked` | 6 passed |
| `quota-core` / `src-tauri` 测试 | 48 / 5 passed |
| `check_tokens.py` / `check_contrast.py` / `fmt_parity.py` | 0 mismatch / 18/18 / 45 例全等 |
| `demo/ui-demo.html` + README 截图 | 与 `main` 逐字节相同；README 图字节未变 |

P4 的 `--test-login` / `--test-signin` 本轮没有重跑：登录链路代码没有改动，`spike/p4/REPORT.md` 的结论仍然有效。

## 7. 没证明的

- **`ci-slint.yml` 没在 GitHub runner 上跑过**：本机没有 push 权限动作，第一次推送才会看到真实结果。本地已按 workflow 里的原命令（含 `--locked`）跑过。
- `PARITY.md` §12 的 7 条现场缺口仍然成立：100/150/200% DPI、混合 DPI/多屏、键盘/IME、成功登录（exit 0）的 settle、三种网络失败文案、父进程退出时子进程卡在 WebView2 初始化、托盘 tooltip 悬停视觉。

## 8. 复现

见 `PARITY.md` §13（面板/窗口/登录/门禁的全套命令）。P5 新增的两条：

```bash
PYTHONUTF8=1 python scripts/parity/check_tokens.py
./slint/target/debug/quota-tray-slint.exe --test-close --no-poll --config-dir "$(pwd -W)/.shots/p5/close"
```

## 9. 文件

```
PARITY.md
scripts/parity/check_tokens.py
.github/workflows/ci-slint.yml
slint/src/{shell,checks,main}.rs
slint/src/win/skills.rs
slint/ui/{panel,menu,settings,skills,tokens}.slint
quota-core/src/{credentials,quota}.rs
src/theme.css
demo/{heights,check_contrast,build_ui_demo,shot,make_icon}.py + ui-demo.html
AGENTS.md / CONTRIBUTING.md / CHANGELOG.md / PLAN.md / spike/HANDOFF.md
```
