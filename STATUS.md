# STATUS — Quota Tray

> 快照：2026-11-14 00:05（第三轮）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

P1（基线导入 + 改名）和 P2 的视觉部分（四窗统一 D 版 + 内容自适应高度）都已完成并端到端验过。**下一步是 P3：DeepSeek 后端移植**；tab 顺延到 P3 之后再做（用户 2026-11-14 决定：先让 DeepSeek 有真实数据，tab 一次做对）。

## 阶段状态

| 阶段 | 状态 |
|---|---|
| P0 设计探索 + 定稿 | 完成 |
| P1 仓库化 / 导入 / 改名 | 完成（`b43b3e7`） |
| P2 视觉 + 高度 | **视觉与高度完成**（`d892eeb`、`7dc7761`、`1e593b0`、`3b02236`）；剩余 tab，已顺延到 P3 之后 |
| P3 DeepSeek 后端 | **下一个任务**，未开始 |
| P4 设置窗分区 / 托盘合并 / 登录捕获 | 未开始 |
| P5 打包 / 发布 | 未开始 |

## P2 已完成的部分（供核对）

| 范围 | 结果 |
|---|---|
| `src/theme.css` | D token + 共享原语：`--paper` / `--ink-1..3` / `--hair` / `--accent` / `--seg-*` / `--danger`、30/600 数字、10/500 大写微标签、4px 胶囊条、发丝线行、chip、分段控件、ghost 按钮、开关、焦点 ring、`prefers-reduced-motion` |
| 主面板 | D 骨架：字标 + 时间戳（失败转红）、两列裸读数、Extra usage 行、CLI 行、四格 ghost footer；去掉 header logo |
| 设置窗 | 主题 3 格 / 刷新间隔 4 格 = 分段控件（原生 radio 组）；自启 = 单色开关（38×20，开启态 `--ink-1` 实心）；Save 全宽 ghost；窗口内容自适应 |
| 技能窗 | 发丝线行 + 大写分组标签 + ghost Refresh，细滚动条 |
| 托盘菜单 | ghost 列表；阴影留白保持 24px（`panel.rs` 的定位公式依赖该值） |
| 后端 | `panel_height` / `settings_height` 命令；主面板右下角钉在距工作区 12px（`LogicalSize` 改高 + `PhysicalPosition` 定位 + 工作区夹取）；设置窗内容自适应、改高后居中（同高度重复打开不重置用户拖过的位置）；两者先定尺寸再显示 |
| 高度上报 | **同步**测量，不用 rAF（隐藏窗口不产生帧，冷启动会先按 520 显示再跳） |

窗口尺寸（`--test-ui` 实测，无数据占位）：主面板 **424×314**、设置 **404×351**、技能 **404×520**；托盘菜单 conf 188×160，运行时按上报 **188×220**。有真实数据时主面板卡片 277（窗口 333）。

## kimi-cu 端到端验证（2026-11-14，屏 225% DPI）

| 项 | 结果 |
|---|---|
| 面板渲染 | 深色下字标、0% / 1%、`Resets in 5d 20h` / `2h 11m`、`Not activated`、CLI 2.6.1、四个 ghost 按钮 |
| 右下角锚定 | 窗口 954×738 物理像素位于 (1962,990)；换算卡片右下恰好距工作区角 12 DIP（工作区 2880×1692，卡片 2853/1665） |
| 高度变化 | 314 → 328 DIP，底边不动 |
| 失败语义 | 配额取数失败（`no-token`）时保留上次数据 + 时间戳转红，不弹窗 |
| 设置窗 | 分段控件键盘可用（点 Light 后按 → 选中 Dark）；开关 `role=switch` 可切换；主题/间隔回填正确 |
| 设置窗抑制 | 设置窗有焦点时面板不收起；关掉后仍留，直到下一次失焦 |
| 失焦收起 | 点面板取得焦点 → 激活资源管理器 → 900ms 后面板已隐藏 |
| 未验到 | 300ms 重入守卫（需要人手在 300ms 内连点托盘）、Skills 窗（面板无入口，只在托盘右键菜单）、主题即时生效（要 Save 才会落盘，避免改真实配置） |

验证时进程与 vite 均已关闭；`settings.json` 未被修改（没点 Save）。

## 接手必须知道的七件事

1. **源仓库在本机同级目录**：`~/Documents/trae_projects/github/kimi-planbar-tray`（只用 `rust/`，已导入，仅作对照）、`~/Documents/trae_projects/github/deepseek-planbar`（P3 用 `tray-wpf/` + 根目录 `tests/fixtures/`）。两个原仓库不改不删。
2. **视觉基准只有一个文件**：`demo/panel-demo-d.html`；数值与 token 在 `PLAN.md` §3。A/B/C 三版只是对照。
3. **本机 Chrome 未安装，只有 Edge**。截图/自动化必须运行时探测（Chrome → Edge），不得写死路径。headless Edge 的视口比 `--window-size` 宽约 54px、矮约 98px，截图要按这个补偿。
4. **`core.autocrlf=true`**，仓库有 `.gitattributes` 强制 `eol=lf`；脚本改回 CRLF 会让 Git Bash `bad interpreter`。
5. **提交会过全局 pre-commit 钩子**（gitleaks 8.29.1 + 密钥 + 隐私 + 本机路径三层）。文档/代码里不写本机绝对路径，用户目录写 `~`。
6. **改行为先改 `PLAN.md`**，同一提交里完成；`AGENTS.md` 的陷阱清单同步。`docs/SPEC.md` 是继承来的 Kimi 行为规格，与 `PLAN.md` 冲突时 `PLAN.md` 优先。
7. **调试版 exe 走 vite devUrl**：要看 UI 必须先 `npm run dev`；前端改动热更新、Rust 改动要 `cargo build` + 重启；`--test-ui` 若在 vite 未起时跑，页面不加载、窗口停在 conf 尺寸（会误判成"高度没生效"）。

## 环境现状（2026-11-14）

- **Kimi 配额取数当前返回 `no-token`**：凭证链拿不到可用 token（大概率已过期）。验面板前先跑一次 `kimi` 或重新登录让 CLI 刷新；`--test-update` 正常（`local=2.6.1 latest=2.6.1`）。
- **旧的 `KimiPlanbarTray-rust.exe` 可能仍在跑**，托盘里会出现两个一样的 K 图标；悬停区分（`Quota Tray …` vs `Kimi Planbar Tray …`）。
- 单实例互斥锁 `QuotaTray.SingleInstance` 与两个父版互不干扰，可以同时运行。

## 定稿设计速览（细节见 `PLAN.md` §3）

| 维度 | 结论 |
|---|---|
| 结构 | 424 窗口（卡片 368 + 28 阴影留白）· 双行 header（字标 13/500 + 时间戳 11/400，下接全宽分段控件）· 两列裸读数区（无填充/无描边/无圆角，列距 24）· 1px 发丝线分行 · 4 个 ghost 按钮 |
| 字体 | 全系统 sans：`'Segoe UI', system-ui, …`，零 mono、零 web font；数字 30/600 + `tabular-nums`；微标签 10/500 大写 `.08em` |
| 颜色 | 暖纸底 `#fbfaf9` / `#181817`；alpha 墨阶 90/60/25%；单一 Kimi 蓝 `#1783ff` / `#1a88ff`（条形 `#0d6fe8`）；唯一红只给故障 |
| 语义 | 蓝 = 用量 · 红 = 故障 · 墨 = 正常（用量百分比永不变红，`Available` 不着色）；开关/分段/勾选一律单色，不占蓝 |
| 高度（demo 实测） | Kimi tab 330 · DeepSeek 2 模型 403 · 4 模型 545（卡片高；窗口 = +56）。demo 含 58px tab，应用还没做 tab，当前 Kimi 卡片 277 |

## 下一步：P3 DeepSeek 后端

目标（`PLAN.md` §5.2 / §6 / §8）：把 `deepseek-planbar` 的 WPF 服务移植成 Rust，**不动 UI**（DeepSeek 面板与 tab 属于 P2 剩余项，排在 P3 之后）。

**移植源**（只读参考，不要修改）：

- `~/Documents/trae_projects/github/deepseek-planbar/tray-wpf/Services/DeepseekService.cs`（530 行，余额 + 用量 + 解析 + 缓存）
- `.../tray-wpf/FixtureTests.cs`（118 行，解析断言）
- `.../tests/fixtures/{balance_ok,amount_ok,amount_sparse,cost_ok}.json`

**建议的落地顺序**：

1. `src-tauri/src/deepseek/`：凭证（`DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` 环境变量 → `~/.deepseek-planbar/credentials.json`）、余额（`api.deepseek.com/user/balance`）、用量（`platform.deepseek.com/api/v0/usage/{amount,cost}`，Bearer user token + `x-app-version` + Chrome UA）、解析、`cache.json` 原子写 + 保留上次成功值。
2. 候选端点链（`/api/v0/usage` → `/api/v1/usage`），404/410 逐个试，成功写 `~/.deepseek-planbar/endpoints.json`；本阶段可只做常量 + 候选，登录窗学习留 P4。
3. 展示名：静态表 → `GET api.deepseek.com/models` 的 `name` → 规则派生 → 原始 id；缓存 `models.json` TTL 24h，失败静默回落（`PLAN.md` §6.1）。
4. 双 poller：`polling.rs` 目前只服务 Kimi，需要拆成两个独立 poller，共用间隔与失败语义，但**各存各的 last-good**。
5. IPC：把 DeepSeek 状态加进 `get_state`（或新增 `get_deepseek`），DTO 与 TS 类型同步。
6. `--test-deepseek`（`--verbose` 打印端点、响应顶层键、每模型聚合、`/models` 命中情况），在 `main.rs` 里排在互斥锁之前。
7. 测试：把 4 个 fixture 拷进 `src-tauri/tests/fixtures/`，把 `FixtureTests.cs` 的断言翻成 Rust（`tests/` 集成测试或 `#[cfg(test)]`）。

**P3 验收**（`PLAN.md` §8）：`cargo test` 通过；同机对比旧版 `--test-fetch` 逐字段一致；`--test-deepseek` 通过。注意本机当前没有 DeepSeek 凭证，开工前需确认 `DEEPSEEK_API_KEY` 或 `~/.deepseek-planbar/credentials.json` 可用。

## P3 之后：P2 剩余（tab）

- header 下的分段控件（Kimi / DeepSeek），规格见 `PLAN.md` §3.4（`seg` 类已在 `theme.css` 里，设置窗在用它）。
- 每个 tab 的内容区：Kimi 沿用现有 pane；DeepSeek 用 P3 的真实数据，**不需要空态面板**（这正是把 tab 顺延到 P3 之后的原因）。
- `settings.json` 增加 `ActiveTab`（`SettingsData` + DTO + 回填），缺省回落 `kimi`。
- 高度：先按 `ActiveTab` 定尺寸再显示（`panel_height` 机制已就绪）；验收是连点 tab 30 次不跳位、底边与右边缘不动。

## 未决（等你拍板）

1. **tab 活动块 ring**：中性灰对轨道约 1.2–1.3:1，低于 WCAG；可改强调蓝 ring（3.8:1）。当前保留中性。
2. **托盘图标**是否换新（当前沿用 kimi 的 K 标）。
3. **GitHub 仓库名 / 是否建 remote**（建议 `quota-tray`）。目前无 remote。
4. **失败文案**：header 的红字现在是 `Update failed`，但它表示**配额取数失败**（CLI 版本检查是另一条路）。要不要改成 `Fetch failed` / `Unavailable`。
5. **关掉设置窗后面板滞留**：设置窗开着时抑制了失焦，关掉后没有新的 blur 事件，面板要等下一次失焦才收。要不要在关设置时补一次"已失焦则收起"。

## 新对话开场建议

> 接手 Quota Tray（当前目录已是 git repo）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`，视觉基准是 `demo/panel-demo-d.html`。P1 与 P2 的视觉/高度已完成并端到端验过，**现在做 P3：DeepSeek 后端移植**（源在 `~/Documents/trae_projects/github/deepseek-planbar/tray-wpf/`，fixtures 在同仓库 `tests/fixtures/`）；tab 等 P3 完成后再做。开工前先确认 DeepSeek 凭证可用。
