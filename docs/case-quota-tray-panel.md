# Case: Quota Tray — Quiet Utility Panel (narrow, dense, two-theme)

Source: Quota Tray, a Windows tray app (Tauri 2 + vanilla TS/HTML/CSS, MIT). Distilled
2026-11-14 from the shipped `src/theme.css` / `main.css` / `settings.css`; there is no
public URL yet, so the palette and measurements below are the source.
Temperament: **quiet utility**. Density comes from real readings, not decoration, and one
hue family is spent on exactly two roles.

## When to Use

Narrow, information-dense surfaces: tray panels, 320–480 px side panels, extension
popups, mini dashboards, editor/IDE webviews. The content must be real readings — quotas,
balances, token counts, versions, statuses — because the style has no other way to be
dense. Do not use it for marketing pages, docs sites, or anything whose job is persuasion
rather than monitoring.

## Colors

Two ladders, one per theme. A single value nearly works — the light accent still clears
3:1 on the dark paper (4.44) and the dark accent on the light paper (3.01) — but the
bar/ring shadings collapse against the other theme's track (2.98 and 2.76), so each theme
gets its own set instead of one shared ladder.

| Role | Light | Dark | Usage |
|---|---|---|---|
| paper | `#fbfaf9` | `#181817` | Card ground (the only opaque surface) |
| ink-1 | `#000000e6` | `#ffffffd6` | Values, headings |
| ink-2 | `#00000099` | `#ffffff8f` | Labels, captions, units |
| ink-3 | `#0000004d` | `#ffffff42` | Icons and decoration only (never text); disabled text uses ink-2 |
| hair | `#00000021` | `#ffffff1f` | Row rules, control borders |
| hair-soft | `#00000012` | `#ffffff12` | Bar tracks |
| accent | `#8c74b1` | `#9d88bc` | The one hue (see roles below) |
| accent-bar | `#7d61a6` | `#9d88bc` | Bar fills — 4.17 / 4.68 against the track; a focus ring in this colour measures 4.87 / 5.65 against the paper |
| accent-ink | `#6c5293` | `#b5abc4` | Chip text (5.04 / 6.55 on tint) |
| tint | `#8c74b12e` | `#9d88bc26` | Chip fill, 18% / 15% |
| seg-track | `#f1f0ec` | `#262624` | Segmented-control track |
| seg-on | `#ffffff` | `#3a3a37` | Active cell fill |
| seg-ring | `#7d61a6` | `#9d88bc` | Active cell ring (4.46 / 4.82 vs track) |
| danger | `#b42318` | `#ff6b6b` | Failure text only |
| shadow | `0 8px 18px #0000001f` | `0 8px 18px #00000059` | Card, must fade inside the 28 px margin |

**The accent has exactly two roles: usage and selection.** Bars and chips carry usage; the
segmented control's active cell carries selection. Nothing else may take it — no switch
tint, no button fill, no headline color, no hover glow. Red is the second and last
semantic color, and it means *failure* only: a 96% quota is not red, an inactive wallet is
not red.

Selection must not rely on a fill difference: the active cell's fill measured only
1.14:1 (light) and 1.33:1 (dark) against the track, under WCAG 1.4.11's 3:1 for a state
cue. Ring it with the accent instead — 4.46 / 4.82.

## Type Scale

- **Readout numeral**: 30 px / 600, tracking `-0.01em`, line-height 1.05, tabular numerals;
  the unit (15 px / 500, ink-2) sits on the baseline beside it, never as a superscript
- **Micro-label**: 10 px / 500, uppercase, `0.08em` tracking, ink-2 — one per data group
- **Data row**: label 12 px / 400 in ink-2, value 13 px / 600 in ink-1, both tabular so
  the column never shimmers
- **Header**: brand 13 px / 500; timestamp 11 px in ink-2; captions and notes 10.5 px
- System sans only; tabular numerals via `font-variant-numeric: tabular-nums` in the body
  font — no mono switch "to look technical", no web font
- Radius family: 12 card and segmented track / 8 controls and cells / 6 small icon and
  inline buttons / 4 badge / 2 bar; the switch is a 38×20 capsule (`9999px` track, `50%`
  dot) and is the one deliberate outsider — a toggle has to read as a toggle

## Layout Concept

**One card on a transparent window.** The window is 424 px wide (520 px is only the initial
height — real height follows the content) and holds a 368 px card plus 28 px of
shadow-fade room on every side; the card paints its own radius 12, 1 px hairline and
shadow, and the shadow's blur must fit inside that margin or the window edge cuts it into
a hard ring around the card. Inside: 14 px pane padding, groups separated by 1 px
hairlines, and the last row carries the group's bottom padding — leave it to a parent
container and the closing row reads optically high.

Height is content-driven and **bottom-anchored**: the panel grows upward from the tray
corner and re-pins its bottom-right edge, so a tab switch never moves the corner the
cursor is heading for.

## Signature Element

**The bar that is the number.** Each quota window is a micro-label, a 30 px numeral with
its unit, and a 4 px capsule bar whose fill length is that same value. Every hairline
separates two facts; there is no decorative axis anywhere in the panel. Group totals close
with a hairline above them, never a box around them.

## Key Components

- **Segmented control**: track `--seg-track`, 3 px padding, 2 px gap, 30 px cells, radius 8;
  active cell = `--seg-on` fill + inset 1 px `--seg-ring`. The same component is the
  provider tab, the theme picker and the interval picker — one way to say "selected" in
  the whole app
- **Ghost button**: 34 px tall, 1 px hair border, radius 8, transparent fill, 11.5 px / 500
- **Switch**: monochrome by rule — outlined track + ink dot off, ink track + paper dot on;
  a switch is state, not usage, so the accent never touches it
- **Data row**: name left, cost right, tabular numerals so the column never shimmers; a
  per-row 3 px share bar encodes token share, not cost share
- **Chip**: radius 4, `--tint` fill, `--accent-ink` text

## Anti-Examples

- Accent outside usage/selection: switch tint, filled buttons, accent headlines, glow on
  hover — the plan is incomplete the moment it appears
- Red for high usage, for "not activated", or for a credential that was never configured;
  red is failure only (an expired key is a failure, a missing one is not)
- A shadow blurrier than its fade room (40 px of blur in a 28 px margin)
- Cross-fading two stacked panes in a bottom-anchored window: the stack measures the
  taller pane and the window jumps to the wrong height
- Equal multi-column dashboards with centered values; this style is left-aligned labels
  with right-aligned numerals
- Mono numerals or uppercase everything "to read technical" — one micro-label tier is the
  whole allowance

## Why It Holds Together

- **Tokens are code, layouts only place them**: one CSS file owns every color and type
  primitive; per-window stylesheets never declare a color of their own, so a theme is a
  two-block swap
- **Contrast is a gate, not a taste**: the demo tooling ships an assertion script that
  checks ≥3:1 (non-text) / ≥4.5:1 (text) over 65 token pairs across the five design
  editions — 10 of them the shipped violet's — and it is rerun after any token change; the
  ratios above come from those runs, not from a color picker eyeball
- **The reference cannot drift**: a frozen static demo is the visual origin, while the
  shipped snapshot is generated from the real HTML/CSS — reviewing the product and
  reviewing the reference are the same act

---

## 并入 skill 时的备注（本仓库留存，复制进 skill 时删除本节）

- 目标位置：`~/.agents/skills/rationalism-design/references/case-quota-tray-panel.md`；
  `SKILL.md` 的 Case Library 表加一行（"Quiet utility panel | narrow dense tray/side
  panels, 320–480 px | `references/case-quota-tray-panel.md`"）。
- `SKILL.md` 的 Scope 现在写「只覆盖单页 web」，需补一句：case 亦覆盖窄面板 / 桌面壳
  界面（托盘面板、侧边栏、扩展弹窗）。
- 如要验证触发准确率，再往 `evals/evals.json` 加一条 prompt（如「给一个 400px 宽的
  监控侧栏，深色，像系统托盘面板」）。
- **不要带截图**：仓库里的截图含真实账户数字（余额、模型费用）。本文件是纯文字，
  与现有 case 的格式一致。
- 来源写 `Quota Tray (MIT, local repo)`，暂无 URL；等仓库公开后可补。
