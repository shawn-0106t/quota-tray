# AGENTS.md — Quota Tray

Guidance for AI coding agents working in this repository. Read this first; then read `PLAN.md` for the design contract.

## Project overview

**Quota Tray** is a Windows-only system tray app that shows two providers' quota in one panel, switched by tabs:

- **Kimi** — Kimi Code plan quota (5-hour + weekly windows, reset countdowns, Extra Usage wallet), read from the local Kimi Code CLI OAuth token and `GET https://api.kimi.com/coding/v1/usages`.
- **DeepSeek** — account balance (official API) and per-model token/cost usage (the platform's internal usage endpoint, with a WebView2 sign-in window to capture the user token).

This repository is the merge of two earlier first-party projects: `kimi-planbar-tray` (Tauri/rust edition) and `deepseek-planbar` (WPF edition). See `NOTICE` for lineage.

**Current state: P5 is complete (2026-11-14).** P1 (baseline import), P2 (all four windows on the E tokens/layout, content-sized heights, the Kimi/DeepSeek segmented tab and the DeepSeek pane), P3 (DeepSeek credentials, balance, usage, display names, dual pollers, `--test-deepseek`, fixture tests), P4 (settings sections, the DeepSeek sign-in window, the merged tray tooltip, `--test-login`) and P5 (the release packaging — a portable exe only, no installer —, the CI/draft-release workflows, the bilingual README, `scripts/release/make_release_zip.py`) are done. The panel renders both providers for real: Kimi from `get_state`/`quota-updated`, DeepSeek from `get_deepseek`/`deepseek-updated`, with the provider tab persisted as `ActiveTab`. Three releases are out: `v0.1.0` (2026-11-14 12:35 UTC), `v1.0.0` (13:02 UTC) and `v1.0.1` (2026-11-15 07:30 UTC, marked Latest — the tab switch rebuilt on Windows' own motion values plus the five-carrier version gate `scripts/release/check_versions.py`), each produced by the tag-triggered workflow and published by hand. After P5 the maintainer tested the build and three real defects were fixed — the sign-in window built on the main thread (blank, unclosable), `Clear` being undone by the sign-in window's own capture, and the settings sheet clipping its bottom — see `CHANGELOG.md` and the traps below. Follow `PLAN.md` §8.

## Source of truth

| Question | Answer |
|---|---|
| 当前进度 / 交接给新 agent | `STATUS.md` |
| What should the UI look like? | `demo/ui-demo.html` — every window on one page, generated from the shipped HTML/CSS by `demo/build_ui_demo.py`; regenerate after a UI change, never hand-edit. `demo/panel-demo-e.html` stays the frozen design origin (structure, type, the violet accent and every other token **except** `--shadow`, which was tightened to `0 8px 18px` after E because 40px is clipped by the 28px margin — `PLAN.md` §3.2 / `src/theme.css` win there, do not restore E's value); `panel-demo-d.html` is the previous one and `panel-demo-a/b/c.html` the rejected alternatives — comparison only, do not evolve them. |
| Layout numbers, tokens, colours | `PLAN.md` §3 |
| Behaviour (polling, credentials, failure semantics, traps) | `PLAN.md` §5, §7 |
| Inherited Kimi behaviour (credential chain, parsing, tray/window lifecycle, WebView2 recovery) | `docs/SPEC.md` (Chinese) / `docs/SPEC_EN.md` — read the scope table at the top first; `PLAN.md` wins where they differ |
| Phases and their acceptance criteria | `PLAN.md` §8 |

If code and plan disagree, the plan wins until it is deliberately amended — change `PLAN.md` in the same commit as the behaviour change.

## Repository layout

```
index.html            # main panel window  (src/main.ts + main.css)
settings.html         # settings window    (src/settings.ts + settings.css)
skills.html           # Kimi skills window (src/skills.ts + skills.css)
menu.html             # tray menu window   (src/menu.ts + menu.css)
src/                  # vanilla TS frontend (Vite multi-page build)
  main.ts / settings.ts / skills.ts / menu.ts   # per-window logic
  common.ts           # shared DTO types + formatting helpers
  theme.css           # E tokens + shared primitives (PLAN §3.2, §3.4)
  main.css / settings.css / skills.css / menu.css   # layout only
  vite-env.d.ts
vite.config.ts        # 4-page rollup input; 100 KB asset inlining budget
tsconfig.json / package.json / package-lock.json
src-tauri/            # Rust backend (Tauri 2)
  Cargo.toml          # crate quota-tray, lib quota_tray_lib
  tauri.conf.json     # 4 windows: main 424x520, settings 404x360, skills 404x520, menu 188x160; bundle.targets = [] (portable exe only)
  capabilities/default.json
  icons/              # generated double-bar mark, see demo/make_icon.py (PLAN.md §9)
  src/                # main/lib/credentials/quota/deepseek/kimi_usage/login/polling/tray/panel/settings/skills/update/theme_watch/recovery/state
    deepseek/         # mod (types, fetch, cache) + credentials (DPAPI) + client (HTTP) + parse (pure, tested)
  tests/fixtures/     # inert JSON ported from deepseek-planbar; read by include_str! in parse.rs tests
  build.rs / Cargo.lock
docs/SPEC.md          # inherited Kimi behaviour spec (Chinese)
docs/SPEC_EN.md       # ... English mirror, same chapter numbers
docs/case-quota-tray-panel.md  # design-case draft for the rationalism-design skill (not a product doc)
docs/screenshot-*.png # README screenshots - generated by demo/make_readme_shots.py
.github/workflows/    # ci.yml (gitleaks + build/test) and release.yml (tag -> draft release)
scripts/release/      # check_versions.py: the five-carrier version gate + --tag
                      # make_release_zip.py: source zip + staged exes + SHA256SUMS.txt
demo/                 # frozen design demos + review tooling (self-contained, zero deps)
  panel-demo-e.html   # THE design reference (D structure + A type + quiet violet accent)
  panel-demo-d.html   # previous reference (same as E but Kimi blue, neutral tab ring, pill chip)
  panel-demo-a/b/c.html
  compare.html        # side-by-side viewer, theme/provider/state switchers
  seg-ring-problem.html  # diagnostic: why the settings control needs E's ring too (PLAN §9)
  shot.sh             # headless screenshot with ?query presets; probes Chrome -> Edge
  crop_panel.py       # crop to the 424px window, trim backdrop
  heights.py          # measured card/window height for every demo x tab x state
  check_contrast.py   # WCAG ratios, parsed from src/theme.css (must stay all-pass)
  make_icon.py        # generates src-tauri/icons/{icon.png,icon.ico,tray.png} + demo/icon-demo.html
  icon-demo.html      # generated icon review page - do not hand-edit
  ui-demo.html        # ALL windows on one page - generated by build_ui_demo.py, do not hand-edit
  build_ui_demo.py    # builds ui-demo.html from the real HTML + CSS (scoped per window)
  make_readme_shots.py  # crops docs/screenshot-{light,dark}.png out of ui-demo.html
README.md             # bilingual: English half, then 中文
PLAN.md               # design + behaviour contract
STATUS.md             # progress snapshot / handoff — read this first when resuming work
CHANGELOG.md          # Keep a Changelog, English + Chinese per release
AGENTS.md             # this file
CONTRIBUTING.md       # bilingual: how to build, the gates, the docs contract
CODE_OF_CONDUCT.md    # Contributor Covenant 2.1, bilingual
SECURITY.md           # threat model, private reporting route, scope
LICENSE / NOTICE      # MIT + upstream attribution
.github/              # ci.yml + release.yml, dependabot.yml, issue + PR templates
```

`src-tauri/gen/` is generated by `tauri build` and is gitignored; `node_modules/`, `dist/` and `src-tauri/target/` are build output.

## Build and run

```bash
npm install
npm run dev            # vite only (frontend)
npx tauri dev           # full app
npx tauri build         # release exe in src-tauri/target/release/quota-tray.exe
                        #   (no bundles: tauri.conf.json sets bundle.targets = [])
npx tauri build --no-bundle   # same thing; kept for the explicit form
```

Caveats inherited from the parent project:

- A plain `cargo build` debug exe does **not** embed the frontend (it points at the Vite devUrl). Only the release build (or `npx tauri dev`) renders the UI; headless `--test-*` args work anyway.
- `npm run build` is Vite/esbuild only and does no type checking — run `npx tsc --noEmit`.

## Testing / self-checks

Runtime checks are command-line flags, dispatched **before** the single-instance mutex so they work next to a live instance:

```bash
quota-tray.exe --test-fetch        # Kimi quota once, JSON to stdout
quota-tray.exe --test-deepseek     # DeepSeek balance + usage once, JSON to stdout
quota-tray.exe --test-deepseek --verbose   # ... plus PLAN §6.1 diagnostics on stderr
quota-tray.exe --test-update       # CLI version check once, one line: local=… latest=… updateAvailable=… checkFailed=…
quota-tray.exe --test-ui           # construct every window, print "… OK" lines
```

`--test-deepseek --verbose` prints `credential source`, the endpoint and response shape it used, the per-model aggregation and the `/models` hit, all on **stderr** so stdout stays valid JSON. It runs on the real config dir, so the first invocation performs the one-time import from `~/.deepseek-planbar/credentials.json`.

`--test-login` is two checks in one run: it prints the credential source and verifies the stored browser token with a real `amount` request (`verify=ok`; the walk can cost two requests when the remembered base is dead), then builds the sign-in window and fires the injected hook's own sender (`kind=probe`). `LoginWindow OK` + `LoginWindow capture channel OK` is the proof that injection, the cancelled-navigation channel and the host parser all work without a human signing in. `--verbose` adds the remembered usage base. It needs network (it loads the real console page).

`--test-usage` scans the local Kimi session logs twice and prints the aggregated JSON: `scan1` walks the tree, `scan2` only re-stats known files, and the two request counts must be identical (that pair is the incremental-scan proof). Its numbers are checked against an independent calculation over the same logs.

`cargo test` covers pure logic only: 31 tests — the recovery classifier/backoff/log-tail pair, the skills frontmatter parser, the DeepSeek `parse`/`credentials` suite, the login-window URL/version sanitisers (`usage_base_from`, `sanitize_version`), six `kimi_usage` tests (turn-scope filtering, speed/hit-rate as ratios of sums, per-day buckets, index round-trip, the no-usage fast path, local day keys), and three `settings` tests pinning the `ActiveTab` contract (a legacy file without the key loads as `kimi`, a saved tab round-trips, unknown names normalise). The DeepSeek numbers are asserted against the four fixtures in `src-tauri/tests/fixtures/` (ported from `deepseek-planbar/tests/fixtures/`, pulled in with `include_str!`, so `tests/` holds data only). `FixtureTests.cs` in the parent repo asserts the same values; running the old exe with `--test-fixtures <abs path>` and the Rust suite together is how parse equivalence is proven — the live account moves every minute, so a numeric diff between two live fetches proves nothing.

Design review is part of the deliverable: after any visual change, regenerate the all-windows demo and the screenshots, and compare in both themes:

```bash
PYTHONUTF8=1 python demo/build_ui_demo.py
bash demo/shot.sh demo/ui-demo.html .shots/ui.png "theme=dark" 1480 1400 1
bash demo/shot.sh demo/panel-demo-e.html .shots/x.png "bare=1&theme=dark&tab=ds" 560 900 2
PYTHONUTF8=1 python demo/crop_panel.py .shots/x.png .shots/x-crop.png 424 2
PYTHONUTF8=1 python demo/heights.py
PYTHONUTF8=1 python demo/check_contrast.py   # reads theme.css; --demos for the frozen table
```

`.shots/` is gitignored; delete screenshots after reviewing.

The README's two screenshots (`docs/screenshot-{light,dark}.png`) are generated too — `PYTHONUTF8=1 python demo/make_readme_shots.py` re-crops them out of `demo/ui-demo.html` after a UI change. Never hand-edit or hand-crop them.

## Code style and conventions

- Code, comments and commit messages in English; conversation with the user in Chinese. Docs: `PLAN.md` / `STATUS.md` in Chinese, `AGENTS.md` / `CHANGELOG.md` in English, README bilingual.
- Comments cite `PLAN.md` sections (e.g. `PLAN §3.2`) the way the parent project cited its SPEC — keep citations accurate when behaviour changes.
- **Error handling baseline**: all IO, registry, process and HTTP failures are swallowed and surfaced only as UI text or state fields. Never pop up an error dialog.
- Frontend renders external data (skill names, model names) with `textContent` only, never `innerHTML`.
- Tauri IPC goes through the `#[tauri::command]`s registered in `lib.rs`, plus events. Windows are singletons — hidden, never destroyed; open paths re-pin the logical size (DPI guard) before showing.
- No third-party frontend framework, no web fonts, no CDN assets. Vanilla TS + CSS variables.
- Python helper scripts: `PYTHONUTF8=1` prefix, explicit `encoding='utf-8'`, no emoji in stdout.

## Critical behavioural traps (do not regress)

Inherited from `kimi-planbar-tray`:

- **Extra Usage units**: `booster_wallet.balance.amountLeft` is 1e-8 yuan → cents via `(raw + 500000) / 1000000`. The live payload (2026-11-19) keys the wallet `booster_wallet` — proto snake_case at the top level while the wallet's own fields are camelCase; `boosterWallet` is still accepted so an older payload parses. The not-activated trap has two faces: `isEnabled: false`, and (the live shape, which has no `isEnabled` at all) a `status` other than `STATUS_ACTIVE`. `priceInCents` is already cents. All API numbers are modelled as strings, numeric fallback tolerated.
- **`isEnabled=false`** (and the live `status != STATUS_ACTIVE`): the wallet balance is an estimate, not real — the whole card shows `Not activated`.
- **Credential chain**: `~/.kimi-code/credentials/kimi-code.json` `access_token` (only if `expires_at` > now + 30 s) → `api_key` in `~/.kimi-code/config.toml` whose provider `base_url` contains `api.kimi.com/coding`. `KIMI_CODE_HOME` overrides the home dir.
- **Failure semantics**: keep last-good values, show "Fetch failed", retry after 30 s, return to the configured interval on success.
- **Single instance**: named mutex `QuotaTray.SingleInstance` — do not rename.
- **DPI**: size windows with `LogicalSize`, position them with `PhysicalPosition`.
- **Portable mode**: empty `portable.dat` next to the exe moves `settings.json` to the exe directory.
- **Win11 corners**: every window opts out of DWM auto-rounding; the CSS paints the radius.
- **WebView2 recovery**: rebuild windows in place on `ProcessFailed`, never restart the process; `prevent_exit` on implicit exit; single-flight rebuilds with 1/2/5 s backoff, cap 3 in 30 s; log to `webview.log`, never tokens.
- **Tray**: left click toggles the panel, right click opens the menu window at the cursor, 300 ms guard after an auto-hide, 10 s throttled hover prefetch.

New in this project:

- **Panel height**: content-driven and **bottom-anchored**. The frontend measures and reports (`panel_height`); the backend resizes with `LogicalSize` and re-pins the window's bottom-right corner with `PhysicalPosition`, clamped to the work area. The card inside that window is bottom-anchored too (`body.panel > #root` in `main.css`), which is what makes a tab switch's pre-sized window invisible: a window that is briefly taller than the card keeps the card glued to the corner instead of floating above it. The panel reports `scrollHeight + 2 + 56` — the CONTENT height, never the box — because the `.clamped` cap (see **Provider tab**) would otherwise feed the backend its own cap and shrink the window. The settings window (content-sized too, `settings_height`, re-centred) reports its content height the same way: title bar + the pane's `scrollHeight` + the card's 2px borders, plus one spare pixel — the window height is rounded to physical pixels, so at e.g. 125% the viewport can come out a fraction shorter than the content and park a scrollbar in a sheet that fits; being centred, that pixel is free (the panel is bottom-anchored and therefore decides its cap in JS instead). The settings card is a flex column: the title bar stays outside the scroll box and only `.pane` scrolls (`#root { max-height: calc(100vh - 56px) }` + `#root > .pane { overflow-y: auto }` in `settings.css`, the same 4px themed scrollbar, starting below the title). `#root.clamped` in `main.css` is the panel's own equivalent, where the card itself scrolls. The settings sheet's expanded manual-paste block is ~776px, taller than this machine's 752px logical work area, so it scrolls while the manual block is open. Never leave the pane's bottom padding to a parent container — the last row must carry it, or that row reads optically high (this was a real defect in the demos; see `PLAN.md` §3.4).
- **Provider tab**: the panel tab is the shared `.seg` component from `theme.css` (same markup as the settings window, radio inputs in labels); only `.seg.provider`'s margin is the panel's. Never build a second selected-state treatment — the accent ring comes from `--seg-ring`. In steady state only the active pane is laid out (the other carries `hidden`), so `panel_height` walks in step with the tab on screen and the backend keeps one height, not one per tab; during a switch both are laid out (the outgoing one absolutely positioned) and the window is deliberately held at `max(h0, h1)`, with `reportHeight()` a no-op until `settleTab()` reports the live height. Switching animates the `.panes` cell, never the window (PLAN §3.6): the outgoing pane is lifted out of flow (`.pane-leaving`) so it cannot contribute height, both panes cross-fade over 167ms — outgoing on the exit curve `cubic-bezier(1,0,1,1)`, incoming on the entrance curve `cubic-bezier(0,0,0,1)` — and the cell's own height runs 200ms growing on the entrance curve / 167ms shrinking on the exit curve. The outgoing fade deliberately matches the shrink rather than WinUI's 83ms `ControlFasterAnimationDuration`: it paints above the incoming pane, so finishing early leaves a bare `--paper` band while the cell is still closing. The window is pre-sized to the taller tab before the first animated frame (and re-sent once if data lands during the resize) — Windows never animates a top-level window's size per frame — and the card is bottom-anchored (`body.panel > #root { position: absolute; inset: auto 28px 28px; margin: 0 }`), so the extra room opens upward, invisibly, instead of floating the card above the tray corner. Never stack two in-flow panes to get a cross-fade: that cell measures the taller pane and the window then sizes itself for the tab that is not on screen. The end keyframe and the pre-size are re-read after the resize lands: a poll tick that adds rows mid-switch retargets the window and the animation instead of snapping at the settle. Bottom anchoring moves the clamp clip to the card's TOP, so `main.ts` flags the card `.clamped` (`max-height: calc(100vh - 56px)` + internal scrolling, the same 4px scrollbar the settings pane uses) whenever the content is taller than the live viewport — once from `playTabSwitch` before the first animated frame (otherwise the header and the tab are clipped for the whole animation) and again from `reportHeight()` after each resize. That flag is not a CSS `100vh` cap because the rounded window can be a fraction of a pixel shorter than its content and would park a scrollbar in a card that fits. `tabRun` is bumped inside `resetTabMotion()`, not only in `playTabSwitch`: a switch settled by the fallback timer (panel hidden, rAF paused) must not resume later with stale heights. `prefers-reduced-motion` is checked a second time in `main.ts` because `theme.css`'s `@media` block cannot stop a Web Animation. Tab choice persists through `set_active_tab` (`ActiveTab` in settings.json, anything else normalises to `kimi`).
- **Header stamp belongs to the active provider**: `Updated HH:MM` / `Fetch failed` / `Not configured` for the tab on screen, and a cross-session cache older than today prints `Updated M/D HH:MM` — an HH:MM-only stamp reads as if yesterday's numbers were fetched today. It flips with the PANES (`renderStamp()` inside `playTabSwitch`, just before the animation starts), not with the radio: the segmented control answering the click immediately is the feedback, but flipping the stamp at click time left it reading "DeepSeek" for the 1–50ms the body was still Kimi. DeepSeek thresholds (`cache <60%` red, balance `<¥10` or `isAvailable=false` red) live in `common.ts`; they colour output only and stay out of Rust, where they would be dead code.
- **Two provider pollers** share the interval and failure semantics but must not overwrite each other's last-good state. Each owns its own `Schedule` (reschedule/retime Notifies): `notify_one` wakes a single waiter, so a shared pair would starve one loop.
- **DeepSeek parsing**: `data.biz_data` is an object for `amount` and an array for `cost` — tolerate both; `REQUEST` counts requests, not tokens, and is excluded from money too; new token types fall through to the total; a model with an empty `usage[]` still gets an all-zero entry; all-zero placeholder models are filtered; top 4 by month cost.
- **DeepSeek list is pre-filtered at the fetch boundary**: `fetch_diag` applies `visible_models()` to the payload it returns, so the IPC result, the `deepseek-updated` event and `deepseek-cache.json` already carry only the rows the pane draws. Render `byModel` as-is — a second filter in the frontend is how the two copies start to disagree. The `--verbose` per-model lines are printed before that, so diagnostics still show every model.
- **DeepSeek money is yuan, not cents**: the payload's `total` / `monthCost` / `todayCost` are yuan snapped to a 1e-8 quantum, so format them with `fmtMoney` — `fmtYuan` takes cents and belongs to Kimi. Both print **two decimals always** (`¥3.00`, `¥0.00`; user decision 2026-11-14), so the WPF parent's whole-yuan omission is gone on purpose — do not "restore" it. `demo/panel-demo-e.html` follows that rule (`Granted ¥10.00`); the rejected A–D demos keep their historical strings and are not a format reference. A model's share bar is its **token** share (`monthTokens` over the group max), not its cost share.
- **DeepSeek money**: token counts round with `round_ties_even` (= .NET `Math.Round`), and money aggregates are snapped to 1e-8 yuan. Raw f64 sums drift in the last bits and print `24.279870719999998` where the console and the WPF version show `24.27987072`; touching this makes the two apps disagree digit for digit.
- **DeepSeek endpoints**: constants + candidate fallback + learn-from-login-window (`PLAN.md` §6). Model names are data — never hardcode a model list.
- **Model names are metadata, not constants**: display name falls back official `GET https://api.deepseek.com/models` `name` → static table → derived → raw id. The official name **wins** (user decision 2026-11-14): a vendor rename must reach the panel without a new release, so never put the static table back in front. The `/models` fetch itself runs at most once per process (`MODELS_REFETCHED`), which is what `PLAN.md` §6.1 asks for: a stale or unknown cache is refreshed on the next launch, and the 24 h TTL decides whether that launch may reuse the cached names. A long-resident instance therefore shows the previous launch's names — do not "fix" that by refetching on a timer, the endpoint is not a quota source. Never drop a model's usage because its id is missing from `/models` — the usage endpoint aggregates the platform console, which also reports combined rows (`A & B`) and retired models. Derivation only rewrites `deepseek-*` ids; anything containing whitespace stays verbatim. Long official names must ellipsis inside the row, never push the cost out.
- **DeepSeek sign-in window is the one temporary window**: built on demand (`login::open`), destroyed on close, and deliberately **not** in the recovery set or in `tauri.conf.json`. `on_window_event` lets its `CloseRequested` through instead of preventing it; everything else reuses hidden windows. Closing it ends the console session too, so the next sign-in starts clean. It is `always_on_top`, like every other window here: without that flag it opens *under* the settings sheet (the one the user just clicked "Sign in…" in), which reads as "the window is not in front". Opening it also hides the sheet and closing it restores the sheet, so there is never more than one app window to deal with (a `--test-*` run skips both, it shows all windows on purpose).
- **Never build a window on the command's own thread** (`open_login` / `close_login` spawn): Tauri runs a sync `#[tauri::command]` on the **main thread**, and `WebviewWindowBuilder::build()` dispatches the webview creation to the main thread and waits for it. From the main thread that wait is never satisfied — the window is created, the command never returns, nothing after it runs (no log line, no sheet hide) and the half-built window is blank (never navigates) and unclosable (never registered with the app). This was the real cause of the "Sign in… gives a blank, dead window" report; reproduced by clicking the real button, fixed by spawning the open, and confirmed by the log order `login: sign-in window opened` → `settings: hide`. The sheet hide also stays after the build so a failed build cannot cost the user the sheet.
- **A windowed app has no console, so panics are invisible**: `install_panic_log()` (main.rs) appends `panic: … (file:line)` to `webview.log`. Keep it — a silent panic looks exactly like a hung window.
- **The capture channel is a cancelled navigation, not IPC**: the injected hook (`initialization_script`) assigns `location.href = 'quotatray://capture?…'`; `on_navigation` parses it and returns `false`. Do not "simplify" this into `invoke()` — remote-origin IPC needs a `remote` capability plus ACL entries and hands the third-party page a command surface. Top-level navigation is chosen over a hidden frame because a frame is subject to the console's own `frame-src`. `--test-login` proves the channel with a probe fired through the same function.
- **Verify before save**: a captured or pasted token replaces the stored one only after a real `amount` request answers 2xx **and its body carries no non-zero `code`** — the platform answers **HTTP 200** with the failure in the body (`{"code":40003,"msg":"Authorization Failed (invalid token)","data":null}`), so status-only verification accepts every candidate. That was v1.0.1's defect and v1.0.2's fix; do not 'simplify' the envelope check away (PLAN §5.2). A token the platform refuses makes the walk try the next candidate base (a remembered base can be dead); a transport failure does not. A rejected candidate keeps the window open, the reason in the window title, and the stored credential untouched. A **transient** failure (`network-error` / `rate-limited` / `server-error` — every 5xx maps to `server-error`, so a raw `http-5…` never reaches the predicate) re-tries the SAME candidate once: the page dedupes by its own `lastToken`, so it never resends a token it already reported, and a blip on the finished one used to cost a whole sign-in. A newer candidate that arrives during that pause wins over the retry, and a transport failure on a later base still gets that retry even after an earlier base refused the candidate (`is_transient` is the one list). Window boundaries bump `LoginState::epoch` (`new_session`): a verification that outlived its window must not save, retitle or close the replacement — `reset()` has exactly one caller, `new_session()` (build, close, Clear); reopening an existing sign-in window deliberately touches neither counter, because that is still the same sign-in. While one verification is in flight, a newer candidate replaces the queued one and is re-tried when that verification fails. The `--test-login` probe marks its URL (`quotatray=probe`), and the host refuses to learn an endpoint from it — a diagnostic must not overwrite what the console taught the app. The **usage fetch** runs the same envelope check on every 2xx: a dead token's `200 + code` otherwise parsed as an all-zero month, overwrote the last-good cache and pinned the failing base. `credentials::save` returns whether the write landed; callers must use it.
- **Credential statusing never carries a value**: `credential_status` returns booleans plus a source string (`env | file | imported | undecryptable | none`, and `oauth | api-key | none` for Kimi). The settings sections mask nothing because there is nothing to mask — keep it that way.
- **Two builds, one account**: the Slint spike in the sibling worktree
  (`../quota-tray-slint`, branch `spike/slint-demo`) is a second app of the
  same product, not a second copy of this one. `shared_dir()`
  (`%APPDATA%\QuotaTray\shared`) holds what belongs to the ACCOUNT —
  `deepseek-credentials.json`, `deepseek-endpoints.json`, `deepseek-models.json`,
  `kimi-usage.json` — and both apps implement the same absolute rule (portable
  mode: never shared). Everything else stays in `config_dir()`, because it is
  this app's own memory: `settings.json` (theme, interval, `ActiveTab` — the
  two UIs differ), `webview.log`, and `deepseek-cache.json` (its timestamp
  means "when THIS app last succeeded", so a shared cache would stamp one
  app's last-good numbers with the other's clock). Each app keeps its own
  single-instance mutex, autostart value name and WebView2 user-data folder:
  sharing any of those makes one app refuse to start, silently take over the
  other's autostart, or corrupt the other's browser profile. A pre-split
  `config_dir()/deepseek-credentials.json` is copied into the shared dir on
  the first `load()` and left in place; `Clear` blanks every copy it can see,
  so a build that has not been mirrored onto `shared_dir()` yet cannot answer
  with the values the user just removed. There is **no cross-process lock**: the
  shared files are written as a per-process temp plus a rename, so a reader
  never sees a half-written file and the last writer wins. A field-level merge
  between the two apps is not implemented — each write is a read-modify-write
  of the whole file, so two saves landing in the same second can drop one
  field. Keep saves user-triggered and rare; do not add background writers.
- **Clearing writes an empty credential file instead of deleting it**: deleting would make the one-time legacy import run again and bring the cleared credentials straight back. `env`-sourced values survive a Clear by design, and the settings window says so on its own line. Clear also closes an open sign-in window and bumps `LoginState.epoch`, so a capture or an in-flight verification cannot resurrect the token it just removed — the console's own session would otherwise hand the hook a valid token seconds later, and the user reads that as "Clear does nothing". Re-opening "Sign in…" will still store the token again; that is that window's job, not a bug. The **manual paste** field is a second, independent write path and cannot use `epoch`: that counter also moves when the sign-in window is opened or closed, and doing so must not throw away a paste the user is waiting on. It is guarded by `LoginState::clears`, which only Clear bumps, and the comparison sits **between verify and save** (`lib.rs::save_credential`) — saving first and rejecting afterwards would still leave the value on disk. The frontend maps the `cleared` error code to "cleared while verifying — not saved"; `credentials::save` returns whether the write landed, and a failed write is reported as `save-failed` ("could not write the credential — not saved") rather than a false success.

- **Tray tooltip is three lines, one per provider** (`Quota Tray` / `Kimi 5h … · week …` / `DeepSeek ¥…`), each provider carrying its own ` (fetch failed)`. Both pollers call `tray::update_tooltip`, so a DeepSeek-only refresh must not drop the Kimi line or vice versa. DeepSeek money is formatted here too (two decimals, `$` for USD) — keep it identical to `fmtMoney` in `common.ts`.
- **The tray / exe icon is generated, not drawn by hand**: `demo/make_icon.py` owns `icons/icon.png` (256), `icons/icon.ico` (8 sizes) and `icons/tray.png`, and regenerates `demo/icon-demo.html` from the same geometry. The tray embeds the **48px** cut because a tray icon is one RGBA buffer: the size has to cover the 100-300% DPI range by downscaling (a native 16 goes soft at 225%). `tray.rs`'s `fallback_icon()` draws the same two bars - keep its geometry in step with the script. The icon violet is brand identity, not the panel's accent semantics (§3.5).
- **Kimi cache numbers are local, not account-wide**: they are aggregated from `~/.kimi-code/sessions/**/agents/*/wire.jsonl` (`PLAN.md` §5.1), so the pane labels them `local`. The quota API has no token data at all — do not "find" it there. Only `usage.record` rows with `usageScope=turn` count; `usageScope=session` rows are totals and `message.meta.usage` is a copy of the same numbers, so counting either double-counts. Hit rate and decode speed are **ratios of sums**, never the mean of per-call ratios. The scan reads only those counters: it must never retain message content, and the index (`shared_dir()/kimi-usage.json`) stores per-file byte offsets plus per-day totals only. The timing rows (`event`) carry **no `usageScope`** and their `usage.event.output` is one call's output (measured 2026-11-19: 108–761 tokens, n=38), which is what makes decode speed a ratio of sums; if a cumulative value ever appears there the ratio inflates by the session length, so re-measure before trusting the speed block. The tree walk costs ~0.1 s on this machine's 6.5 k files, so the loop walks only every other minute and merely stats in between; `--test-usage` prints both timings.
- **Colour semantics**: the accent (a quiet violet as of 2026-11-14: `#8c74b1` light / `#9d88bc` dark) has exactly two roles — usage (bars, chip) and "this cell is selected" (`--seg-ring` on a segmented control's active cell). Nothing else may take it: no switch tint, no button fill. Red = failure only; never colour a high-usage percentage; `Available` stays neutral. The rule is "the accent means usage or selection", not "the accent is blue" — the hue is a brand decision, the mapping is not.
- **The card shadow has to fit the 28px shadow margin** (`--shadow: 0 8px 18px`): the window is transparent and only 28px of padding is reserved for the shadow, so anything blurrier is cut by the window edge and reads as a hard dark ring around the card instead of a shadow. Enlarging the blur means enlarging the margin in `tauri.conf.json` *and* `SHADOW_ROOM` in `common.ts` together.

## Security considerations

- The Kimi OAuth token / API key is read-only and goes **only** to `https://api.kimi.com/coding/v1/usages` as a Bearer token. Never log, persist elsewhere, or send to any other endpoint.
- DeepSeek credentials live in `shared_dir()/deepseek-credentials.json` — `%APPDATA%\QuotaTray\shared` (portable mode: next to the exe) — not in the parent app's `~/.deepseek-planbar/` directory. Values are DPAPI-protected with a `dpapi:` prefix; key names stay readable. The old file is imported **once** and never written back, so the parent app keeps working. The app talks only to `api.deepseek.com` and `platform.deepseek.com`. The sign-in window captures the user token from the user's own session and verifies it with a real `amount` request before saving.
- The sign-in window loads third-party content and is given **no** Tauri IPC surface: the only thing leaving that page is a cancelled navigation carrying a candidate token. Never add a `remote` capability for `platform.deepseek.com`.
- Never log, print or transmit the API key or user token — `--test-deepseek --verbose` may print `credential source: env | file | imported | none | undecryptable`, never a value; `--test-login` prints presence flags and a verification verdict only. A field without the `dpapi:` prefix is accepted as plaintext (debug escape hatch); decryption failure means "not configured", never an error dialog.
- Portable mode is user-bound on purpose: a DPAPI blob copied to another Windows account does not decrypt, and re-entry is the expected fix.
- Autostart writes HKCU only — never HKLM, never elevation. Keep the non-affiliation notice and `LICENSE` / `NOTICE` intact.
- Repo hygiene: no absolute local paths, no secrets, no real personal or customer data in tracked files; `*.example` templates only.
- GitHub-side gates exist too: secret-scanning push protection rejects a push carrying a detected credential (rotate it, do not bypass), and CodeQL default setup runs on PRs and `main` — an alert is a real finding until proven otherwise (`py/bad-tag-filter` in `demo/build_ui_demo.py` was one).

## Release process

Ported from the parent project's tag-triggered draft pipeline (2026-11-14).

**Cutting a release**

1. Bump the version in **five** carriers — `package.json`, `src-tauri/tauri.conf.json`, `Cargo.toml`, then `npm install --package-lock-only` (both version fields) and a `cargo build` (or `cargo update -p quota-tray --precise <version>`) for `src-tauri/Cargo.lock` — and add the P5 row to `PLAN.md` §8 if the phase plan moves. `python scripts/release/check_versions.py` is the checklist: it fails until all five agree.
2. Finalize `CHANGELOG.md`: move the section out of `Unreleased` and date it (`## [1.0.0] — 2026-11-14`). `.github/workflows/release.yml` refuses to compose notes for a section that still says `Unreleased`, so this is a hard gate, not a habit.
3. Run the local gates: `npx tsc --noEmit`, `npm run build`, `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test` in `src-tauri`, then `npx tauri build` and the `--test-*` self-checks against the release exe.
4. Push the `v<x.y.z>` tag. The tag and all five carriers must agree — `scripts/release/check_versions.py` is the one place that knows the list; CI runs it on every push and the release workflow runs it with `--tag` before building.
5. `release.yml` runs gitleaks over the full history, builds the portable exe (`--no-bundle`), packs the source zip and checksums, and **opens a draft Release** with three assets: `QuotaTray.exe` (portable), `QuotaTray-v<version>.zip` (source + portable exe at its root) and `SHA256SUMS.txt`. There is no installer: the app ships as a single portable exe. The body is generated from the matching CHANGELOG section (English half plus the 中文 half, which that file keeps in the same section).
6. Review the draft: assets present, `SHA256SUMS.txt` matches, notes read correctly. Publishing stays manual.

**CI** (`.github/workflows/ci.yml`) runs on every PR and on pushes to `main` (a tag push does **not** trigger it, so `release.yml` repeats the same code gates — `tsc --noEmit`, `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked` — between `npm ci` and the build): gitleaks over the full history, then on `windows-latest` `python scripts/release/check_versions.py` (the five version carriers, plus `npm install --package-lock-only` when it complains) → `npm ci` → `tsc --noEmit` → `npm run build` → `cargo fmt --check` → `cargo clippy -D warnings` → `cargo build --locked` → `cargo test --locked`. No runtime `--test-*` self-check runs on a hosted runner: they need this machine's credentials and an interactive desktop.

**Packaging** (`scripts/release/make_release_zip.py`): the version comes from `tauri.conf.json` only; the source file set comes from `git ls-files` so it can never drift from `.gitignore`. The zip and the checksum file are staged as `.part` and renamed only when complete (checksums first, zip last), so a failed run leaves the previous pair intact instead of a truncated one. Asset names at the repo root are gitignored (`*.exe`, `QuotaTray-*.zip*`, `SHA256SUMS.txt`).

**State (2026-11-21)**: the repository is public at `shawn-0106t/quota-tray`. `main` is **PR-only** (since 2026-11-19: a pull request is required, `enforce_admins` is on so an admin can no longer push past the rule, self-merge needs 0 approvals, the `Build & test (windows-latest)` check must pass, and force-push/deletion stay off), secret-scanning **push protection** is on, **CodeQL default setup** scans PRs and `main`, Dependabot runs weekly (cargo `/src-tauri`, npm, github-actions) with security updates enabled, merged PRs auto-delete their head branch (`delete_branch_on_merge` on since 2026-11-14, so the Dependabot branches a squash merge used to strand no longer accumulate), and private vulnerability reporting is the route in `SECURITY.md`. The wiki is enabled but unused — docs live in this repository. Every `v*` tag is cut and published the same way, the tag list living in `CHANGELOG.md` — `v0.1.0` and `v1.0.0` on 2026-11-14, `v1.0.1` on 2026-11-15, `v1.0.2` on 2026-11-19; the workflow stops at a draft, so publishing by hand stays a manual decision.

## Environment notes (this machine)

- Windows 10/11, Chinese locale (code page 936). Shell is Git Bash: POSIX syntax, forward slashes, single quotes.
- Python 3.13 (`PYTHONUTF8=1`), Node 24, Rust 1.97 (MSVC), WebView2 runtime present, .NET 8 SDK present.
- Chrome is **not** installed; Edge is. Always probe at runtime (Chrome → Edge → other Chromium) — never hardcode a browser path.
- Windows binaries cannot write to MSYS paths (`/tmp`): convert with `cygpath` (the demo scripts already do).
- **Code-review delegation is degraded on this machine** (2026-11-14): the session `Agent` tool registers only `plan` / `coder` / `explore` — there is no `code-reviewer` type — and `~/.kimi-code/bin/kimi-review.cmd` is pinned to `kimi-code/k3.1-pro`, which this subscription cannot access (`provider.auth_error`). Use a read-only `explore` subagent pointed at `~/.agents/skills/code-review/SKILL.md`, and re-run the CI gates yourself afterwards: that reviewer found 8 real defects but missed a `cargo clippy -D warnings` error.
