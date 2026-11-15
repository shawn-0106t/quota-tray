# P4 DeepSeek 登录链路 — 结果

> 结论先行：**登录窗跑通了，而且通道、拦截、拒收、Clear 都被自动化证据钉住。** 登录窗是**子进程**（`quota-tray-slint.exe --login-host`，tao + wry/WebView2），父进程只负责派生、藏设置窗、轮询退出、还原设置窗并刷新；捕获仍是「取消的导航」到 `quotatray://capture`，页面没有 IPC 面。`--test-login` 的四项全 PASS（窗口 / 通道 / fetch 拦截 / 拒收），`--test-signin` 的两轮全 PASS（用户关窗 → 父进程还原设置窗；Clear → 杀子进程 + 清凭证 + 不回写）。
>
> **顺带修掉一个真缺陷**：`quota-core::deepseek::client::verify_user_token` 原来只看 HTTP 状态码，而 DeepSeek 对无效 token 回 **200 + `{"code":40003,…}`**，所以「验证后保存」实际上从不拒绝任何候选——这是 `--test-login` 的拒收探针第一次跑就抓到的（§4.1）。

## 1. 验收

| 验收项 | 结果 | 证据 |
|---|---|---|
| `--login-host` 子进程：窗口 + 注入钩子 + 取消导航 | PASS：`LoginWindow OK`（窗口建好）、`LoginWindow capture channel OK`（`kind=probe` 从页面发出、宿主收到并取消导航） | `--test-login` |
| 拦截控制台自己的请求 | PASS：`LoginWindow fetch capture OK`——探针用页面自己调 `window.fetch` 的方式带一个假 Bearer，包装器在**请求发出前**就把 token 报给宿主 | `--test-login` |
| 验证后保存（坏候选必须被拒） | PASS：假 token 打真实 `amount`，结果 `token-expired`；**凭证文件前后逐字节相同**（`reject=ok`），窗口标题给出原因，文件不写 | `--test-login` |
| 设置窗「Sign in…」 | PASS：按下按钮 → 子进程起来（pid 可查）→ 设置窗立即隐藏；窗标题 `DeepSeek sign-in` 的顶层窗口出现，且 `GetWindowThreadProcessId` 证明它属于**本次子进程** | `--test-signin` 第 1 轮 |
| 用户关掉登录窗 | PASS：向真实窗口投递 `WM_CLOSE` → 子进程自己退出（`code=3`）→ 父进程的退出轮询在 **~600 ms** 内 `settle`：设置窗回来、note 写 `Sign-in cancelled.`、凭证仍无 token | `--test-signin` 第 1 轮 |
| Clear 与登录窗 | PASS：登录窗开着时 Clear → 子进程被杀（`try_wait` 立即结束）、`cred_epoch` 0→1、凭证文件无 token、note 是 Clear 那句；1.2 s 后仍未被回写（保证来自 kill → wait → 清文件这个顺序，不是 epoch 推断） | `--test-signin` 第 2 轮 |
| 主题 / 尺寸不回归 | PASS：P2 的 17 个高度变体与 DOM 仍 Δ=0.00 | `spike/p2/heights_ref.py --compare` |
| 不回归 P2/P3 | PASS：`--test-windows` / `--test-dialogs` / `--test-active-tab` / `--test-settings` / `--test-menu` / `--test-skills` / `--test-tray-click` / `--test-blur` / `--test-corner` / `--test-tab-anim` 全 PASS | 见 §6 |

## 2. 关键数字

**`--test-login`（热 profile）**

```
credentialSource=file
apiKeyConfigured=true
userTokenConfigured=true
verify=ok
LoginWindow OK
LoginWindow capture channel OK
LoginWindow fetch capture OK
LoginWindow rejects a bad candidate OK
test-login: window=ok sender=ok fetch=ok reject=ok verdict=PASS
```

整轮 **6 s**（探针脚本在窗口就绪后 4.5 s 触发，捕获 + 拒收再 1 s）。冷 profile 首轮 **175 s**——全部花在 WebView2 首次初始化上，不是本程序的等待。

**`--test-signin`**

```
signin: round 1: sheet-before=true running=true pid=Some(8032) sheet-after=false ok
signin: console window present (pid=8032, attempt 1) ok
signin: round 1: the console exited on its own (WM_CLOSE, 1 polls) ok
[  1688.1ms] login: exited code=Some(3) killed=false reopen=true
signin: round 1: running=false sheet-open=true code=Some(Some(3)) note="Sign-in cancelled." token=false ok
[  1768.0ms] login: sign-in window spawned pid=Some(23404) sheet-hidden=true
signin: round 2: running=false epoch=0->1 token=false note="Cleared. Signing in again stores the usage token the console already has." ok
signin: round 2: running=false token=false sheet-open=true ok
[  3473.2ms] signin: credential file restored
signin: verdict=PASS
```

整轮 **2.5 s**（两次派生，各自 ~0.4 s 出窗）。窗口出现是 `attempt 1`（400 ms 内）：tao 窗口在 WebView2 初始化之前就存在，所以自检不必等冷启动。

**`--test-login` 冷/热 profile 对比**（同一台机器）

| 情形 | 从派生到探针触发 | 从派生到 PASS |
|---|---|---|
| 冷 profile（`%TEMP%` 下全新目录） | 175 s | ~180 s |
| 热 profile（上一次的目录） | 5.0 s | **6 s** |

所以探针 profile 放在 `%TEMP%\quota-tray-slint-login-probe` 并**跨轮复用**；只有它坏了（被残留浏览器进程锁住，`0x800700AA`）才删掉重来一次。

## 3. 架构

```
父进程（Slint 事件循环）                    子进程（tao + wry，一次一个）
  设置窗「Sign in…」 ── login::open ──▶ Command::new(current_exe) --login-host --parent-pid N
        │                                  │
        └─ 设置窗隐藏（reopen_sheet=true）  ├─ tao 窗 900×680，always_on_top，居中
                                           ├─ WebView2 加载 https://platform.deepseek.com/
  250 ms 轮询 Child::try_wait ◀───────────┤ 注入 HOOK_JS（每份文档）
        │                                  ├─ 捕获 kind=token → verify → credentials::save → exit 0
        └─ settle：epoch 变了就再清一次       ├─ kind=request → learn_request（不带 token）
           写 note、刷新轮询、还原设置窗      ├─ kind=probe（仅 --probe）→ 自检回执
                                           └─ WM_CLOSE / 父进程消失 → exit 3
```

- **为什么是子进程**：主进程里再开一个事件循环会嵌套消息泵；而且 WebView2 的初始化可以阻塞几十秒（本机实测 175 s 冷启动），放在托盘进程里就等于整个应用卡死。子进程还自带一个好处：`Clear` 只要 `Child::kill()`，页面里的钩子立刻失去写盘的机会。
- **捕获通道不变**：`location.href = 'quotatray://capture?…'` + `with_navigation_handler` 返回 `false`。页面拿不到任何 IPC；`--probe` 只是用钩子自己的发送函数发一条回执。
- **数据目录**：交互路径用 `%LOCALAPPDATA%\com.shawnqi.quotatray\EBWebView-login`（与现版 Tauri 的 `EBWebView` 同父目录、不同 profile——一个 profile 不能被两个浏览器进程同时打开，而现版可能正在运行）。`QUOTA_TRAY_APP_DATA_DIR` 有值时（自检隔离）落在该目录下。
- **`Clear` 的跨进程语义**：子进程看不到 `cred_epoch`，所以 `login::settle` 在子进程退出时比对「派生时的 epoch」，不同就**再清一次凭证**并把 note 换成「Captured token was not kept」。这把「子进程刚好在 Clear 之前写完」的窗口也堵住了。

## 4. 发现

### 4.1 `verify_user_token` 从未真正拒绝过候选（已修）

`quota-core/src/deepseek/client.rs` 只判断 `(200..300)`，而平台对无效 token 回的是：

```
HTTP/1.1 200 OK
{"code":40003,"msg":"Authorization Failed (invalid token)","data":null}
```

`--test-login` 的拒收探针第一次跑就撞上：假 token 被判「已验证」，日志里出现 `probe mode refused to save a verified token`（探针模式不写盘，才没有把假 token 存进去）。手动粘贴的 `token-expired` 分支因此也是死代码。

修法：2xx 之后**读 body**，`token_accepted` 只在「没有非零 `code`、且 `data` 不是 `null`」时算通过；不认识的形状（比如代理回了一张 HTML）按通过处理——厂商换信封不该把用户锁在登录外面，而数字本身在面板里仍会以 `Fetch failed` 暴露。单测 `a_2xx_body_is_only_a_pass_when_the_envelope_is_clean` 钉住三种失败形状与三种通过形状。

`--test-deepseek` / 面板取数的路径本来就会把错误信封变成 `parse-error`（`parse_usage` 失败），所以只影响「验证后保存」这一处。

审查又指出这条修复本身的两个坑，都已改掉：判成「拒绝」后原本立刻 `return`，于是**一个被记忆的坏 base 会永久锁死有效 token**（`candidate_bases()` 把记住的排第一）——现在 `continue` 走完候选；`data: null` 原本也判拒绝，但「本月没有任何用量」的合法账号可能长这样，**误杀等于用户彻底登不进去**，现在只认非零 `code`。另外 `resp.text()` 失败原本被 `unwrap_or_default()` 变成空 body（→ 判通过），现在是 `network-error`。单测同步覆盖这两种形状。

### 4.2 自检的环境陷阱（都已处置）

1. **WebView2 冷启动 175 s**：不是 bug，但任何「从进程启动开始算」的超时都会误判。探针把 4.5 s 计时起点放在**窗口就绪之后**，父进程给 240 s 预算并复用热 profile。
2. **profile 会被残留的浏览器进程锁住**：`HRESULT(0x800700AA)`（"请求的资源在使用中"）。`--test-login` 因此第一次失败就删目录重试一次；`--test-signin` 用另一套 profile，两者互不干扰。
3. **子进程在 WebView2 初始化期间收不到 `WM_CLOSE`**（事件循环被占住）。`--test-signin` 投递后等最多 8 s，超时就 `kill_child`（**不**走 `cancel`，让退出轮询照常 `settle`）——这条路径和用户点关闭在父进程侧是同一段代码，日志会写明走的是哪条。
4. **`--test-signin` 结束时窗口若还在，会留下孤儿浏览器进程**：清理逻辑在检查结束时无条件 `cancel`，运行完 `spike/p4/list_webview_procs.ps1` 查过 `msedgewebview2.exe` 没有残留。

## 4.3 独立审查（explore + code-review skill，只读）

审查抓到 **1 Critical / 3 Major / 9 Minor**，全部处置：

| 级 | 发现 | 处置 |
|---|---|---|
| Critical | `parent_alive` 把「父进程已不存在」也当成活着（`Err(_) => true` 吞掉了 `ERROR_INVALID_PARAMETER`）→ 点托盘 Quit 后登录窗永远留着，还连带把 `EBWebView-login` 锁给下一次登录 | 只有 `ERROR_INVALID_PARAMETER` 判死，其余失败（真正的 access denied）保持保守。实测：`--login-host --parent-pid 999999` 在初始化结束后打印 `parent exited, closing` 并退出 3 |
| Major | `settle` 的 epoch 兜底会清掉**用户在登录期间手动粘贴并保存**的凭证（`cred_epoch` 也被 `save_credentials` +1），而它想防的 Clear 场景根本走不到 `settle`（`cancel` 把 `Child` 取走了） | 整段 epoch 兜底删掉：Clear 的顺序保证（`cancel` = kill + wait，**之后**才 `credentials::clear()`）已经足够，父进程不该再有第二套推断 |
| Major | `verify_user_token` 判拒后立刻 `return`，被记忆的坏 base 会永久锁死有效 token；`data: null` 会误杀零用量账号；`text()` 失败被当成通过 | 见 §4.1 末段（继续走候选 / 只认 `code` / 读 body 失败按网络错误） |
| Major | `--test-signin` 无条件覆盖并清空目标 config 目录的凭证文件，`--config-dir` 指到真实 profile 就不可恢复 | 开始前把原文件字节存进 `SIGNIN_CREDENTIAL_BACKUP`，`finish_signin` 无论走哪条路径都还原（原本没有文件就删掉）。日志 `signin: credential file restored` |
| Minor | 「子进程收不到 `WM_CLOSE`」的兜底路径必然 FAIL（`kill` 给的是退出码 1，note 成了「could not open」） | `login::kill_child` 置 `killed`，`settle` 把它读成「已取消」（note 与正常关窗一致），自检接受 `3` 或 `1` |
| Minor | 离线时 `reject=ok` 也成立（`network-error` 同样不改文件）→ 假 PASS | 只有 `token-expired` / `http-4xx`（服务器明确拒绝）才算拒收，其余记 inconclusive |
| Minor | 登录期间用户主动关掉设置窗，控制台随后关闭会把窗又弹回来 | `settings::close` 清 `login.reopen_sheet` |
| Minor | 第二次点「Sign in…」是死按钮（现版会 refocus 已有登录窗） | `login::open` 找到该子进程的控制台窗口并 `SetForegroundWindow`（复用 `shell::focus`） |
| Minor | `FindWindowW` 按标题找，可能先命中上一次运行留下的孤儿窗口 → 自检一直等不到自己的窗 | 改成 `EnumWindows` + `GetWindowThreadProcessId == pid`（`login::console_hwnd`），生产路径与自检共用 |
| Minor | 验证通过但文件没写成功也退 0，父进程显示「Signed in」 | 子进程新增退出码 **4**，父进程 note 改成「Verified, but the credential file could not be written — try again.」 |
| Minor | `--test-login` 的 PASS 与 `verify=` 无关，脚本容易误读 | 输出一行说明「verify 仅供参考，结论是下面的通道判定」；退出码语义不变 |
| Minor | 两个进程共用 `deepseek-credentials.json.tmp`，交错 rename 可能产生半截文件，进而让一次性 legacy import 重跑、把刚清掉的旧凭证导回来 | tmp 名带 pid：`deepseek-credentials.<pid>.tmp` |
| Minor | `settle` 的 `notify_waiters()` 在轮询正 `await fetch` 时会丢 | 改 `notify_one`（留 permit），登录后数字立刻刷新而不是等一个周期 |

审查还确认了几件事：新代码没有把 token 写进任何日志（只有 pid、退出码、错误码、布尔与 base URL）；`--probe` 模式不可能落盘；捕获仍是取消导航、页面没有 IPC 面；Clear 的 kill+wait→clear 顺序是对的；没有找到 `RefCell` 重入；`try_wait` 不会吞掉退出码；`tao`/`wry` 在 lock 里只有一份。

## 4.4 发版前对 `main` 的复查（2026-11-15）

v1.0.2 定版后又对 `main` 上的两个提交（`5d95f1f` 修复 + `f92748f` 定版）做了一轮只读审查，报 **2 Major / 10 Minor**，全部处置（`c7f0594` + `96c8a3c`，并镜像回本分支）：

| 级 | 发现 | 处置 |
|---|---|---|
| Major | 修复把 `Err` 从死代码推成热路径，却没有任何重试：页面按 `lastToken` 去重不会再发，完成态 token 上的一次抖动等于白登一次 | 子进程与 Tauri 版都加了「同一候选遇临时性失败重试一次」（`network-error` / `rate-limited` / `server-error` / `http-5xx`）；`token-expired` 不重试——那是答案，不是抖动 |
| Major | `AGENTS.md` 的不可回退陷阱仍写「2xx 即通过」，与本次修复相反 | 改写成信封规则 + 走候选 + 临时重试；后续 agent 不会再把它「简化」回去 |
| Minor | `last_error` 被循环顺序覆盖：被拒 token 可能被报成 `http-404` | 单独记 `refused`，结束时优先返回 `token-expired` |
| Minor | 只认无符号整数 `code`；`"40003"` / 负数会放行 | 新增 `code_is_error`：数字与数字字符串、任意非零都算错误 |
| Minor | 200 + 空 body 算通过，与旁边注释矛盾 | 把「读不出」与「只是空」分开写清楚（空 body 仍通过，理由写在注释里） |
| Minor | 单测只钉 `token_accepted`，没钉 `verify_user_token` 调用它 | 该缺口如实记录：谓词有单测，**接线**靠运行时证据（下面那条） |
| Minor | `AGENTS.md` 说 `--test-login` 只发一次 `amount`、tag 停在 v1.0.0 | 两处措辞按现状改写 |
| Minor | `STATUS.md` 快照时间晚于提交时间；「只剩两件」列了三步 | 时间改对，改成「只剩发布链」 |
| Minor | bug 模板的版本示例仍是 `v1.0.0` | 改成 `v1.0.2` |

审查同时确认：移植保真（与 `quota-core` 版逐行只差三处 doc comment）、记忆的 base 只在真正接受后才写、`data:null`/`code:0`/非 JSON/空 body 不会误杀、粘贴路径把 `token-expired` 映射成「invalid or expired — not saved」且不清空输入框。**缺的运行时证据也补上了**：修复后 `--test-login` 在真机打印 `verify=ok`，四个窗口 OK、`LoginWindow capture channel OK` 与 `fetch capture OK`。

## 5. 陷阱（给后面接手的人）

1. **设置窗在登录期间是「隐藏但记住」**：`login::open` 只翻 `settings_open` 并 `hide()`，不动 `skills`/`menu`；`reopen_sheet` 决定退出时是否 `settings::open`。不要在 `login::open` 里调 `settings::close`——那会顺带触发「关闭后收起面板」的判定。
2. **`settle` 是异步的**：它跑在 250 ms 的退出轮询上，任何断言（包括自检）都要等它落地，不能 `kill` 完立刻查 `settings_open`。
3. **`cancel` 与 `kill_child` 语义不同**：`cancel` 把 `Child` 从槽位取走（Clear 用，退出轮询不再 `settle`）；`kill_child` 只杀进程、留在槽位（自检用，让轮询照常 `settle`）。写反了就是「Clear 后设置窗不回来」或「自检永远 PASS 不了某一条」。
4. **子进程的 stdout 是给父进程读的契约**（`login-host: window ready` / `sender=ok` / `fetch=ok` / `reject=ok` / `verdict=…`），其余日志走 `slint.log` 与 stderr。改这几行会同时改 `--test-login` 的译文。
5. **`verify_user_token` 的 2xx 不等于通过**（§4.1）；任何新的「验证」入口都必须走 `token_accepted` 或等价判断。
6. **探针模式永远不写凭证**：即使 profile 里还有会话、真 token 先到（日志 `probe saw a real console token, not saving`），`--probe` 也只验证、不落盘。
7. **父进程退出检测不能把 `OpenProcess` 的任何失败都当「活着」**：父进程退出后 pid 立刻失效，`OpenProcess` 回 `ERROR_INVALID_PARAMETER`——那正是「已死」，把 `Err(_)` 一律当活着等于这条兜底从不生效（审查 C1，已修）。
8. **凭证的跨进程互斥只有一条**：Clear 的 `kill` → `wait` → 清文件。不要再加「epoch 变了就清」这类父进程推断——`cred_epoch` 也被手动粘贴 +1，那会把用户刚保存的凭证一起抹掉（审查 M2）。
9. **`--test-signin` 会写凭证文件**，所以它自带快照/还原；新写「会动凭证」的自检时照抄这个模式，别假设 `--config-dir` 一定是临时目录。
10. **自检的 PASS 必须依赖真实副作用**：`reject=ok` 只认服务器明确拒绝（`token-expired` / `http-4xx`），网络错误不算；`settled` 认 `code 3`（用户关窗）或 `1`（父进程杀），「任何退出」不算。

## 6. 回归（本次全部重跑）

| 检查 | 结果 |
|---|---|
| `--test-windows` | PASS（四窗 + 面板 Settings 动作） |
| `--test-dialogs` | PASS（4 条） |
| `--test-active-tab` | PASS（5 条） |
| `--test-settings`（`--max-settings-h 300`） | PASS（落盘重启读回、四窗主题、夹取滚动、Clear、拒收粘贴、在途作废） |
| `--test-menu` / `--test-skills` | PASS（定位 / 逐行动作 / 失焦关闭；124 技能 / 134 行 / 滚动） |
| `--test-tray-click` / `--test-blur` | PASS（两次点击；失焦 180 ms） |
| `--test-corner` / `--test-tab-anim` | PASS（2.25× 下 gap 27.0、drift 0 px；30 次连点 178 采样 drift 0） |
| `spike/p2/heights_ref.py --compare` | PASS（17 个变体 Δ=0.00） |
| `cargo test`（quota-core） | **41 passed**（新增 envelope 单测） |
| `cargo test`（src-tauri） | 3 passed（本次未改，随共享 crate 一起跑） |
| `cargo fmt --check` / `cargo clippy -D warnings`（两个 crate） | 干净 |

## 7. 没证明的

- **成功登录（exit 0）那条 settle 分支没有自动证据**：它需要真 token，而真 token 只有人能签出来。P0 在 2026-11-15 17:49 用旧壳走通过一次真实闭环（子进程 `token verified and saved` → 现版 `--test-deepseek` 读到同一文件），本阶段的子进程写盘代码就是那一段迁进 `quota-core` 的版本；父进程侧「退出 → 还原设置窗 → 写 note → 刷新轮询」由 `--test-signin` 第 1 轮（exit 3）完整覆盖，与 exit 0 只差一句文案。
- **真实网络下的失败文案**：`rate-limited` / `server-error` / `network-error` 三条只有单测级别的把握（客户端错误码映射），没有在真机上打出来过。
- **多屏 / 混合 DPI 下的 900×680 居中**：只用本机 225% 单屏验证；`center()` 用 `outer_size()` 与主显示器，跨屏没有现场。
- **`WM_CLOSE` 在冷 profile 下不被应答**：已按上述 fallback 处理，但「用户在控制台卡住时如何处理」不是本阶段的范围——真机上用户可以用任务管理器，进程退出后父进程同样会 `settle`（`--parent-pid` 与退出轮询都覆盖）。
- **父进程退出时子进程可能还卡在 WebView2 初始化里**：`--parent-pid` 的存活检查跑在事件循环里，所以那个窗口要等初始化结束（冷 profile 可达 175 s）才会自己关；期间它的 profile 锁会让下一次 Sign in 以 `0x800700AA` 失败一次，父进程显示「The sign-in window could not open.」。这不是正确性缺陷，但没有现场验证过「用户此时点托盘退出」的时序。
- **`--test-signin` 的退出码恒为 0**：与 P2/P3 的检查一致，判定看 `verdict=PASS|FAIL` 那一行。
- **失败文案的三种网络错误**（`rate-limited` / `server-error` / `network-error`）仍然只有单测级别的把握。

## 8. 复现

```bash
cd ../quota-tray-slint
export SKIA_BINARIES_URL='https://ghproxy.net/https://github.com/rust-skia/skia-binaries/releases/download/{tag}/skia-binaries-{key}.tar.gz'
(cd slint && cargo build)

EXE=./slint/target/debug/quota-tray-slint.exe
$EXE --test-login                 # 凭证来源 + 真实 amount 校验 + 探针（热 profile ~6 s）
$EXE --test-login --verbose       # 多打一行 usageBase
$EXE --test-login --config-dir "$(pwd -W)/.shots/p4/cfg-login"   # 用隔离 profile 跑

$EXE --test-signin                # Sign in 接线 + 用户关窗 + Clear（~2.5 s）

# 交互：托盘左键 -> ⚙ -> Sign in…
$EXE
```

日志同时写 stderr 与 `<config>/slint.log`（`SLINT_LOG` 可改路径）；子进程与父进程共用同一个文件。

## 9. 文件

```
slint/src/login_host.rs       --login-host：tao + wry、注入钩子、取消导航、验证与落盘、探针
slint/src/login.rs            父进程：派生、藏/还原设置窗、退出轮询、settle、Clear 的 cancel、--test-login
slint/src/checks.rs           --test-signin 的两轮（start_signin 起）
slint/src/main.rs             --login-host / --test-login 的分派（单实例互斥量之前）
slint/src/shell.rs            Mode::SignIn、Shell.login
slint/src/win/settings.rs     「Sign in…」接 login::open；Clear 先 login::cancel
quota-core/src/deepseek/client.rs   verify_user_token 读 body + token_accepted + 单测（§4.1）
slint/Cargo.toml              + tao / wry / url
```

## 10. 给 P5 的提醒

- 这个子进程模式改了两处发布面：`slint/Cargo.toml` 新增 3 个依赖（tao/wry 与 Tauri 版同一套底层），以及 `quota-core` 的行为修复。
- **`verify_user_token` 的修复已经单独开了 PR**（`fix/verify-before-save`，[#4](https://github.com/shawn-0106t/quota-tray/pull/4)）：`main` 上没有 `quota-core/`（P1 只在这条分支上），所以它是**移植**到 `src-tauri/src/deepseek/client.rs` 的同一份修复 + 单测，外加 `PLAN.md` §5.2 与 `CHANGELOG.md` 的 `[Unreleased]`；`credentials.rs` 的 tmp 名带 pid 那处**没有**带过去（那是 P4 引入第二个写者才需要的）。合并后要不要打 `v1.0.2` 是发布决策，尚未拍板。
- 两条线收敛时：`quota-core/src/deepseek/client.rs` 已经是修好的版本，`src-tauri` 那份在抽取完成的那一刻应当删掉，不要留两份实现。
- 若 P5 要把新壳并进发布线，`profile.webview` 的路径规则要与现版统一（现在新壳用 `EBWebView-login`，现版用 `EBWebView`），并决定便携模式下这个目录是否跟着 exe 走（当前固定 `%LOCALAPPDATA%`，与 Tauri 一致）。
