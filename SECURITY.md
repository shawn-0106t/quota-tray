# Security Policy / 安全策略

English | [中文](#安全策略)

## Supported versions

Only the latest release receives security fixes. There is no long-term branch.

| Version | Supported |
|---|---|
| latest tag | ✅ |
| older tags | ❌ (upgrade first) |

## Reporting a vulnerability

Please use GitHub's **private vulnerability reporting** (Security tab → *Report a
vulnerability*). Do **not** open a public issue or discussion for a security matter, and do
not paste credentials into one.

## What this app touches

The threat model is small on purpose; these are the boundaries worth checking.

- **Kimi credentials** are read only from `~/.kimi-code/` (`credentials/kimi-code.json` or
  `api_key` in `config.toml`) and sent as a Bearer token to
  `https://api.kimi.com/coding/v1/usages` — nowhere else, never logged.
- **DeepSeek credentials** live in `shared_dir()/deepseek-credentials.json`
  (`%APPDATA%\QuotaTray\shared`; portable mode: the exe directory), values sealed
  with Windows DPAPI (`dpapi:` prefix, keys readable). They are sent only to
  `api.deepseek.com` and `platform.deepseek.com`. A DPAPI blob does not decrypt under
  another Windows account — that is intentional.
- **The DeepSeek sign-in window** loads `platform.deepseek.com` and is given **no** Tauri
  IPC surface: the only thing that leaves that page is a cancelled navigation carrying a
  candidate token, and the hook only accepts a token from `platform.deepseek.com` itself
  (an SSO or help page's own `Bearer` header is ignored). The token is verified with a
  real request before it replaces the stored one.
- **The four app windows** (panel, settings, skills, tray menu) are local pages and share
  the application-command surface: Tauri's ACL gates plugin commands, not the app's own,
  and the capability list covers `core:default` only. The boundary that matters is the one
  above — remote content never gets a command surface — so anything that would let
  untrusted text reach those pages as markup would be a real finding.
- **No telemetry, no auto-update check of its own.** The app also calls the GitHub API to
  compare the local Kimi CLI version with the latest release; nothing about the user is
  sent.
- **Autostart and settings** write HKCU and the config directory only. The app never asks
  for elevation and never writes HKLM.
- Logs (`webview.log`, the local usage index) record counts and states — never tokens,
  keys or message content.

## Examples worth reporting

- A token or API key leaving the machine, or landing in a log, a cache or a crash report.
- The sign-in window gaining any IPC/command surface, or capturing anything beyond the
  candidate token.
- Path handling that lets a crafted config, session log or credential file read or write
  outside its directory.
- Privilege escalation attempts, or writes outside HKCU / the config directory.

Out of scope: vulnerabilities in upstream dependencies (report them upstream) and purely
visual problems (a normal issue is fine).

---

# 安全策略

[English](#security-policy) | 中文

## 支持版本

仅最新 Release 接受安全修复，没有 LTS 分支。

| 版本 | 支持 |
|---|---|
| 最新 tag | ✅ |
| 更早的 tag | ❌（请先升级） |

## 报告漏洞

请使用 GitHub 的**私密漏洞报告**（Security 标签页 → *Report a vulnerability*）。安全
问题**不要**开公开 issue 或 Discussion，也不要把凭证贴进任何 issue。

## 这个程序会碰什么

威胁模型刻意做得很小，以下是值得核对的边界。

- **Kimi 凭证**只从 `~/.kimi-code/`（`credentials/kimi-code.json` 或 `config.toml` 的
  `api_key`）读取，只作为 Bearer token 发往
  `https://api.kimi.com/coding/v1/usages`，不外发、不写日志。
- **DeepSeek 凭证**存在 `shared_dir()/deepseek-credentials.json`（`%APPDATA%\QuotaTray\shared`；便携模式则是 exe 目录），值经 Windows DPAPI
  加密（`dpapi:` 前缀，键名明文），只发往 `api.deepseek.com` 与
  `platform.deepseek.com`。DPAPI 密文换一个 Windows 账号解不开——这是有意的。
- **DeepSeek 登录窗**加载 `platform.deepseek.com`，且**不给**该页面任何 Tauri IPC
  命令面：离开页面的只有一次被取消的导航，携带候选 token，且必须先通过一次真实请求
  验证才会替换已存凭证。
- **没有遥测**，也没有自建的自动更新检查；程序会调 GitHub API 比较本机 Kimi CLI 版本
  与最新 release，不发送任何用户信息。
- **自启与设置**只写 HKCU 和配置目录，永不请求提权，永不写 HKLM。
- 日志（`webview.log`、本机用量索引）只记计数与状态，不含 token、密钥或对话内容。

## 值得报告的问题

- token 或 API key 离开本机，或被写进日志、缓存、崩溃报告。
- 登录窗获得任何 IPC/命令面，或捕获了候选 token 之外的东西。
- 路径处理缺陷：构造过的配置、会话日志或凭证文件能读写其目录之外的路径。
- 提权企图，或写到 HKCU / 配置目录之外。

不在范围内：上游依赖自身的漏洞（请报给上游）；纯视觉问题（开普通 issue 即可）。
