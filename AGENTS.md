# AGENTS.md — Quota Tray

Guidance for AI coding agents working in this repository. Read this first; then read `PLAN.md` for the design contract.

## Project overview

**Quota Tray** is a Windows-only system tray app that shows two providers' quota in one panel, switched by tabs:

- **Kimi** — Kimi Code plan quota (5-hour + weekly windows, reset countdowns, Extra Usage wallet), read from the local Kimi Code CLI OAuth token and `GET https://api.kimi.com/coding/v1/usages`.
- **DeepSeek** — account balance (official API) and per-model token/cost usage (the platform's internal usage endpoint, with a WebView2 sign-in window to capture the user token).

This repository is the merge of two earlier first-party projects: `kimi-planbar-tray` (Tauri/rust edition) and `deepseek-planbar` (WPF edition). See `NOTICE` for lineage.

**Current state: P2 is complete (2026-11-14).** P1 (baseline import), P2 (all four windows on the E tokens/layout, content-sized heights, the Kimi/DeepSeek segmented tab and the DeepSeek pane) and P3 (DeepSeek credentials, balance, usage, display names, dual pollers, `--test-deepseek`, fixture tests) are done. The panel renders both providers for real: Kimi from `get_state`/`quota-updated`, DeepSeek from `get_deepseek`/`deepseek-updated`, with the provider tab persisted as `ActiveTab`. Next is P4 (settings sections, tray tooltip merge, `--test-login`). Follow `PLAN.md` §8.

## Source of truth

| Question | Answer |
|---|---|
| 当前进度 / 交接给新 agent | `STATUS.md` |
| What should the UI look like? | `demo/panel-demo-e.html` — the frozen design (D's structure and type, violet accent, accent tab ring, tag chip). `demo/panel-demo-d.html` is the previous frozen design (same layout, Kimi blue); `demo/panel-demo-a/b/c.html` are the rejected alternatives. All of them are kept only for comparison; do not evolve them. |
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
  tauri.conf.json     # 4 windows: main 424x520, settings 404x464, skills 404x520, menu 188x160
  capabilities/default.json
  icons/              # inherited K mark (replacement still undecided, PLAN.md §9)
  src/                # main/lib/credentials/quota/deepseek/polling/tray/panel/settings/skills/update/theme_watch/recovery/state
    deepseek/         # mod (types, fetch, cache) + credentials (DPAPI) + client (HTTP) + parse (pure, tested)
  tests/fixtures/     # inert JSON ported from deepseek-planbar; read by include_str! in parse.rs tests
  build.rs / Cargo.lock
docs/SPEC.md          # inherited Kimi behaviour spec (Chinese)
docs/SPEC_EN.md       # ... English mirror, same chapter numbers
demo/                 # frozen design demos + review tooling (self-contained, zero deps)
  panel-demo-e.html   # THE design reference (D structure + A type + quiet violet accent)
  panel-demo-d.html   # previous reference (same as E but Kimi blue, neutral tab ring, pill chip)
  panel-demo-a/b/c.html
  compare.html        # side-by-side viewer, theme/provider/state switchers
  seg-ring-problem.html  # diagnostic: why the settings control needs E's ring too (PLAN §9)
  shot.sh             # headless screenshot with ?query presets; probes Chrome -> Edge
  crop_panel.py       # crop to the 424px window, trim backdrop
  heights.py          # measured card/window height for every demo x tab x state
  check_contrast.py   # WCAG ratios for the token pairs (must stay all-pass)
PLAN.md               # design + behaviour contract
STATUS.md             # progress snapshot / handoff — read this first when resuming work
CHANGELOG.md          # Keep a Changelog, English + Chinese per release
AGENTS.md             # this file
LICENSE / NOTICE      # MIT + upstream attribution
```

`src-tauri/gen/` is generated by `tauri build` and is gitignored; `node_modules/`, `dist/` and `src-tauri/target/` are build output.

## Build and run

```bash
npm install
npm run dev            # vite only (frontend)
npx tauri dev           # full app
npx tauri build         # release exe in src-tauri/target/release/quota-tray.exe
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

`--test-login` belongs to P4 and is **not implemented yet** — an unknown flag falls through to the normal GUI launch, so do not script it before that phase lands.

`cargo test` covers pure logic only: 21 tests — the recovery classifier/backoff/log-tail pair, the skills frontmatter parser, the DeepSeek `parse`/`credentials` suite, and three `settings` tests pinning the `ActiveTab` contract (a legacy file without the key loads as `kimi`, a saved tab round-trips, unknown names normalise). The DeepSeek numbers are asserted against the four fixtures in `src-tauri/tests/fixtures/` (ported from `deepseek-planbar/tests/fixtures/`, pulled in with `include_str!`, so `tests/` holds data only). `FixtureTests.cs` in the parent repo asserts the same values; running the old exe with `--test-fixtures <abs path>` and the Rust suite together is how parse equivalence is proven — the live account moves every minute, so a numeric diff between two live fetches proves nothing.

Design review is part of the deliverable: after any visual change, regenerate screenshots and compare against `demo/panel-demo-e.html` at 424px, in both themes:

```bash
bash demo/shot.sh demo/panel-demo-e.html .shots/x.png "bare=1&theme=dark&tab=ds" 560 900 2
PYTHONUTF8=1 python demo/crop_panel.py .shots/x.png .shots/x-crop.png 424 2
PYTHONUTF8=1 python demo/heights.py
PYTHONUTF8=1 python demo/check_contrast.py
```

`.shots/` is gitignored; delete screenshots after reviewing.

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

- **Extra Usage units**: `boosterWallet.balance.amountLeft` is 1e-8 yuan → cents via `(raw + 500000) / 1000000`. `priceInCents` is already cents. All API numbers are modelled as strings, numeric fallback tolerated.
- **`isEnabled=false`**: the wallet balance is an estimate, not real — the whole card shows `Not activated`.
- **Credential chain**: `~/.kimi-code/credentials/kimi-code.json` `access_token` (only if `expires_at` > now + 30 s) → `api_key` in `~/.kimi-code/config.toml` whose provider `base_url` contains `api.kimi.com/coding`. `KIMI_CODE_HOME` overrides the home dir.
- **Failure semantics**: keep last-good values, show "Fetch failed", retry after 30 s, return to the configured interval on success.
- **Single instance**: named mutex `QuotaTray.SingleInstance` — do not rename.
- **DPI**: size windows with `LogicalSize`, position them with `PhysicalPosition`.
- **Portable mode**: empty `portable.dat` next to the exe moves `settings.json` to the exe directory.
- **Win11 corners**: every window opts out of DWM auto-rounding; the CSS paints the radius.
- **WebView2 recovery**: rebuild windows in place on `ProcessFailed`, never restart the process; `prevent_exit` on implicit exit; single-flight rebuilds with 1/2/5 s backoff, cap 3 in 30 s; log to `webview.log`, never tokens.
- **Tray**: left click toggles the panel, right click opens the menu window at the cursor, 300 ms guard after an auto-hide, 10 s throttled hover prefetch.

New in this project:

- **Panel height**: content-driven and **bottom-anchored**. The frontend measures and reports (`panel_height`); the backend resizes with `LogicalSize` and re-pins the window's bottom-right corner with `PhysicalPosition`, clamped to the work area. The settings window is content-sized too (`settings_height`) and re-centred instead. Never leave the pane's bottom padding to a parent container — the last row must carry it, or that row reads optically high (this was a real defect in the demos; see `PLAN.md` §3.4).
- **Provider tab**: the panel tab is the shared `.seg` component from `theme.css` (same markup as the settings window, radio inputs in labels); only `.seg.provider`'s margin is the panel's. Never build a second selected-state treatment — the accent ring comes from `--seg-ring`. Only the active pane is laid out (the other carries `hidden`), so `panel_height` is always the height of the tab on screen; the backend keeps one height, not one per tab. Switching is a 120ms fade-in of the incoming pane, not a cross-fade: stacked panes measure the taller one and the bottom-anchored window would jump to the wrong height. Tab choice persists through `set_active_tab` (`ActiveTab` in settings.json, anything else normalises to `kimi`).
- **Header stamp belongs to the active provider**: `Updated HH:MM` / `Fetch failed` / `Not configured` for the tab on screen, and a cross-session cache older than today prints `Updated M/D HH:MM` — an HH:MM-only stamp reads as if yesterday's numbers were fetched today. DeepSeek thresholds (`cache <60%` red, balance `<¥10` or `isAvailable=false` red) live in `common.ts`; they colour output only and stay out of Rust, where they would be dead code.
- **Two provider pollers** share the interval and failure semantics but must not overwrite each other's last-good state. Each owns its own `Schedule` (reschedule/retime Notifies): `notify_one` wakes a single waiter, so a shared pair would starve one loop.
- **DeepSeek parsing**: `data.biz_data` is an object for `amount` and an array for `cost` — tolerate both; `REQUEST` counts requests, not tokens, and is excluded from money too; new token types fall through to the total; a model with an empty `usage[]` still gets an all-zero entry; all-zero placeholder models are filtered; top 4 by month cost.
- **DeepSeek list is pre-filtered at the fetch boundary**: `fetch_diag` applies `visible_models()` to the payload it returns, so the IPC result, the `deepseek-updated` event and `deepseek-cache.json` already carry only the rows the pane draws. Render `byModel` as-is — a second filter in the frontend is how the two copies start to disagree. The `--verbose` per-model lines are printed before that, so diagnostics still show every model.
- **DeepSeek money is yuan, not cents**: the payload's `total` / `monthCost` / `todayCost` are yuan snapped to a 1e-8 quantum, so format them with `fmtMoney` — `fmtYuan` takes cents and belongs to Kimi. Both print **two decimals always** (`¥3.00`, `¥0.00`; user decision 2026-11-14), so the WPF parent's whole-yuan omission is gone on purpose — do not "restore" it. `demo/panel-demo-e.html` follows that rule (`Granted ¥10.00`); the rejected A–D demos keep their historical strings and are not a format reference. A model's share bar is its **token** share (`monthTokens` over the group max), not its cost share.
- **DeepSeek money**: token counts round with `round_ties_even` (= .NET `Math.Round`), and money aggregates are snapped to 1e-8 yuan. Raw f64 sums drift in the last bits and print `24.279870719999998` where the console and the WPF version show `24.27987072`; touching this makes the two apps disagree digit for digit.
- **DeepSeek endpoints**: constants + candidate fallback + learn-from-login-window (`PLAN.md` §6). Model names are data — never hardcode a model list.
- **Model names are metadata, not constants**: display name falls back official `GET https://api.deepseek.com/models` `name` → static table → derived → raw id. The official name **wins** (user decision 2026-11-14): a vendor rename must reach the panel within the 24 h cache TTL without a new release, so never put the static table back in front. Never drop a model's usage because its id is missing from `/models` — the usage endpoint aggregates the platform console, which also reports combined rows (`A & B`) and retired models. Derivation only rewrites `deepseek-*` ids; anything containing whitespace stays verbatim. Long official names must ellipsis inside the row, never push the cost out.
- **Colour semantics**: the accent (a quiet violet as of 2026-11-14: `#8c74b1` light / `#9d88bc` dark) has exactly two roles — usage (bars, chip) and "this cell is selected" (`--seg-ring` on a segmented control's active cell). Nothing else may take it: no switch tint, no button fill. Red = failure only; never colour a high-usage percentage; `Available` stays neutral. The rule is "the accent means usage or selection", not "the accent is blue" — the hue is a brand decision, the mapping is not.

## Security considerations

- The Kimi OAuth token / API key is read-only and goes **only** to `https://api.kimi.com/coding/v1/usages` as a Bearer token. Never log, persist elsewhere, or send to any other endpoint.
- DeepSeek credentials live in `config_dir()/deepseek-credentials.json` (portable mode: next to the exe), not in the parent app's `~/.deepseek-planbar/` directory. Values are DPAPI-protected with a `dpapi:` prefix; key names stay readable. The old file is imported **once** and never written back, so the parent app keeps working. The app talks only to `api.deepseek.com` and `platform.deepseek.com`. The sign-in window captures the user token from the user's own session and verifies it with a real `amount` request before saving.
- Never log, print or transmit the API key or user token — `--test-deepseek --verbose` may print `credential source: env | file | imported | none | undecryptable`, never a value. A field without the `dpapi:` prefix is accepted as plaintext (debug escape hatch); decryption failure means "not configured", never an error dialog.
- Portable mode is user-bound on purpose: a DPAPI blob copied to another Windows account does not decrypt, and re-entry is the expected fix.
- Autostart writes HKCU only — never HKLM, never elevation. Keep the non-affiliation notice and `LICENSE` / `NOTICE` intact.
- Repo hygiene: no absolute local paths, no secrets, no real personal or customer data in tracked files; `*.example` templates only.

## Release process

Not defined yet. The parent project used a tag-triggered draft Release with a gitleaks gate; port that pipeline in Phase 5 and document it here before publishing anything.

## Environment notes (this machine)

- Windows 10/11, Chinese locale (code page 936). Shell is Git Bash: POSIX syntax, forward slashes, single quotes.
- Python 3.13 (`PYTHONUTF8=1`), Node 24, Rust 1.97 (MSVC), WebView2 runtime present, .NET 8 SDK present.
- Chrome is **not** installed; Edge is. Always probe at runtime (Chrome → Edge → other Chromium) — never hardcode a browser path.
- Windows binaries cannot write to MSYS paths (`/tmp`): convert with `cygpath` (the demo scripts already do).
