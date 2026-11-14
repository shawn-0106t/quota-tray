# STATUS — Quota Tray

> 快照：2026-11-14 21:05（**v1.0.0 已 publish**；`main` 干净、CI 绿、告警清零）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

**v1.0.0 已在 `https://github.com/shawn-0106t/quota-tray` 发布**（2026-11-14 13:02 UTC，Latest），行为与 0.1.0 一致——版本号表达的是「发布门禁到位 + 依赖当前」：全量审查、每次推送跑 CI、branch protection、CodeQL、Dependabot、社区文档齐备，依赖升到 vite 8.3.8 / tauri-plugin-opener 2.7.3 / base64 0.23.1。

## 两次发版的记录（2026-11-14）

| 项 | v1.0.0 | v0.1.0 |
|---|---|---|
| tag | `v1.0.0`（annotated）→ `e984c61` | `v0.1.0`（annotated）→ `39f691d` |
| publish | 2026-11-14 13:02 UTC（Latest） | 2026-11-14 12:35 UTC |
| 资产 | `QuotaTray.exe` 7.17 MB、`QuotaTray-v1.0.0.zip` 3.97 MB（92 项 = 91 源文件 + 根目录 exe）、`SHA256SUMS.txt` | 同名同构，版本 0.1.0 |
| 资产校验 | 三件下载到本地 `sha256sum -c` → 两条 **OK** | 同 |
| 发布产物自检 | `--test-ui` 四个窗口 OK；`FileVersion` = 1.0.0；zip 内 `Cargo.toml` = 1.0.0 | `--test-ui` OK、`--test-fetch` `error: null` |
| notes | 取自 CHANGELOG 的 `## [1.0.0] — 2026-11-14` 段（英 + 中） | 取自 0.1.0 段 |
| 发版决策 | 依赖升级本身不单独发版（无用户可见变化），但用户要求以 1.0.0 声明稳定态，于是并入本次 | 首次发布 |

其余基础设施（两次发版共用）：仓库 public、`main` 受 `Build & test (windows-latest)` required check 保护且禁 force-push；secret scanning + push protection、CodeQL default setup（actions / python / js-ts / rust）、Dependabot + security updates、私密漏洞报告全部开启，且 PR 合并后自动删 head branch（`delete_branch_on_merge` 已开启，squash merge 后残留的 Dependabot 分支不再堆积）；社区文档八项齐、`community/profile` health = **100**。

| 事件 | 处置 |
|---|---|
| CodeQL `py/bad-tag-filter`（high，`demo/build_ui_demo.py`） | `aea5218` 加 `re.I`；第二轮换成尾标签 `[^>]*` 的写法（`4e123c2`），`<body>` 也加 `re.I`；`main` 扫描 0 结果，两条告警 **fixed**，生成物字节未变 |
| Dependabot 三个升级 | #1 vite 8.3.8、#2 tauri-plugin-opener 2.7.3、#3 base64 0.23.1（major）全部 squash 合并；合并后重跑 `tsc` / `vite build` / 29 tests（含 DPAPI 往返）全绿 |
| `glib` 依赖告警（GHSA-wrw7-89jp-8q8g） | Windows 目标下不进依赖图（`cargo tree -i glib --target x86_64-pc-windows-msvc` 无输出），当前 Tauri 线无 0.20 可解 → 按 `not_used` 关闭并附证据 |
| 本地验证的一个洞 | 合并后那次 `npm run build` 用的是 `node_modules` 里的旧 vite（lock 已更新但没重装）；已用 `npm ci` + 重建补齐，并跑真实路径验证 |

**没有待办的动作**：release 已发布、PR 已合并、告警已清。

## 发版前的三步（已完成，留档）

| 步 | 内容 | 结果 |
|---|---|---|
| 1 | 去掉 NSIS 安装包，只留便携 exe | `tauri.conf.json` 的 `bundle.targets = []`、`bundle.windows.nsis` 整块删除；`release.yml` 改 `--no-bundle`、资产列表三件、notes 与清单同步；`make_release_zip.py` 只认一个 exe；README / AGENTS / PLAN / CHANGELOG 一并改口 |
| 2 | 对整棵树做一次全量 code review | 只读 `explore` + `~/.agents/skills/code-review`，报 1 Major / 2 Minor / 8 处文档矛盾，**全部修掉**（明细见下节） |
| 3 | 全量测试 | 见「本轮验证结果」；含 CI 那条 `--no-bundle` 路径与便携模式实测 |

另新增 `docs/case-quota-tray-panel.md`：把现在这套 UI 写成 `rationalism-design` skill 的 case 草稿（窄面板高密度这一档，现有 8 个 case 都缺）。正文是英文、按现有 case 的骨架（When to Use / Colors / Type / Layout / Signature / Components / Anti-Examples / Why It Holds），末尾附「并入 skill 时还要做什么」。它是**草稿、不是产品文档**，用户会在新对话里并进 skill。

## 审查发现与处置

| 级 | 发现 | 处置 |
|---|---|---|
| Major | `demo/ui-demo.html` 的舞台 CSS 排在 `theme.css` 之前，被同特异性的 `html, body { background: transparent; overflow: hidden }` 覆盖：评审页不能滚动（半页窗口滚不到）、浅色底丢成纯白、`.win-skills .root { height: 464px }` 是死规则（同名规则被 scoped 注入版压掉），skills 卡被拉成视口高 | 舞台规则改为最后注入（`html, body` + `.win-skills .root`），重新生成 `demo/ui-demo.html` 与两张 README 截图：浅色底由纯白回到 `#e7e7e4`，与深色的 `#0f0f12` 成对；实测 skills 卡从视口高回到 **464px**、settings 606px |
| Minor（复验时补） | 舞台 30px 上内边距恢复后整页下移 60 设备像素，`make_readme_shots.py` 的固定裁切框因此切进第三张卡、顶部还留半行 demo 小标题；脚本只校验画布尺寸，静默通过 | 裁切框按 `getBoundingClientRect()` 实测重算为 `(30, 500, 2682, 1810)`，并加一条「裁切外圈必须是单一背景色」断言：以后布局再漂移会直接报错，不再悄悄切掉标题。新图 1200×593 |
| Minor | 评审页 skills 行写了 `skill-item`，产品是 `skill`（`src/skills.ts:36`），所以发丝线与 9px 内边距全丢 | 改类名并重新生成 |
| Minor | recovery 的 `ReloadThenRebuild` 首次失败只 `window.reload()`，之后不发 `panel-show`；而 `#root` 初始 `opacity: 0`，只有 `panel-show` 会加 `enter` → 面板可见时渲染进程崩一次，就留下一个能点、却什么都没有的窗口 | `main.ts` 在 bootstrap 末尾判断窗口是否已经可见，可见就先画首帧（`getCurrentWindow().isVisible()`，权限在 `core:default` 的 `window:default` 里）；启动时窗口隐藏，首帧淡入不受影响 |
| 文档 | `PLAN.md` 阴影行仍是 `0 12px 40px`（与同表 `--shadow` 行和 `theme.css` 相反）；`AGENTS.md` 写 settings 404x464（实际 404x360）；`docs/SPEC{,_EN}.md` 说 `--test-login` 未实现、发布线“待 P5 定义”；`demo/panel-demo-e.html` 与 `PLAN.md` / `AGENTS.md` 把 E 说成「冻结的 token 来源」（实际 `--shadow` 在 E 之后按实测收紧过）；`demo/make_icon.py` 页脚说托盘嵌 32px（代码是 48px）；`CHANGELOG.md` 写「one blue」（实际紫）与「404x347 today」；`README.md` 写「Nothing is logged」（实际写 `webview.log`，只是不含 token） | 全部按**已实现的 UI** 改掉，方向一律是「代码为准、反写文档」：产品 HTML/CSS/TS 一行没为了迁就 demo 或文档而改。E 的像素保持冻结，只在文件头注释里写明 `--shadow` 这一处刻意例外（PLAN §3.2 与 `theme.css` 为准，别照 E 复原）；`make_icon.py` 的页脚改动同时重新生成了 `demo/icon-demo.html`（三张图标本身字节未变） |

## 本轮验证结果

| 项 | 结果 |
|---|---|
| `cargo fmt --check` / `cargo clippy --all-targets --locked -- -D warnings` | 通过 |
| `cargo test --locked` | **29 passed** |
| `npx tsc --noEmit` / `npm run build` | 通过 |
| `npx tauri build`（本地路径） | exe 7.20 MB，`target/release/bundle/` **不再生成** |
| `npx tauri build --no-bundle`（CI 路径） | 同样只出 exe，无 bundle 目录 |
| `PYTHONUTF8=1 python scripts/release/make_release_zip.py` | `QuotaTray.exe` + `QuotaTray-v0.1.0.zip`（3.8 MB，82 个文件，根目录含便携 exe）+ `SHA256SUMS.txt` 三件 |
| `sha256sum -c SHA256SUMS.txt` | 两条全 OK；zip 内 `CHANGELOG.md` / `AGENTS.md` / `src/main.ts` / `demo/ui-demo.html` 与工作区逐字节一致 |
| `--test-fetch` | `error: null`（Kimi OAuth token 已恢复有效） |
| `--test-deepseek --verbose` | 余额与用量正常；`deepseek-flash` 官方名生效 |
| `--test-update` | `local=2.6.1 latest=2.6.2 updateAvailable=True` |
| `--test-usage` | 两遍扫描请求数一致（3815 / 1453）；`scan1` 218ms、`scan2` 72ms |
| `--test-ui` / `--test-login` | 四个窗口 OK；`verify=ok`、`LoginWindow capture channel OK`、`fetch capture OK` |
| 便携模式 | exe + 空 `portable.dat` 放临时目录跑 `--test-deepseek`，凭证 / 端点 / 模型缓存落在该目录，不写 `%APPDATA%` |
| 开机自启 | HKCU Run 下无 QuotaTray 值，与 `settings.json` 的 `AutoStart=false` 一致（只验了「关」这一侧） |
| 设计评审 | `build_ui_demo.py` / `make_readme_shots.py` / `make_icon.py` / `heights.py` / `check_contrast.py` 全部跑过；高度表未变，对比度 65/65 |
| 旧产物 | 仓库根的 `QuotaTray_0.1.0_x64-setup.exe` 已删除，不再属于任何一步 |

**没验到的**：recovery 的 reload 补救没有用真实的渲染进程崩溃跑过（强制崩溃要 DevTools 或注入故障）。已确认的是类型检查 / 构建通过、`allow-is-visible` 在 ACL 里、启动路径（窗口隐藏时 `isVisible()=false`）行为不变。

## 剩下（都不是发版动作）

1. **Kimi CLI 的 token 在 2026-11-14 20:50 过期了**（`expires_at` 正好落在那一刻），需要人在 CLI 里重新登录一次；在那之前面板的 Kimi 页显示 `Expired — sign in with the Kimi CLI`，`--test-fetch` 报 `no-token`。这是环境状态，不是缺陷。
2. 下次发版的流程：CHANGELOG 加 `## [Unreleased]` 段记录变更 → 定版写日期 → 打 `v<x.y.z>` tag → workflow 开草稿 → 核对资产与 `SHA256SUMS.txt` → publish。触发新版本的条件：用户可见的 bug 修复、安全修复、功能变更。
3. Dependabot 每周会开新 PR；合并走 required check，major 单独评估，合完本地按 `npm ci` + 重建的方式验一遍（别只看 `npm run build`）。

## 接手必须知道的

1. **工作区干净、已全部推送**：`main` = `39f691d`，tag `v0.1.0` 指向同一提交。`docs/case-quota-tray-panel.md`（设计 case 草稿，不是产品文档）也已入库。上一轮的三条交接（主线程建窗 / 窗口程序没有控制台 / Clear 的边界）仍然有效。
   **UI 的单一真源是产品本身**（`index.html` / `settings.html` / `skills.html` / `menu.html` + `src/*.css|ts` + Rust 行为）：`demo/ui-demo.html`、`demo/icon-demo.html`、`docs/screenshot-*.png` 都是从它生成的，文档是照着它反写的。冻结的 A–E 五版 demo 保留历史像素，其中 E 的 `--shadow` 仍写 40px —— 那是刻意例外，`PLAN.md` §3.2 / `src/theme.css` 才是准。
2. **同步命令跑在主线程**：任何 `#[tauri::command]` 里都**不要**直接 `WebviewWindowBuilder::build()`——它会跳主线程并等待，结果就是「窗口出现但一片空白、关不掉」。`open_login` / `close_login` 都已交给 `tauri::async_runtime::spawn`。
3. **reload 与 rebuild 是两条路**：`rebuild` 会等页面加载再调 `show_panel`（所以有 `panel-show`）；`reload` 不会，才需要在 `main.ts` 里按窗口可见性补首帧。以后再动是恢复逻辑，两条路都要想一遍。
4. **评审页的样式顺序是契约**：`demo/build_ui_demo.py` 把 `#root` 换成 `.root`（`body_of()` 去重复 id、`read().replace("#root", ".root")`），所以舞台覆盖必须写成 **`.win-skills .root`**，与 scoped 版同特异性、靠「排在最后」取胜；写成 `#root` 会一条也匹配不到（这个坑已经踩过一次）。产品 CSS 先注入、舞台规则最后注入，挪回前面评审页就又不能滚、浅色底又丢。
5. **设置窗高度是「上限 90% + 卡片内滚动」**：`#root` 自己 `max-height` + `overflow-y: auto`，滚动条 4px 内缩细线；上报 `max(offsetHeight, scrollHeight)` 才不会自我缩小。
6. **阴影必须收在 28px 留白内**：`--shadow: 0 8px 18px`；更大的模糊会被窗口边缘裁成硬边。`PLAN.md` 里那张表已删掉旧的 40px 行。
7. **发版只发便携 exe**：`bundle.targets = []` + `--no-bundle`，资产三件；`make_release_zip.py` 的 `binaries()` 只有一个 exe。别再往回收安装包。
8. 原有几条仍然有效：图标是生成物、托盘嵌 48px、Kimi 数字是本机口径、扫描节奏、`ui-demo.html` 是生成物、登录窗没有 IPC 面。
9. **README 截图有护栏**：`make_readme_shots.py` 的 `CROP` 是按 DOM 实测的固定框，脚本会先断言「裁切外圈是单一背景色」再出图。改舞台 padding、改窗宽、改演示页排版后，它要么自己过、要么直接报错要求重测——不要手工裁图。

## 环境现状（2026-11-14 21:05）

- **Kimi**：**token 已过期**（20:50），需在 CLI 重新登录；`--test-fetch` 目前报 `no-token`，面板显示过期提示。
- **DeepSeek**：API key 与 usage token 都已配置（`--test-login` 报 `credentialSource=file`、`apiKeyConfigured=true`、`verify=ok`）；余额 ¥34.64。
- `%APPDATA%\QuotaTray`：credentials / endpoints / models / cache / kimi-usage / `settings.json`（`Theme=system`、`RefreshMinutes=5`、`AutoStart=false`、`ActiveTab=kimi`）/ `webview.log`。
- 仓库根留三件本地构建的产物（gitignored）：`QuotaTray.exe`（1.0.0）、`QuotaTray-v1.0.0.zip`、`SHA256SUMS.txt`；哈希与 Release 里 runner 构建的那份不同（正常，两台机器/两次构建）。
- **仓库根那个 `QuotaTray.exe` 进程被关掉了**（它锁着产物文件，`make_release_zip.py` 会 `PermissionError`）。要再跑就双击根目录的 exe；再次打包前先退出它。
- git：`main` 干净并与 `origin/main` 同步；两个 tag `v0.1.0`（`39f691d`）与 `v1.0.0`（`e984c61`）。

## 新对话开场建议

> 接手 Quota Tray（`https://github.com/shawn-0106t/quota-tray`，本地目录同名仓库，工作区干净）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`；UI 看 `demo/ui-demo.html`（生成物，别手改），README 截图由 `demo/make_readme_shots.py` 生成。
>
> 状态：**v1.0.0 已发布**（2026-11-14 13:02 UTC，Latest；v0.1.0 留在同一页），三件资产下载校验过、发布 exe 自检通过；CodeQL 与依赖告警清零；Dependabot 三个升级已合并。当前没有待办动作；唯一的环境事项是本机 Kimi CLI token 在 20:50 过期，需要重新登录一次。
>
> 用户已拍板：只发便携 exe（无安装包）；托盘图标保持纯紫双条；设置窗高度维持「工作区 90% 上限 + 卡片内滚动」。
