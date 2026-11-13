# AGENTS.md — Quota Tray

Guidance for AI coding agents working in this repository. Read this first; then read `PLAN.md` for the design contract.

## Project overview

**Quota Tray** is a Windows-only system tray app that shows two providers' quota in one panel, switched by tabs:

- **Kimi** — Kimi Code plan quota (5-hour + weekly windows, reset countdowns, Extra Usage wallet), read from the local Kimi Code CLI OAuth token and `GET https://api.kimi.com/coding/v1/usages`.
- **DeepSeek** — account balance (official API) and per-model token/cost usage (the platform's internal usage endpoint, with a WebView2 sign-in window to capture the user token).

This repository is the merge of two earlier first-party projects: `kimi-planbar-tray` (Tauri/rust edition) and `deepseek-planbar` (WPF edition). See `NOTICE` for lineage.

**Current state: design frozen, code not yet landed.** The repository currently contains only design artifacts (`demo/`) and the plan. Phase 1 copies the `kimi-planbar-tray/rust/` source tree into the repo root — until that happens, there is nothing to build. Do not invent source files ahead of Phase 1; follow `PLAN.md` §8.

## Source of truth

| Question | Answer |
|---|---|
| What should the UI look like? | `demo/panel-demo-d.html` — the frozen design. `demo/panel-demo-a/b/c.html` are the rejected alternatives, kept only for comparison; do not evolve them. |
| Layout numbers, tokens, colours | `PLAN.md` §3 |
| Behaviour (polling, credentials, failure semantics, traps) | `PLAN.md` §5, §7 |
| Phases and their acceptance criteria | `PLAN.md` §8 |

If code and plan disagree, the plan wins until it is deliberately amended — change `PLAN.md` in the same commit as the behaviour change.

## Repository layout

```
demo/                 # frozen design demos + review tooling (self-contained, zero deps)
  panel-demo-d.html   # THE design reference (C structure + A type + slim palette)
  panel-demo-a/b/c.html
  compare.html        # side-by-side viewer, theme/provider/state switchers
  shot.sh             # headless screenshot with ?query presets; probes Chrome -> Edge
  crop_panel.py       # crop to the 424px window, trim backdrop
  heights.py          # measured card/window height for every demo x tab x state
  check_contrast.py   # WCAG ratios for the token pairs (must stay all-pass)
PLAN.md               # design + behaviour contract
AGENTS.md             # this file
LICENSE / NOTICE      # MIT + upstream attribution
```

After Phase 1 the plan is for the `kimi-planbar-tray/rust/` tree to sit at the repo root (`index.html`, `settings.html`, `skills.html`, `menu.html`, `src/`, `src-tauri/`, `vite.config.ts`), with `docs/SPEC.md` carried over as the inherited behaviour spec. Update this section when that lands.

## Build and run (valid from Phase 1)

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
quota-tray.exe --test-deepseek     # DeepSeek balance + usage once (add --verbose for endpoint/diagnosis)
quota-tray.exe --test-ui           # construct every window, print "… OK" lines
quota-tray.exe --test-login        # construct the DeepSeek sign-in window, print init result
```

`cargo test` covers pure logic only (parsers, recovery classification, skills frontmatter). DeepSeek parse tests run against the fixtures carried over from the parent repo (`tests/fixtures`).

Design review is part of the deliverable: after any visual change, regenerate screenshots and compare against `demo/panel-demo-d.html` at 424px, in both themes:

```bash
bash demo/shot.sh demo/panel-demo-d.html .shots/x.png "bare=1&theme=dark&tab=ds" 560 900 2
PYTHONUTF8=1 python demo/crop_panel.py .shots/x.png .shots/x-crop.png 424 2
PYTHONUTF8=1 python demo/heights.py
PYTHONUTF8=1 python demo/check_contrast.py
```

`.shots/` is gitignored; delete screenshots after reviewing.

## Code style and conventions

- Code, comments and commit messages in English; conversation with the user in Chinese. Docs: `PLAN.md`/`AGENTS.md` in Chinese, README bilingual.
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
- **Failure semantics**: keep last-good values, show "Update failed", retry after 30 s, return to the configured interval on success.
- **Single instance**: named mutex `QuotaTray.SingleInstance` — do not rename.
- **DPI**: size windows with `LogicalSize`, position them with `PhysicalPosition`.
- **Portable mode**: empty `portable.dat` next to the exe moves `settings.json` to the exe directory.
- **Win11 corners**: every window opts out of DWM auto-rounding; the CSS paints the radius.
- **WebView2 recovery**: rebuild windows in place on `ProcessFailed`, never restart the process; `prevent_exit` on implicit exit; single-flight rebuilds with 1/2/5 s backoff, cap 3 in 30 s; log to `webview.log`, never tokens.
- **Tray**: left click toggles the panel, right click opens the menu window at the cursor, 300 ms guard after an auto-hide, 10 s throttled hover prefetch.

New in this project:

- **Panel height**: content-driven and **bottom-anchored**. The frontend measures and reports; the backend resizes with `LogicalSize` and re-pins the window's bottom-right corner with `PhysicalPosition`, clamped to the work area. Never leave the pane's bottom padding to a parent container — the last row must carry it, or that row reads optically high (this was a real defect in the demos; see `PLAN.md` §3.4).
- **Two provider pollers** share the interval and failure semantics but must not overwrite each other's last-good state.
- **DeepSeek parsing**: `data.biz_data` is an object for `amount` and an array for `cost` — tolerate both; `REQUEST` counts requests, not tokens; new token types fall through to the total; all-zero placeholder models are filtered; top 4 by month cost.
- **DeepSeek endpoints**: constants + candidate fallback + learn-from-login-window (`PLAN.md` §6). Model names are data — never hardcode a model list.
- **Colour semantics**: blue = usage, red = failure only. Never colour a high-usage percentage; `Available` stays neutral.

## Security considerations

- The Kimi OAuth token / API key is read-only and goes **only** to `https://api.kimi.com/coding/v1/usages` as a Bearer token. Never log, persist elsewhere, or send to any other endpoint.
- DeepSeek credentials stay in `~/.deepseek-planbar/credentials.json`; the app talks only to `api.deepseek.com` and `platform.deepseek.com`. The sign-in window captures the user token from the user's own session and verifies it with a real `amount` request before saving.
- Autostart writes HKCU only — never HKLM, never elevation. Keep the non-affiliation notice and `LICENSE` / `NOTICE` intact.
- Repo hygiene: no absolute local paths, no secrets, no real personal or customer data in tracked files; `*.example` templates only.

## Release process

Not defined yet. The parent project used a tag-triggered draft Release with a gitleaks gate; port that pipeline in Phase 5 and document it here before publishing anything.

## Environment notes (this machine)

- Windows 10/11, Chinese locale (code page 936). Shell is Git Bash: POSIX syntax, forward slashes, single quotes.
- Python 3.13 (`PYTHONUTF8=1`), Node 24, Rust 1.97 (MSVC), WebView2 runtime present, .NET 8 SDK present.
- Chrome is **not** installed; Edge is. Always probe at runtime (Chrome → Edge → other Chromium) — never hardcode a browser path.
- Windows binaries cannot write to MSYS paths (`/tmp`): convert with `cygpath` (the demo scripts already do).
