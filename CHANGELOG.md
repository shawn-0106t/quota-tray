# Changelog

All notable changes to Quota Tray are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each section carries
the English notes first, then the Chinese mirror.

## [1.0.2] — 2026-11-15

### Fixed

- "Verify before save" now actually verifies. The DeepSeek platform answers
  **HTTP 200** for a bad usage token and puts the failure in the body
  (`{"code":40003,"msg":"Authorization Failed (invalid token)","data":null}`), so
  the sign-in window and the manual paste field both accepted every candidate
  they were handed — a wrong or half-finished token replaced a working one, and
  `--test-login` printed `verify=ok` for any string longer than 20 characters.
  Verification reads the envelope now, and a base that rejects the candidate is
  skipped instead of ending the walk: a remembered dead endpoint could otherwise
  lock a valid token out. (Reproduced against the live endpoint 2026-11-15; the
  balance endpoint is not affected — `api.deepseek.com` answers 401 for a bad
  key.)

### 中文

- **修复**：「验证后保存」现在真的会验证。DeepSeek 平台对无效的 usage token 回
  **HTTP 200**，失败写在 body 里（`{"code":40003,…,"data":null}`），只判状态码等于
  任何候选都算通过——登录窗与手工粘贴兜底都可能把错的或没完成的 token 顶掉原本
  正常的那个，`--test-login` 也会对任何超过 20 个字符的串打印 `verify=ok`。
  现在会读信封里非零的 `code`，并且某个候选 base 被判拒时继续试下一个：记住的坏
  base 否则会让有效 token 永远登不进去。（2026-11-15 对真实接口复现；余额接口不受
  影响，`api.deepseek.com` 对坏 key 回 401。）

## [1.0.1] — 2026-11-15

### Added

- `scripts/release/check_versions.py`, run by CI on every push and by the
  release workflow before it builds: a release is one version written in five
  files (`package.json`, `package-lock.json`, `tauri.conf.json`, `Cargo.toml`,
  `Cargo.lock`) and only one of them is read by the build, so a half-bumped
  tree used to stay green. `--tag vX.Y.Z` also checks the pushed tag.

### Changed

- Tab switching between Kimi and DeepSeek now follows Windows' own motion
  values instead of a 120ms linear fade. The card grows into the taller tab
  (200ms, entrance curve `cubic-bezier(0,0,0,1)`) or shrinks back (167ms, exit
  curve `cubic-bezier(1,0,1,1)`); both panes cross-fade over the same 167ms (the
  old content fades out on the exit curve so it covers the cell until it has
  closed), and the window is pre-sized to the taller tab before the first
  animated frame
  (and re-sized once more if data lands while that resize is in flight), and the
  animation re-reads the live height before it starts, so the card's top edge no
  longer jumps: it animates, and a data tick that lands mid-switch retargets
  instead of snapping at the end. The bottom edge, the action footer and the
  tray-corner anchoring stay put. On a clamped work area the card caps itself at
  the viewport and scrolls, so the header and the provider tab stay reachable.
  `prefers-reduced-motion` switches instantly.

### 中文

- **新增**：`scripts/release/check_versions.py`——一次发版的版本号写在五个文件里
  （`package.json`、`package-lock.json`、`tauri.conf.json`、`Cargo.toml`、
  `Cargo.lock`），而构建只读其中一个，半升级的树以前不会报错；现在 CI 每次推送
  都跑一遍，发版工作流在建 exe 之前带 `--tag` 跑（顺带校验 tag）。
- **变更**：Kimi / DeepSeek 的 tab 切换改用 Windows 原生 motion 的时长与曲线，
  不再是 120ms linear。卡片长高到较高的那个 tab（200ms，进场
  `cubic-bezier(0,0,0,1)`）、缩回 167ms（退场 `cubic-bezier(1,0,1,1)`）；两个 pane
  用同一组 167ms 交叉淡入淡出（旧内容走退场曲线，压在上面盖到格子收完才消失），
  三者同帧起跑。窗口在切换
  开始时放大到较高者（resize 期间数据到达时会再放大一次），动画起跑前再重读一次
  实时高度，所以卡片顶边不再瞬移而是平滑移动；切换途中到达的数据会让窗口与终点
  关键帧一起重新对齐，不会在收尾时「啪」一下补齐。底边、底部动作行与托盘角的贴边
  关系都不动。后端把窗口夹到工作区时，卡片改为限制在视口内并自行滚动，header 与
  provider tab 仍然点得到。`prefers-reduced-motion` 下瞬时切换。

## [1.0.0] — 2026-11-14

First stable release. Behaviour is unchanged from 0.1.0 — same app, with the
release gate now in place (full review, CI on every push, branch protection,
CodeQL, Dependabot) and the dependency set current.

### Changed

- Dependency updates: `vite` 8.3.8, `tauri-plugin-opener` 2.7.3 and `base64`
  0.23.1. The base64 major only touches the DPAPI credential blob's
  `STANDARD.encode` / `STANDARD.decode`, whose output is unchanged — the
  round-trip test and `--test-login`'s real request both still pass.

### 中文

首个稳定版本。行为与 0.1.0 一致——同一套程序，只是发布门禁到位（全量审查、
每次推送跑 CI、branch protection、CodeQL、Dependabot），依赖也升到当前版本。

- **变更**：依赖升级——`vite` 8.3.8、`tauri-plugin-opener` 2.7.3、`base64`
  0.23.1（只影响 DPAPI 凭证 blob 的 `STANDARD.encode` / `STANDARD.decode`，
  输出不变；往返测试与 `--test-login` 的真实请求都通过）。

## [0.1.0] — 2026-11-14

### Added

- Phase 1 baseline: the `kimi-planbar-tray` rust edition (Tauri 2 + vanilla
  TS/HTML/CSS) is imported at the repository root as the starting point for
  Quota Tray.
- `docs/SPEC.md` / `docs/SPEC_EN.md`: the inherited behaviour specification,
  with a scope table marking which chapters still govern the merged app.
- Phase 3 DeepSeek backend, ported from `deepseek-planbar`'s WPF service:
  balance on `api.deepseek.com`, console usage on `platform.deepseek.com`,
  the `data.biz_data` object/array tolerance, `REQUEST` excluded from tokens
  and money, today-only `days[]` aggregation, all-zero placeholder filtering
  and the top-4-by-month-cost selection. Nothing renders yet — the data stops
  at the `get_deepseek` command and the `deepseek-updated` event.
- DeepSeek credentials in `config_dir()/deepseek-credentials.json`: key names
  readable, values DPAPI-sealed behind a `dpapi:` prefix. Resolution is
  `DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` → that file → a one-time import of
  the parent app's plaintext `~/.deepseek-planbar/credentials.json` (read-only;
  the parent keeps working). A decrypt failure means "not configured".
- DeepSeek display names follow the vendor: the official `name` from
  `GET api.deepseek.com/models` wins over the built-in table, so a rename
  reaches the panel within the 24 h cache TTL without a new release. Aggregate
  console rows (`A & B`) and non-`deepseek-*` ids are shown verbatim.
- Usage endpoint candidate chain (`/api/v0/usage` → `/api/v1/usage`): a 404/410
  walks to the next candidate and the answer is remembered.
- `--test-deepseek [--verbose]` self-check, and 13 new Rust tests covering the
  DeepSeek parser (four fixtures ported from the parent repo) plus the
  credential prefix handling and a DPAPI round trip.
- A separate poller for DeepSeek with its own schedule and last-good slot, so
  one provider's failure never blanks or retimes the other. Refresh and hover
  prefetch now cover both providers.
- The Kimi/DeepSeek segmented tab and the DeepSeek pane: balance block
  (`Available` / `Insufficient balance`, `Granted … · Topped up …`), one row
  per model (official display name, month cost, token-share bar, cache hit rate
  and today's cost), and a month total row. Usage failures replace the rows
  with a single hint line; balance and usage report independently.
- The selected tab persists as `ActiveTab` in `settings.json`, so the next
  launch opens on the same provider.
- Thresholds from `PLAN.md` §5.2 are live in the pane: a cache hit rate under
  60% and a balance under ¥10 (or `is_available=false`) turn red. They only
  colour output and live in the frontend, so no unused Rust is shipped.
- The settings window gained the two read-only sections from `PLAN.md` §4: CLI
  version (with the newer release when one exists), Kimi credential source and
  OAuth expiry, and the DeepSeek credential statuses. Statuses only — the IPC
  payload has no field that could carry a token or a key.
- The DeepSeek sign-in window (`login.rs`): the console itself, with an injected
  hook that watches the `Authorization` header the page already sends (and
  `localStorage.userToken` as the fallback). It reports a candidate through a
  navigation to `quotatray://capture` that the host cancels, so the remote page
  gets no Tauri IPC surface — no `remote` capability, no ACL entry. The window
  is temporary: destroyed on close, outside the recovery set, built again for
  the next sign-in.
- The DeepSeek sign-in window now opens **above** the settings sheet instead of
  underneath it: it is `always_on_top` like every other window in the app, and
  the sheet is hidden while the console is up and restored when it closes.
- **Signing in no longer leaves a blank, dead window.** The sign-in window was
  built inside the `open_login` command, and Tauri runs a sync command on the
  main thread while `WebviewWindowBuilder::build()` dispatches to that same
  thread and waits for it — the window appeared, the command never returned,
  and everything after it (page navigation, logging, the sheet hide) never ran.
  Both login commands now hand the work to the async runtime. The sheet is also
  hidden only after the window is built, so a failure there cannot cost the user
  the sheet. Panics are logged to `webview.log` (`panic: … (file:line)`) —
  a windowed app has no console, and a silent panic looks like a hung window.
- A candidate token is verified with one real `amount` request before it
  replaces the stored one; a rejected candidate leaves the credential alone and
  the window open with the reason in its title. The console emits several
  intermediate tokens during sign-in, so a token that arrives while another is
  being verified replaces the queued one and is retried when that verification
  fails — the page never resends a token it already reported. `Clear` empties
  the credential file but keeps it, so the one-time legacy import cannot
  resurrect what the user just removed.
- The manual fallback survived: a collapsed "Paste a credential instead" block
  in the settings window verifies each non-empty field and saves only what
  passes.
- `Clear` now also closes an open sign-in window and discards any verification
  that is still in flight, so a capture cannot put the usage token back right
  after the user removed it. The settings window says what can still bring it
  back: opening "Sign in…" stores the token the console's own session already
  carries — that is that window's job, and it is why a cleared token can
  reappear without the user typing anything.
- `--test-login [--verbose]`: credential source, a real token verification, then
  the sign-in window plus a probe fired through the injected hook — `LoginWindow
  capture channel OK` proves injection, the cancelled-navigation channel and the
  host parser without a human signing in.
- The sign-in hook also reports the shape of the console's own usage request, so
  a moved endpoint and a bumped `x-app-version` are learned from the live page
  and remembered in `deepseek-endpoints.json` (`PLAN.md` §6.3). The
  `--test-login` probe carries a marker the host refuses to learn from, so a
  diagnostic run cannot overwrite what the console taught the app.
- The settings sheet scrolls when its content is taller than the window. The
  backend caps the window at 90% of the work area; on a short work area (this
  machine: 752 logical px) the expanded "Paste a credential instead" block
  (824) used to be clipped by `overflow: hidden`, so the bottom of the sheet —
  and the Save credentials button — was unreachable. The card now scrolls
  itself (`#root { max-height: calc(100vh - 56px) }`) behind a 4px themed
  scrollbar inset from its edge, and the height report uses
  `max(offsetHeight, scrollHeight)` so the window still sizes itself from the
  real content.
- The window shadow is tighter (`0 8px 18px` instead of `0 12px 40px`). The
  40px blur reached past the 28px shadow margin and was cut off by the window
  edge, which read as a hard dark ring around the card rather than a shadow.
- A bilingual `README.md` (English half, then 中文) with requirements,
  install/portable/autostart notes, the data-source table per provider, the
  privacy boundary, the self-check table and the build commands.
- `docs/screenshot-{light,dark}.png`: the README's two screenshots, cropped out
  of the all-windows demo by `demo/make_readme_shots.py` - generated like every
  other review asset, never hand-cropped.
- A release pipeline: `.github/workflows/ci.yml` (gitleaks over the full
  history, then fmt/clippy/build/test and the frontend type-check on
  `windows-latest`) and `.github/workflows/release.yml` (a `v*` tag must match
  `tauri.conf.json`, then build, package and open a **draft** Release whose
  notes come from the matching CHANGELOG section; publishing stays manual).
- `scripts/release/make_release_zip.py`: source snapshot (`git ls-files`, so it
  cannot drift from `.gitignore`) with the portable exe at the zip root, the
  staged release assets and a `sha256sum`-compatible `SHA256SUMS.txt`, all
  written through `.part` files so a failed run never leaves a mismatched pair.

### Changed

- The release ships as a single portable exe: `bundle.targets` is empty and the
  build runs `--no-bundle`, so there is no NSIS installer and no Start Menu
  shortcut. The draft release carries `QuotaTray.exe`, `QuotaTray-v0.1.0.zip`
  and `SHA256SUMS.txt`, and `scripts/release/make_release_zip.py` knows only
  that exe.
- The panel's header stamp follows the active tab (`Updated HH:MM` /
  `Fetch failed` / `Not configured`), and a DeepSeek cache older than today
  prints `Updated M/D HH:MM` instead of a bare time.
- Every money value now prints two decimals (`¥3.00`, `¥0.00`, `¥49.52`) in
  both providers. The WPF parent's "whole yuan drops the fraction" rule is
  deliberately not carried over: next to `¥49.52`, a `¥3` reads like a
  different unit.
- The DeepSeek payload is now filtered where it is produced:
  `visible_models` (drop all-zero placeholder rows, keep the top four by month
  cost) runs at the end of `fetch_diag`, so the IPC result, the
  `deepseek-updated` event and the cache carry exactly the rows the pane draws,
  and the frontend does not repeat the rule.
- Switching tabs fades the incoming pane in over 120ms while the window height
  changes in the same frame. A true cross-fade was not used: stacked panes
  measure the taller one, and the bottom-anchored window would have jumped to a
  height belonging to the tab that is not on screen.
- All four windows now share the frozen D design language: warm paper cards,
  hairline rows, uppercase micro-labels, 30/600 tabular numerals, one violet
  accent for usage and one red for failures. The settings and skills windows
  drop their old flat WPF skin, and the tray menu becomes a plain ghost list.
- The main panel and the settings window are now content-sized. The frontend
  reports `panel_height` / `settings_height`; the backend resizes with
  `LogicalSize`, re-pins the panel's bottom-right corner `PhysicalPosition`
  and re-centres the settings sheet.
- The settings sheet swaps the radio buttons and pills for the shared
  segmented control, and its window height now follows the content instead of a
  fixed 464.
- Product identity: `Quota Tray` / `com.shawnqi.quotatray` / `quota-tray.exe`,
  version reset to 0.1.0.
- Single-instance mutex renamed to `QuotaTray.SingleInstance`, so the app can
  coexist with the two parent apps during the migration.
- Config directory `%APPDATA%\QuotaTray` (portable mode unchanged: an empty
  `portable.dat` next to the exe); autostart value name `QuotaTray`; the
  diagnostic override variable is now `QUOTA_TRAY_APP_DATA_DIR`.
- Tray tooltip, window titles, fallback quit menu and the GitHub `User-Agent`
  now say Quota Tray.
- The tray tooltip carries both providers on their own lines (`Quota Tray` /
  `Kimi 5h 31% · week 12%` / `DeepSeek ¥46.81`), each with its own
  ` (fetch failed)` suffix, and either poller renews the whole text.
- The settings sheet's Save button is now `Save settings`, next to the
  credentials block's own `Save credentials`: the two actions write different
  things and no longer share a verb.
- The failure hint reads `Fetch failed` (panel header) and ` (fetch failed)`
  (tray tooltip) instead of `Update failed`, which read like the CLI version
  check had failed — that is a separate path.
- Closing the settings window now collapses the panel when it lost focus:
  the suppressed blur never arrives on its own, so the panel used to linger
  until the next click elsewhere.
- The single accent is a quiet violet (`#8c74b1` light / `#9d88bc` dark)
  instead of Kimi blue. Same jobs — usage bars, chips, focus rings — and each
  theme keeps its own lightness ladder so both stay above 3:1 on the track and
  4.5:1 for chip text.
- The `Update` chip is a tag-shaped rounded rectangle (radius 4) instead of a
  pill, and its light-theme fill rises from 10% to 18% of the accent so it
  reads as a tinted block the way the dark one already did.
- `demo/panel-demo-e.html` becomes the frozen design: D's structure and type
  with the reworked accent, an accent ring on the selected cell (the neutral
  hairline measured 1.16/1.82 against the track, under WCAG's 3:1 for a state
  indicator), and the tag chip. `panel-demo-d.html` is kept unchanged as the
  previous reference. `demo/compare.html`, `heights.py` and `check_contrast.py`
  cover E too.
- The ring lives in `--seg-ring`, so the settings window's segmented control is
  ringed the same way the panel tab is: one token, one look, and no second way
  to express "selected". `demo/seg-ring-problem.html` records why.
- The inherited Kimi "K" mark is replaced by Quota Tray's own icon: two accent
  bars on a transparent field, i.e. the panel's progress-bar motif. It is
  generated from geometry by `demo/make_icon.py` (256 PNG, eight-size ICO, and
  the 48px cut the tray embeds), never hand-drawn, and the tray's programmatic
  fallback draws the same bars instead of the old blue ball.
- A Kimi cache / decode block in the panel (`PLAN.md` §5.1):
  two readout columns (`Cache 7d · local`, `Cache today`) with bars and request
  counts, plus `Decode 135.7 tok/s` and the mean time to first token. The numbers
  come from the CLI's own session logs — the quota API has no token data — so the
  label says `local`. Hidden entirely until a scan finds prompt tokens.
- `--test-usage`: scans the session logs twice (walk, then stat-only) and prints
  the aggregated JSON; the two passes report the same counts, which is the
  incremental-scan proof.
- `demo/ui-demo.html`: every window on one page - both panel tabs, the settings
  sheet, the skills window, the tray menu, the notification area and the
  sign-in title bar - generated from the shipped markup and stylesheets by
  `demo/build_ui_demo.py`, so it cannot drift from the product.

### Fixed

- A recovery reload left the panel blank: the renderer was reloaded while its
  window stayed visible, and no `panel-show` follows a reload, so `#root` kept
  its initial `opacity: 0` inside a window the user could still see and click.
  The page now paints its first frame when it loads into an already-visible
  window, so a recovered crash no longer reads as a hung, empty panel.
- `demo/ui-demo.html` was losing to its own product CSS: the demo stage rules
  came before `theme.css`, which resets `html`/`body` at equal specificity, so
  the page could not scroll, lost its light-theme floor and stretched the skills
  card to the viewport. The stage rules now come last, and the skills rows use
  the product's `skill` class. The README screenshots were re-cropped from the
  corrected page, and `make_readme_shots.py` now refuses to write an image whose
  crop ring is not one flat background colour — the restored 30 px stage padding
  had moved the cards 60 device px and the fixed crop was silently cutting a
  card and leaving half a caption line at the top.
- `demo/shot.sh` produced no screenshot when the output path was absolute: the
  script pasted `$PWD` in front of it unconditionally, so the browser was handed
  `C:/repo/C:/elsewhere/x.png`. Absolute paths now pass through untouched.

### 中文

- **新增**：Phase 1 基线导入——`kimi-planbar-tray` 的 rust 版（Tauri 2 +
  vanilla TS/HTML/CSS）落到仓库根目录；`docs/SPEC.md` 与 `docs/SPEC_EN.md`
  作为继承行为规格一并拷入，并在首页标注各章在合并版的效力。
- **新增**：Phase 3 DeepSeek 后端（移植自 `deepseek-planbar` 的 WPF 服务）——
  `api.deepseek.com` 余额、`platform.deepseek.com` 控制台用量、`biz_data`
  对象/数组两种形态、`REQUEST` 既不计 token 也不计金额、`days[]` 只取本地今天、
  过滤全零占位模型、按本月消费取前 4。尚未接 UI：数据停在 `get_deepseek`
  命令与 `deepseek-updated` 事件。
- **新增**：DeepSeek 凭证存放在 `config_dir()/deepseek-credentials.json`，
  键名明文、值以 `dpapi:` 前缀经 DPAPI 加密；读取顺序为环境变量
  `DEEPSEEK_API_KEY` / `DEEPSEEK_USER_TOKEN` → 该文件 → 一次性导入旧版明文的
  `~/.deepseek-planbar/credentials.json`（只读，旧版照常可用）；解密失败按
  「未配置」处理。
- **新增**：展示名以厂商上游为准——`GET api.deepseek.com/models` 的官方
  `name` 压过内置表，改名后最长 24 小时（缓存 TTL）即生效，无需发版；
  控制台的组合行（`A & B`）与非 `deepseek-*` id 原样显示。
- **新增**：用量端点候选链（`/api/v0/usage` → `/api/v1/usage`），遇 404/410
  换下一个并记住可用的那个。
- **新增**：`--test-deepseek [--verbose]` 自检；13 个 Rust 测试覆盖 DeepSeek
  解析（4 份 fixtures 从父仓库移植）、凭证前缀处理与 DPAPI 往返。
- **新增**：DeepSeek 独立 poller，自带调度与 last-good 槽位——任一 provider
  失败都不会清空或改写另一个；Refresh 与托盘悬停预取现在覆盖两个 provider。
- **新增**：Kimi/DeepSeek 分段 tab 与 DeepSeek 面板——余额块（`Available` /
  `Insufficient balance`、`Granted … · Topped up …`）、每模型一行（官方展示名、
  本月金额、token 份额条、命中率与今日消费）、本月合计行；用量失败时整块换成
  一行提示，余额与用量各自报错互不影响。
- **新增**：选中的 tab 持久化到 `settings.json` 的 `ActiveTab`，下次启动仍停在
  同一个 provider。
- **新增**：`PLAN.md` §5.2 的两个阈值落地——命中率 <60% 标红、余额 <¥10 或
  `is_available=false` 标红；只决定颜色，放在前端，不往 Rust 里塞死代码。
- **新增**：设置窗加上 `PLAN.md` §4 的两个只读分区——CLI 版本（有新版时一并显示）、
  Kimi 凭证来源与 OAuth 到期时间、DeepSeek 两个凭证的状态。只报状态：IPC 载荷
  里没有任何能带出 token 或密钥的字段。
- **新增**：DeepSeek 登录窗（`login.rs`）——窗口本身就是 console，注入的 hook 监听
  页面自己发出的 `Authorization` 头（兜底读 `localStorage.userToken`），把候选 token
  通过一次被宿主取消的 `quotatray://capture` 导航送出来，远程页面拿不到任何 Tauri
  IPC 命令面（因此不需要 `remote` capability，也不需要 ACL 条目）。窗口是临时的：
  关闭即销毁、不在 recovery 集合里，下次登录重新创建。
- **修复**：DeepSeek 登录窗现在**置顶**，不再开在设置窗下面——它与其它窗口同一档
  `always_on_top`；打开登录窗时收起设置窗，关窗后再放回来。
- **修复**：点 `Sign in…` 不再留下「空白且关不掉」的窗口。原来登录窗是在 `open_login`
  命令里直接建的，而 Tauri 把同步命令跑在主线程上，`WebviewWindowBuilder::build()`
  又要跳到主线程并等它完成——窗口出现了、命令却永远回不来，后面的事（页面导航、日志、
  收起设置窗）一件都没发生。现在两个登录命令都把活交给 async runtime；收起设置窗也挪到
  建窗之后，建窗万一失败不会连设置窗一起丢掉。另加 panic 日志（`webview.log` 里的
  `panic: … (file:line)`）：窗口程序没有控制台，静默 panic 和卡死看起来一模一样。
- **新增**：候选 token 先用一次真实 `amount` 请求验证，通过才替换已存的凭证；被拒的
  候选保持窗打开、原因写在窗口标题上，已存凭证不受影响。登录过程会吐多个中间
  token，所以验证期间到达的新候选会顶掉排队中的那个，并在本次验证失败后重试——
  页面不会把已经上报过的 token 再发一次。`Clear` 把凭证文件写成空值
  而不删除它，否则一次性旧版导入会把刚清掉的凭证导回来。
- **新增**：手工兜底保留——设置窗里折叠的「Paste a credential instead」会逐个验证
  非空字段，只有通过的才落盘。
- **修复**：`Clear` 现在会顺手关掉开着的登录窗，并丢弃仍在验证中的候选——否则清掉之后
  那次捕获又会把 usage token 写回去。设置窗也把「什么会让它回来」写清楚了：点
  `Sign in…` 会把 console 自己会话里已有的 token 存下来，这正是那个窗口的职责，所以
  清空的 token 可能在没手动输入的情况下自己回来。
- **新增**：`--test-login [--verbose]`——先报凭证来源并做一次真实验证，再建登录窗并用
  注入的 hook 自己打一发 probe；`LoginWindow capture channel OK` 即「注入生效 +
  导航被拦 + 宿主解析」三件事的证据，不需要真人登录。
- **新增**：登录窗的 hook 还上报 console 自己用量请求的形状，端点搬家与
  `x-app-version` 升版都能从活页面学到并记进 `deepseek-endpoints.json`（`PLAN.md` §6.3）。
  `--test-login` 的 probe 带一个宿主拒绝学习的标记，诊断命令不会覆盖 console
  教会的端点。
- **变更**：header 时间戳跟着当前 tab 走（`Updated HH:MM` / `Fetch failed` /
  `Not configured`）；DeepSeek 缓存跨天时显示 `Updated M/D HH:MM`，避免把前一天的
  数据读成刚取的。
- **变更**：所有金额一律两位小数（`¥3.00` / `¥0.00` / `¥49.52`），两个 provider
  统一；旧版「整元省略小数」的规则不再沿用——放在 `¥49.52` 旁边，`¥3` 会被读成
  另一个单位。
- **变更**：DeepSeek 载荷在生产处就过滤——`visible_models`（去掉全零占位行、
  按本月消费取前 4）改在 `fetch_diag` 结尾执行，IPC 结果、`deepseek-updated`
  事件与缓存都只留面板要画的行，前端不再重复同一条规则。
- **变更**：切 tab 时入场 pane 淡入 120ms，窗口高度同帧变更。没有做真正的交叉
  淡入——两个 pane 叠在一格里量出的是较高的那个，底部锚定的窗口会跳到没显示的
  那个 tab 的高度。
- **变更**：四个窗口统一到定稿 D 版视觉——暖纸底、发丝线分行、大写微标签、
  30/600 等宽数字、单一蓝表示用量、单一红表示故障；设置窗与技能窗不再是旧的
  扁平皮肤，托盘菜单改为 ghost 列表。
- **变更**：主面板与设置窗改为内容自适应高度（`panel_height` /
  `settings_height`）——主面板右下角钉在工作区角 12px，设置窗改高后重新居中；
  设置窗用分段控件替代 radio / pill，窗口高度跟着内容走。
- **变更**：产品标识改为 `Quota Tray` / `com.shawnqi.quotatray` /
  `quota-tray.exe`，版本归零到 0.1.0；单实例互斥锁改名
  `QuotaTray.SingleInstance`（迁移期可与两个旧版并存）；配置目录
  `%APPDATA%\QuotaTray`，自启值名 `QuotaTray`，诊断覆盖变量改为
  `QUOTA_TRAY_APP_DATA_DIR`；托盘 tooltip、窗口标题、兜底退出菜单与
  GitHub `User-Agent` 统一为 Quota Tray。
- **变更**：托盘 tooltip 改为两个 provider 各占一行（`Quota Tray` /
  `Kimi 5h 31% · week 12%` / `DeepSeek ¥46.81`），各自带自己的
  ` (fetch failed)` 后缀；任一侧刷新都会重算整段文字。
- **变更**：设置窗底部按钮改名 `Save settings`，与凭证区自己的
  `Save credentials` 并列——两者写的是不同的东西，不再共用一个动词。
- **变更**：失败文案改为 `Fetch failed`（面板 header）与 ` (fetch failed)`
  （托盘 tooltip），原来的 `Update failed` 读起来像 CLI 版本检查失败，
  而那是另一条独立的路。
- **变更**：关掉设置窗时若面板已失去焦点就顺手收起——被抑制的那次 blur 不会
  自己补来，之前面板要等到下一次点击别处才收。
- **变更**：唯一强调色从 Kimi 蓝换成静紫（浅 `#8c74b1` / 深 `#9d88bc`）——
  用途不变（用量条、chip、focus ring），只换色相让面板更安静；深浅两套明度
  阶梯分别达标（对轨道 ≥3:1、chip 文字 ≥4.5:1）。
- **变更**：`Update` chip 从胶囊改成 tag 形状的圆角矩形（radius 4），浅色底纹
  从 10% 提到 18%，才和深色一样看得出是一块带色的底。
- **新增**：`demo/panel-demo-e.html` 成为新的定稿——结构、字体与 D 完全一致，
  换上静紫强调色、活动格改紫 ring（中性发丝线对轨道只有 1.16 / 1.82，低于 WCAG
  对状态标识的 3:1）、chip 改 tag 形状。`panel-demo-d.html` 原样保留作为上一版
  定稿；`compare.html`、`heights.py`、`check_contrast.py` 都已覆盖 E。
- **变更**：ring 落在 `--seg-ring` 上，设置窗的分段控件与面板 tab 从此共用同一个
  值——一个 token、一种观感，不再有第二处「只靠填充差表达选中」。
  `demo/seg-ring-problem.html` 记录了这个取舍的前后对比。
- **变更**：图标从继承来的 Kimi「K」标换成 Quota Tray 自己的双条配额条（透明底、
  紫 `#8c74b1`）——就是面板进度条的语言。三件套由 `demo/make_icon.py` 用几何生成
  （256 PNG、8 尺寸 ICO、托盘用的 48px），不手绘；托盘失败兜底也改成同样的双条，
  不再是那颗蓝球。
- **新增**：Kimi 面板的缓存 / 解码块（`PLAN.md` §5.1）——
  两列读数（`Cache 7d · local` / `Cache today`，各带条与请求数），下面一行
  `Decode 135.7 tok/s` 与平均首 token 时间。数字来自 CLI 自己的会话日志（额度接口
  没有 token 数据），所以标签写 `local`；扫不到数据时整块隐藏。
- **新增**：`--test-usage`——把会话日志扫两遍（先走树、再只 stat）并打印聚合 JSON；
  两遍请求数一致即证明增量扫描没有重复计数。
- **新增**：`demo/ui-demo.html`——所有窗口一页看全（两个面板 tab、设置窗、技能窗、
  托盘菜单、通知区、登录窗标题栏），由 `demo/build_ui_demo.py` 从真实 HTML/CSS
  生成，不会与产品走样。
- **修复**：设置窗内容比窗口高时改为**卡片自己滚动**。后端把窗口夹在工作区 90% 以内，
  在工作区矮的机器上（本机逻辑高度 752）展开「Paste a credential instead」后的 824 会被
  `overflow: hidden` 切掉，底部的 Save credentials 根本够不着。现在 `#root` 自己限高并
  滚动，滚动条是 4px、嵌在卡片边缘内侧的细条；上报高度改用
  `max(offsetHeight, scrollHeight)`，窗口仍按真实内容定尺寸。
- **变更**：窗口阴影收紧（`0 8px 18px`，原 `0 12px 40px`）。40px 模糊超出 28px 的阴影
  留白，被窗口边缘裁断，看起来像卡片外面套了一圈硬边而不是阴影。
- **新增**：中英双语 `README.md`（英文在前、中文在后）——运行要求、安装 / 便携 /
  自启、两个 provider 的数据来源表、隐私边界、自检命令表与构建命令。
- **新增**：`docs/screenshot-{light,dark}.png`——README 的两张截图，由
  `demo/make_readme_shots.py` 从「所有窗口一页」里裁出；与其它评审产物一样是
  生成物，不手工截图。
- **新增**：发布线——`.github/workflows/ci.yml`（全历史 gitleaks，再在
  `windows-latest` 上跑 fmt / clippy / build / test 与前端类型检查）与
  `.github/workflows/release.yml`（`v*` tag 必须与 `tauri.conf.json` 版本一致，
  然后构建、打包并开一个**草稿** Release，正文取自 CHANGELOG 对应小节；
  发布仍是人工动作）。
- **新增**：`scripts/release/make_release_zip.py`——源码快照（取自 `git ls-files`，
  不会与 `.gitignore` 走样）内含便携版 exe，外带 release 资产改名与
  `sha256sum` 兼容的 `SHA256SUMS.txt`；全部经 `.part` 中转，失败时不会留下
  与校验和不匹配的那一半。
- **修复**：recovery 的 reload 路径会让面板变成一片空白——渲染进程崩溃后页面重载，
  但窗口仍是可见的，而 `panel-show` 只由 `show_panel` 发出，`#root` 于是停在初始的
  `opacity: 0`，用户看到的是一个能点、却什么都没有的窗口。现在页面加载时若发现窗口
  已经可见就先画出首帧，崩一次不再像是卡死。
- **修复**：`demo/ui-demo.html` 的舞台样式被它自己的产品 CSS 盖掉——舞台规则排在
  `theme.css` 之前，同特异性下主题对 `html`/`body` 的重置（透明、不滚动）胜出，于是
  评审页不能滚动、浅色底丢失、skills 卡被拉成视口高。舞台规则改为最后注入，skills 行
  也改用产品里的 `skill` 类名。README 截图随后按新布局重裁，`make_readme_shots.py`
  加了一条「裁切外圈必须是单一背景色」的断言——舞台 30px 内边距恢复后整页下移 60
  设备像素，旧裁切框正悄悄切掉第三张卡、顶上还留半行小标题。
- **修复**：`demo/shot.sh` 在输出路径为绝对路径时截不到图——脚本无条件把 `$PWD`
  拼在前面，浏览器收到的是 `C:/repo/C:/elsewhere/x.png`；现在绝对路径原样透传。
- **变更**：只发便携 exe——`bundle.targets` 置空、构建走 `--no-bundle`，不再提供
  NSIS 安装包与开始菜单快捷方式；草稿 Release 只有 `QuotaTray.exe`、
  `QuotaTray-v0.1.0.zip` 与 `SHA256SUMS.txt` 三件，打包脚本也只认这一个 exe。
