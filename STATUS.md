# STATUS — Quota Tray

> 快照：2026-11-14 13:30（第四轮）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

P1（基线导入 + 改名）、P2 的视觉与高度、**P3（DeepSeek 后端）全部完成并端到端验过**。下一步是 **P2 剩余的分段 tab + DeepSeek 面板**——现在有真实数据可用了，这正是当初把 tab 顺延到 P3 之后的理由。

## 阶段状态

| 阶段 | 状态 |
|---|---|
| P0 设计探索 + 定稿 | 完成 |
| P1 仓库化 / 导入 / 改名 | 完成（`b43b3e7`） |
| P2 视觉 + 高度 | **视觉与高度完成**（`d892eeb`、`7dc7761`、`1e593b0`、`3b02236`）；剩余 tab，现在可以做 |
| P3 DeepSeek 后端 | **完成**（本轮）：模块、DPAPI 凭证、双 poller、IPC、`--test-deepseek`、13 个测试 |
| P4 设置窗分区 / 托盘合并 / 登录捕获 | 未开始（`--test-login` 仍未实现） |
| P5 打包 / 发布 | 未开始 |

## P3 交付了什么

**模块** `src-tauri/src/deepseek/`（4 个文件，约 1200 行含测试）：

| 文件 | 内容 |
|---|---|
| `mod.rs` | DTO（`DeepseekResult` / `UsageInfo` / `ModelUsage` / `BalanceInfo`）、`fetch()` / `fetch_diag()`、`Diag` 诊断收集、缓存原子读写、`visible_models()`（过滤全零 + 按本月消费降序取前 4） |
| `parse.rs` | 纯解析 + 展示名规则，9 个测试 |
| `credentials.rs` | 凭证链 + DPAPI + 一次性导入，4 个测试 |
| `client.rs` | 余额 / 用量 / 候选端点链 / `/models` 展示名 / 错误码映射 |

**接线的既有文件**：`state.rs`（`Schedule` 结构 + 两个独立调度 + `last_deepseek`）、`polling.rs`（`run_loop(provider)` 单循环驱动两个 provider）、`lib.rs`（`get_deepseek` 命令、`deepseek-updated` 事件、双 provider 的 refresh/save_settings）、`tray.rs`（悬停预取两个 provider）、`panel.rs`（`collapse_after_settings_close`）、`main.rs`（`--test-deepseek`）、`Cargo.toml`（`base64 = "0.22"` + `Win32_Security_Cryptography`）。

**新增数据** `src-tauri/tests/fixtures/{balance_ok,amount_ok,amount_sparse,cost_ok}.json`（从 `deepseek-planbar/tests/fixtures/` 拷入，只作为 `include_str!` 的数据）。

## P3 验证结果（2026-11-14 13:15–13:30）

| 项 | 结果 |
|---|---|
| `cargo test` | **18 passed**（recovery 3 + skills 2 + deepseek 13） |
| 旧版 `--test-fixtures <绝对路径>` | **ALL PASS**（同一套 fixtures，与 Rust 断言互为证明） |
| `--test-deepseek --verbose` | `credential source: imported`；`/api/v0/usage` 命中并记住；`biz_data` 为对象（amount）/ 数组（cost）；`/models` 返回 2 组 id/name |
| **新旧同机逐字段对比** | **完全一致**：`isAvailable=true`、`currency=CNY`、`total=49.52`、`granted=0`、`toppedUp=49.52`、`monthTokens=557673032`、`monthRequests=2451`、`monthCost=24.42687432`、`cacheHitRate=99.53375476259454`、`todayTokens=5352622`、`todayCost=0.45480968`、单模型各项全同 |
| 唯一差异（已批准） | `displayName`：旧版 `deepseek-flash`（不在其静态表 → 原始 id），新版 `DeepSeek-V4.1-Flash`（`/models` 官方名） |
| 真机（用户手动） | `Fetch failed` 红字正常；关设置窗后**面板不收起**（焦点被交还面板，保留分支）；设置窗开着期间点面板不收起、点资源管理器也不收起（`settings_open` 抑制 blur，符合设计） |
| 凭证落盘 | `%APPDATA%\QuotaTray\deepseek-credentials.json`，两个值都是 `dpapi:` 前缀，键名 `v`/`apiKey`/`userToken`/`importedFrom` 明文 |
| 旧凭证文件 | `~/.deepseek-planbar/credentials.json` mtime 仍是 **Aug 10 19:41**，未被写入 |
| 端点缓存 | `%APPDATA%\QuotaTray\deepseek-endpoints.json`、`deepseek-models.json` 已生成 |
| `--test-deepseek` 时间 | 单次约 1.3s（两次平台请求 + 一次 `/models`） |

## 接手必须知道的九件事

1. **源仓库在本机同级目录**：`~/Documents/trae_projects/github/kimi-planbar-tray`（只用 `rust/`）、`~/Documents/trae_projects/github/deepseek-planbar`（`tray-wpf/` 是移植源，`publish/DeepseekPlanbarTray.exe` 是可跑的参考实现）。两个原仓库不改不删。
2. **视觉基准只有一个文件**：`demo/panel-demo-d.html`；数值与 token 在 `PLAN.md` §3。A/B/C 三版只是对照，不要演进。**本轮改了 D 的 err 态文案**（`Update failed` → `Fetch failed`）。
3. **本机 Chrome 未安装，只有 Edge**。截图/自动化必须运行时探测（Chrome → Edge），不得写死路径。headless Edge 的视口比 `--window-size` 宽约 54px、矮约 98px。
4. **`core.autocrlf=true`**，仓库有 `.gitattributes` 强制 `eol=lf`；脚本改回 CRLF 会让 Git Bash `bad interpreter`。
5. **提交会过全局 pre-commit 钩子**（gitleaks + 密钥 + 隐私 + 本机路径三层）。文档/代码里不写本机绝对路径，用户目录写 `~`。**`%APPDATA%\QuotaTray\deepseek-credentials.json` 是密文，永远不要 `cat`**，要看只 `grep -c '"dpapi:'`。
6. **改行为先改 `PLAN.md`**，同一提交里完成；`AGENTS.md` 的陷阱清单同步。`docs/SPEC.md` 是继承来的 Kimi 行为规格，与 `PLAN.md` 冲突时 `PLAN.md` 优先（它还写着 `Update failed`，那是历史规格，不要照抄）。
7. **调试版 exe 走 vite devUrl**：要看 UI 必须先 `npm run dev`；前端改动热更新、Rust 改动要 `cargo build` + 重启；`--test-ui` 若在 vite 未起时跑，窗口会停在 conf 尺寸。
8. **`--test-deepseek` 会写真实文件**：首次跑会做一次性凭证导入（写 `deepseek-credentials.json`），并刷新 `deepseek-{endpoints,models}.json`。它**不写**缓存（缓存只由 poller 写）。诊断行走 stderr，stdout 保持可 `| jq`。
9. **金额与显示名两个坑别踩回去**：金额按 1e-8 元量子归整（否则面板会显示 `24.279870719999998`）；展示名的官方名**必须**压过内置静态表（用户 2026-11-14 决定：以厂商上游最新命名为准），否则改名要等发版。

## 环境现状（2026-11-14 13:30）

- **DeepSeek 全链路可用**：余额 ¥49.52、本月 557,673,032 token、命中率 99.53%，`deepseek-flash` 一个模型。账号正在使用中，数值每分钟都在变——**不要用「两次实时取数是否相等」当验收**，用 fixtures + 旧版 `--test-fixtures` 证明解析等价。
- **`/models` 当前返回 2 组**，`deepseek-flash` 的官方名是 `DeepSeek-V4.1-Flash`。那个名字明显比内置表长，做面板时注意「长名字在行内省略号，不能把金额挤出去」（`PLAN.md` §6.1、`AGENTS.md`）。
- **Kimi 配额仍返回 `no-token`**（凭证链拿不到可用 token，大概率过期）；`--test-update` 正常。
- **`%APPDATA%\QuotaTray` 现已存在**，含 credentials / endpoints / models 三个文件。`settings.json` 仍未生成（没点过 Save）。
- **旧的 `KimiPlanbarTray-rust.exe` 可能仍在跑**，托盘里会出现两个一样的 K 图标；单实例互斥锁 `QuotaTray.SingleInstance` 与两个父版互不干扰。

## 下一步：P2 剩余（分段 tab + DeepSeek 面板）

`PLAN.md` §3.4 有规格，`theme.css` 的 `seg` 类已就绪（设置窗在用）。

1. header 下的分段控件（Kimi / DeepSeek），`PLAN.md` §3.4。
2. 两个 tab 的内容区：Kimi 沿用现有 pane；DeepSeek 用 `get_deepseek` / `deepseek-updated` 的真实数据，渲染 `visible_models()`（已含过滤与 top 4）。**不需要空态面板**。
3. `settings.json` 增加 `ActiveTab`（`SettingsData` + `SettingsDto` + 回填），缺省回落 `kimi`。
4. 高度：先按 `ActiveTab` 定尺寸再显示（`panel_height` 机制已就绪）；验收是连点 tab 30 次不跳位、底边与右边缘不动。
5. `src/common.ts` 需要补 DeepSeek 的 TS 类型（Rust 侧字段名已是 camelCase，直接对应）。
6. 阈值（`PLAN.md` §5.2）在面板落地时实现：命中率 <60% 标红、余额 <¥10 或 `isAvailable=false` 标红。**目前后端没有实现这两个判定函数**，是有意的（避免未使用的死代码）。

## 未决（等你拍板）

1. **tab 活动块 ring**：中性灰对轨道约 1.2–1.3:1，低于 WCAG；可改强调蓝 ring（3.8:1）。当前保留中性。
2. **托盘图标**是否换新（当前沿用 kimi 的 K 标）。
3. **GitHub 仓库名 / 是否建 remote**（建议 `quota-tray`）。目前无 remote，历史都在本地。
4. ~~失败文案~~ → **已定**（`Fetch failed` / ` (fetch failed)`）。
5. ~~关掉设置窗后面板滞留~~ → **已实现并真机验过**（2026-11-14）：`close_settings` 之后 150ms 检查「面板可见且未获焦点」才收起。**真机结果是面板不收起、留在屏上**——设置窗隐藏后 Windows 把焦点交还给了置顶的面板，走的是「已聚焦所以留着」这条保留分支（**因果为推断，没有从代码侧取证**；面板无边框，肉眼也看不出焦点）。用户已接受该行为，**不要再"修"它**。残留价值：设置窗开着期间把焦点点去别的程序、再关设置窗时，面板会正常收起。

## 新对话开场建议

> 接手 Quota Tray（当前目录已是 git repo）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`，视觉基准是 `demo/panel-demo-d.html`。P1、P2 的视觉/高度、**P3 DeepSeek 后端**都已完成并验过（`cargo test` 18 passed、`--test-deepseek` 与旧版逐字段一致）。**现在做 P2 剩余：Kimi/DeepSeek 分段 tab + DeepSeek 面板**，数据从 `get_deepseek` 命令和 `deepseek-updated` 事件取，渲染用 `visible_models()`。
