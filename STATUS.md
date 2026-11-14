# STATUS — Quota Tray

> 快照：2026-11-14 15:23（第六轮）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

**P2 全部完成并已提交**（代码 `26ff363`、文档 `8ab1f86`）：Kimi / DeepSeek 分段 tab 与 DeepSeek 面板落地，两个 provider 都渲染真实数据，`ActiveTab` 持久化，金额一律两位小数。下一步是 **P4**——开工前有三个小决定待拍板（见文末「要你拍板」）。

## 阶段状态

| 阶段 | 状态 |
|---|---|
| P0 设计探索 + 定稿 | 完成 |
| P1 仓库化 / 导入 / 改名 | 完成（`b43b3e7`） |
| P2 视觉 + 高度 + tab | **完成**（`d892eeb`、`7dc7761`、`1e593b0`、`3b02236`、`26ff363`、`8ab1f86`） |
| P3 DeepSeek 后端 | 完成（`13822ce`） |
| P4 设置窗分区 / 托盘合并 / 登录捕获 | 未开始（`--test-login` 仍未实现） |
| P5 打包 / 发布 | 未开始 |

## 本轮交付

**前端**（`26ff363`）

| 文件 | 内容 |
|---|---|
| `index.html` | header 下加 `.seg.provider`（Kimi / DeepSeek，复用设置窗那个组件与 `--seg-ring`）；新 `#pane-deepseek`：余额块 / 模型行容器 / 合计行 / 提示行 |
| `src/main.ts` | 双 provider 渲染、tab 切换（入场淡入 + 同帧改高 + `set_active_tab`）、header 时间戳跟当前 tab、阈值与错误文案 |
| `src/main.css` | `.seg.provider` 的 margin、`.pane.tab-in` 120ms 淡入、DeepSeek 面板全部样式（`.balance` / `.mrows` / `.mrow` / `.mname` 省略 / `.meta` / `.total` / `.hint`） |
| `src/common.ts` | DeepSeek DTO、`ProviderTab`、`fmtMoney` / `fmtTokens` / `fmtHitRate` / `fmtUpdated` / `isLowBalance` / `isLowCacheHit`、`SettingsDto.activeTab` |
| `src/theme.css` | `prefers-reduced-motion` 现在连 animation 一起关 |

**后端**（`26ff363`）

| 文件 | 内容 |
|---|---|
| `src-tauri/src/settings.rs` | `SettingsData.active_tab`（PascalCase `ActiveTab`）、`normalize_tab()`、`load()` 归一化、3 个 serde 测试 |
| `src-tauri/src/lib.rs` | `SettingsDto.active_tab`、`set_active_tab` 命令（只改这一个字段并落盘，不碰 poller） |
| `src-tauri/src/deepseek/mod.rs` | `visible_models` 移到 `fetch_diag` 结尾执行：IPC / 事件 / 缓存里只留面板要画的行 |

**模型名省略**（用户问过，记在这里省得再查）：纯 CSS flex 收缩，无 JS 截字符串——`.mname` 用 `min-width: 0` + `overflow: hidden` + `text-overflow: ellipsis` + `white-space: nowrap`，`.mcost` 用 `flex: none` 保证金额整宽不被挤（`src/main.css:90`），完整名字在 `title` 里（`src/main.ts:147`）。

**金额口径**（`26ff363` + `8ab1f86`）：用户 2026-11-14 决定「一律两位小数」，`fmtMoney` 与 Kimi 的 `fmtYuan` 都已改（`¥3.00` / `¥0.00` / `¥10.00`）。规则同时写进 `demo/panel-demo-e.html`（6 处示例金额 + 文件头说明）、`docs/SPEC.md` / `SPEC_EN.md` §12.5 与两篇的章节表、`PLAN.md` §5.2、`AGENTS.md`、`CHANGELOG.md`。A–D 四版是被否决的历史对照，金额字符串保持原样，**不要拿它们当格式依据**。

## 本轮验证结果

| 项 | 结果 |
|---|---|
| `cargo test` | **21 passed**（原 18 + settings 3） |
| `npx tsc --noEmit` / `npm run build` | 通过 |
| `npx tauri build --no-bundle` | 通过（release 约 7 分钟；`dist/assets/main-BBFc-G_r.js` 与源码一致） |
| 面板高度 vs demo E | **kimi 数据态 386、DS 两模型 459、DS 四模型 601、DS 提示态 321** —— 与 `heights.py` 的 E 列逐项相同；DS 单模型（真实形状数字）388 |
| 连续切换 | 5 次来回：386 / 388 / 386 / 388 / 386 / 388，无漂移；`set_active_tab` 每次都被调用 |
| 真机 `--test-ui` + 持久化 `ActiveTab=deepseek` | 窗口 954×585 物理 = 424×260 CSS（DeepSeek 空态）；点 Kimi → 837（372 CSS，= demo 的 kimi nocred）；点回 DeepSeek → 585；**三次 `bottom=1728 / right=2916` 完全不动** |
| 最终 exe 冒烟 | 837 → 点 DeepSeek → 585，`bottom/right` 不动 |
| `--test-deepseek`（真实账号） | 余额 ¥46.81、本月 633,873,123 tok / ¥27.14073964、命中率 99.55%、`byModel` 只有 1 条（`DeepSeek-V4.1-Flash`），无错误 |
| 视觉对照 | demo E 与真实前端（vite + 桩 IPC）并排：结构、字体、紫 ring、条宽、红色阈值、两位小数全部一致；60 字符官方名省略成 `DeepSeek-V4.1-Reasoner-Pro-Max-Extended…`，金额 `¥1234.56` 一位不少 |

## 验证方法（下一轮直接复用）

- **有数据的渲染**：临时 harness 的生成思路——读 `index.html`，在 `<script type="module" src="/src/main.ts">` 前注入桩掉的 `window.__TAURI_INTERNALS__`（`get_state` / `get_deepseek` / `panel_height` / `set_active_tab`），每次 `panel_height` 写进 `#probe[data-heights]`，`--dump-dom` 即可当高度断言。`npm run dev` 起在 1420，headless Edge 打开 `http://localhost:1420/...`。harness 里要把 `#root` 的 opacity/transform 钉死，否则虚拟时间会把入场淡入冻在半透明。用完即删，不要提交。
- **真机**：`quota-tray.exe --test-ui` 只建窗口**不启动 poller**，所以面板里 DeepSeek 永远是空态、Kimi 永远是 `--`；它 6 秒后自杀，够抓 2–3 张图 + 2 次点击。配置目录用 `QUOTA_TRAY_APP_DATA_DIR` 指一个自己写的 `settings.json`（`--test-ui` 启动时把它拷进临时 root）。
- **kimi-cu 截图**：透明 Tauri 窗口直接 `get_window_state` 会拿到**全黑帧**，先 `activate_window` 再抓就正常。点击坐标是**物理像素**（本机 225%：`954/424 = 2.25`）。托盘 / 任务栏窗口被 CU 排除，点托盘图标这条路走不通。
- **`ActiveTab` 读取**：把 `settings.json` 的 `ActiveTab` 设成 `deepseek` 再跑 `--test-ui`，窗口高度会是 DeepSeek 的 260 CSS 而不是 Kimi 的 372 CSS——高度本身就是「哪个 pane 在显示」的证据。

## 接手必须知道的九件事

1. **源仓库在本机同级目录**：`~/Documents/trae_projects/github/kimi-planbar-tray`（只用 `rust/`）、`~/Documents/trae_projects/github/deepseek-planbar`（`tray-wpf/` 是移植源）。两个原仓库不改不删。
2. **视觉基准只有一个文件**：`demo/panel-demo-e.html`；数值与 token 在 `PLAN.md` §3。A/B/C/D 四版只是对照，不要演进。
3. **本机 Chrome 未安装，只有 Edge**。截图/自动化必须运行时探测（Chrome → Edge），不得写死路径。headless Edge 的视口比 `--window-size` 宽约 54px、矮约 98px。
4. **`core.autocrlf=true`**，仓库有 `.gitattributes` 强制 `eol=lf`；git 会提示 "CRLF will be replaced by LF"，属正常。
5. **提交会过全局 pre-commit 钩子**（gitleaks + 密钥 + 隐私 + 本机路径三层）。文档/代码里不写本机绝对路径，用户目录写 `~`。**`%APPDATA%\QuotaTray\deepseek-credentials.json` 是密文，永远不要 `cat`**，要看只 `grep -c '"dpapi:'`。
6. **改行为先改 `PLAN.md`**，同一提交里完成；`AGENTS.md` 的陷阱清单同步。`docs/SPEC.md` 是继承来的 Kimi 行为规格，与 `PLAN.md` 冲突时 `PLAN.md` 优先（§12.5 金额格式化已被取代）。
7. **调试版 exe 走 vite devUrl**：要看 UI 必须先 `npm run dev`；前端改动热更新、Rust 改动要 `cargo build` + 重启。**release 版才内嵌前端**，验收视觉请用 `npx tauri build --no-bundle` 出来的 exe。
8. **`--test-deepseek` 会写真实文件**：刷新 `deepseek-{endpoints,models}.json`，首次跑会做一次性凭证导入。它**不写**缓存。诊断行走 stderr，stdout 保持可 `| jq`。`--test-ui` 不跑 poller、不读真实缓存，看到的永远是空态。
9. **金额与显示名两个坑别踩回去**：金额按 1e-8 元量子归整（否则会显示 `24.279870719999998`），且**一律两位小数**；展示名的官方名**必须**压过内置静态表。

## 环境现状（2026-11-14 15:23）

- **DeepSeek 全链路可用**：余额 ¥46.81、本月 633,873,123 token、命中率 99.55%，`deepseek-flash` 一个模型（官方名 `DeepSeek-V4.1-Flash`）。账号正在使用中，数值每分钟在变——**不要用「两次实时取数是否相等」当验收**，用 fixtures + 高度/布局断言。
- **Kimi 配额仍返回 `no-token`**（凭证链拿不到可用 token，大概率过期），所以真机 Kimi 面板是 `--` + `Fetch failed`；不是本轮引入的问题。
- **`%APPDATA%\QuotaTray`** 有 credentials / endpoints / models / cache 四个文件，**仍无 `settings.json`**（没点过 Save，也没在真机切过 tab）。
- 旧的 `KimiPlanbarTray-rust.exe` 可能仍在跑，托盘里会出现两个一样的 K 图标；单实例互斥锁 `QuotaTray.SingleInstance` 与两个父版互不干扰。
- 本轮结束时无后台进程残留（vite、`--test-ui` 实例均已结束）。

## 下一步：P4

`PLAN.md` §8 的验收：设置落盘后重启生效；真账号走通「捕获 → 验证 → 保存 → 面板出数」；`--test-login` 通过。建议按风险从低到高：

1. **设置窗分区**（纯增量）：通用 / Kimi 只读信息 / DeepSeek 凭证。设置窗高度已内容自适应（`settings_height`），加分区不用写死尺寸。Kimi 段显示 CLI 版本、凭证来源（token / api-key）、token 是否有效；DeepSeek 段显示两个凭证**是否已配置与来源**（永不含值）、登录按钮、清除按钮。
2. **登录捕获窗**（最大风险，单独攻坚）：`--test-login` 随窗一起做。WebView2 COM 互操作要与 wry 0.39 对齐（`PLAN.md` §9 列为最高风险）；从控制台真实会话抓 userToken → 用一次真实 `amount` 请求验证 → DPAPI 落盘 → 通知面板刷新；顺带把捕获到的非敏感请求模板喂给 `PLAN.md` §6.3 的「从登录窗学习」。
3. **托盘 tooltip 合并**（最小）：现在只有 Kimi（`Quota Tray  5h 31% · week 12%` + ` (fetch failed)`，见 `src-tauri/src/tray.rs`），合并后覆盖两个 provider，失败后缀语义不变（`PLAN.md` §5.3）。

## 要你拍板

1. **tooltip 形态**：单行 `Quota Tray  5h 31% · week 12% · DS ¥46.81`，还是两行（Kimi 一行、DeepSeek 一行，失败时在对应行尾加 ` (fetch failed)`）？建议两行，可读性更好。
2. **DeepSeek 凭证区的兜底**：除了「打开登录窗」，是否保留手动粘贴 token 的入口（旧版是 F12 手抄）？建议保留。
3. **登录窗的失败语义**：抓不到 token 时静默关闭 + 日志，还是窗内留一行提示可重试？建议后者（`PLAN.md` §7 只禁止弹对话框）。
4. **托盘图标**是否换新（当前沿用 kimi 的 K 标）。
5. **GitHub 仓库名 / 是否建 remote**（建议 `quota-tray`）。目前无 remote，历史都在本地。

## 新对话开场建议

> 接手 Quota Tray（当前目录已是 git repo，无 remote）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`，视觉基准是 `demo/panel-demo-e.html`。P1–P3 与 **P2 剩余的分段 tab + DeepSeek 面板**都已完成并提交（`26ff363` 代码、`8ab1f86` 文档；`cargo test` 21 passed、真机点击 585↔837 物理像素、`bottom/right` 不动）。现在做 **P4**：设置窗分区 → 登录捕获窗（`--test-login`）→ 托盘 tooltip 合并。开工前先把 STATUS 末尾「要你拍板」的五条过一遍。
