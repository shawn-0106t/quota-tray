# P3 设置 / skills / menu — 结果

> 结论先行：**三个窗口都建起来了，并接上了 `quota-core` 的真实状态与设置。** 设置窗是内容尺寸、居中、90% 工作区上限 + 卡片内滚动，**落盘 → 子进程重启读回** 的往返验证通过；skills 窗按需扫描（124 个技能、10 个分组、134 行模型）并证明滚动位；托盘菜单在光标处打开，且**关闭顺序、失焦关闭、每一行动作**都由 `--test-menu` 驱动真实窗口验证。8 组截图与 DOM 的文本带对比（3 个窗口 × 深浅两主题 + 展开态）全部 PASS，文本带最差 **0.28–1.00 CSS px**；两处超过 1 px 的都是**按钮下边框**（1.56 / 1.33 px——抗锯齿在 2.0× 与 2.25× 下的起始行不同，不是布局差）。

## 1. 验收

| 验收项 | 结果 | 证据 |
|---|---|---|
| 设置窗：三档主题 / 四种刷新间隔 / 开机自启 | PASS：单选项与开关都从 `settings.json` 回填，保存后落盘 | `--test-settings` |
| 落盘重启生效 | PASS：`--test-settings` 用**子进程**（`current_exe --print-settings`）在保存前后各读一次文件，`theme=dark interval=10 autostart=true` 两次都对得上 | `--test-settings` |
| 主题立即生效 | PASS：保存后四个窗口各自的 `Tokens.dark` 全部切换（Slint 的 global 是 per-component，必须逐个设） | `--test-settings` |
| 凭证状态只读、不携带值 | PASS：Kimi 的 CLI 版本 / 凭证来源、DeepSeek 的 API key / usage token 只显示状态；种入明文凭证后显示 `Configured`，Clear 后变 `Not configured` 并保留「清除」说明 | `--test-settings` |
| 手工粘贴（验证后保存） | PASS：向真实 `api.deepseek.com` 发一次**无效 key**，窗口显示 `API key: invalid key — not saved`、凭证文件没有该值、状态仍是 `Not configured`、按钮从 `Verifying…` 恢复；在一次验证在途时按 Clear，晚到的结果被 epoch 丢弃且文件仍为空 | `--test-settings` |
| 主题跟随（system / 即时切换） | PASS：Save 后四个窗口的 `Tokens` 全部切到新主题；只带主题变化的消息也能让四个窗口重绘（这是审查抓到的 Major：`Msg::Theme` 没置 `changed`） | `--test-settings` |
| 设置窗尺寸 / 居中 / 上限 | PASS：404 宽、内容高 662、窗口中心 = 工作区中心（1440,846）；强制 `--max-settings-h 300` 时窗口 300、卡片 244、内容 604 → 卡片内滚动 | `--test-settings` |
| skills 按需扫描 + 刷新 | PASS：打开才扫描（`load` 前只显示 `LOADING…`），`--test-skills` 断言 count=124 = `skills::scan()`、summary `124 SKILLS`、134 行（含分组标题）、首行是分组标题、skill 首行 y=17.5；点 Refresh 重扫后仍是 124 | `--test-skills` |
| skills 滚动 | PASS：列表视口 388、内容 8955.6，滚到 -8567.6 后帧内容变化 | `--test-skills` |
| 菜单：光标处定位 + 贴边翻转 | PASS：合成光标 (2790,1665) 时窗口 `2482,1195`，与 `panel.rs` 公式逐 0.2 物理像素一致；可见卡片左上角 (1127.1,555.1) vs 光标推导 (1127.0,555.0) | `--test-menu` |
| 菜单：点击先关菜单再执行 | PASS：Settings / Skills / Open / Refresh 四行依次执行后 `menu-open=false settings-open=false skills-open=false panel-visible=true` | `--test-menu` |
| 菜单：失焦关闭 | PASS：20 ms 内关闭（2 s 上限内） | `--test-menu` |
| 面板显隐语义 | PASS：设置/skills 打开时面板失焦**不**隐藏（`settings_open`/`skills_open` 抑制）；关设置后面板若不在前台则收起；关 skills 后 Windows 把焦点交还面板，面板保持「可见且前台」（不留下「可见但没焦点」的坏状态） | `--test-dialogs` |
| 四个窗口都能构造 | PASS：`--test-windows` 打印 4 个 OK，并验证面板自身的 Settings 按钮能重新打开设置窗 | `--test-windows` |
| 视觉对齐 | PASS：8 组截图 vs DOM 文本带，最差文本带 0.28–1.00 CSS px；浅色展开态 0.61 | `spike/p3/compare_windows.py` |
| 不回归 P0/P1/P2 | PASS：`--test-corner` / `--test-tab-anim` / `--test-blur` / `--test-tray-click` / `--test-clamp` / `--test-active-tab` 全 PASS；17 个高度变体与 DOM 仍 Δ=0.00 | 见 §6 |

## 2. 关键数字

**窗口几何（逻辑 px）**

| 窗口 | 尺寸 | 来源 |
|---|---|---|
| 设置 | 404 × 606（卡片）→ 662 窗口；展开手工块 778 → 834；再加 note → 869（本机 90% 上限 676.8，后两者被夹取） | Slint 布局求和（`settings: size natural=…`） |
| skills | 404 × 520 固定（卡片 348 × 464） | `tauri.conf.json` 同尺寸 |
| menu | 188 × 220（卡片 140 × 172） | 五行 × 32 + 内边距；窗口含 24 px 阴影留白 |
| 面板 | 424 × 560/601…（未变） | P2 |

**文本带（CSS px，DOM 以 DPR 2、Slint 以 2.25 渲染后各自换算）**

| 截图 | 带数 | 最差（全部带） | 最差（文本带） |
|---|---|---|---|
| settings / light | 17 | 0.28 | 0.28 |
| settings 展开 / light | 18 | 0.61 | 0.61 |
| skills / light | 15 | 0.67 | 0.67 |
| menu / light | 5 | 0.28 | 0.28 |
| settings / dark | 17 | 1.56 | **0.33** |
| settings 展开 / dark | 18 | 1.33 | **0.33** |
| skills / dark | 15 | 1.00 | 1.00 |
| menu / dark | 5 | 0.28 | 0.28 |

1.56 与 1.33 px 都落在「Save settings」按钮的**下边框**上：1 px 边框在 2.0× 与 2.25× 下的抗锯齿起始行不同，检测器又把竖边框和下边框并成一条带，带起点随之差一行——所有字形带的偏差仍 ≤ 0.33。

**设置窗状态高度**（卡片 = 内容 + 2 px 边框）

| 状态 | 内容 | 窗口（自然） | 应用 |
|---|---|---|---|
| 默认（手工块收起、无 note） | 604 | 662 | 662 |
| env note 显示（+35） | 639 | 697 | 676.8（夹取） |
| 手工块展开（+172） | 776 | 834 | 676.8（夹取） |
| 展开 + 验证 note（+207） | 811 | 869 | 676.8（夹取） |

DOM 侧同一状态的卡片是 606 / 641 / 778 / 799：最后一行差在 note——产品里「Verified and saved.」只有一行（+21），本实现预留两行（+35，见 §5.7 的同类取舍）。

**skills**：124 个技能、10 个分组（Agents / Kimi Code / 8 个 Plugin）→ 134 行；内容 8955.6、视口 388、可滚 8567.6。

**菜单**：窗口 2482,1195 → 2905,1690（物理）= 188 × 220 逻辑；卡片左上 (1127.1, 555.1)，光标 (1240, 740) → 推导 (1127.0, 555.0)；翻转分支触发，卡片底边在光标上方 13 px。

## 3. 结构

```
slint/ui/
  settings.slint          # 设置窗：TitleBar + 主题/间隔两个 seg + 自启开关
                          #  + Kimi/DeepSeek 两段只读状态 + Sign in/Clear
                          #  + 手工粘贴（未展开时高度不占位）+ Save
  skills.slint            # skills 窗：TitleBar + summary + Flickable 列表 + 6px 滚动条
  menu.slint              # 托盘菜单：5 × 32px 行
  primitives.slint        # + CardBg / Btn / Seg3 / Seg4 / ChromeClose / TitleBar
slint/src/
  win/settings.rs         # 开/关、回填、保存（落盘→自启→主题→重排期）、Clear、手工粘贴验证
  win/skills.rs           # 按需扫描、行模型（分组标题 + skill 行，y 由 Rust 前缀和算）
  win/menu.rs             # 光标处定位（逐字移植 panel.rs 公式）+ 行动作路由
  checks.rs               # 新增 --test-windows/-settings/-skills/-menu/-dialogs/--shots/--print-skills
  app.rs                  # 刷新间隔与主题改为共享单元；新增 Msg::Skills / CredentialChecked
  shell.rs                # 四窗口 + 面板失焦抑制 + 菜单右键 + 关闭后的收起判定
spike/p3/
  ref_windows.py          # 三个参考页 + rect 基准（真实 skills 列表由 --print-skills 注入）
  compare_windows.py      # 截图 vs DOM 的文本带对比（两主题 × 三个窗口 × 展开态）
  rects-windows.json      # 参考测量的落盘结果
  evidence/               # 7 张：设置（浅/深/展开）、skills（浅/深）、菜单、设置窗 50% 叠加。
                          # skills 截图默认用固定 6 项列表（`start_shots` 内建，不需要传参）：
                          # 本机真实技能清单属于个人配置，不入库。
```

窗口模型：**一个 Slint 事件循环里 4 个 `Window` 组件**（+ 屏幕外的 1×1 `Anchor` keepalive）。设置与 skills 复用同一对开/关函数：打开时先置 `settings_open` 再 `show()`/`focus()`，因此面板自己的失焦已经被抑制；关闭时隐藏窗口、清标志，150 ms 后若面板仍在且不在前台则收起面板。

## 4. 行为对齐（与 Tauri 版逐条）

| 行为 | 实现 |
|---|---|
| 每次打开回填设置 | `apply_controls()`：theme/interval/autostart + 凭证状态；手工块与输入内容保持上次状态（Tauri 的隐藏窗也是复用的） |
| Save：保存→自启→主题→重排期→关窗 | `save_from_ui()`；刷新间隔改成共享单元，`notify_waiters` 让两个轮询立刻改用新周期；主题同时写 `Tokens` 的四个实例 |
| Clear 不被登录回写 | `cred_epoch`（`AtomicU64`）在 Clear 与每次提交验证时自增；验证任务保存前比对 epoch，晚了就丢弃结果 |
| 手工粘贴先验证后保存 | `fetch_balance` / `verify_user_token` 成功才 `credentials::save`；失败按 PLAN §5.2 码转文本，输入框保留 |
| 凭证值永不出现 | 设置窗只收到 `Configured / Not configured / Unreadable — re-enter` 与过期时间；`--test-settings` 只断言这些字符串 |
| skills 懒加载 | 窗口打开才 `--print-skills` 同一条扫描；结果经 `Msg::Skills` 回到 UI 线程，窗口隐藏时也更新缓存 |
| skills 分组与排序 | `quota-core::skills::scan()` 已按 source/name 排好；分组标题首字母大写（CSS `text-transform` 在 Slint 里不存在，模型里直接存大写） |
| 菜单在光标处 | `win/menu.rs::place` 是 `panel.rs::show_menu` 的逐字移植（含 legacy 150/5 的补偿），单元测试钉住三个分支 |
| 菜单动作 | 先 `hide()` 再执行：open=切换面板、refresh=广播、settings/skills=开窗、quit=退出 |
| 面板失焦抑制 | `on_blur` 里 `settings_open || skills_open` 直接返回；关闭窗口后 150 ms 复核前台 |
| 四个窗口的性质 | 全部 no-frame / 透明 / always-on-top / skip-taskbar / 不可缩放，DWM 圆角关闭 |
| 标题栏拖动 | `winit::Window::drag_window()`（与 `start_dragging` 同一条系统路径） |

## 5. 实现期发现的坑（都能复现）

1. **`windows` 这个名字不能用**：`mod windows` 会把 `use windows::Win32::…` 解析成自己的模块，整个 win32 层报「could not find Win32」。模块改名 `win`。
2. **Slint 的 global 是 per-component**：`Tokens` 在每个窗口里各有一份实例，`ui.global::<Tokens>().set_dark()` 只影响那个窗口。切主题要四个实例都设；`--test-settings` 断言了四个都切。
3. **`callback close()` 会与 `Window` 内建的 `close()` 重名**：构建只警告，但名字被遮蔽后容易误读。改用 `dismiss`。
4. **`TextInput` 没有 placeholder 属性**：占位文字只能叠一个 `visible: text == ""` 的 `Text`。
5. **U+2715（✕）在 Segoe UI 里没有字形**：Skia 画成豆腐块，WebView2 靠字体回退才正常。关闭按钮改成 7 × 7 的两条 `Path` 线。
6. **`Flickable` 不会自己量内容**：`content-height` 必须由外部给（Slint 的 `content-width/height` 默认 0）。skills 的行 y 与总高只能在 Rust 里算前缀和。
7. **skills 的一行描述**：CSS 用 `-webkit-line-clamp: 2` 收两行，但真正的单行行高 54.72、两行 69.94。Slint 量不到换行后的文本高度，本实现**统一预留两行**：124 个技能里 11 个描述只有一行（多是折叠 YAML 留下的裸 `>` / `|`），这 11 行会多 15.22 px 空白；其余 113 行与 DOM 完全一致。行高固定也让滚动范围稳定。
8. **单次定时器会排在窗口创建后面**：`show()` 在首次创建窗口时会阻塞 100–200 ms，把同一个 tick 里排好的后续定时器整体推迟。`--test-menu` 的时间轴因此按「每次动作后留 400–700 ms」排。
9. **`--config-dir` 必须是绝对路径**：`quota-core::settings::config_dir` 只在 `QUOTA_TRAY_APP_DATA_DIR` 是绝对路径时采用它，相对路径会被忽略并落回真实 `%APPDATA%`（日志会打一行提示）。所有自检命令都用 `$(pwd -W)` 转出来的绝对路径。
10. **`--print-settings` / `--print-skills` 子进程是最便宜的「重启」与「参考数据」证明**：同一个 exe 换一个 config dir 再跑一次，比在进程内 `settings::load()` 更能证明「落盘 → 重启生效」；skills 的参考页也靠它拿到与窗口完全相同的列表。
11. **`AtomicU64::fetch_add` 返回旧值**：epoch 守卫必须用 `fetch_add(1, …) + 1` 当本次世代，否则「本世代 == 当前」恒不成立——表现是手动粘贴永远被当成「被更新的一次请求取代」：一次都不保存、原因不显示、按钮永久停在 `Verifying…`。只读审查抓到的 Critical，`--test-settings` 现在覆盖「无效 key 不落盘 + 按钮恢复」与「Clear 作废在途结果」。
12. **只带一个字段的消息批次也要置 `changed`**：`Msg::Theme` 若不置，`apply_theme` 整块被跳过；面板可能在下一次轮询时顺带换色，另外三个窗口永远不跟随（审查抓到的 Major）。`--test-settings` 直接投一条 `Msg::Theme` 验证。
13. **关闭子窗后焦点不一定会回到面板**：Windows 通常会把焦点还给上一个前台窗口（所以关 skills 后面板保持可见且前台），但只在面板原本就是上一个前台窗口时如此。两个分支都合法，判定条件是「不可见 或 前台」，不是「必须不可见」。

## 6. 复现

```bash
cd ../quota-tray-slint
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'
ABS="$(pwd -W)/.shots/p3"

# 构建 + 门禁
(cd slint && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
(cd quota-core && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)

EXE=./slint/target/debug/quota-tray-slint.exe

# 四个窗口 / 设置 / skills / 菜单 / 面板与对话框的显隐
$EXE --test-windows   --no-poll --config-dir "$ABS/cfg-windows"
$EXE --test-settings  --no-poll --config-dir "$ABS/cfg-settings" --max-settings-h 300
$EXE --test-skills    --no-poll --config-dir "$ABS/cfg-skills"
$EXE --test-menu      --no-poll --config-dir "$ABS/cfg-menu"
$EXE --test-dialogs   --no-poll --config-dir "$ABS/cfg-dialogs"

# 截图（两主题）。入库证据默认用固定 skills 列表；
# 与 DOM 对比必须用 `--variant skills-real`（参考页里画的是真实列表）。
$EXE --shots --no-poll --theme light --out .shots/p3/light --config-dir "$ABS/cfg-shots"   # 入库证据（skills 固定列表）
$EXE --shots --no-poll --theme dark  --out .shots/p3/dark  --config-dir "$ABS/cfg-shots"
$EXE --shots --no-poll --variant skills-real --theme light --out .shots/p3/real-light --config-dir "$ABS/cfg-shots"
$EXE --shots --no-poll --variant skills-real --theme dark  --out .shots/p3/real-dark  --config-dir "$ABS/cfg-shots"
PYTHONUTF8=1 python spike/p3/ref_windows.py                  # 参考页 + rect 基准（含真实 skills）
PYTHONUTF8=1 python spike/p3/compare_windows.py --shot .shots/p3/real-light/settings.png --window settings --theme light
PYTHONUTF8=1 python spike/p3/compare_windows.py --shot .shots/p3/real-light/settings-manual.png \
    --window settings --state manual --theme light
PYTHONUTF8=1 python spike/p3/compare_windows.py --shot .shots/p3/real-dark/skills.png --window skills --theme dark
PYTHONUTF8=1 python spike/p3/compare_windows.py --shot .shots/p3/real-light/menu.png --window menu --theme light

# P2 回归
PYTHONUTF8=1 python spike/p2/heights_ref.py --compare
$EXE --test-corner --variant ds-4 --tab ds --config-dir "$ABS/reg/corner"
$EXE --test-tab-anim --variant ds-4 --tab ds --config-dir "$ABS/reg/anim"
$EXE --test-blur --variant kimi-full --config-dir "$ABS/reg/blur"
$EXE --test-tray-click --variant kimi-full --config-dir "$ABS/reg/tray"
$EXE --test-clamp --no-poll --variant ds-4 --tab ds --max-window-h 300 --out .shots/p3/clamp --config-dir "$ABS/reg/clamp"
$EXE --test-active-tab
```

## 7. 本期没做的 / 已知偏差

- **DeepSeek 登录窗**（P4）：设置窗的「Sign in…」目前只把说明写进 note（`The sign-in window arrives in P4…`），点了不会有窗口。Clear 的 epoch 机制已经就位，P4 直接接上。
- **手工粘贴只用无效 key 打过真实接口**：`--test-settings` 走完整链路（真实 HTTPS、真实 401、不写文件、按钮恢复、Clear 作废在途结果），但没有用**有效** key 的成功路径——那需要真实凭证，留给 P4 的登录自检。
- **skills 行高固定两行**：11/124 行会多 15.22 px 空白（§5.7）。P5 若要严格对齐，得引入文本测量或改产品端解析折叠 YAML（`description: >` 现在显示成裸 `>`，两个壳都一样）。
- **设置窗的滚动条**：画在卡片右侧 4 px 内栏，但内容宽度不随滚动条收窄（Chromium 会把内容区缩 8 px）。视觉上只差在「滚动时右侧内容是否左移 8 px」。
- **菜单的键盘导航 / 焦点环**：没做（产品也没有）。
- **混合 DPI / 多屏**：菜单用光标所在显示器的 DPI 换算（移植了 `GetDpiForMonitor`），但只在单屏 225% 上实测。
- **P3 没有恢复逻辑**（没有 WebView2，窗口本身就是 Slint 的）；renderer 崩溃/驱动问题不在本期范围。

## 8. 文件

```
slint/ui/{settings,skills,menu}.slint         # 三个新窗口
slint/ui/primitives.slint                     # + CardBg / Btn / Seg3 / Seg4 / ChromeClose / TitleBar
slint/ui/app.slint                            # 导出四个窗口 + SkillRow
slint/src/win/{mod,settings,skills,menu}.rs   # 窗口行为
slint/src/{app,shell,view,checks,main}.rs     # 共享单元、四窗口编排、新自检
slint/Cargo.toml                              # 无新依赖
spike/p3/{REPORT.md,ref_windows.py,compare_windows.py,rects-windows.json,evidence/*.png,.gitignore}
spike/HANDOFF.md                              # P3 完成、坑、下一阶段
AGENTS.md                                     # 分支状态 + 布局/命令
STATUS.md                                     # 分支提示
```

`spike/p3/ref-*.html` 是生成物且包含本机 skills 列表，已加进 `spike/p3/.gitignore`，只提交 `rects-windows.json`（纯几何）。
