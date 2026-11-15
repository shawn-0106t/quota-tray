#!/usr/bin/env python3
"""Build the HTML reference for the Slint spike, straight from the shipped markup.

`index.html` + `src/theme.css` + `src/main.css` are the product's own DOM and
styles; this script inlines them into a single static page, paints the same
sample data `demo/build_ui_demo.py` paints, pins the body to 424 x (card + 56),
and then dumps every element's bounding rect. The `.slint` panel is built from
those numbers, so the comparison measures the renderer, not a hand-computed
layout model.

Usage: PYTHONUTF8=1 python spike/slint-demo/ref/build_ref.py
Writes spike/slint-demo/ref/{panel-ref.html,rects-<theme>-<tab>.json} and
.shots/slint-demo/ref-<theme>-<tab>.png (848 px wide at scale 2).
"""
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SHOTS = ROOT / ".shots" / "slint-demo"
SCALE = 2

BROWSERS = [
    Path(os.environ.get("LOCALAPPDATA", "")) / "Google/Chrome/Application/chrome.exe",
    Path("C:/Program Files/Google/Chrome/Application/chrome.exe"),
    Path("C:/Program Files (x86)/Google/Chrome/Application/chrome.exe"),
    Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"),
    Path("C:/Program Files/Microsoft/Edge/Application/msedge.exe"),
]

PAINT = r"""
<script>
(function () {
  const params = new URLSearchParams(location.search);
  const dark = params.get('theme') === 'dark';
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  const tab = params.get('tab') === 'ds' ? 'ds' : 'kimi';

  document.body.classList.add('enter');
  document.getElementById('pane-kimi').hidden = tab !== 'kimi';
  document.getElementById('pane-deepseek').hidden = tab !== 'ds';
  document.querySelector('input[name="provider"][value="' + (tab === 'ds' ? 'deepseek' : 'kimi') + '"]').checked = true;

  const $ = (id) => document.getElementById(id);
  const money = (yuan) => '\u00a5' + yuan.toFixed(2);
  const count = (n) => n.toLocaleString('en-US');
  const tokens = (n) => (n >= 1e6 ? (n / 1e6).toFixed(2).replace(/\.?0+$/, '') + 'M'
    : n >= 1e3 ? (n / 1e3).toFixed(1).replace(/\.?0+$/, '') + 'K' : String(n));

  // Kimi — the same figures as demo/build_ui_demo.py (the README screenshots).
  $('last-updated').textContent = 'Updated 16:42';
  $('week-pct').textContent = '12';
  $('week-fill').style.width = '12%';
  $('week-reset').textContent = 'Resets in 2d 18h';
  $('five-pct').textContent = '31';
  $('five-fill').style.width = '31%';
  $('five-reset').textContent = 'Resets in 3h 33m';
  $('kimi-cache').hidden = false;
  $('cache-week').textContent = '99.5';
  $('cache-week-fill').style.width = '99.5%';
  $('cache-week-note').textContent = count(3169) + ' requests';
  $('cache-today').textContent = '99.7';
  $('cache-today-fill').style.width = '99.7%';
  $('cache-today-note').textContent = count(807) + ' requests';
  $('kimi-speed').hidden = false;
  $('speed-value').textContent = '135.8 tok/s';
  $('speed-note').textContent = 'First token 1.04 s \u00b7 ' + count(3166) + ' calls';
  $('extra-balance').textContent = 'No data';
  $('cli-version').textContent = '0.39.1';
  $('badge').hidden = false;

  // DeepSeek — four models, one long official name, one sub-60% hit rate.
  $('ds-status').textContent = 'Available';
  $('ds-total').textContent = money(45.44);
  $('ds-detail').hidden = false;
  $('ds-detail').textContent = 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44);
  const models = [
    ['DeepSeek-V4.1-Flash', 665797039, 28.50316864, 99.6, 4.531104],
    ['DeepSeek-V4.1-Pro', 184200311, 12.88204117, 71.3, 1.902244],
    ['DeepSeek-V3.2', 41203311, 2.13409922, 58.4, 0.301188],
    ['DeepSeek-V4.1-Reasoner-Pro-Max-Extended', 9033112, 0.88201133, null, 0.112004],
  ];
  const maxTokens = models[0][1];
  const list = $('ds-models');
  list.replaceChildren();
  for (const [name, monthTokens, monthCost, hitRate, todayCost] of models) {
    const row = document.createElement('div');
    row.className = 'mrow';
    const main = document.createElement('div');
    main.className = 'mrow-main';
    const label = document.createElement('span');
    label.className = 'mname';
    label.textContent = name;
    label.title = name;
    const cost = document.createElement('span');
    cost.className = 'mcost';
    cost.textContent = money(monthCost);
    main.append(label, cost);
    const bar = document.createElement('div');
    bar.className = 'bar share';
    const fill = document.createElement('i');
    fill.style.width = Math.min(100, (monthTokens / maxTokens) * 100) + '%';
    bar.append(fill);
    const meta = document.createElement('div');
    meta.className = 'meta';
    const used = document.createElement('span');
    used.textContent = tokens(monthTokens) + ' tok \u00b7 ';
    const hit = document.createElement('span');
    hit.className = hitRate != null && hitRate < 60 ? 'hit is-bad' : 'hit';
    hit.textContent = hitRate == null ? 'cache --' : 'cache ' + String(hitRate).replace(/\.0$/, '') + '%';
    used.append(hit);
    const today = document.createElement('span');
    today.textContent = 'today ' + money(todayCost);
    meta.append(used, today);
    row.append(main, bar, meta);
    list.append(row);
  }
  $('ds-sum').textContent = 'This month ' + money(44.40) + ' \u00b7 897M tok';
  $('ds-sum-today').textContent = 'today ' + money(6.85);
  $('ds-summary').hidden = false;

  // The window is card + 2 x 28 px of shadow room. `?h=` pins it so the
  // screenshot and the rect dump cannot disagree about the body height.
  const card = document.getElementById('root');
  const h = card.getBoundingClientRect().height;
  const pinned = params.get('h');
  const win = pinned ? Number(pinned) : Math.round(h) + 56;
  document.body.style.height = win + 'px';

  // Every element inside the card, with its box relative to the window origin.
  const rects = {};
  const seen = new Map();
  const origin = document.body.getBoundingClientRect();
  const key = (node) => {
    const cls = (node.getAttribute('class') || '').trim().split(/\s+/).filter(Boolean).join('.');
    let base = node.tagName.toLowerCase() + (node.id ? '#' + node.id : '') + (cls ? '.' + cls : '');
    const n = (seen.get(base) || 0) + 1;
    seen.set(base, n);
    return n === 1 ? base : base + '#' + n;
  };
  for (const node of card.querySelectorAll('*')) {
    const r = node.getBoundingClientRect();
    if (r.width < 0.5 || r.height < 0.5) continue;
    rects[key(node)] = [r.x - origin.x, r.y - origin.y, r.width, r.height].map((v) => Math.round(v * 100) / 100);
  }
  const rr = card.getBoundingClientRect();
  rects['#root'] = [rr.x - origin.x, rr.y - origin.y, rr.width, rr.height].map((v) => Math.round(v * 100) / 100);
  rects['__window'] = [Math.round(win), Math.round(h * 100) / 100];

  const pre = document.createElement('pre');
  pre.id = 'rects';
  pre.style.display = 'none';
  pre.textContent = JSON.stringify(rects);
  document.body.append(pre);
})();
</script>
"""


def find_browser() -> str:
    for candidate in BROWSERS:
        if candidate.is_file():
            return str(candidate)
    raise SystemExit("no Chromium browser found (Chrome -> Edge)")


def strip_scripts(inner: str) -> str:
    return re.sub(r"<script\b[^>]*>.*?</script>", "", inner, flags=re.S | re.I)


def build_page() -> Path:
    index = (ROOT / "index.html").read_text(encoding="utf-8")
    theme_css = (ROOT / "src" / "theme.css").read_text(encoding="utf-8")
    main_css = (ROOT / "src" / "main.css").read_text(encoding="utf-8")
    body = re.search(r"<body[^>]*>(.*)</body>", index, re.S)
    if not body:
        raise SystemExit("index.html has no <body>")
    inner = strip_scripts(body.group(1))
    page = f"""<!doctype html>
<html lang="en" data-theme="light">
<head>
<meta charset="utf-8">
<title>Quota Tray — spike reference</title>
<style>
{theme_css}
{main_css}
/* spike only: the stage the README screenshots sit on, and the shown state */
:root[data-theme='light'] {{ --stage: #e7e7e4; }}
:root[data-theme='dark'] {{ --stage: #0f0f12; }}
html, body {{ background: var(--stage); }}
body.panel {{ width: 424px; position: relative; }}
/* Pin the card to the top instead of the bottom: the card's height is font
   dependent, and a bottom-anchored card moves every child by whatever the two
   browser runs disagree on. With a fixed top the content's y is deterministic;
   only the bottom edge (and the shadow under it) can move by a fraction. */
body.panel > #root {{ top: 28px; bottom: auto; }}
</style>
</head>
<body class="panel">
{inner}
{PAINT}
</body>
</html>
"""
    target = HERE / "panel-ref.html"
    target.write_text(page, encoding="utf-8")
    print(f"written: {target.relative_to(ROOT)}")
    return target


def measure(browser: str, page: Path, theme: str, tab: str, height: int | None = None) -> tuple[dict, int]:
    url = f"file:///{page.as_posix()}?theme={theme}&tab={tab}"
    if height is not None:
        url += f"&h={height}"
    with tempfile.TemporaryDirectory() as profile:
        out = subprocess.run(
            [
                browser, "--headless=new", "--disable-gpu", "--no-first-run",
                # Same DPR as the screenshot: Chrome rounds `line-height: normal`
                # to the device grid, so a 1x dump describes a different layout
                # than the 2x render (0.5-1 px per text block, accumulating).
                f"--force-device-scale-factor={SCALE}",
                "--virtual-time-budget=4000", f"--user-data-dir={profile}",
                "--dump-dom", url,
            ],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120,
        ).stdout
    match = re.search(r'<pre id="rects"[^>]*>(.*?)</pre>', out, re.S)
    if not match:
        raise SystemExit(f"rect dump missing for {theme}/{tab}")
    rects = json.loads(match.group(1).strip())
    window = int(rects.pop("__window")[0])
    return rects, window


def shot(theme: str, tab: str, window: int) -> Path:
    raw = SHOTS / f"ref-{theme}-{tab}-raw.png"
    SHOTS.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["bash", (ROOT / "demo" / "shot.sh").as_posix(), (HERE / "panel-ref.html").as_posix(),
         raw.as_posix(), f"theme={theme}&tab={tab}&h={window}", "424", str(window), str(SCALE)],
        check=True, cwd=ROOT,
    )
    image = Image.open(raw)
    crop = image.crop((0, 0, 424 * SCALE, window * SCALE))
    target = SHOTS / f"ref-{theme}-{tab}.png"
    crop.save(target)
    return target


def main() -> int:
    browser = find_browser()
    page = build_page()
    for theme in ("light", "dark"):
        for tab in ("kimi", "ds"):
            _, window = measure(browser, page, theme, tab)
            rects, window = measure(browser, page, theme, tab, window)
            (HERE / f"rects-{theme}-{tab}.json").write_text(json.dumps(rects, indent=1), encoding="utf-8")
            png = shot(theme, tab, window)
            print(f"{theme}/{tab}: window={window} px  rects={len(rects)}  {png.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
