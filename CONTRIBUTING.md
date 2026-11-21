# Contributing

English | [中文](#贡献指南)

Thanks for looking at Quota Tray. It is a small, single-purpose app — the fastest way to
get a change merged is to keep it that way.

## Ground rules

- **Scope**: Windows 10/11 only, one tray icon, two providers (Kimi Code plan quota and
  DeepSeek balance/usage). Features that turn it into a general dashboard, add a third
  provider, or need a background service are out of scope.
- **No telemetry, no third-party frontend framework, no web fonts, no CDN assets.** The
  frontend is vanilla TS + CSS variables; the backend is Rust on Tauri 2.
- **Read [`AGENTS.md`](AGENTS.md) and [`PLAN.md`](PLAN.md) first.** `PLAN.md` is the design
  and behaviour contract; where code and plan disagree, the plan wins until it is amended
  in the same change.
- **Security issues never go to public issues.** Use the private channel in
  [SECURITY.md](SECURITY.md).
- **Be excellent to each other.** All community spaces follow the
  [Code of Conduct](CODE_OF_CONDUCT.md).

## Getting started

Prerequisites: Windows 10/11 with the WebView2 runtime, Node 22+, a stable MSVC Rust
toolchain, and the Kimi Code CLI signed in if you want live Kimi data.

```sh
npm install
npx tauri dev            # full app
npx tauri build          # release exe in src-tauri/target/release/quota-tray.exe
```

Runtime self-checks are command-line flags and work next to a live instance:

```sh
./src-tauri/target/release/quota-tray.exe --test-fetch
./src-tauri/target/release/quota-tray.exe --test-deepseek --verbose
./src-tauri/target/release/quota-tray.exe --test-usage
./src-tauri/target/release/quota-tray.exe --test-update
./src-tauri/target/release/quota-tray.exe --test-ui
./src-tauri/target/release/quota-tray.exe --test-login
```

They read *your* local credentials. `--test-deepseek --verbose` prints a credential
*source*, never a value — keep it that way.

## Required checks

CI (`.github/workflows/ci.yml`) must be green; branch protection requires the build job.
Run the same commands locally before opening a PR:

| Gate | Command |
|---|---|
| Secrets | gitleaks over the full commit history (CI) |
| Version carriers | `python scripts/release/check_versions.py` — `package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`, `Cargo.lock` must agree |
| Design tokens | `python scripts/parity/check_tokens.py` — `src/theme.css` and `slint/ui/tokens.slint` must agree, token by token, shadow geometry included |
| Rust format | `cargo fmt --check` (in `src-tauri` and `quota-core`) |
| Rust lints | `cargo clippy --all-targets --locked -- -D warnings` (in `src-tauri`); `cargo clippy --all-targets -- -D warnings` (in `quota-core`, no `--locked`: its lockfile is generated) |
| Rust tests | `cargo test --locked` (in `src-tauri`) and `cargo test` (in `quota-core`) — the shared data layer holds most of the suite |
| TypeScript | `npx tsc --noEmit` |
| Frontend build | `npm ci && npm run build` |
| Code scanning | automatic — CodeQL default setup runs on PRs and on `main` |

GitHub's secret-scanning **push protection** is enabled: a push whose diff contains a
detected credential is rejected before it lands. If it is a genuine false positive, allow
it from the push dialog; otherwise rotate the credential and rewrite the commit.

Never commit secrets, tokens or credentials; CI scans the entire history as well.

## UI changes

The design surface is reviewed, not eyeballed. After any visual change, regenerate the
derived artifacts and keep them in the same PR:

```sh
PYTHONUTF8=1 python demo/build_ui_demo.py     # all windows on one page
PYTHONUTF8=1 python demo/make_readme_shots.py # docs/screenshot-*.png
PYTHONUTF8=1 python demo/check_contrast.py    # parses theme.css; every pair must pass
PYTHONUTF8=1 python scripts/parity/check_tokens.py  # theme.css vs slint/ui/tokens.slint
```

`demo/ui-demo.html` and `docs/screenshot-*.png` are generated — never hand-edit them. The
frozen demos (`demo/panel-demo-{a,b,c,d,e}.html`) are history and are not evolved.

## Documentation

This project keeps both languages in one file rather than a `.zh-CN` companion:

- `README.md` — English first, then 中文. The English half is authoritative.
- `CHANGELOG.md` — one section per release, English notes then the 中文 mirror.
- `PLAN.md` / `STATUS.md` are in Chinese; `AGENTS.md` is in English; `docs/SPEC.md` is the
  inherited Chinese spec with an English mirror in `docs/SPEC_EN.md`. `PARITY.md` (the
  Tauri-vs-Slint matrix) is in Chinese and carries a check or a reason per row.

A behaviour change updates `PLAN.md` (the contract), `AGENTS.md` (the traps worth
remembering) and `CHANGELOG.md` in the same PR. A change that reaches the Slint shell also
updates its `PARITY.md` row and evidence.

## Submitting changes

1. Fork the repo and create a topic branch.
2. Use [conventional commits](https://www.conventionalcommits.org) with short subjects:
   `feat:`, `fix:`, `docs:`, `chore(deps):`.
3. Open a pull request using the template and fill in every section that applies. Do not
   bypass required checks — fix the cause.
4. Dependency and CI-action updates are managed by Dependabot; please do not open
   competing upgrade PRs.

---

# 贡献指南

[English](#contributing) | 中文

谢谢你愿意看 Quota Tray。它是个小而单一用途的程序——让改动合进来最快的办法，就是保持
这一点。

## 基本规则

- **范围**：仅 Windows 10/11，一个托盘图标、两个 provider（Kimi Code 套餐额度、
  DeepSeek 余额与用量）。把它做成通用仪表盘、加第三个 provider、或需要常驻服务的特性
  都不在本项目范围内。
- **不做遥测、不引第三方前端框架、不用 web 字体、不用 CDN 资源。** 前端是 vanilla
  TS + CSS 变量，后端是 Tauri 2 上的 Rust。
- **先读 [`AGENTS.md`](AGENTS.md) 与 [`PLAN.md`](PLAN.md)。** `PLAN.md` 是设计与行为的
  契约；代码与它不一致时，在同一次改动里先改契约。
- **安全问题绝不进公开 issue。** 走 [SECURITY.md](SECURITY.md) 的私密渠道。
- **互相尊重。** 所有社区空间遵循[行为准则](CODE_OF_CONDUCT.md)。

## 上手

前提：Windows 10/11 + WebView2 运行时、Node 22+、stable MSVC Rust 工具链；想要真实的
Kimi 数据还需要本机已登录 Kimi Code CLI。

```sh
npm install
npx tauri dev            # 完整程序
npx tauri build          # 产物在 src-tauri/target/release/quota-tray.exe
```

运行期自检是命令行开关，可与正在运行的实例并存：

```sh
./src-tauri/target/release/quota-tray.exe --test-fetch
./src-tauri/target/release/quota-tray.exe --test-deepseek --verbose
./src-tauri/target/release/quota-tray.exe --test-usage
./src-tauri/target/release/quota-tray.exe --test-update
./src-tauri/target/release/quota-tray.exe --test-ui
./src-tauri/target/release/quota-tray.exe --test-login
```

它们读的是**你自己的**本地凭证。`--test-deepseek --verbose` 只打印凭证**来源**，从不打
印值——这条别破。

## 必过检查

CI（`.github/workflows/ci.yml`）必须全绿，branch protection 要求构建 job 通过。开 PR
前在本地跑同样的命令：

| 门禁 | 命令 |
|---|---|
| 敏感信息 | gitleaks 扫全历史（CI） |
| 版本号一致 | `python scripts/release/check_versions.py`——`package.json`、`package-lock.json`、`tauri.conf.json`、`Cargo.toml`、`Cargo.lock` 必须相同 |
| 设计 token | `python scripts/parity/check_tokens.py`——`src/theme.css` 与 `slint/ui/tokens.slint` 必须逐项一致，含投影几何 |
| Rust 格式 | `cargo fmt --check`（在 `src-tauri` 与 `quota-core`） |
| Rust lint | `cargo clippy --all-targets --locked -- -D warnings`（在 `src-tauri`）；`cargo clippy --all-targets -- -D warnings`（在 `quota-core`，不带 `--locked`：它的锁文件是生成的） |
| Rust 测试 | `cargo test --locked`（在 `src-tauri`）与 `cargo test`（在 `quota-core`）——共享数据层承载大部分测试 |
| TypeScript | `npx tsc --noEmit` |
| 前端构建 | `npm ci && npm run build` |
| 代码扫描 | 自动——CodeQL default setup 在 PR 与 `main` 上运行 |

GitHub 的 secret scanning **push protection** 是开着的：diff 里带被识别出的凭证时，推送
在落地前就被拒。确认是误报可以从推送对话框里放行；否则请轮换凭证并改写那次提交。

绝不提交密钥、token 或凭证；CI 还会扫描整个提交历史。

## UI 改动

设计面是评审出来的，不是眼睛估的。任何视觉改动都要重新生成派生产物，并与代码同一个 PR：

```sh
PYTHONUTF8=1 python demo/build_ui_demo.py     # 所有窗口一页
PYTHONUTF8=1 python demo/make_readme_shots.py # docs/screenshot-*.png
PYTHONUTF8=1 python demo/check_contrast.py    # 读 theme.css；所有对必须通过
PYTHONUTF8=1 python scripts/parity/check_tokens.py  # theme.css 与 slint/ui/tokens.slint
```

`demo/ui-demo.html` 与 `docs/screenshot-*.png` 是生成物，**不要手改**。冻结的 demo
（`demo/panel-demo-{a,b,c,d,e}.html`）是设计史，不再演进。

## 文档

本项目把双语放在同一个文件里，而不是配 `.zh-CN` 副本：

- `README.md` —— 英文在前、中文在后，英文为权威版本。
- `CHANGELOG.md` —— 每个 release 一节，英文在前、中文镜像在后。
- `PLAN.md` / `STATUS.md` 为中文，`AGENTS.md` 为英文；`docs/SPEC.md` 是继承来的中文
  规格，英文镜像在 `docs/SPEC_EN.md`；`PARITY.md`（Tauri 与 Slint 的对齐矩阵）为中文，
  每行都带证据或理由。

行为变更要在同一个 PR 里同时更新 `PLAN.md`（契约）、`AGENTS.md`（值得记的坑）与
`CHANGELOG.md`。触及 Slint 壳的行为还要同步它的 `PARITY.md` 行与证据。

## 提交变更

1. Fork 本仓库并创建 topic branch。
2. 使用 [conventional commits](https://www.conventionalcommits.org)，subject 保持简短：
   `feat:`、`fix:`、`docs:`、`chore(deps):`。
3. 用模板开 pull request，填齐每个适用小节。不要绕过 required checks——修根因。
4. 依赖与 CI action 的升级由 Dependabot 管理，请不要开竞争的升级 PR。
