# STATUS — Quota Tray

> 快照：2026-11-14 17:12（第八轮结束，已全部提交，交接给新对话）。
> 本文件是**进度快照**，不是契约。契约在 `PLAN.md`，代理规则在 `AGENTS.md`。
> 用途：换新对话 / 换 agent 时，读这一份就能接手，不需要历史对话。

## 一句话

**P1–P4 全部完成**，同日又做了三件计划外的事：托盘 / exe 图标换成自绘双条、Kimi **缓存命中与解码速度**统计上线、**所有窗口合成一页 demo**。以上全部已提交（`855035a` + `9b23b74`），工作区干净。下一步是 **P5**：打包 / 发布线 / README / 独立 code review，中间补一次真人登录验证。

## 阶段状态

| 阶段 | 状态 |
|---|---|
| P0 设计探索 + 定稿 | 完成 |
| P1 仓库化 / 导入 / 改名 | 完成（`b43b3e7`） |
| P2 视觉 + 高度 + tab | 完成（`d892eeb`…`8ab1f86`） |
| P3 DeepSeek 后端 | 完成（`13822ce`） |
| P4 设置窗分区 / 托盘合并 / 登录捕获 | 完成（`855035a`） |
| 计划外：图标 / Kimi 统计 | 完成（`855035a`） |
| 计划外：所有窗口一页 demo | 完成（`9b23b74`） |
| P5 打包 / 发布 | 未开始 |

## 提交记录（第八轮）

```
9b23b74 docs(demo): add the generated all-windows demo
855035a feat(app): settings sections, DeepSeek sign-in, tray tooltip, icon and Kimi stats
21e55ae docs(status): hand off to Phase 4
```

**为什么是两个提交而不是原计划的四个**：P4、图标、Kimi 统计在很多文件里是同一处上下文——`lib.rs` 同时多了 `pub mod login` 与 `pub mod kimi_usage`、`state.rs` 同时多了 `LoginState` 与 `kimi_usage` 字段、`tray.rs` 同时改了 tooltip 与图标 include、`common.ts` 同时有凭证与缓存类型。按文件拆会让中间那个提交**编译不过**（`pub mod` 指向尚未提交的模块）。所以：产品代码 + 其文档合成一个 `feat(app)`，demo 页单独一个 `docs(demo)`，每个提交都能独立构建。

## 本轮验证结果

| 项 | 结果 |
|---|---|
| `cargo test` | **29 passed**（原 23 + `kimi_usage` 6） |
| `--test-usage`（release） | `scan1 files=427 requests(7d)=3168 requests(today)=806 ms=104` / `scan2 … ms=30`，两遍请求数一致 |
| 与独立实现对齐 | Rust 聚合与另一份 Python 计算**逐位相同**（7d `99.51469034651184%`、today `99.68969791930937%`） |
| `--test-login`（release） | `credentialSource=file`、`verify=ok`、五个窗口 OK、`LoginWindow capture channel OK`、`LoginWindow fetch capture OK` |
| `--test-ui` / `--test-fetch` / `--test-deepseek` | 全过；Kimi 这次 `error: null`（0%），DeepSeek 余额 ¥45.44 |
| `tsc --noEmit` / `npm run build` / `tauri build --no-bundle` | 通过（release 内嵌前端，2 分左右） |
| 图标 | `ExtractAssociatedIcon` 抽 release exe = 双条；真机 GUI 起 5 s 无异常 |
| 一页 demo | `demo/ui-demo.html` 浅/深各截图核对；点 Dark 生效（headless 断言 `after-dark:dark|false|true`） |

## 第九轮要做的（按顺序）

1. **一次真人登录验证**（唯一没被自动化覆盖的一步）：点设置窗 `Sign in…` → 在窗里登账号 → 标题变 `verified, saved` → 面板出数。注意这会覆盖 `deepseek-credentials.json` 的 `userToken`（同账号等价）。
2. **P5**：
   - 打包：`npx tauri build` 单 exe + 安装包；确认登录窗不被打包遗漏；
   - 发布线：AGENTS「Release process」仍是占位，移植父仓库的 tag 触发 draft Release + gitleaks 门；
   - README（中英双语）+ `LICENSE` / `NOTICE` 校对；
   - **独立 code review**（AGENTS 要求委派 code-reviewer，证伪导向；委派前要先问用户模型 / 强度）。
3. 可选：把缓存块也放进设置窗的 Kimi 只读区（现在只在面板）；托盘图标再做一版带 DeepSeek 元素的双色标。

## 接手必须知道的

1. **图标是生成物**：`demo/make_icon.py` 生成 `icons/{icon.png,icon.ico,tray.png}` 与 `demo/icon-demo.html`。`src-tauri/build.rs` 那三行 `cargo:rerun-if-changed=icons/*` **不能删**——tauri-build 只盯 `tauri.conf.json`，删了换图标后 exe 里仍是旧图（本轮真踩过，`ExtractAssociatedIcon` 抽出来还是 K 标）。
2. **托盘只吃一张 RGBA**：所以 `tray.png` 是 48px，让 100–300% DPI 都走缩小；16px 原图在 225% 会被放大发虚。
3. **Kimi 缓存数字是本机的**：来自 `~/.kimi-code/sessions/<ws>/<sid>/agents/<agent>/wire.jsonl`，标签写 `local`；只计 `usage.record` + `usageScope=turn`（`session` 合计与 `message.meta.usage` 副本会翻倍）；命中率与速率都是**求和后再相除**。服务端 `usages` 接口没有任何 token 数据，别去那儿找。
4. **扫描节奏**：走树（已剪到 `sessions/<ws>/<sid>/agents/<agent>/wire.jsonl` 三层）约 0.1 s，只 stat 的增量约 30 ms；后台任务每 60 s 一次、**隔分钟才走树**。索引 `config_dir()/kimi-usage.json`，只留 30 天。若布局真的变了，`collect_sessions` 会自动回落到递归搜索而不是报 0。
5. **`demo/ui-demo.html` 是生成物**：`demo/build_ui_demo.py` 从真实 HTML/CSS 拼出来（每窗口 CSS 加作用域前缀），改 UI 后重跑，别手改。主题切换函数 `applyTheme()` 是页面里唯一的交互脚本（之前漏了 click 绑定，点 Dark 没反应，已修）。
6. **登录窗**：临时窗、关闭即销毁、不进 recovery；捕获走「`quotatray://` 被 `on_navigation` 取消的导航」，**远程页面没有 Tauri IPC 面，不要给它加 `remote` capability**。
7. **环境事实**：只有 Edge（截图脚本运行时探测）、本机 225% 缩放、`QUOTA_TRAY_APP_DATA_DIR` 隔离测试、`--test-ui` 不跑配额 poller 但**会跑缓存扫描**（所以它也能看到真实缓存数字）。
8. 密钥 / 密文永远不 `cat`：`%APPDATA%\QuotaTray\deepseek-credentials.json` 是 DPAPI 密文，要看只 `grep -c '"dpapi:'`。

## 环境现状（2026-11-14 17:05）

- **Kimi 配额接口这次是通的**：`--test-fetch` 返回 `error: null`，5h 与 week 都是 0%，Extra usage `NotActivated`。
- **Kimi 本地统计**：7d 99.5% / 3,169 请求 / 135.8 tok/s / 首 token 1.04 s；today 99.7% / 807 请求。
- **DeepSeek**：余额 ¥45.44（账号在动，别拿数值当验收）。
- `%APPDATA%\QuotaTray` 现在有：credentials / endpoints / models / cache / **kimi-usage.json**（新，来自本轮的缓存扫描）；仍无 `settings.json`（没点过 Save）。
- 本轮结束时无后台进程残留。

## 要你拍板

1. **托盘图标**：现在定的是纯紫双条（A 方案）；要不要再做一版带 DeepSeek 元素的双色标？
2. **P5 的发布线**：GitHub 仓库名（建议 `quota-tray`）、是否建 remote、是否沿用父仓库的 gitleaks 门。
3. **缓存块位置**：只留在面板，还是设置窗的 Kimi 只读区也列一份？

## 新对话开场建议

> 接手 Quota Tray（本地 git 仓库，无 remote）。按顺序读 `STATUS.md` → `AGENTS.md` → `PLAN.md`；要看 UI 直接开 `demo/ui-demo.html`（生成物，别手改）。P1–P4 与三大计划外改动都已完成并提交（HEAD=`9b23b74`，工作区干净，`cargo test` 29 passed）。下一步做 P5：打包 → 发布线 → README → 独立 code review，中间补一次真人登录验证（窗内输账号密码那一步）。
