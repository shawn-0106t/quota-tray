# STATUS — Quota Tray

> 快照：2026-11-14 17:27（第九轮结束，P5 完成，工作区即将提交）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

**P1–P5 全部完成**。第九轮做完 P5：`npx tauri build` 出便携 exe + 按用户 NSIS 安装包、`scripts/release/make_release_zip.py` 打包（源码 zip + 三件资产 + SHA256SUMS）、`.github/workflows/{ci,release}.yml`（gitleaks 门 + tag 触发草稿 Release）、中英双语 README + 生成的两张截图，并委派了一次独立 code review（报 2 Blocker / 3 Major / 3 Minor，全部已修并复验）。

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
| **P5 打包 / 发布线 / README / 代码审查** | **完成（本轮）** |

## 本轮做了什么

1. **打包**：`src-tauri/tauri.conf.json` 的 `bundle.targets` 从 `all` 改成 `["nsis"]`，`installMode = currentUser`、安装界面 English + SimpChinese。产物：`target/release/quota-tray.exe`（便携）+ `target/release/bundle/nsis/Quota Tray_0.1.0_x64-setup.exe`（按用户安装包，不弹 UAC）。
2. **发布线**：
   - `.github/workflows/ci.yml`：gitleaks（全历史、8.29.1 钉版）+ `windows-latest` 上 `npm ci` / `tsc --noEmit` / `npm run build` / `cargo fmt --check` / `cargo clippy --all-targets -D warnings` / `cargo build --locked` / `cargo test --locked`。
   - `.github/workflows/release.yml`：`v*` tag 必须等于 `tauri.conf.json` 的 version；CHANGELOG 对应小节若仍写 `Unreleased` 直接失败；构建 + 打包 + `gh release create --draft`（四个资产），发布仍人工。
   - `scripts/release/make_release_zip.py`：版本只从 `tauri.conf.json` 读；文件集取自 `git ls-files`；zip 根带便携 exe；三件资产改名 + `sha256sum` 兼容的 `SHA256SUMS.txt`；`.part` 中转，失败不会留下不匹配的校验对。
3. **README.md**：英文在前、中文在后；截图由 `demo/make_readme_shots.py` 从 `demo/ui-demo.html` 裁出（`docs/screenshot-{light,dark}.png`），不手工截图。
4. **独立审查**：委派只读 review（`explore` agent + code-review skill，见下方「审查」）。
5. **`demo/shot.sh` 修复**：输出路径为绝对路径时浏览器拿到 `C:/repo/C:/...`，截不出图；现在绝对路径原样透传。

## 本轮验证结果

| 项 | 结果 |
|---|---|
| `cargo test --locked` | **29 passed** |
| `cargo fmt --check` | 通过（此前不过：7 个文件有 diff，已 `cargo fmt`） |
| `cargo clippy --all-targets --locked -- -D warnings` | 通过（此前 `client.rs:117` needless_borrow） |
| `npx tsc --noEmit` / `npm run build` | 通过 |
| `npx tauri build` | 通过：exe 7.18 MB + NSIS 2.50 MB（`targets = ["nsis"]`） |
| `--test-fetch`（release exe） | `error: null`，5h 0% / week 1%，Extra `NotActivated` |
| `--test-deepseek` | 余额 ¥41.85，1 个模型，余额与用量均无错误 |
| `--test-update` | `local=2.6.1 latest=2.6.2 updateAvailable=True` |
| `--test-usage` | `scan1 files=430 req(7d)=3424 req(today)=1062 ms=109` / `scan2 … ms=31`，两遍一致 |
| `--test-login` | `credentialSource=file`、`verify=ok`、五个窗口 OK、`capture channel OK`、`fetch capture OK` |
| 打包产物 | `sha256sum -c SHA256SUMS.txt` 三条全 OK；zip 82 个文件 + 根目录 `QuotaTray.exe`，无 `target/`、`dist/`、`node_modules/` |
| 安装包 | 静默安装 `exit=0`（无 UAC）→ `%LOCALAPPDATA%\Quota Tray` 下 exe 可跑 `--test-update` → 静默卸载 `exit=0`，开始菜单与安装目录都清干净 |
| 探测污染守卫 | `--test-login` 跑完后 `deepseek-endpoints.json` 的 mtime 仍是 13:26（未被 probe 改写） |
| 跨天戳 | `fmtUpdated` 用 node type-strip 直接跑：同日 `Updated 18:14`、昨天 `Updated 11/13 09:05`、去年 `Updated 12/31 23:59`、坏值空串 |

## 审查（第九轮）

委派了一个只读 review agent（本会话的 `Agent` 只有 `plan` / `coder` / `explore` 三种，没有 `code-reviewer`；`~/.kimi-code/bin/kimi-review.cmd` 走 `kimi-code/k3.1-pro`，本机订阅无权限，实测报 `provider.auth_error`）。用的是 `explore`（工具集只读，不能改代码）+ `~/.agents/skills/code-review` 的方法，任务文本要求证伪、假设至少两处错误。

它报了 8 条，全部成立、全部已修：

| # | 级别 | 问题 | 修法 |
|---|---|---|---|
| 1 | Blocker | CI 的 `cargo fmt --check` 在现有树上直接失败（7 文件 20 处） | `cargo fmt` |
| 2 | Blocker | `release.yml` 的 notes 步骤在 Windows runner 上按 ANSI 码页写中文，`UnicodeEncodeError` 或乱码 | 该步骤加 `PYTHONUTF8: "1"` |
| 3 | Major | `Updated M/D HH:MM` 只写在文档里，`fmtUpdated` 从来不带日期 → 昨天缓存读成今天 | `common.ts` 按本地日期加前缀 |
| 4 | Major | README 截图的裁剪框切掉设置窗右边约 41px | `CROP` 改 `(40, 430, 2740, 1720)`，并加画布尺寸断言 |
| 5 | Major | 验证进行中到达的新 token 被丢弃，页面又不会重发 → 等于白登一次 | `LoginState.pending` + 失败后重试队列里的候选 |
| 6 | Minor | `--test-login` 的 probe 会把学到的端点覆盖成编译期默认值 | probe URL 带 `quotatray=probe`，宿主拒绝学习 |
| 7 | Minor | 打包脚本 rollback 会恢复一份描述旧 exe 的校验文件 | 失败时删掉校验文件，不留不匹配的一对 |
| 8 | Minor | 截图脚本在布局漂移时给黑边而不报错 | 尺寸断言后再裁 |

我自己复跑 CI 的门时又抓到一条它没跑到的：`cargo clippy -D warnings` 报 `client.rs:117` needless_borrow（`client()` 已返回 `&Client`），已修。

## 下一步（都只能人做或要你拍板）

1. **真人登录验证**（唯一没被自动化覆盖的一步）：设置窗点 `Sign in…` → 在窗里登账号 → 标题变 `verified, saved` → 面板出数。会覆盖 `deepseek-credentials.json` 的 `userToken`（同账号等价）。
2. **发布**：建 remote（建议仓库名 `quota-tray`）→ 把 `CHANGELOG.md` 的 `## [0.1.0] — Unreleased` 改成带日期的标题 → `git tag v0.1.0` → push tag。workflow 会开一个草稿 Release，人工 review 后再 publish。
3. **要你拍板**：
   - 安装包留不留？现在 release 里是「便携 exe 为主 + 可选按用户 NSIS」。不想要安装包就把 `bundle.targets` 改成 `[]`、删掉 release 里的那件资产即可。
   - 托盘图标要不要再做一版带 DeepSeek 元素的双色标（现在定的是纯紫双条）。
   - 缓存块是否也放进设置窗的 Kimi 只读区（现在只在面板）。

## 接手必须知道的（在原有 8 条之上新增）

1. **CI 有 fmt / clippy 门**：`cargo fmt --check` 与 `cargo clippy --all-targets -- -D warnings` 都是硬门，改完 Rust 先本地跑这两条，否则推上去必红。
2. **发布资产是四件**：`QuotaTray.exe`、`QuotaTray_<ver>_x64-setup.exe`、`QuotaTray-v<ver>.zip`、`SHA256SUMS.txt`；名字由 `scripts/release/make_release_zip.py` 定，workflow 用 `test -f` 精确校验，改名要三处一起改（script / workflow / `AGENTS.md`）。
3. **安装包是 per-user**：`installMode = currentUser` → 装进 `%LOCALAPPDATA%\Quota Tray`，只写 HKCU，不弹 UAC；便携 exe 才是主推。静默安装/卸载要用 `Start-Process -Wait -PassThru`（直接从 Git Bash 调用会返回 1 且什么也不做）。
4. **截图是生成物**：`PYTHONUTF8=1 python demo/make_readme_shots.py`（依赖 Pillow + `shot.sh` 探测到的 Chromium）。改 UI 后重跑，别手裁。
5. **`shot.sh` 现在接受绝对输出路径**：以前会拼成 `C:/repo/C:/...`。
6. **登录候选有队列**：验证中到达的新 token 存 `LoginState.pending`，本次验证失败后重试；页面按 `lastToken` 去重，丢了就不会再来。
7. **`--test-login` 不再污染端点**：probe URL 带 `quotatray=probe`，宿主跳过学习。
8. 原有 8 条（图标生成物 / 托盘 48px / Kimi 数字是本机 / 扫描节奏 / ui-demo 生成物 / 登录窗无 IPC 面 / 环境事实 / 不 cat 密文）仍然有效。

## 环境现状（2026-11-14 17:26）

- **Kimi 配额接口**：通（`error: null`），5h 0% / week 1%，Extra `NotActivated`。
- **Kimi 本机统计**：7d 3,424 请求 / today 1,062 请求；走树 109 ms、只 stat 31 ms。
- **DeepSeek**：余额 ¥41.85（账号在动，别拿数值当验收）。
- `%APPDATA%\QuotaTray`：credentials / endpoints / models / cache / kimi-usage.json；仍无 `settings.json`（没点过 Save）。
- 本轮安装验证后系统已清干净：无 `%LOCALAPPDATA%\Quota Tray`、无开始菜单快捷方式、无 Uninstall 注册项。
- 本轮结束时无后台进程残留。

## 新对话开场建议

> 接手 Quota Tray（本地 git 仓库，无 remote，HEAD 是 P5 那一提交）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`；要看 UI 直接开 `demo/ui-demo.html`（生成物，别手改），文档里的截图由 `demo/make_readme_shots.py` 生成。P1–P5 全部完成、CI 与发布线已就位、独立审查的 8 条全修完。剩下的是人的动作：一次真人登录验证，以及建 remote / 打 tag / publish；安装包留不留等待拍板。
