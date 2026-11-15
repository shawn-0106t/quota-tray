# P2 真实面板 — 结果

> 结论先行：**面板已经接上 `quota-core` 的真实数据，高度由数据算出来而不是写死。** 17 个数据变体的卡片/窗口高度与 Chromium 的 DOM **逐像素相等（Δ = 0.00）**；8 个布局变体的文本带位置与 DOM 差 **≤ 1.00 CSS px**（全部来自字形栅格化，DPR 不同所致）；真实 225% 缩放下窗口 954 px = 424 × 2.25，卡片距工作区角 27 px = 12 × 2.25，10 次切换约 470 个采样的**漂移 0 px**；30 次连点底边漂移 **0 px**；`ActiveTab` 落盘/回读 5 项断言全过；工作区夹取时卡片**真的滚动**（120 px 内容位移、帧内容变化）。
>
> 实现期挖出两个 Slint 的坑（都不是文档里写明的），已写进 HANDOFF §6：**外层 layout 遇到兄弟组件里的 repeater 会静默丢掉后面所有兄弟**；**没有显式 x/y 的子元素会被居中**。

## 1. 验收

| 验收项 | 结果 | 证据 |
|---|---|---|
| 真实数据（Kimi 配额 / Extra usage / 本机 cache+decode / CLI 版本 / DeepSeek 余额+用量） | **PASS**：`quota-core` 的 `quota::fetch`、`deepseek::fetch`、`kimi_usage::scan`、`update::check` 各跑一条后台循环，UI 只读 `Data`；实机快照见 `evidence/panel-kimi-full.png`（本机 token 过期，配额显示 `--`，cache 99.6% / 186.1 tok/s 来自真实会话日志）与 `evidence/panel-ds-4.png`（¥14.14 真实余额） | `slint/src/{app,view}.rs` |
| 1–4 模型行，高度随数据变 | **PASS**：17 个变体（0/1/2/3/4 行、hint、无详情、无 key、失败、cache 有/无、monthly 有/无）与 DOM 高度**全部相等** | `spike/p2/heights_ref.py --compare` |
| 数据变化时高度正确 | **PASS**：长高立即改窗口（底边不动，多出的空间在卡片上方）；收缩等 220 ms 让格子动画落地再改，卡片不会被窗口上沿裁掉 | `sync_height` + `--test-corner`（采样里包含真实 poll 落地引起的高度变化） |
| 三档 DPI 贴角不跳位 | **算术 PASS**：`height::window_pos` / `max_window_h` 的 4 个单测覆盖 8 档 scale（1.0–3.0）× 3 个工作区 × 4 个高度；**现场 225% PASS**：954 px 宽、卡片 gap 27 px、10 次切换漂移 0 px。**100/150/200% 未做真机验证**（需改显示设置，用户选择留人工清单） | `cargo test` + `--test-corner` |
| ActiveTab 持久化 | **PASS**：无文件时读 `kimi`；切到 deepseek 落盘；切回 kimi 落盘；无 `ActiveTab` 的旧文件读 `kimi`；`ActiveTab=deepseek` 回读正确 | `--test-active-tab` |
| 不先做全量 UI 重写 | **PASS**：只做面板；settings/skills/menu 仍是 P3，登录仍走 P4。`src-tauri/` 与 `src/` 一行未改 | `git status` |
| 文本/几何与 DOM 对齐 | **PASS**：8 个布局变体（Kimi full / no-cache / cache-only / extra-monthly，DS 4 行 / 3 行 / hint / no-key）文本带数量一致、位置差 ≤ 1.00 CSS px（浅色与深色各测过 ds-4） | `spike/p2/compare_bands.py` |

## 2. 关键数字

**高度（Slint 算出的 card / window vs Chromium DOM 的 `#root` 高度）**

| 变体 | DOM card | Slint card | Δ | window |
|---|---|---|---|---|
| kimi-full | 504.00 | 504.00 | 0.00 | 560.00 |
| kimi-nocache | 331.50 | 331.50 | 0.00 | 387.50 |
| kimi-cache-only | 447.50 | 447.50 | 0.00 | 503.50 |
| kimi-extra-monthly | 537.00 | 537.00 | 0.00 | 593.00 |
| kimi-not-activated / no-badge | 504.00 | 504.00 | 0.00 | 560.00 |
| ds-4 / 3 / 2 / 1 | 545 / 474 / 403 / 332 | 同左 | 0.00 | 601 / 530 / 459 / 388 |
| ds-hint ×4 / ds-nokey | 265.50 | 265.50 | 0.00 | 321.50 |
| ds-balance-only | 228.00 | 228.00 | 0.00 | 284.00 |
| ds-failed | 244.50 | 244.50 | 0.00 | 300.50 |

**文本带（CSS px，DOM 以 DPR 2 渲染、Slint 以 DPR 2.25 渲染后各自换算）**

- kimi-full：14 / 14 带，最差 **0.78**；kimi-nocache 9 / 9 带，最差 1.00；kimi-cache-only 12 / 12，最差 1.00；kimi-extra-monthly 15 / 15，最差 0.89。
- ds-4：16 / 16 带，最差 **0.89**（浅色）/ 0.89（深色）；ds-3 14 / 14，最差 1.00；ds-hint 8 / 8，最差 0.94；ds-nokey 7 / 7，最差 0.94。
- 残差全部来自字形栅格化：同一串在 2.25× 与 2× 下的整像素取整不同，不是布局漂移（对照 P0 的 0 px 结论：那里两边同 DPR）。

**交互与几何**

- 贴角：scale 2.25、窗口宽 954 = 424 × 2.25、卡片右/下 gap 27.0 = 12 × 2.25、可见采样（每轮 467–472 个）内 **right-drift 0 px / bottom-drift 0 px**。
- 连点 30 次（间隔 40 ms）：底边 `1728..1728`（drift 0）、右边 `2916..2916`、窗口高度恒 1352。
- 失焦 → 隐藏：**180 ms**（预算 170 ms + 60 ms 容差；P0 是 171–174 ms，这里多一个 50 ms 数据泵轮询的抖动）。
- 托盘：左键开 / 再左键关各 1 次 PASS；tooltip 三行 `Quota Tray / Kimi 5h 31% · week 12% / DeepSeek --`（变体数据）。
- 夹取（强制 cap 300）：窗口 300、卡片 244、内容 543（可滚 299）、滚动 120 px 后帧内容变化 → PASS。
- 单测：`slint` 4 个（贴角公式）；`quota-core` 40 个不变。

## 3. 结构

```
slint/                        # 新壳（crate quota-tray-slint，bin quota-tray-slint.exe）
  ui/tokens.slint             # 从 P0 复制（PLAN §3.2）
  ui/primitives.slint         # Txt/Lbl/Note/HintText/Bar/SegCell/Hair + Metrics 常量表
  ui/kimi.slint               # Kimi pane：QuotaBlock / CacheBlock / DecodeRow / ExtraRow / VersionRow
  ui/ds.slint                 # DeepSeek pane：BalanceBlock / ModelsBlock(repeater) / HintBlock / SummaryBlock
  ui/panel.slint              # 窗口：header + tab + cell(动画) + footer + Flickable 夹取滚动
  ui/anchor.slint             # keepalive（1×1 透明、屏幕外）
  src/main.rs                 # 分派、日志（config_dir/slint.log）、panic 日志、单实例
  src/app.rs                  # Data + 4 条后台循环（Kimi / DeepSeek / 本地用量 / CLI 版本）+ 主题轮询
  src/view.rs                 # quota-core 数据 → Slint 属性（字符串全部走 quota_core::format）
  src/shell.rs                # 定位/显隐/切换动效/高度策略/托盘/数据泵
  src/height.rs               # 贴角与夹取的纯函数 + 4 个单测
  src/checks.rs               # 7 个自检模式 + 17 个数据变体
spike/p2/
  REPORT.md                   # 本文
  heights_ref.py              # 高度 oracle（生成 ref-variants.html + heights-ref.json，--compare 对比 exe）
  compare_bands.py            # 文本带对比（DOM @DPR2 vs Slint @live DPR，换算到 CSS px）
  evidence/*.png              # 5 张：Kimi/DS 实机、深色 DS、夹取前后
```

高度只有**一处**来源：`primitives.slint` 的 `Metrics` 常量。panes 用它给每个块定位（绝对 y），`panel.slint` 用同一批常量按同一组可见性条件求和，得出 `natural-kimi` / `natural-ds` 交给 Rust 定窗口。改一个块的高度必须同时改常量——两处不会各写一个数字。

## 4. 行为对齐（与 Tauri 版逐条）

| 行为 | 实现 |
|---|---|
| 启动 2 s 后首拉，之后按 `RefreshMinutes`；失败 30 s 快速重试 | `app.rs` 的两条循环；失败保留 last-good（`fill_missing_from`），DeepSeek 成功即写 `deepseek-cache.json`、启动先读缓存 |
| 两个 provider 各有自己的 last-good 与调度 | 两条独立 task + 一个广播 `Notify`（`notify_waiters`，不是 `notify_one`——后者会饿死另一条） |
| 本机用量：走树 0.1 s，隔分钟只 stat | 60 s 一次，`tick % 2 == 0` 走 `scan()`，否则 `scan_known()`；只有 `signature()` 变化才发给 UI |
| header 时间戳跟随屏幕上的 tab | `stamp-kimi` / `stamp-ds` 两套文本 + `*-bad`；`panel.slint` 里按 `tab-ds` 选，和 panes 一起翻（不是点击时翻） |
| 失败语义 | Kimi 有 error → `Fetch failed`（红）；DeepSeek `no-api-key` → `Not configured`（红），其余错误 → `Fetch failed`；余额/用量各自独立 |
| 阈值 | `is_low_balance`（<¥10 或不可用）/ `is_low_cache_hit`（<60%）只在 `view.rs` 决定颜色 |
| 托盘 tooltip 三行 | `quota_core::format::tray_tooltip`，任一侧刷新都重算 |
| 托盘左键开合、300 ms 防重入、hover 10 s 节流预取 | `shell.rs`，与 P0 相同 |
| 失焦隐藏 170 ms | 同上；settings/skills 打开时抑制是 P3 |
| 单实例 | `QuotaTray.Slint.SingleInstance`——**故意不叫** `QuotaTray.SingleInstance`：现版 Tauri 还占着那个名字，spike 必须能并行跑。新壳成为发布产物时改回（PLAN §7） |
| 窗口不占任务栏、置顶、无边框、禁 DWM 圆角 | 同 P0 |

## 5. 实现期发现的坑（都能复现）

1. **兄弟组件里的 repeater 会吃掉外层 layout 后面的所有兄弟**（最贵的坑）。`DsPane` 的 `VerticalLayout` 里，`ModelsBlock`（内部有 `for row[i] in models`）之后的所有块都不渲染：`SummaryBlock`、"This month …" 那行、甚至一个 20 px 的绿色探针都不出现，而**高度表达式仍然把它们算进去了**（窗口 601、内容只画到 470）。排查方式：临时给被测块加 `background: #ff000060`、在布局首/尾各放一个绿色探针，逐次移动块的位置——只有 repeater 之后的兄弟消失。**修法：panes 不再用 layout**，改成按 `Metrics` 常量算 y 绝对定位（Kimi 5 块、DS 4 块），本报告所有高度/文本带数字都是在修复后测的。
2. **Slint 会居中"没有显式位置"的子元素**。`QuotaBlock` 的两个 `Field` 只写了 `x`，结果整列内容比 DOM 低 **5 px**——正好是 `(115 − 105) / 2`（块高 115、Field 高 105）。写 `y: 0` 后归零。凡是"位置由父级决定"的元素，x/y 都显式写。
3. **`Window::size()` 返回物理像素**（`PhysicalSize`），不是逻辑尺寸：225% 下 300 逻辑的窗口报 675。所有"我认为是逻辑"的高度都要除以 `scale_factor()`。
4. **`Flickable.content-y` 滚动时是负值**（内容偏移），设 `+120` 会被夹回 0；另外 1.18 把 `viewport-width/height` 改名为 `content-width/height`（旧名只告警）。`scroll-y <=> card-scroll.content-y` 两向绑定后，Rust 可以直接驱动它做测试。
5. **Chromium 在分数 DPR 下的布局会变**：同一份 DOM 在 DPR 2 量到 504 px、在 2.25 量到 501 px（`line-height: normal` 按设备像素取整）。所以参考图必须固定 DPR 2，对比时把两边各自换算成 CSS px——直接用 2.25 的截图对比会凭空多出 3 px"布局差"。
6. **参考页的 body 高度为 0 时卡片会被裁掉**：`body { overflow: hidden }` + 绝对定位的 `#root`（top:28）→ `getBoundingClientRect()` 正常、DOM 报告 504 px，**截图却是空白**。参考页加 `html, body { height: 100% }` 才画得出来（这个问题能骗过所有基于 DOM 的检查）。
7. **Slint 的 `Text` 没有 `line-height`**：`.hint` 的 1.5 行高只能用 `height: 16.5px` + 垂直居中表达；换行文案会溢出固定框（现在的 4 条 hint 都是一行，已量）。
8. **`take_snapshot()` 的 buffer 是物理尺寸**：按逻辑宽度存 PNG 会把图像切成斜条（我第一版夹取截图就是这样）。
9. `slint::Timer` 不是 `Clone`（要放进 `Rc`）；`tokio::select!` 需要 tokio 的 `macros` feature；`ui.global::<Tokens>()` 需要 `use slint::ComponentHandle`。
10. **Skia 的预编译包要每次 cargo 调用都带 `SKIA_BINARIES_URL`**：漏一次，build script 会重试 GitHub（curl 35）并回落到源码构建，本机无 LLVM 直接报 `Unable to locate LLVM installation`。

## 6. 复现

```bash
cd ../quota-tray-slint
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'

# 构建 + 单测 + 门禁
(cd slint && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
(cd quota-core && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)

# 高度 oracle（17 个变体；--compare 会跑 exe 并逐行对差）
PYTHONUTF8=1 python spike/p2/heights_ref.py --compare

# 文本带对比（先出一张 2.25× 快照，再与 DPR2 的 DOM 参考换算对比）
./slint/target/debug/quota-tray-slint.exe --hold --no-poll --variant ds-4 --tab ds \
    --theme light --out .shots/p2/ds-4
PYTHONUTF8=1 python spike/p2/compare_bands.py --shot .shots/p2/ds-4/panel.png \
    --variant ds-4 --theme light

# 自检（都跑在隔离的 config 目录里，不碰用户设置）
EXE=./slint/target/debug/quota-tray-slint.exe
$EXE --test-active-tab
$EXE --test-corner    --variant ds-4 --tab ds --config-dir .shots/p2/cfg-corner
$EXE --test-tab-anim  --variant ds-4 --tab ds --config-dir .shots/p2/cfg-anim
$EXE --test-blur      --variant kimi-full --config-dir .shots/p2/cfg-blur
$EXE --test-tray-click --variant kimi-full --config-dir .shots/p2/cfg-tray
$EXE --test-clamp --no-poll --variant ds-4 --tab ds --max-window-h 300 \
    --out .shots/p2/clamp --config-dir .shots/p2/cfg-clamp

# 实机（真实凭证/缓存）
$EXE --hold --seconds 9 --out .shots/p2/live            # 当前 tab
$EXE --hold --tab kimi --seconds 6 --out .shots/p2/live-kimi
```

`--no-poll` 关掉四条后台循环：`--variant` 截图必须显示变体本身，而不是两秒后真实账户把它盖掉（第一次测 `kimi-nocache` 就踩了这个）。

## 7. 本期没做的

- **settings / skills / menu 三个窗口**（P3）：面板的 Settings 按钮只记日志，托盘右键仍是原生 `muda` 菜单（P0 的结论：`tray-icon` 有右键回调，P3 才能做光标处的自定义菜单窗）。
- **DeepSeek 登录链路**（P4）：没有 `--login-host`，没有 `--test-login`；`--test-deepseek` 等 CLI 参数也没搬过来（P2 只做面板；`quota-core` 的 fetch 已经在这条链上跑着）。
- **recovery**：没有 WebView2 就没有 `ProcessFailed` 那套；Slint 窗口自己重建是另一个话题（P5 再谈）。
- **100/150/200% 真机 DPI**：需要改显示缩放（用户选择留人工清单）。贴角公式已有 8 档 scale 的单测；现场只验了本机 225%。
- **混合 DPI / 多屏**、键盘与 IME、托盘 tooltip 的悬停视觉（本会话仍无法合成鼠标输入，P0 的结论未变）。
- **设置窗抑制失焦隐藏**、关闭设置窗后补一次收起判定——都在 P3。
- `save_tab` 只写 `ActiveTab`，其余字段按启动时读到的值原样回写；P3 的设置窗才会改 `Theme`/`RefreshMinutes`/`AutoStart`。

## 8. 文件

```
slint/                                 # 新 crate（14 个文件：5 个 .slint + 6 个 .rs + Cargo.toml/lock + build.rs）
spike/p2/{REPORT.md, heights_ref.py, compare_bands.py, heights-ref.json, ref-variants.html}
spike/p2/evidence/{panel-kimi-full, panel-ds-4, panel-ds-4-dark, clamp-top, clamp-scrolled}.png
spike/HANDOFF.md                       # P2 完成、坑清单、下一阶段
AGENTS.md                              # 布局表 + spike 分支状态 + 新命令
STATUS.md                              # 分支提示更新
```

`spike/p2/ref-variants.html` 与 `heights-ref.json` 是生成的（`heights_ref.py`），与 `spike/slint-demo/ref/` 的约定一致：保留可复算的参考数据。
