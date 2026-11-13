# STATUS — Quota Tray

> 快照：2026-11-13（第二轮）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

Phase 1 已落地；**Phase 2 的视觉部分已完成**——四个窗口（主面板 / 设置 / 技能 / 托盘菜单）统一到 D 版（`demo/panel-demo-d.html`），主面板与设置窗改为内容自适应高度。P2 只剩分段 tab + DeepSeek 空态面板，等 P3 的数据接口。

## 已完成

| 阶段 | 产物 |
|---|---|
| P0 | 四版设计探索 + A/B/C/D 并排对比；`demo/panel-demo-d.html`（定稿）；`shot.sh` / `crop_panel.py` / `heights.py` / `check_contrast.py`；`PLAN.md`、`AGENTS.md`；仓库初始化 |
| P1 | 基线导入 + 全量改名 Quota Tray（0.1.0）；`docs/SPEC.md` + `docs/SPEC_EN.md`（首页适用范围表）；`CHANGELOG.md` |
| P2（视觉） | 四窗统一 D 版视觉；主面板 / 设置窗内容自适应高度（`panel_height` / `settings_height`） |

## P2 视觉部分做了什么（供核对）

| 范围 | 结果 |
|---|---|
| `src/theme.css` | 重写为 D token + 共享原语：`--paper` / `--ink-1..3` / `--hair` / `--accent` / `--seg-*` / `--danger`、30/600 数字、10/500 大写微标签、4px 胶囊条、发丝线行、chip、分段控件、ghost 按钮、`.chrome-close`、焦点 ring、`prefers-reduced-motion` |
| 主面板 `index.html` / `main.css` | D 骨架：字标 + 时间戳（`Update failed` 转红）；两列裸读数（WEEKLY / 5-HOUR，数字与 `%` 分离）；Extra usage 行（含月度条）；Kimi Code CLI 行 + Update chip；4 格 ghost footer。**去掉 header logo**（D 版只有字标） |
| 设置窗 `settings.html` / `settings.css` | 主题 3 格 / 刷新间隔 4 格都用分段控件（替代 radio + pill）；自启改为单色开关（38×20 轨道，开启态 `--ink-1` 实心）；Save 为全宽 ghost 按钮；窗口内容自适应（404×351） |
| 技能窗 `skills.html` / `skills.css` | 卡片列表改发丝线行 + 大写 micro-label 分组；summary 行（计数 + ghost Refresh）；细滚动条 |
| 托盘菜单 `menu.css` | 188 宽的 ghost 列表；阴影留白保持 24px（`panel.rs` 的定位公式依赖该值） |
| 后端 | 新增 `panel_height` / `settings_height` 命令；主面板按上报高度 `LogicalSize` 改高并用 `PhysicalPosition` 把**右下角钉在距工作区 12px**，工作区高度夹取；设置窗首次打开或高度变化时重新居中（同高度重复打开只钉尺寸，不重置用户拖动过的位置）；两者都先定尺寸再显示 |
| 行为兼容 | 所有 IPC id、事件、`settings.json` 字段不变；调试版 exe 仍走 Vite devUrl，要看渲染先 `npm run dev` |
| 高度上报时机 | **同步**测量，不走 `requestAnimationFrame`：面板在隐藏窗口里渲染，Chromium 对隐藏页暂停 rAF，用 rAF 会让冷启动先按 520 显示一帧再跳回 |

窗口尺寸：`--test-ui` 实测（无数据占位）主面板 **424×314**、设置 **404×351**、技能 **404×520**。托盘菜单的 188×160 只是 `tauri.conf.json` 的初值——`show_menu` 会按前端上报改高（菜单卡片 172 + 48 = **188×220**）。主面板有真实数据时卡片 277（窗口 333），高度随内容变化，`--test-ui` 那次是占位态。

## 最近一次自检（2026-11-13 第二轮，本机）

| 命令 | 结果 |
|---|---|
| `npx tsc --noEmit` | 通过 |
| `npm run build` | 通过（4 页） |
| `cargo fmt --check` | 干净 |
| `cargo build` | 通过 |
| `cargo test` | 5 passed（recovery ×3、skills ×2） |
| `quota-tray.exe --test-fetch` | live JSON，`error: null` |
| `quota-tray.exe --test-ui` | 四窗 OK，退出码 0；尺寸见上（需 vite 在跑，否则页面不加载、窗口停在 conf 尺寸） |
| `check_contrast.py` | 53/53 PASS |
| 视觉对照 | 四个窗口明暗两套截图与 `demo/panel-demo-d.html` 对照：字标/微标签/数字/发丝线/footer 一致；设置窗与菜单为同一套原语的延伸 |
| 冷启动隐藏态 | 主面板窗口在**首次显示之前**已是内容高度（位置 872,454 = 工作区右下角 12px），证明高度上报不依赖窗口可见 |

## 接手必须知道的六件事

1. **源仓库在本机同级目录**：`~/Documents/trae_projects/github/kimi-planbar-tray`（只用 `rust/`，已导入完毕，后续只作对照）、`~/Documents/trae_projects/github/deepseek-planbar`（只用 `tray-wpf/`，P3/P4 移植用）。两个原仓库保持独立，不改不删。
2. **视觉基准只有一个文件**：`demo/panel-demo-d.html`；数值与 token 在 `PLAN.md` §3。A/B/C 三版只是对照，不要在上面继续改。
3. **本机 Chrome 未安装，只有 Edge**。任何截图/自动化都必须运行时探测（Chrome → Edge → 其他 Chromium），不得写死路径。
4. **`core.autocrlf=true`**，所以仓库有 `.gitattributes` 强制 `eol=lf`。若把 `demo/shot.sh` 之类脚本改回 CRLF，Git Bash 会 `bad interpreter` 直接跑不起来。
5. **提交会过全局 pre-commit 钩子**（gitleaks 8.29.1 + 密钥正则 + 隐私 + 本机路径三层）。文档和代码里不要写本机绝对路径（盘符 + 用户目录的形式会被拦），用户目录一律写 `~`。
6. **改行为先改 `PLAN.md`**，同一提交里完成；`AGENTS.md` 的陷阱清单同步更新。`docs/SPEC.md` 记录的是继承来的 Kimi 行为，与 `PLAN.md` 冲突时 `PLAN.md` 优先。

## 定稿设计速览（细节见 `PLAN.md` §3）

| 维度 | 结论 |
|---|---|
| 结构 | 424 窗口（卡片 368 + 28 阴影留白）· 双行 header（字标 13/500 + 时间戳 11/400，下接全宽分段控件）· 两列裸读数区（无填充/无描边/无圆角，列距 24）· 1px 发丝线分行 · 4 个 ghost 按钮 |
| 字体 | 全系统 sans：`'Segoe UI', system-ui, …`，**零 mono、零 web font**；数字 30/600 + `tabular-nums`；微标签 10/500 大写 `.08em` |
| 颜色 | 暖纸底 `#fbfaf9` / `#181817`；alpha 墨阶（90/60/25%）；**单一 Kimi 蓝**（`#1783ff` / `#1a88ff`，条形用深一档 `#0d6fe8`）；**唯一红**只给故障 |
| 语义 | 蓝 = 用量 · 红 = 故障（余额不足 / token 过期 / Key 失效 / 更新失败 / 解析失败 / 命中率 <60%）· 墨 = 正常（用量百分比永不变红，`Available` 不着色） |
| tab 控件 | 灰底轨道 radius 12 + 等宽格 radius 8 + 活动格 1px 内描边 ring（**无投影**） |
| 高度（demo 实测） | Kimi tab 330 · DeepSeek 2 模型 403 · 4 模型 545（卡片高；窗口高 = +56）。注意 demo 含 58px 分段 tab，应用还没做 tab，当前 Kimi 卡片 277 |

## 下一步

1. **P2 剩余**：分段 tab（Kimi / DeepSeek）+ DeepSeek 空态面板 + `settings.json` 的 `ActiveTab`；验收是连点 tab 30 次不跳位、底边与右边缘不动。tab 需要 P3 的数据源才有真实内容，是否先做空态由你拍板。
2. **P3**：移植 DeepSeek 后端（余额 + 用量 + `/models` 展示名）、双 poller、`tests/fixtures` 解析测试、`--test-deepseek`。
3. **P4**：设置窗增补 Kimi 只读信息与 DeepSeek 凭证分区（窗口已内容自适应，加分区不用改高度常量）、托盘合并 tooltip、登录捕获窗。

## 未决（等你拍板）

1. **tab 活动块的 ring 对比度**：中性灰对轨道只有约 1.2–1.3:1，低于 WCAG 对状态标识的 3:1；可选改成强调蓝 ring（对轨道 3.8:1，代价是标题区多一个蓝元素）。当前保留中性。
2. **托盘图标**是否换新（当前沿用 kimi 的 K 标）。
3. **GitHub 仓库名 / 是否建 remote**（建议 `quota-tray`）。目前无 remote。

## 新对话开场建议

> 接手 Quota Tray（当前目录已是 git repo）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`，视觉基准是 `demo/panel-demo-d.html`。Phase 1 与 Phase 2 的视觉部分已完成，下一步是 P2 剩余的分段 tab（或直接进 P3 DeepSeek 后端），完成后按 `PLAN.md` §8 的验收项自检。
