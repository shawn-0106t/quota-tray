# P1 `quota-core` 抽取 — 结果

> 结论先行：**数据层已经是一个不引用 `tauri` 的独立 crate，两个壳共用同一份代码；`src-tauri` 只剩窗口/托盘/登录壳。** 原有 29 个测试一个不少（26 个迁进 `quota-core`，3 个 recovery 留在 `src-tauri`），另加 14 个（格式化 9 + quota 单位/守卫 5）全绿；改前/改后两个 exe 在同一分钟内对跑，`--test-fetch` / `--test-deepseek` / `--test-deepseek --verbose` / `--test-usage` 的 8 个输出文件在掩掉 4 处 live 字段后**逐字节一致**（4 组，7 处/组）；`quota-core::format` 与 `src/common.ts` 的 45 个格式化用例**全部逐串相同**。

## 1. 验收

| 验收项 | 结果 | 证据 |
|---|---|---|
| 29 个原有 `cargo test` 全绿 | **PASS**：`src-tauri` 3（recovery）+ `quota-core` 40（26 个迁走 + 14 个新增），原 29 个测试名逐个对照**无一缺失** | `cargo test` 两个 crate；`git show HEAD` 的 29 个 `#[test]` 与当前 43 个做 `comm -23` 为空 |
| `--test-fetch` 输出不变 | **PASS**：只有 `fetchedAt` 不同；本机 OAuth token 已过期 → 两边都是 `"error": "no-token"`。另用临时 `KIMI_CODE_HOME` + 假 token 走通 HTTP 分支：两边都是 `"error": "HttpRequestException"` | `spike/p1/compare_test_output.py`；`.shots/p1/pair*/` |
| `--test-deepseek` 输出不变 | **PASS**：stdout 768 B 上下、stderr 681 B（`--verbose`；时间戳位数不同会差 1–3 B），掩掉 `fetchedAt` 后与改前逐字节相同；`--verbose` 的逐模型诊断行也相同 | 同上，4 组 |
| `--test-usage` 输出不变 | **PASS**：`scan1`/`scan2` 的 files/requests 与整段 JSON 相同，只有 `ms=` / `scannedAt` / `scanMs` 不同 | 同上，4 组 |
| 不先做全量 UI 重写 | **PASS**：`spike/` 与 `src/` 前端逻辑零改动（`common.ts` 只加了一条指向 Rust 端口的注释）；`--test-ui` 四窗口仍 OK | 见 §6 |

“逐字节一致”的准确含义：**唯一被掩码的 4 处是 live 值** —— `fetchedAt`、`scannedAt`、`scanMs`、`ms=`。它们跨进程不可能相同（时间戳长度本身随纳秒位数变化：`…341458+08:00` 6 位 / `…864360600+08:00` 9 位，chrono `AutoSi` 的正常行为）。其余所有字节（键序、serde 的数字格式、错误串、扫描计数）必须相同。

## 2. 结构

```
quota-core/                  # 独立 package，0 处 tauri 引用
  src/lib.rs                 # 模块表 + app_data_dir_override（settings 用它）
  src/credentials.rs         # Kimi 凭证链
  src/quota.rs               # 配额 DTO + fetch + 解析（+5 个单测）
  src/settings.rs            # SettingsData / config_dir / normalize_tab / autostart
  src/skills.rs              # SKILL.md 扫描
  src/update.rs              # CLI 版本检查
  src/kimi_usage.rs          # 本地会话日志扫描（只留扫描，不含 Tauri 循环）
  src/format.rs              # 新增：fmtYuan/fmtMoney/fmtTokens/formatReset/fmtUpdated/阈值/tray_tooltip
  src/deepseek/{mod,client,credentials,parse}.rs
  examples/fmt_probe.rs      # 与 TS 对拍用的只读探针
  tests/fixtures/*.json      # 从 src-tauri/tests/fixtures 迁来（4 个）
src-tauri/
  src/lib.rs                 # pub use quota_core::{credentials, deepseek, quota, settings, skills, update}
  src/kimi_usage.rs          # 只剩 run() 循环：spawn_blocking + emit + 日志
  src/tray.rs                # tooltip_text 改调 quota_core::format::tray_tooltip
  （其余 login/panel/polling/recovery/state/theme_watch/main 不变）
```

搬迁的 10 个文件里，**7 个与改前逐字节相同**（`credentials`、`settings`、`skills`、`update`、`deepseek/{mod,client,credentials}`）；`quota.rs` 只多 `Debug` derive 与 5 个测试；`parse.rs` 只改了一行注释；`kimi_usage.rs` 切掉 `run()` 与两个 `use`。

`src-tauri` 用 `pub use` 保住全部 `crate::quota` / `crate::deepseek` / `crate::settings` 路径，所以 `login.rs`、`polling.rs`、`state.rs`、`tray.rs`、`recovery.rs`、`main.rs` 一行未改。`app_data_dir_override` 随 `settings::config_dir` 一起下沉（`lib.rs` 里 `pub use` 回来），否则 crate 边界会把 override 拆成两处实现。

**不建 workspace**：cargo 拒绝根目录之外的 member（`is not hierarchically below the workspace root`，已实测）；把 workspace 根放到仓库根又会把 `target/` 和 `Cargo.lock` 从 `src-tauri/` 挪走，而 `scripts/release/check_versions.py`、release 工作流、CI 的 rust-cache 都读 `src-tauri/` 那一份。代价是测试分两条命令跑：`(cd src-tauri && cargo test)` + `(cd quota-core && cargo test)`。

## 3. 输出一致性怎么验的

1. 改前先构建并留住 exe：`git stash -u` → `cargo build` → 拷成 `.shots/p1/quota-tray-before.exe` → `git stash pop` → 再构建改后 exe。
2. 同一分钟内对每种模式前后各跑一次（4 个模式 × 2 个 exe × 4 组 = 32 次运行）：
   `--test-fetch`、`--test-deepseek`、`--test-deepseek --verbose`、`--test-usage`。
3. `PYTHONUTF8=1 python spike/p1/compare_test_output.py BEFORE_DIR AFTER_DIR`：
   只掩 `"fetchedAt"` / `"scannedAt"` 的值、`"scanMs"` 的数字、`ms=` 的数字，其余逐字节比对。4 组都是 `identical modulo the four live fields`（每组 7 处掩码：fetch 1、deepseek 1、verbose 1、usage 4）。

为什么必须两个 exe 同分钟跑：这台机器上 DeepSeek 账户与 Kimi 会话日志一直在动。隔 3.5 分钟直接 diff 改前/改后，会看到 `monthRequests 5975 → 5986`、`requests(7d) 4958 → 4972`、余额 `¥16.98 → ¥16.91`——全是数据在动，不是代码（`AGENTS.md` 里“两次 live fetch 的数字差异证明不了任何事”就是这个意思）。

## 4. 格式化对拍

`spike/p1/fmt_parity.py`：剥掉 `src/common.ts` 开头两行 `@tauri-apps` import → node 24 直接跑 TS（原生剥类型）→ `quota-core` 的 `fmt_probe` 示例读同一份 `cases.json` → 比 TSV。

**45 个用例全部相同**，覆盖：`fmtYuan`（0 / 5 / 100 / 1050 / 4952 / **-150** → `-¥1.50`）、`fmtMoney`（¥/$、`3.00`、`0.00`）、`fmtTokens`（`0` / `96` / `999` / `1K` / `284K` / `1.33M` / `1M`）、`fmtHitRate`（`cache --`、`cache 80%`、`cache 99.5%`）、`fmtPct`、`fmtCount`（`3,097` / `1,234,567` / `-3,097`）、`clampPercent`、`isLowBalance`（`9.99` 红、`10.00` 不红、unavailable 恒红）、`isLowCacheHit`（`59.9` 红、`60` 不红、null 不红）、`formatReset`（`Resets soon` / `1m` / `42m` / `2h 5m` / `2d 3h`）、`fmtUpdated`（`Updated 09:05` / `Updated 11/14 23:59`）。

## 5. 测试清单

| crate | 数量 | 内容 |
|---|---|---|
| `src-tauri` | 3 | recovery 分类 / 退避上限 / 日志尾行对齐 |
| `quota-core` | 26（迁走） | parse 9、deepseek credentials 4（含 DPAPI 往返）、client 2（`usage_base_from`、`sanitize_version`）、kimi_usage 6、settings 3、skills 2 |
| `quota-core` | 9（新增 format） | 上述 45 例里每个函数一条，钉住 TS 行为 |
| `quota-core` | 5（新增 quota） | `amountLeft` 1e-8 元→分（含 0.5 分进位、499999 舍去）、`isEnabled=false` → Not activated、limit=0 的除零守卫（limit 用 1，不是 0）、resetTime 四种形状、monthly 的 `priceInCents` 本身就是分 |
| 合计 | **43** | 原 29 全部保留 + 14 新增 |

## 6. 门禁与自检（本机实测）

| 项 | 结果 |
|---|---|
| `cargo fmt --check` | `src-tauri` / `quota-core` 都干净 |
| `cargo clippy --all-targets -- -D warnings` | 都干净（`quota-core` 无 `--locked`，它的 lock 不提交） |
| `cargo build --locked`（src-tauri） | 通过；`Cargo.lock` 新增 `quota-core` 条目，`quota-tray` 的依赖表去掉 reqwest/regex/base64（它们仍以 `quota-core` 的传递依赖存在） |
| `cargo test` | 3 + 40 全绿 |
| `npx tsc --noEmit` / `npm run build` | 通过（前端零改动；先跑过 `npm ci`，本 worktree 原先没有 `node_modules`） |
| `python scripts/release/check_versions.py` | 五载体 1.0.1 全 ok（`quota-core` 不是发布载体，不参与） |
| `quota-tray.exe --test-ui` | MainWindow / SettingsWindow / SkillsWindow / TrayMenuWindow 四行 OK |
| `quota-tray.exe --test-login` | `verify=ok`、`LoginWindow OK`、`LoginWindow capture channel OK`、`LoginWindow fetch capture OK` |
| `--test-update`（改前 vs 改后） | 同一行：`local=2.6.1 latest=2.6.2 updateAvailable=True checkFailed=False` |
| `npx tauri build --no-bundle`（发布路径） | 通过（5 min 48 s）；`src-tauri/target/release/quota-tray.exe` 7,201,792 B；release exe 的 `--test-deepseek` 正常、`--test-ui` 四窗口 OK（前端已嵌入，说明 path 依赖不影响打包） |

## 7. 复现

```bash
cd ../quota-tray-slint

# 1) 两个 crate 的测试
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets --locked -- -D warnings && cargo test --locked)
(cd quota-core && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)

# 2) 格式化对拍（45 例，TS == Rust）
PYTHONUTF8=1 python spike/p1/fmt_parity.py

# 3) 输出一致性：需要改前/改后两个 exe
git stash -u && (cd src-tauri && cargo build) && cp src-tauri/target/debug/quota-tray.exe .shots/p1/quota-tray-before.exe && git stash pop
(cd src-tauri && cargo build) && cp src-tauri/target/debug/quota-tray.exe .shots/p1/quota-tray-after.exe
# 对每种模式、两个 exe 各跑一次，把 8 个文件按 before/ after/ 分目录放好
PYTHONUTF8=1 python spike/p1/compare_test_output.py .shots/p1/pair4/before .shots/p1/pair4/after
```

## 8. 这一期没做的

- 没有提交/推送：改动留在工作区（`git status` 可见 `quota-core/` 与 `spike/p1/` 为新增）。
- 没有改版本号、没有动 `CHANGELOG.md`（P1 不是发布事件；`quota-core` 也不进五载体门禁）。
- 没有打 release zip：`scripts/release/make_release_zip.py` 用 `git ls-files` 取文件，未提交的 `quota-core/` 不会进包。真正发版前必须先提交这笔改动，否则 zip 里的 `src-tauri/Cargo.toml` 会引用一个不存在的 path 依赖。
- `--test-fetch` 的**成功解析分支**没有 live 证据：本机 `~/.kimi-code/credentials/kimi-code.json` 的 `expires_at` 已过期约 2.9 小时，凭证链按设计返回 `no-token`。`quota.rs` 是原样搬迁，解析逻辑由新增的 5 个单测覆盖；要让 live 跑到成功分支，先 `kimi` 登录刷新一次 token 再跑。
- 前端仍在用 `src/common.ts` 的格式化（P2 才接 Slint）。两壳并存期间，改任一侧都要跑 `spike/p1/fmt_parity.py`。

## 9. 文件

```
quota-core/                                  # 新 crate（12 个源文件 + 4 个 fixture + 1 个示例）
src-tauri/src/lib.rs                         # 模块表改 pub use；app_data_dir_override 下沉
src-tauri/src/kimi_usage.rs                  # 只剩 Tauri 循环
src-tauri/src/tray.rs                        # tooltip 改调 quota_core::format
src-tauri/Cargo.toml / Cargo.lock            # + quota-core path 依赖；去掉 reqwest/regex/base64
.github/workflows/ci.yml                     # + quota-core 的 fmt/clippy/test；rust-cache 两个目录
.gitignore                                   # + quota-core/Cargo.lock（library，不提交）
spike/p1/REPORT.md                           # 本文
spike/p1/compare_test_output.py              # --test-* 输出比对（掩 4 处 live 字段）
spike/p1/fmt_parity.py / fmt_cases.ts        # 格式化对拍
quota-core/examples/fmt_probe.rs             # 对拍用的 Rust 侧探针
spike/HANDOFF.md / AGENTS.md / STATUS.md     # 进度与规则同步
```
