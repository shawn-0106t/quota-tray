# STATUS — Quota Tray

> 快照：2026-11-21 11:20（**PR #11 已 squash 合并**，`main` = `183a686`；v1.0.2 仍是 Latest；`[Unreleased]` 是这次的数据层修复，定版/打 tag 待拍板）。
> 2026-11-19 更新：1.0.2 原本只含「验证后保存」一条；因为一直没打 tag，未发布的改动都并进了这一版——除了 DeepSeek
> 的「验证后保存」，还包括设置窗标题固定、共享账号目录（`%APPDATA%\QuotaTray\shared`）、
> Kimi 报文解析修复（`booster_wallet` / 周窗口百分比）以及一批审查 Minor；`[Unreleased]` 已折进
> `[1.0.2] — 2026-11-19`，`check_versions.py` 六项仍 ok。
> 「验证后保存」这一条在五轮独立只读审查里被磨了五遍（表格见下）：
> `verify_user_token` 不再把 200 + `{"code":40003}` 的错误信封当成通过（PR #4 → squash `5d95f1f`）；临时性失败对同一个候选重试一次；被拒的 token 优先于其他 base 的错误；epoch 只在真正的窗口边界（`new_session`：建窗 / 关窗 / Clear）移动；重试循环的闸门排在一切副作用之前（被取代的 future 不写标题、不偷队列、不清别人的 `verifying`）。
> 五载体 1.0.2、`CHANGELOG` 的 `[1.0.2] — 2026-11-19`、`check_versions.py` 六项 ok；`tsc` / `vite build` / `cargo fmt+clippy+test --locked`（31 passed）全绿；`--test-login` 在真机跑过（`verify=ok` + 四窗 OK + 两项捕获 OK）；`main` 的 CI 在 `2d06418`（2m3s）与 `35308af`（2m14s）上 success，CodeQL 仍在结算。
> **下一步**：见下方 2026-11-21 段——Slint 发布线拍板、是否给 `[Unreleased]` 打 `v1.0.3`、以及真机缺口。`main` 上没有已知未修项。
> **本轮收尾（2026-11-20）**：`demo/heights.py` 默认量产品页（`--demos` 保留 A–E 历史表，读 `ui-demo.html` 的 `#m-product` 读数）；`demo/check_contrast.py` 改成解析 `theme.css`——当场抓出 `--ink-3` 只有 2.11/2.34（已提到 3.22/3.35，`PLAN.md` §3.2 同步），把 `--seg-ring` 改坏会真的 FAIL；`expires_at` 秒/毫秒归一；`limits[0]` 被当作 5h 写进 PLAN 并有测试钉住；`MODELS_REFETCHED` 每进程一次仍是 PLAN 的设计，不是缺陷。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 2026-11-21 PR #11（已合并）：usage 错误信封 + 数据层加固

- PR [#11](https://github.com/shawn-0106t/quota-tray/pull/11) squash 为 `183a686`（base `940e789`）。这是 Slint 分支全量审查里 **`main` 也有** 的那部分：usage 抓取的每个 2xx 先过 `token_accepted`（死 token 的 `200 + {"code":40003}` 不再变成全零月份、覆盖 last-good、记住坏 base）；`get_i64` 接受浮点/十进制字符串；`used:null` 回退到 `limit - remaining`；聚合计数器饱和；`settings.json` 临时文件 + rename；Clear 先 bump `new_session`/`note_clear` 再清文件；`credentials::save` 返回 bool，登录窗与手工粘贴报 `save-failed`；新增 2 个测试（null used、numeric fallback）。
- CI：`Build & test (windows-latest)` 2m4s、gitleaks 10s、CodeQL rust 9m0s，全 pass；合并前 `mergeStateStatus=CLEAN`。
- Slint 线（`spike/slint-demo`，worktree `../quota-tray-slint`）：P5 完成；`PARITY.md` 66 行矩阵无 ❌；全量审查 1 Critical / 9 Major / 若干 Minor 全部修复（`7040338` + `d18137d`）；`ci-slint.yml` 首跑因 pinned toolchain 缺 `rustfmt` 失败，补 `components: rustfmt, clippy` 后 **5m10s 全绿**（run `44897311480`）；分支在 `32f4157` 并已推送。审查明细见 `spike/p5/REPORT.md` §9。
- **下一步**：① 拍板 Slint 与 Tauri 的发布线（`spike/HANDOFF.md` §9：是否以 Slint 为 v2.0.0 主线；若是，`quota-core`/`slint` 要进五载体门禁、身份名与发布资产要统一）；② 决定 `[Unreleased]` 是否打 `v1.0.3`（用户可见的数据完整性修复）；③ 真机缺口（100/150/200% DPI、多屏、登录成功 exit 0、网络失败文案）。`main` 上无已知未修项。

## v1.0.2 的四轮审查与处置（2026-11-15）

发版的是一条修复，但它被独立只读审查磨了四遍；每一轮都抓到真东西，记录在此以免重复劳动：

| 轮 | 范围 | 发现 | 处置 |
|---|---|---|---|
| 1 | P4 的登录链路（spike 线） | 1 Critical / 3 Major / 9 Minor（父进程存活检测、父进程侧 epoch 兜底会抹掉手工粘贴的凭证、`--test-signin` 会覆盖凭证文件……） | 全部处置，见 `spike/p4/REPORT.md` §4.3 |
| 2 | `5d95f1f` + `f92748f`（v1.0.2 定版） | 2 Major / 10 Minor（`Err` 从死代码变热路径却无重试；`AGENTS.md` 仍写「2xx 即通过」） | `c7f0594` + `96c8a3c` |
| 3 | `c7f0594` + `b016576` 的增量 | 1 Major / 6 Minor（拒绝优先级没覆盖所有出口；睡眠期间的新候选被吞；`reset()` 不 bump epoch） | `b016576` + `4ba92ee` |
| 4 | `4ba92ee` 的增量 | 2 Major / 7 Minor（拒绝优先级反过来吞掉了重试；`reset()` 把 bump 放到了「重新前置已有窗口」路径上） | `e6e30f7` |
| 5 | 整条验证路径通读（`a63b271` 之前的所有补丁） | 2 Major / 6 Minor（重试循环里 `set_title` 排在 epoch 闸门之前，被取代的 future 会写坏新窗口标题并偷走它的队列；`open()` 的前置分支 `reset()` 又破坏单飞） | `2d06418` |

第 3–5 轮抓的都是**上一轮修复自己引入的**问题（第 5 轮之前没人通读过拼起来的整条路径）。**已知缺口**（不打算在发版前修）：`verify_and_save` 的决策表没有测试，只测了 `transient()`——要钉住需要把决策抽成纯函数，那是一次结构改动，留给 P5；以及 `--test-login` 无法复现「验证在途时重新前置窗口」的时序，这条只能靠真人登录验一次。

## 一句话

**v1.0.2 已在 `https://github.com/shawn-0106t/quota-tray` 发布**（2026-11-19 16:07 UTC，Latest）：「验证后保存」读信封（1.0.1 对坏 token 照样通过）、设置窗标题固定、DeepSeek 凭证与网络缓存移入共享的 `%APPDATA%\QuotaTray\shared`、Kimi 报文解析修好（`booster_wallet` 与周窗口百分比）、Clear 在手工粘贴验证途中也留得住；另有一批审查 Minor 与发版门禁（`release.yml` 在构建前重跑 tsc/fmt/clippy/test）。上一版是 v1.0.1（2026-11-15）。

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
5. **设置窗高度是「上限 90% + pane 内滚动、标题固定」**：`#root` 是 flex 列（`max-height: calc(100vh - 56px)`），只有 `.pane` 滚动，滚动条 4px 内缩细线、从标题下方才开始；上报「标题栏 + `pane.scrollHeight` + 2px 边框 + 56 + 1」——报内容高，报被夹过的 `offsetHeight` 会让窗口跟着缩小，多出的 1px 是物理像素取整的余量（125% 下视口可能比内容短 0.4px）。
6. **阴影必须收在 28px 留白内**：`--shadow: 0 8px 18px`；更大的模糊会被窗口边缘裁成硬边。`PLAN.md` 里那张表已删掉旧的 40px 行。
7. **发版只发便携 exe**：`bundle.targets = []` + `--no-bundle`，资产三件；`make_release_zip.py` 的 `binaries()` 只有一个 exe。别再往回收安装包。
8. 原有几条仍然有效：图标是生成物、托盘嵌 48px、Kimi 数字是本机口径、扫描节奏、`ui-demo.html` 是生成物、登录窗没有 IPC 面。
9. **README 截图有护栏**：`make_readme_shots.py` 的 `CROP` 是按 DOM 实测的固定框，脚本会先断言「裁切外圈是单一背景色」再出图。改舞台 padding、改窗宽、改演示页排版后，它要么自己过、要么直接报错要求重测——不要手工裁图。
10. **两个应用共用账号、不共用记忆**（2026-11-19）：`shared_dir()` =
    `%APPDATA%\QuotaTray\shared` 放 DeepSeek 凭证、两个网络缓存（endpoints / models）与扫描索引，`config_dir()` 放
    `settings.json` / `webview.log` / `deepseek-cache.json`；spike 线
    （`../quota-tray-slint`）必须实现同一条绝对规则，并用自己的互斥名、自启值名和
    WebView2 数据目录。拆分前的 `config_dir()/deepseek-credentials.json` 在首次
    `load()` 时被原样复制到 shared 并保留；`Clear` 两处都写空。

## 环境现状（2026-11-20 00:10）

- **Kimi**：token 可用（2026-11-19 23:57 `--test-fetch` 通过：`fiveHour 1%` / `week 0.13%` / `extra Ready ¥0.15`）。中途 23:55 曾报 `no-token`，两分钟内被刷新，属凭证轮换。
- **DeepSeek**：API key 与 usage token 都已配置（`--test-login` 报 `credentialSource=file`、`apiKeyConfigured=true`、`verify=ok`）；余额 ¥34.64。
- `%APPDATA%\QuotaTray`：credentials / endpoints / models / cache / kimi-usage / `settings.json`（`Theme=system`、`RefreshMinutes=5`、`AutoStart=false`、`ActiveTab=kimi`）/ `webview.log`。
- 仓库根留三件本地构建的产物（gitignored）：`QuotaTray.exe`（1.0.0）、`QuotaTray-v1.0.0.zip`、`SHA256SUMS.txt`；哈希与 Release 里 runner 构建的那份不同（正常，两台机器/两次构建）。
- 本机已无常驻托盘进程：为验证关掉了 `trae_projects\kimi\quota-tray.exe`（旧副本，会抢 `QuotaTray.SingleInstance`）。要用就双击 `src-tauri/target/release/quota-tray.exe`（1.0.2 本地构建）或下载 Release 里的 `QuotaTray.exe`；那个旧副本建议删掉，否则还会抢单实例。
- git：`main` 干净并与 `origin/main` 同步（`183a686`）；tag `v0.1.0` / `v1.0.0` / `v1.0.1` / `v1.0.2`（Latest）。

## 新对话开场建议

> 接手 Quota Tray（`https://github.com/shawn-0106t/quota-tray`，本地目录同名仓库，工作区干净）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`；UI 看 `demo/ui-demo.html`（生成物，别手改），README 截图由 `demo/make_readme_shots.py` 生成。
>
> 状态：**v1.0.2 已发布并标为 Latest**（2026-11-19 16:07 UTC）；PR #11 已合并（`183a686`，usage 错误信封 + 数据层加固，`[Unreleased]` 待定版）。Slint 线 P5 已完成：`PARITY.md` 66 行无 ❌、全量审查修复、`ci-slint.yml` 首次 5m10s 全绿。`main` 现为 PR-only（管理员也不能直推）。下一步是 `spike/HANDOFF.md` §9 的发布线拍板，以及是否给 `[Unreleased]` 打 `v1.0.3`。
>
> 用户已拍板：只发便携 exe（无安装包）；托盘图标保持纯紫双条；设置窗维持「工作区 90% 上限 + 标题固定、只有 pane 内滚动」；`main` 只接受 PR。
