# STATUS — Quota Tray

> 快照：2026-11-15 15:30（**v1.0.1 已 publish 并标记 Latest**；三项资产下载后 `sha256sum -c` 通过；工作区干净）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

> **本分支（`spike/slint-demo`）是 Slint 调研线，不是下面这份 Tauri 主线的快照。**
> 新对话先读 `spike/HANDOFF.md`（现状 / 决策 / 坑 / 阶段），实测数字在 `spike/slint-demo/REPORT.md`（静态视觉）、`spike/p0/REPORT.md`（P0 交互）、`spike/p1/REPORT.md`（P1 `quota-core` 抽取）、`spike/p2/REPORT.md`（P2 真实面板）、`spike/p3/REPORT.md`（P3 三个窗口）与 `spike/p4/REPORT.md`（P4 登录链路）。P0–P4 已完成：新壳在 `slint/`（crate `quota-tray-slint`），面板接 `quota-core` 真实数据（17 个变体与 DOM 逐像素相等）、`ActiveTab` 持久化、225% 贴角漂移 0 px；P3 补齐设置 / skills / 托盘菜单三个窗口（设置窗内容尺寸 + 居中 + 90% 上限并在卡片内滚动，保存后**子进程重启读回**确认落盘生效、主题即时切到四个窗口；skills 按需扫描 124 技能 / 134 行并可滚动；托盘右键在光标处开自定义菜单，动作先关菜单再执行）；**P4 把 DeepSeek 登录窗做成子进程**（`--login-host`，tao + wry/WebView2），`--test-login` 证明窗口 / 取消导航通道 / `fetch` 拦截 / 拒收坏候选（凭证文件逐字节不变），`--test-signin` 证明「用户关窗 → 设置窗还原」与「Clear → 杀子进程 + 不回写」。三个窗口的截图与 DOM 文本带最差 1.00 CSS px。下一步 P5（对齐门禁）。`src-tauri` 依赖仓库根的 `quota-core/`：纯逻辑都在那边，`src-tauri` 只剩窗口/托盘/登录壳；发布产物与 v1.0.1 行为不变。
>
> **P4 顺带修了一个 `quota-core` 的真缺陷**：DeepSeek 对无效 token 回 **HTTP 200** + `{"code":40003,…}`，`verify_user_token` 原来只看状态码，等于「验证后保存」从不拒绝任何候选。现在读 body（`token_accepted`，只认非零 `code`）、被拒时继续走下一个候选 base，并有单测。**这条修复也已经单独开了 PR** [#4](https://github.com/shawn-0106t/quota-tray/pull/4)（`fix/verify-before-save`，从 `main` 切出；`main` 上没有 `quota-core/`，所以是移植到 `src-tauri/src/deepseek/client.rs` 的同一份修复 + 单测）。合并后 v1.0.1 的旧行为才会消失；打不打 `v1.0.2` 仍待拍板（见 `spike/HANDOFF.md` §9）。

## 2026-11-15 已定的发布动作（记录）

1. ~~等 PR [#4](https://github.com/shawn-0106t/quota-tray/pull/4) 的 CodeQL 转绿~~ —— **已完成**（CodeQL / gitleaks / `Build & test` 全绿）。
2. ~~squash 合并进 `main`~~ —— **已完成**，`5d95f1f`。
3. ~~五个版本载体升到 1.0.2 + CHANGELOG 定版~~ —— **已完成**，`f92748f`（`chore(release): v1.0.2`，已 push）；`check_versions.py` 六项全 ok，`tsc` / `vite build` / `cargo fmt+clippy+test --locked`（30 passed）全绿。
4. 只剩一步：打 `v1.0.2` tag 才会触发 release workflow（草稿 Release，发布仍手动）——**这一步等用户点头**。
5. 发版前又对 `main` 做了一轮只读审查（2 Major / 10 Minor），全部处置为 `c7f0594` + `96c8a3c`，并镜像回本分支；修复后 `--test-login` 在真机打印 `verify=ok` + 两项捕获 OK（明细见 `spike/p4/REPORT.md` §4.4）。
6. 随后对**增量**再做一轮审查（1 Major / 6 Minor），处置为 `b016576` + `4ba92ee`，同样镜像回本分支（`quota-core` + `src-tauri` 的 `login.rs` / `state.rs`）。已知缺口：`verify_and_save` 的决策表没有测试，只测了 `transient()`——留给 P5（见 `spike/p4/REPORT.md` §4.5）。

注意：本地 `main` 在 `../kimi-deepseek-quota-dashboard`（同一仓库的另一个 worktree），合并后在那里 `git pull --ff-only` 再升版本；不要在 worktree 里 `git switch main`。

## 一句话

**v1.0.1 已在 `https://github.com/shawn-0106t/quota-tray` 发布**（2026-11-15 07:30 UTC，Latest）：tab 切换按 Windows 原生 motion 重做（§3.6），并新增「五个版本载体必须一致」的发版门禁。上一次是 v1.0.0（2026-11-14 13:02 UTC），行为与 0.1.0 一致——那一版表达的是「发布门禁到位 + 依赖当前」。

## 2026-11-15 tab 切换动效重做（按 Windows 原生 motion）

- **起因**：切 tab 时卡片顶边瞬移 73–215px（`demo/heights.py` 实测 Kimi 330 / DeepSeek 单模型 403 / 四模型 545），再叠 120ms linear 淡入、旧 pane 直接 `hidden`，读起来像闪一下。
- **做法**（契约见 `PLAN.md` §3.6，已重写）：窗口只在切换开始时**一次性**放大到两屏较高者（不可见——卡片在窗口内 bottom-anchored，多出来的空间在卡片上方且透明），`.panes` 的高度在 WebView 内按 WinUI 数值动：长高 200ms / 收缩 167ms、进场 `cubic-bezier(0,0,0,1)`；两个 pane 各 167ms 交叉淡入淡出（旧内容走退场曲线，盖到格子收完再消失——不用 WinUI 的 83ms，否则中段会露空白纸色）；结束时窗口缩回新高度、卡片不动。footer 与卡片底边一起钉住，移动的只有卡片顶边（header + tab 条）。
- **结构改动**：`index.html` 两个 pane 外包一层 `.panes`、`<body class="panel">`；`body.panel > #root` 改成窗口内 bottom-anchored；旧 pane 切换时提离文档流（`.pane-leaving`），所以测到的高度永远只由当前 tab 决定。`main.ts` 里的 `MOTION` 是唯一一处可调的时长/曲线，`prefers-reduced-motion` 走瞬时路径。
- **审查**（explore + code-review skill，只读）：1 Major / 6 Minor，全部处置——(1) `resetTabMotion()` 现在自增 `tabRun`，面板收起后停在 rAF 上的 coroutine 醒来只会看到过期世代号，不会再重放一次旧切换；(2) 动画起跑前重读实时高度，resize 期间的 poll tick 会触发一次 retarget，不再在收尾时「啪」一下补齐；(3) 底边锚定把裁切移到顶部，因此内容高于实时视口时给卡片加 `.clamped`（`max-height` + 内部滚动），避免工作区夹取时 header/tab 被裁掉；同时 `reportHeight()` 改报 `scrollHeight`（内容高）而不是被夹过的 `offsetHeight`；(4) 收缩改用退场曲线（原来两个方向都用进场曲线，首帧就走掉 45%）；(5) 「旧 pane 画在新 pane 之上」确认是刻意行为并写进契约；(6) demo 生成器显式剥掉 `body.panel > .root` 规则（demo 的 `<body>` 没有 `panel` 类，这些选择器本来就不命中，留着只是死规则）；(7) 三处文档与代码不一致的说法按实际行为改写。
- **二轮（delta）复查**：只审 7 处修复的 diff，又报 1 Major / 7 Minor，全部处置——**Major**：`.clamped` 在切换路径上完全没被维护，工作区夹取时整段长高动画会把 header + tab 顶出窗口上沿（修：`playTabSwitch` 起跑前按「实际视口 < 目标高度」再判一次）；**Minor**：`.clamped` 用 resize 前的视口判定，每次内容长高会闪一帧滚动条（修：挪到 resize 落地后的 rAF 里判）、460ms 救援定时器可能砍掉正在跑的动画（修：加 `tabStarted` 闸门，只救「动画没起来」的 coroutine）、滚动条 CSS 重复两遍、demo 剥规则的注释理由不成立且正则会跨注释匹配（修：`[^{}*/]*` + 改写理由）、AGENTS/CHANGELOG 的 "pre-sized ONCE" 与 retarget 矛盾、AGENTS 把设置窗的 `max(offsetHeight, scrollHeight)` 公式挂在面板上、STATUS 的验证指针指空。另确认 7 处修复中 `tabRun`、`cardHeight()` 算术、`scrollHeight + 2` 稳态等值、CSS 夹取几何、曲线/`fill: forwards` 清理都成立。
- **三轮（全量复核，逐条按前两轮对照）**：**0 Critical / 0 Major / 7 Minor**，结论「可以发 1.0.1」。已处置 7 条——表头时间戳比内容早 1–4 帧（`renderStamp()` 挪到 pane 翻转处，radio 仍即时反馈）、`switchTab` 的错误兜底缺世代号（`run` 提到调用方再比对）、`chrome` 与 `end` 量的是两个盒模型（`.panes > .pane { display: flow-root }`，今天零布局变化）、`reportHeight` 的 `.clamped` 判定拿旧 `natural`（rAF 里重测）、`package-lock.json` 仍写 1.0.0（已升）、收缩期空白带（`MOTION.fadeOut` 83 → 167，旧内容盖到格子收完）、CI 缺「载体版本一致」校验（新增 `scripts/release/check_versions.py`，CI 每次推送跑、release 带 `--tag` 跑）。
- **四轮（增量：`fadeOut` 167 + 版本门禁）**：2 Major / 8 Minor，全部处置——AGENTS 的发版步骤仍写「三处」而门禁要五处（改成五载体 + `npm install --package-lock-only`，第 4 步同步改口）、`Cargo.toml` 版本解析取「文件里第一个 `version =`」（改用 `tomllib` 读 `[package]`，`Cargo.lock` 也改成 tomllib 扫描，`version.workspace = true` 会如实报 missing）、`--tag ""` 被当成「不校验」（改成 `is not None` + `vX.Y.Z` 格式校验）、缺文件时抛 traceback（改成一行提示 + exit 1）、stdout 阻塞导致摘要印在表格之前（`flush=True`）、`fadeOut` 与格子时长之间的隐含不变量（`Math.min(fadeOut, 格子时长)` + 注释写明）、「v1.0.1 已发布且锁文件过期」措辞不实（1.0.1 其实未发）、AGENTS 仓库布局漏了新脚本、CHANGELOG 没提新门禁且 release 里门禁跑在 `npm ci` 之后（补 `### Added` / 中文「新增」，门禁提到 `npm ci` 之前）。
- **验证**：`npx tsc --noEmit` / `npm run build` / `cargo fmt --check` / `cargo test`（29 passed）/ `--test-ui`（四窗口 OK）全绿；用 CDP 逐帧采样真实打包产物：长高 200ms/进场曲线、收缩 167ms/退场曲线、内联固定值、结束时高度回调、连点打断后无残留，以及「resize 期间数据到达 → 窗口与终点一起 retarget」都对；README 截图重生成后与仓库**字节一致**（稳态几何未变）；真机 CU 截图 + 60fps 录屏：Kimi 1222 ↔ DeepSeek 585 切换后窗口高度精确等于卡片 + 56，录到的收缩过程逐帧无裁切（圆角与阴影完整）、footer 与底边不动。
- **发版（2026-11-15）**：提交 `311f076` push 后 CI 全绿（新门禁 `Version carriers agree` 作为第 3 步通过）；annotated tag `v1.0.1` push → `Release (draft)` 全绿（tag 门禁 → 构建 → 打包 → 取 CHANGELOG → 建草稿）；草稿三件资产 `QuotaTray.exe` 7,171,072 B、`QuotaTray-v1.0.1.zip` 3,981,792 B、`SHA256SUMS.txt` 167 B，下载后 `sha256sum -c` 两条 **OK**，**2026-11-15 07:30:32 UTC 已 publish 并标记 Latest**。

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
