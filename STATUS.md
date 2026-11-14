# STATUS — Quota Tray

> 快照：2026-11-14 19:10（第九轮结束 + 用户实测后的修补，15 个提交，工作区干净；下一轮做发版前的去安装包 + 全量审查 + 全量测试）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

**P1–P5 全部完成**，第九轮又在用户实测后修掉了三个真 bug：登录窗在主线程上建窗导致**空白且关不掉**、`Clear` 被登录窗的自动捕获**写回**、设置窗展开手工粘贴块被夹高**切掉底部**（滚动条还落在窗口边缘、阴影被裁成硬边）。三者都已复现、修复、复验，文档同步。

**还差两件只能人做的事**：① 真人在登录窗里输一次账号密码；② 建 GitHub remote、打 `v0.1.0` tag、publish 草稿 Release。

## 阶段状态

| 阶段 | 状态 |
|---|---|
| P0 设计探索 + 定稿 | 完成 |
| P1 仓库化 / 导入 / 改名 | 完成（`b43b3e7`） |
| P2 视觉 + 高度 + tab | 完成（`d892eeb`…`8ab1f86`） |
| P3 DeepSeek 后端 | 完成（`13822ce`） |
| P4 设置窗分区 / 托盘合并 / 登录捕获 | 完成（`855035a`） |
| 计划外：图标 / Kimi 统计 / 一页 demo | 完成（`855035a`、`9b23b74`） |
| P5 打包 / 发布线 / README / 代码审查 | 完成（`bb57658`…`9252a08`） |
| 实测修补：登录窗 / Clear / 设置窗滚动与阴影 | 完成（`26fbe6a`…`940ae42`） |

## 提交记录（第九轮，15 个，每个都能独立构建）

```
281fd36 docs(agents): the shadow must fit the 28px margin
03622c2 docs: sync P5 and the user-testing fixes
940ae42 fix(settings): scroll inside the card, with a hairline scrollbar; tighten the shadow
4cf6061 fix(settings): scroll the sheet when the window is clamped
5682d67 fix(app): Clear must not be undone by an open sign-in window
f2bc122 fix(app): build the sign-in window off the main thread
e8be518 fix(app): drop the redundant post-build focus call in the sign-in window
cef31b2 fix(app): keep window work off the sign-in open/close critical path
3326bc9 docs(status): record the sign-in window fix
26fbe6a fix(app): open the DeepSeek sign-in window above the settings sheet
4a15ae7 docs(status): record the P5 commits, pin the review route that works
9252a08 docs: record P5, the release process and the review fixes
c2fa3f9 docs(readme): bilingual README with generated screenshots
d0ea0a5 feat(release): NSIS bundle, packaging script, CI and draft-release pipeline
bb57658 fix(app): queue sign-in candidates, date stale stamps, pass fmt/clippy
```

## 用户实测后的三个 bug（都已复现 → 修复 → 复验）

| 现象 | 根因 | 修法 | 验证 |
|---|---|---|---|
| 点 `Sign in…` 出现**空白且关不掉**的窗口（只能最小化），日志里连 `login: sign-in window opened` 都没有 | `open_login` 是同步命令，Tauri 把同步命令跑在**主线程**；`WebviewWindowBuilder::build()` 又要跳到主线程并等它完成 —— 窗口建出来了，命令永远不返回，后面的导航 / 日志 / 收窗全没跑 | `open_login` / `close_login` 把活交给 async runtime；收起设置窗放在建窗之后；去掉建窗后多余的 `set_focus()` | 在设置窗里真实 `click()` 按钮走 IPC：旧构建只有 `clicked`、新构建 `login: sign-in window opened` → `settings: hide`，页面渲染出 DeepSeek 登录页，`WM_CLOSE` 后 `login: sign-in window closed` 且设置窗回来 |
| `Clear` 之后 usage token 还在（API key 确实没了） | `Clear` 本身没问题（文件里 `apiKey` 为空、之后的两次写入都是登录窗的 `candidate verified and saved`）；登录窗的职责就是捕获 console 已有会话里的 token，所以清掉后再点 `Sign in…` 会自动写回 | `Clear` 顺手关掉登录窗；`LoginState.epoch` 让 Clear 之前发起的验证丢弃结果；设置窗写明「再登录会存回 token」；`Clear` 写一行日志 | 设置窗点 Clear 后文件两字段为空；日志有 `credentials: cleared (apiKey=false userToken=false)` |
| 设置窗展开「Paste a credential instead」显示不全；滚动条在窗口边缘、阴影像硬边 | 窗口被夹在工作区 90%（本机 752 逻辑高 → 677），展开后内容 824，`overflow: hidden` 直接切掉；`body` 滚动让滚动条落在透明窗口边缘；`0 12px 40px` 的阴影超出 28px 留白被窗口裁断 | `#root { max-height: calc(100vh - 56px); overflow-y: auto }` 卡片自己滚；滚动条 8px 轨道 / 4px 内缩细线（`--hair`，hover `--ink-3`）；上报高度改 `max(offsetHeight, scrollHeight)`；阴影收到 `0 8px 18px` | 真实页面量：收起 `root=596`（+56=652，不滚动）、展开 `root=768`（+56=824，`scrollHeight=824`）；在 404×677 下渲染确认细滚动条在卡片内、阴影在留白内淡出；`check_contrast.py` 65/65 |

## 本轮验证结果

| 项 | 结果 |
|---|---|
| `cargo test --locked` | **29 passed** |
| `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` | 通过（这两条都是 CI 硬门） |
| `npx tsc --noEmit` / `npm run build` | 通过 |
| `npx tauri build` | exe 7.20 MB + NSIS 2.50 MB，同一次构建打包 |
| `--test-fetch` / `--test-deepseek` / `--test-update` / `--test-usage` | 全绿（Kimi `error: null`；DeepSeek 余额随账号变动；两遍扫描请求数一致） |
| `--test-ui` / `--test-login` | 四个窗口 OK；`verify=ok`、`LoginWindow capture channel OK`、`fetch capture OK` |
| 发布资产 | `sha256sum -c SHA256SUMS.txt` 三条全 OK；zip 82 个文件，含根目录 `QuotaTray.exe` |
| 装机 | 静默安装 → 跑起来 → 静默卸载，开始菜单、安装目录、Uninstall 注册项都清干净 |
| 设计评审 | `demo/ui-demo.html` 与两张 README 截图按新 token 重新生成；`check_contrast.py` 65/65 |
| 独立审查 | 报 2 Blocker / 3 Major / 3 Minor，全部修复；我自己复跑 CI 门又补了一条 clippy |

## 已拍板（2026-11-14，用户在第九轮末尾决定）

| 项 | 决定 | 落地情况 |
|---|---|---|
| 安装包 | **只发便携 exe**，去掉 NSIS 安装包 | **未落地**，见下一节第 1 步 |
| 托盘图标 | 保持现在的纯紫双条，不再出双色版 | 无需改动 |
| 设置窗高度 | 保持「工作区 90% 上限 + 卡片内滚动」，不做 96% 展开 | 无需改动 |

**真人登录验证已完成**：`webview.log` 里有三次 `candidate verified and saved`，`--test-login` 报 `verify=ok`，DeepSeek 用量页有数。这一步不再需要重做。

## 下一轮：发版前（用户在新的对话里发起）

### 第 1 步：去掉安装包，只留便携 exe

按这个清单改，一处都不能漏（漏一处就会让 release 资产与文档对不上）：

| 文件 | 改法 |
|---|---|
| `src-tauri/tauri.conf.json` | `bundle.targets` → `[]`；`bundle.windows.nsis` 整块删掉；`bundle.icon` 保留（exe 图标仍用它） |
| `.github/workflows/release.yml` | 构建步骤 `npx tauri build` → `--no-bundle`；删 `test -f "QuotaTray_${version}_x64-setup.exe"`；`gh release create` 的资产列表去掉 setup.exe；notes 里的 Assets 段改成两件 |
| `scripts/release/make_release_zip.py` | `binaries()` 删掉 NSIS 那条（只剩便携 exe 进 zip + 作为资产）；docstring 的「three assets / four assets」表述同步 |
| `README.md` | 「Optional installer」那条 bullet 删掉（中英两处）；首段「no installer」保持 |
| `AGENTS.md` | Release process 的「four assets」→「three assets」；Build 段里 NSIS 安装包的说法；Remove「安装包是 per-user …」这条 trap（连同 `Start-Process` 静默安装那条环境说明） |
| `PLAN.md` | §11 的「Release 资产」行改成两件 + 校验文件 |
| `CHANGELOG.md` | 中英各加一条 Changed：「不再提供 NSIS 安装包，只发便携 exe」 |

验收：`npx tauri build` 只产出 exe（不再有 `bundle/nsis/`）；`PYTHONUTF8=1 python scripts/release/make_release_zip.py` 产出 `QuotaTray.exe` + `QuotaTray-v0.1.0.zip` + `SHA256SUMS.txt` 三件；`sha256sum -c` 全 OK；`--test-ui` / `--test-login` 仍绿。

### 第 2 步：全量 code review

- 范围是**发版提交的整棵树**，不是 diff：`src-tauri/src/**`、`src/**`、`.github/workflows/**`、`scripts/release/**`、`demo/*.py`、`demo/shot.sh`，以及文档与代码是否互相矛盾（README / AGENTS / PLAN / CHANGELOG）。
- 委派方式按 `AGENTS.md`「Environment notes」：本会话 `Agent` 没有 `code-reviewer` 类型，`kimi-review.cmd` 钉的 `kimi-code/k3.1-pro` 在本机订阅无权限 → 用只读 `explore` + `~/.agents/skills/code-review`，任务文本要求证伪（假设至少两处错误）。**它的结论必须自己复跑 CI 门核对**（上一轮它漏了 clippy）。
- 重点复核上一轮修过的三处，确认没有被后续改动碰坏：登录窗在主线程外建窗、`Clear` 的 epoch 与关窗、设置窗 `max(offsetHeight, scrollHeight)` 上报与卡片内滚动。
- 验收：无 Blocker；每条发现要么修掉，要么在 STATUS 里写明不修的理由。

### 第 3 步：全量测试

```bash
# 静态门
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked)
npx tsc --noEmit && npm run build && npx tauri build

# 运行时自检（release exe，逐条看输出）
./src-tauri/target/release/quota-tray.exe --test-fetch
./src-tauri/target/release/quota-tray.exe --test-deepseek --verbose
./src-tauri/target/release/quota-tray.exe --test-update
./src-tauri/target/release/quota-tray.exe --test-usage
./src-tauri/target/release/quota-tray.exe --test-ui
./src-tauri/target/release/quota-tray.exe --test-login --verbose

# 打包与设计评审
PYTHONUTF8=1 python scripts/release/make_release_zip.py && sha256sum -c SHA256SUMS.txt
PYTHONUTF8=1 python demo/build_ui_demo.py
PYTHONUTF8=1 python demo/make_readme_shots.py
PYTHONUTF8=1 python demo/heights.py && PYTHONUTF8=1 python demo/check_contrast.py
```

另外人工过一遍这几条（自动化覆盖不到）：便携模式（exe 旁放空 `portable.dat` 后配置是否落在同目录）、开机自启开关写/删 HKCU 值、两个 tab 各连点 30 次不跳位且右下角不动、设置保存后重启生效（`ActiveTab` 除外）。

### 第 4 步：发版（只有人能做）

建 remote（建议 `quota-tray`）→ 先 push 看 CI 绿不绿（action pin / NSIS 下载 / gitleaks 这三处只有真 runner 能验）→ `CHANGELOG.md` 的 `## [0.1.0] — Unreleased` 改成带日期 → `git tag v0.1.0 && git push origin v0.1.0` → 草稿 Release 出来人工核对资产与 `SHA256SUMS.txt` → publish。

## 接手必须知道的（新增/更新）

1. **同步命令跑在主线程**：任何 `#[tauri::command]` 里都**不要**直接 `WebviewWindowBuilder::build()`——它会跳主线程并等待，结果就是「窗口出现但一片空白、关不掉」。用 `tauri::async_runtime::spawn` 把建窗挪走（`open_login` / `close_login` 已经是这个写法）。这是本轮最难找的 bug，`--test-login` 和启动钩子都绕开了它，最后靠「真实点击按钮」才复现。
2. **窗口程序没有控制台**：`install_panic_log()`（main.rs）把 panic 写进 `webview.log`；静默 panic 和卡死看起来一模一样。
3. **日志现在能自证**：`login: sign-in window opened` / `login: sign-in window closed` / `settings: hide -> Ok(()) visible=…` / `credentials: cleared …`。用户报「点 X 没反应」时先看这四行。
4. **Clear 的边界**：`Clear` 会关登录窗、丢弃在飞的验证；但**再点 `Sign in…` 仍会把 console 会话里的 token 存回来**——那是那个窗口的职责，不是 bug。设置窗有一行说明。
5. **设置窗高度是「上限 90% + 卡片内滚动」**：`#root` 自己 `max-height` + `overflow-y: auto`，滚动条 4px 内缩细线；上报 `max(offsetHeight, scrollHeight)` 才不会自我缩小。
6. **阴影必须收在 28px 留白内**：`--shadow: 0 8px 18px`；更大的模糊会被窗口边缘裁成硬边。
7. 原有 8 条（图标生成物 / 托盘 48px / Kimi 数字是本机 / 扫描节奏 / ui-demo 生成物 / 登录窗无 IPC 面 / 环境事实 / 不 cat 密文）仍然有效。

## 环境现状（2026-11-14 19:03）

- **Kimi 配额接口**：通（`error: null`）；但 **Kimi CLI 的 OAuth token 已过期**（设置窗红字 `Expired — sign in with the Kimi CLI`），需要在 CLI 里重新登录一次。
- **DeepSeek**：API key 已按用户操作清空；usage token 来自登录窗的自动捕获（同账号可用）。
- `%APPDATA%\QuotaTray`：credentials / endpoints / models / cache / kimi-usage.json / webview.log；仍无 `settings.json`（没点过 Save）。
- 本轮安装验证后系统已清干净：无 `%LOCALAPPDATA%\Quota Tray`、无开始菜单快捷方式、无 Uninstall 注册项。
- 工作区干净，HEAD `940ae42`；`QuotaTray.exe`（便携）、`QuotaTray_0.1.0_x64-setup.exe`、`QuotaTray-v0.1.0.zip`、`SHA256SUMS.txt` 留在仓库根（gitignored，与 HEAD 同哈希）。

## 新对话开场建议

> 接手 Quota Tray（本地 git 仓库，无 remote，HEAD=`281fd36`）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`；UI 看 `demo/ui-demo.html`（生成物，别手改），README 截图由 `demo/make_readme_shots.py` 生成。
>
> 状态：P1–P5 完成、CI 与发布线就位、独立审查 8 条全修、用户实测的三个 bug（登录窗空白 / Clear 被写回 / 设置窗切底）已修并复验、真人登录也验过了。**本轮目标只有一个：把 v0.1.0 发出去之前的准备**——按 `STATUS.md`「下一轮：发版前」四步走：
> 1. 去掉 NSIS 安装包，只发便携 exe（清单在该节，一处都不能漏）；
> 2. 对整棵树做一次全量 code review（委派方式与已知限制写在 `AGENTS.md`「Environment notes」，审查结论必须自己复跑 CI 门核对）；
> 3. 跑全量测试（静态门 + 六条 `--test-*` + 打包校验 + 设计评审脚本 + 四条人工项）；
> 4. 交回给人：建 remote、push 看 CI、给 CHANGELOG 补日期、打 `v0.1.0` tag、核对草稿 Release 后 publish。
>
> 用户已拍板：安装包只留便携 exe；托盘图标与设置窗高度维持现状。
