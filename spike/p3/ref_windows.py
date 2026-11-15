#!/usr/bin/env python3
"""Rect oracle for the P3 windows (settings / skills / menu).

Builds one reference page per window out of the shipped markup and stylesheets,
paints a representative state, and dumps `getBoundingClientRect()` for every
block the Slint layout has to place. The numbers are the source of truth for the
`Metrics` constants in `slint/ui/{settings,skills,menu}.slint` — the same role
`spike/slint-demo/ref/` played for the panel and `spike/p2/heights_ref.py` for
the data-driven pane heights.

Reference pages are rendered at DPR 2, like the panel's: Chrome rounds
`line-height: normal` to the device grid, so at DPR 2.25 the same DOM reports
different heights (spike/p2/REPORT.md §5.5).

Usage:
  PYTHONUTF8=1 python spike/p3/ref_windows.py           # write rects-*.json
  PYTHONUTF8=1 python spike/p3/ref_windows.py --print   # ... and print a table
"""
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCALE = 2

BROWSERS = [
    Path(os.environ.get("LOCALAPPDATA", "")) / "Google/Chrome/Application/chrome.exe",
    Path("C:/Program Files/Google/Chrome/Application/chrome.exe"),
    Path("C:/Program Files (x86)/Google/Chrome/Application/chrome.exe"),
    Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"),
    Path("C:/Program Files/Microsoft/Edge/Application/msedge.exe"),
]


def find_browser() -> str:
    for candidate in BROWSERS:
        if candidate.is_file():
            return str(candidate)
    raise SystemExit("no Chromium browser found (Chrome -> Edge)")


def read(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


def body_of(html: str) -> str:
    match = re.search(r"<body[^>]*>(.*)</body>", html, re.S | re.I)
    if not match:
        raise SystemExit("no <body>")
    inner = re.sub(r"<script\b[^>]*>.*?</script[^>]*>", "", match.group(1), flags=re.S | re.I)
    return inner.replace('<div id="root">', '<div class="root">').strip()


# Every selector the Slint layout mirrors. `@name` entries report the element's
# own box; a list means "all matches in document order".
MEASURE_JS = r"""
<script>
(function () {
  const params = new URLSearchParams(location.search);
  const theme = params.get('theme') === 'dark' ? 'dark' : 'light';
  document.documentElement.dataset.theme = theme;
  const q = (sel) => Array.from(document.querySelectorAll(sel));
  const round = (n) => Math.round(n * 100) / 100;
  const rect = (el) => {
    const r = el.getBoundingClientRect();
    return { x: round(r.x), y: round(r.y), w: round(r.width), h: round(r.height) };
  };
  const out = {};
  out['viewport'] = { x: 0, y: 0, w: window.innerWidth, h: window.innerHeight };
  function take(name, sel, all) {
    const nodes = q(sel);
    if (!nodes.length) return;
    out[name] = all ? nodes.map(rect) : rect(nodes[0]);
  }
  window.__measure = () => {};

  // ---- settings ----
  if (document.getElementById('btn-save')) {
    take('card', '.root');
    take('hdr', '.hdr');
    take('hdr-top', '.hdr-top');
    take('brand', '.brand');
    take('close', '.chrome-close');
    take('pane', '.pane');
    take('label', '.pane > .label', true);
    take('seg', '.pane > .seg', true);
    take('seg-item', '.seg-item', true);
    take('switch-row', '.switch-row');
    take('switch', '.switch');
    take('switch-label', '.switch-row .row-label');
    take('sect', '.sect', true);
    take('sect-label', '.sect > .label', true);
    take('row', '.sect .row', true);
    take('row-main', '.sect .row-main', true);
    take('row-label', '.sect .row-label', true);
    take('row-value', '.sect .row-value', true);
    take('actions', '.actions');
    take('action-btn', '.actions .btn', true);
    take('note', '.note', true);
    take('more', '.more');
    take('more-summary', '.more summary');
    take('cred-field', '.cred-field', true);
    take('cred-input', '.cred-field input', true);
    take('save', '.btn.save');
  }

  // ---- skills ----
  if (document.querySelector('#list')) {
    take('card', '.root');
    take('hdr', '.hdr');
    take('brand', '.brand');
    take('close', '.chrome-close');
    take('pane', '.pane');
    take('summary', '.summary');
    take('summary-label', '.summary .label');
    take('summary-btn', '.link-btn');
    take('list', '.list');
    take('group-title', '.group-title', true);
    take('skill', '.skill', true);
    take('skill-head', '.skill-head', true);
    take('skill-name', '.skill-name', true);
    take('skill-desc', '.skill-desc', true);
  }

  // ---- menu ----
  if (document.querySelector('.menu-item')) {
    take('card', '.root');
    take('item', '.menu-item', true);
  }

  const pre = document.createElement('pre');
  pre.id = 'rects';
  pre.style.display = 'none';
  pre.textContent = JSON.stringify(out);
  document.body.append(pre);
})();
</script>
"""


def page(title: str, css: str, inner: str, paint: str, extra_css: str = "") -> str:
    return f"""<!doctype html>
<html lang="en" data-theme="light">
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>
{css}
/* reference only: a top-pinned card on a neutral stage. The card is measured,
   not positioned, so it must not be bottom-anchored or clipped.
   `max-height: none` is on purpose: settings.css caps the card at the viewport
   so the real window can scroll it, but the layout constants need the UNCLAMPED
   content height (the same number settings.ts reports). */
html, body {{ background: #e7e7e4; height: auto; overflow: visible; }}
body {{ margin: 0; width: 404px; }}
#root, .root {{ position: static; max-height: none !important; }}
{extra_css}
</style>
</head>
<body>
{inner}
{paint}
{MEASURE_JS}
</body>
</html>
"""


SETTINGS_PAINT = r"""
<script>
(function () {
  const p = new URLSearchParams(location.search);
  const $ = (id) => document.getElementById(id);
  const q = (sel) => document.querySelector(sel);
  document.documentElement.dataset.theme = p.get('theme') === 'dark' ? 'dark' : 'light';
  q('input[name="theme"][value="' + (p.get('themeValue') || 'system') + '"]').checked = true;
  q('input[name="interval"][value="' + (p.get('interval') || '5') + '"]').checked = true;
  $('autostart').checked = p.get('autostart') !== '0';
  $('kimi-version').textContent = '0.39.1 \u2192 0.40.0';
  $('kimi-credential').textContent = 'OAuth token \u00b7 expires 17:20';
  $('kimi-credential').classList.toggle('bad', p.get('kimi') === 'expired');
  $('ds-api-key').textContent = 'Configured';
  $('ds-user-token').textContent = p.get('token') === 'missing' ? 'Not configured' : 'Configured';
  const note = $('ds-note');
  if (p.get('env') === '1') {
    note.textContent = 'Values come from DEEPSEEK_API_KEY / DEEPSEEK_USER_TOKEN \u2014 Clear does not remove them.';
    note.hidden = false;
  }
  if (p.get('manual') === '1') {
    $('manual').open = true;
  }
  // `cap=<px>` emulates the product's 90%-of-work-area clamp: the card stops at
  // that height and scrolls, which is what the Slint window does when the sheet
  // is taller than the work area allows.
  const cap = p.get('cap');
  if (cap) {
    const root = document.querySelector('.root');
    root.style.maxHeight = cap + 'px';
    root.style.overflowY = 'auto';
  }
  if (p.get('crednote') === '1') {
    const n = $('cred-note');
    n.textContent = 'Verified and saved.';
    n.hidden = false;
  }
})();
</script>
"""

SKILLS_PAINT = r"""
<script>
(function () {
  // The window paints the REAL scan; this page must too, or the band
  // comparison would diff two different lists. `__SKILLS_JSON__` is filled in
  // by build_skills() from `quota-tray-slint.exe --print-skills`.
  const params = new URLSearchParams(location.search);
  document.documentElement.dataset.theme = params.get('theme') === 'dark' ? 'dark' : 'light';
  // The Slint row reserves two description lines. `natural=1` skips this
  // override and measures what the product's own 2-line clamp produces.
  if (!params.get('natural')) {
    const style = document.createElement('style');
    style.textContent =
      '.skill { height: 69.94px; overflow: hidden; }' +
      '.skill-desc { height: 30.44px; overflow: hidden; }';
    document.head.append(style);
  }
  const items = __SKILLS_JSON__;
  document.getElementById('summary-text').textContent = items.length + ' skills';
  const list = document.getElementById('list');
  list.replaceChildren();
  let last = '';
  for (const s of items) {
    if (s.source !== last) {
      last = s.source;
      const g = document.createElement('div');
      g.className = 'group-title';
      g.textContent = s.source;
      list.append(g);
    }
    const row = document.createElement('div');
    row.className = 'skill';
    row.title = s.description || '';
    const head = document.createElement('div');
    head.className = 'skill-head';
    const n = document.createElement('span');
    n.className = 'skill-name';
    n.textContent = s.name;
    head.append(n);
    const d = document.createElement('div');
    d.className = 'skill-desc';
    d.textContent = s.description || '(no description)';
    row.append(head, d);
    list.append(row);
  }
})();
</script>
"""

MENU_PAINT = r"""
<script>
document.documentElement.dataset.theme =
  new URLSearchParams(location.search).get('theme') === 'dark' ? 'dark' : 'light';
</script>
"""


def theme_scoped() -> str:
    """theme.css with `#root` re-spelled as `.root`: the reference has no id."""
    return read("src/theme.css").replace("#root", ".root")


def build_settings() -> Path:
    css = theme_scoped() + "\n" + read("src/settings.css").replace("#root", ".root")
    inner = body_of(read("settings.html"))
    target = HERE / "ref-settings.html"
    target.write_text(
        page("Quota Tray Settings - rect reference", css, inner, SETTINGS_PAINT, "body { width: 404px; }"),
        encoding="utf-8",
    )
    return target


def skills_json() -> str:
    """The real scan, so the reference paints the same list the window does."""
    exe = ROOT / "slint" / "target" / "debug" / "quota-tray-slint.exe"
    if not exe.is_file():
        raise SystemExit(f"build the shell first (or the reference would paint a fixture): {exe}")
    out = subprocess.run(
        [str(exe), "--print-skills"], capture_output=True, text=True,
        encoding="utf-8", errors="replace", timeout=120,
    ).stdout
    if not out.strip().startswith("["):
        raise SystemExit("--print-skills returned no JSON")
    # A description can contain "</script>": escape the slash so it cannot end
    # the inline script early.
    return out.strip().replace("</", "<\\/")


def build_skills() -> Path:
    css = theme_scoped() + "\n" + read("src/skills.css").replace("#root", ".root")
    inner = body_of(read("skills.html"))
    # The real window is 404x520; `#root { height: calc(100vh - 56px) }` needs the
    # viewport to be exactly that tall, hence --window-size below.
    # Reference-only box model: the Slint row reserves two description lines, so
    # the page pins .skill/.skill-desc to that box too — otherwise the diff would
    # measure the known 1-line-row deviation instead of the layout
    # (spike/p3/REPORT.md §5).
    # Pin the card's height: the real window is 520 tall, so
    # `calc(100vh - 56px)` is 464. `--dump-dom` ignores --window-size (its
    # viewport is not the window), so the measurement path needs the number.
    extra = """body { width: 404px; }
.root { height: 464px; }
"""
    target = HERE / "ref-skills.html"
    paint = SKILLS_PAINT.replace("__SKILLS_JSON__", skills_json())
    target.write_text(
        page("Kimi Skills - rect reference", css, inner, paint, extra),
        encoding="utf-8",
    )
    return target


def build_menu() -> Path:
    css = theme_scoped() + "\n" + read("src/menu.css").replace("#root", ".root")
    inner = body_of(read("menu.html"))
    target = HERE / "ref-menu.html"
    target.write_text(
        page("Tray menu - rect reference", css, inner, MENU_PAINT, "body { width: 188px; }"),
        encoding="utf-8",
    )
    return target


def measure(browser: str, target: Path, query: str, window: str) -> dict:
    url = f"file:///{target.as_posix()}?{query}"
    with tempfile.TemporaryDirectory() as profile:
        out = subprocess.run(
            [
                browser, "--headless=new", "--disable-gpu", "--no-first-run",
                f"--force-device-scale-factor={SCALE}", f"--window-size={window}",
                "--virtual-time-budget=3000", f"--user-data-dir={profile}",
                "--dump-dom", url,
            ],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=90,
        ).stdout
    match = re.search(r'<pre id="rects"[^>]*>(.*?)</pre>', out, re.S)
    if not match:
        raise SystemExit(f"rect dump missing for {target.name}?{query}")
    return json.loads(match.group(1))


def show(name: str, data: dict) -> None:
    print(f"\n== {name} ==")
    for key, value in data.items():
        if isinstance(value, list):
            for i, r in enumerate(value):
                print(f"  {key}[{i}]".ljust(16) + f" x={r['x']:>7} y={r['y']:>7} w={r['w']:>6} h={r['h']:>7}")
        else:
            print(f"  {key:<14} x={value['x']:>7} y={value['y']:>7} w={value['w']:>6} h={value['h']:>7}")


def main() -> int:
    browser = find_browser()
    settings = build_settings()
    skills = build_skills()
    menu = build_menu()

    readings = {}

    def run(key: str, target: Path, query: str, window: str) -> dict:
        data = measure(browser, target, query, window)
        readings[key] = data
        if "--print" in sys.argv:
            show(key, data)
        return data

    # Settings, four states the Slint layout has to size: the shipped default,
    # the env note, the expanded manual block, and the expanded block with a
    # verification note (the tallest state).
    run("settings-default", settings, "theme=light", "404,900")
    run("settings-env", settings, "theme=light&env=1", "404,900")
    run("settings-manual", settings, "theme=light&manual=1", "404,900")
    run("settings-manual-note", settings, "theme=light&manual=1&crednote=1", "404,900")
    run("settings-dark", settings, "theme=dark", "404,900")

    # Skills: the window is fixed 404x520, the viewport below the summary scrolls.
    run("skills", skills, "theme=light", "404,520")
    run("skills-dark", skills, "theme=dark", "404,520")

    # Menu: content-sized card.
    run("menu", menu, "theme=light", "188,400")

    out = HERE / "rects-windows.json"
    out.write_text(json.dumps(readings, indent=2) + "\n", encoding="utf-8")
    print(f"\nwritten: {out.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
