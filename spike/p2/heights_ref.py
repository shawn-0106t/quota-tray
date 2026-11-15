#!/usr/bin/env python3
"""Height oracle for the Slint panel: measure the shipped DOM per data variant.

The panel's card height is data-dependent (0-4 model rows, the local cache block,
the hint that replaces the rows). `slint/ui/*.slint` derives that height from the
`Metrics` constants, and the shell's `--test-heights` prints what it computed.
This script builds the ground truth: `index.html` + the real CSS, painted with
the same variant data, measured by a headless Chromium at DPR 2 (the DPR the
layout was verified at — Chrome rounds `line-height: normal` to the device grid).

Usage:
  PYTHONUTF8=1 python spike/p2/heights_ref.py            # write heights-ref.json
  PYTHONUTF8=1 python spike/p2/heights_ref.py --compare  # ... and diff the Slint exe

Writes spike/p2/ref-variants.html and spike/p2/heights-ref.json.
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

# Variant names are the contract with `slint/src/checks.rs::variant_data`: the
# same name must describe the same pane state on both sides.
PAINT = r"""
<script>
(function () {
  const params = new URLSearchParams(location.search);
  const variant = params.get('variant') || 'kimi-full';
  document.documentElement.dataset.theme = params.get('theme') === 'dark' ? 'dark' : 'light';
  document.body.classList.add('enter');

  const $ = (id) => document.getElementById(id);
  const money = (yuan) => '\u00a5' + yuan.toFixed(2);
  const count = (n) => n.toLocaleString('en-US');
  const tokens = (n) => (n >= 1e6 ? (n / 1e6).toFixed(2).replace(/\.?0+$/, '') + 'M'
    : n >= 1e3 ? (n / 1e3).toFixed(1).replace(/\.?0+$/, '') + 'K' : String(n));

  const showTab = (tab) => {
    const ds = tab === 'ds';
    $('pane-kimi').hidden = ds;
    $('pane-deepseek').hidden = !ds;
    document.querySelector('input[name="provider"][value="' + (ds ? 'deepseek' : 'kimi') + '"]').checked = true;
    $('last-updated').textContent = ds ? 'Updated 16:31' : 'Updated 16:42';
  };

  const paintKimi = (o) => {
    $('week-pct').textContent = o.week;
    $('week-fill').style.width = o.week + '%';
    $('week-reset').textContent = o.weekReset;
    $('five-pct').textContent = o.five;
    $('five-fill').style.width = o.five + '%';
    $('five-reset').textContent = o.fiveReset;
    $('kimi-cache').hidden = !o.cache;
    $('kimi-speed').hidden = !o.speed;
    if (o.cache) {
      $('cache-week').textContent = '99.5';
      $('cache-week-fill').style.width = '99.5%';
      $('cache-week-note').textContent = count(3169) + ' requests';
      $('cache-today').textContent = '99.7';
      $('cache-today-fill').style.width = '99.7%';
      $('cache-today-note').textContent = count(807) + ' requests';
    }
    if (o.speed) {
      $('speed-value').textContent = '135.8 tok/s';
      $('speed-note').textContent = 'First token 1.04 s \u00b7 ' + count(3166) + ' calls';
    }
    $('extra-balance').textContent = o.extra;
    $('extra-monthly').hidden = !o.monthly;
    if (o.monthly) {
      $('extra-fill').style.width = '45%';
      $('extra-monthly-text').textContent = 'Used ' + money(0.45) + ' this month / ' + money(1) + ' limit';
    }
    $('cli-version').textContent = o.cli;
    $('badge').hidden = !o.badge;
  };

  const paintDs = (o) => {
    $('ds-status').textContent = o.status;
    $('ds-status').classList.toggle('bad', !!o.statusBad);
    $('ds-total').textContent = o.total;
    $('ds-total-num').classList.toggle('is-danger', !!o.totalBad);
    $('ds-detail').hidden = !o.detail;
    $('ds-detail').textContent = o.detail || '';
    const list = $('ds-models');
    const summary = $('ds-summary');
    list.replaceChildren();
    if (o.models && o.models.length) {
      const maxTokens = o.models[0][1];
      for (const [name, monthTokens, monthCost, hitRate, todayCost] of o.models) {
        const row = document.createElement('div');
        row.className = 'mrow';
        const main = document.createElement('div');
        main.className = 'mrow-main';
        const label = document.createElement('span');
        label.className = 'mname';
        label.textContent = name;
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
      list.className = 'mrows';
      list.hidden = false;
      $('ds-sum').textContent = 'This month ' + money(o.monthCost) + ' \u00b7 ' + tokens(o.monthTokens) + ' tok';
      $('ds-sum-today').textContent = 'today ' + money(o.todayCost);
      summary.hidden = false;
    } else if (o.hint) {
      list.className = 'hint' + (o.hintBad ? ' bad' : '');
      list.textContent = o.hint;
      list.hidden = false;
      summary.hidden = true;
    } else {
      list.hidden = true;
      summary.hidden = true;
    }
  };

  const KIMI_FULL = {
    week: '12', weekReset: 'Resets in 2d 18h', five: '31', fiveReset: 'Resets in 3h 33m',
    cache: true, speed: true, extra: 'No data', monthly: false, cli: '0.39.1', badge: true,
  };
  const MODELS = {
    4: [
      ['DeepSeek-V4.1-Flash', 665797039, 28.50316864, 99.6, 4.531104],
      ['DeepSeek-V4.1-Pro', 184200311, 12.88204117, 71.3, 1.902244],
      ['DeepSeek-V3.2', 41203311, 2.13409922, 58.4, 0.301188],
      ['DeepSeek-V4.1-Reasoner-Pro-Max-Extended', 9033112, 0.88201133, null, 0.112004],
    ],
    3: [
      ['DeepSeek-V4.1-Flash', 665797039, 28.50316864, 99.6, 4.531104],
      ['DeepSeek-V4.1-Pro', 184200311, 12.88204117, 71.3, 1.902244],
      ['DeepSeek-V3.2', 41203311, 2.13409922, 58.4, 0.301188],
    ],
    2: [
      ['DeepSeek-V4.1-Flash', 665797039, 28.50316864, 99.6, 4.531104],
      ['DeepSeek-V4.1-Pro', 184200311, 12.88204117, 71.3, 1.902244],
    ],
    1: [
      ['DeepSeek-V4.1-Flash', 665797039, 28.50316864, 99.6, 4.531104],
    ],
  };
  const dsRows = (n) => ({
    status: 'Available', total: money(45.44), detail: 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44),
    models: MODELS[n], monthCost: 44.4, monthTokens: 897000000, todayCost: 6.85,
  });

  const VARIANTS = {
    'kimi-full': ['kimi', KIMI_FULL],
    'kimi-nocache': ['kimi', Object.assign({}, KIMI_FULL, { cache: false, speed: false })],
    'kimi-cache-only': ['kimi', Object.assign({}, KIMI_FULL, { speed: false })],
    'kimi-extra-monthly': ['kimi', Object.assign({}, KIMI_FULL, { extra: money(45.44), monthly: true })],
    'kimi-not-activated': ['kimi', Object.assign({}, KIMI_FULL, { extra: 'Not activated' })],
    'kimi-no-badge': ['kimi', Object.assign({}, KIMI_FULL, { badge: false })],
    'ds-4': ['ds', dsRows(4)],
    'ds-3': ['ds', dsRows(3)],
    'ds-2': ['ds', dsRows(2)],
    'ds-1': ['ds', dsRows(1)],
    'ds-hint': ['ds', {
      status: 'Available', total: money(45.44), detail: 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44),
      hint: 'Usage token expired \u2014 sign in again to sync', hintBad: true,
    }],
    'ds-hint-short': ['ds', {
      status: 'Available', total: money(45.44), detail: 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44),
      hint: 'Usage token not configured',
    }],
    'ds-hint-rate': ['ds', {
      status: 'Available', total: money(45.44), detail: 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44),
      hint: 'Usage was rate limited \u2014 retrying in 30s',
    }],
    'ds-hint-empty': ['ds', {
      status: 'Available', total: money(45.44), detail: 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44),
      hint: 'No usage data this month',
    }],
    'ds-balance-only': ['ds', { status: 'Available', total: money(45.44),
      detail: 'Granted ' + money(0) + ' \u00b7 Topped up ' + money(45.44) }],
    'ds-nokey': ['ds', { status: 'Not configured', statusBad: true, total: '--', totalBad: true,
      detail: 'API key not set', hint: 'Usage token not configured' }],
    'ds-failed': ['ds', { status: 'Fetch failed', statusBad: true, total: '--', totalBad: true,
      hint: 'Usage fetch failed \u2014 retrying in 30s', hintBad: true }],
  };

  const [tab, data] = VARIANTS[variant] || VARIANTS['kimi-full'];
  showTab(tab);
  if (tab === 'ds') paintDs(data); else paintKimi(data);

  // Card pinned to the top (not bottom): the height is font dependent, and a
  // bottom-anchored card would move every child by whatever the two engine
  // runs disagree on. Heights are unaffected; only the offsets are stable.
  const card = document.getElementById('root');
  const hint = document.getElementById('ds-models');
  const reading = {
    card: Math.round(card.getBoundingClientRect().height * 100) / 100,
    hint: hint.classList.contains('hint') ? Math.round(hint.getBoundingClientRect().height * 100) / 100 : null,
    hintLines: hint.classList.contains('hint')
      ? Math.round(hint.getBoundingClientRect().height / 16.5) : null,
  };
  const pre = document.createElement('pre');
  pre.id = 'heights';
  pre.style.display = 'none';
  pre.textContent = JSON.stringify(reading);
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
    page = f"""<!doctype html>
<html lang="en" data-theme="light">
<head>
<meta charset="utf-8">
<title>Quota Tray — height reference</title>
<style>
{theme_css}
{main_css}
/* reference only: a neutral stage and a top-pinned card. `height: 100%` keeps
   the absolutely positioned card inside the body's box — with a zero-height
   body, `overflow: hidden` clips the card and the screenshot comes out blank
   (the DOM still reports its rect, so only the pixels expose it). */
html, body {{ background: #e7e7e4; height: 100%; }}
body.panel {{ width: 424px; position: relative; }}
body.panel > #root {{ top: 28px; bottom: auto; }}
</style>
</head>
<body class="panel">
{strip_scripts(body.group(1))}
{PAINT}
</body>
</html>
"""
    target = HERE / "ref-variants.html"
    target.write_text(page, encoding="utf-8")
    return target


VARIANTS = [
    "kimi-full", "kimi-nocache", "kimi-cache-only", "kimi-extra-monthly",
    "kimi-not-activated", "kimi-no-badge",
    "ds-4", "ds-3", "ds-2", "ds-1",
    "ds-hint", "ds-hint-short", "ds-hint-rate", "ds-hint-empty",
    "ds-balance-only", "ds-nokey", "ds-failed",
]


def measure(browser: str, page: Path, variant: str) -> dict:
    url = f"file:///{page.as_posix()}?variant={variant}"
    with tempfile.TemporaryDirectory() as profile:
        out = subprocess.run(
            [
                browser, "--headless=new", "--disable-gpu", "--no-first-run",
                f"--force-device-scale-factor={SCALE}",
                "--virtual-time-budget=3000", f"--user-data-dir={profile}",
                "--dump-dom", url,
            ],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=90,
        ).stdout
    match = re.search(r'<pre id="heights"[^>]*>(.*?)</pre>', out, re.S)
    if not match:
        raise SystemExit(f"height dump missing for {variant}")
    return json.loads(match.group(1))


def slint_heights() -> dict:
    exe = ROOT / "slint" / "target" / "debug" / "quota-tray-slint.exe"
    if not exe.is_file():
        raise SystemExit(f"build the shell first: {exe}")
    out = subprocess.run([str(exe), "--test-heights"], capture_output=True, text=True,
                         encoding="utf-8", errors="replace", timeout=180).stdout
    rows = {}
    for line in out.splitlines():
        m = re.match(r"height: (\S+) card=([\d.]+) win=([\d.]+)", line)
        if m:
            rows[m.group(1)] = {"card": float(m.group(2)), "win": float(m.group(3))}
    missing = [v for v in VARIANTS if v not in rows]
    if missing:
        raise SystemExit(f"slint heights missing: {missing}")
    return rows


def main() -> int:
    browser = find_browser()
    page = build_page()
    ref = {}
    for variant in VARIANTS:
        reading = measure(browser, page, variant)
        ref[variant] = reading
        hint = "" if reading["hint"] is None else f"  hint={reading['hint']}px lines={reading['hintLines']}"
        print(f"ref  {variant:<20} card={reading['card']:>6}px{hint}")
    (HERE / "heights-ref.json").write_text(json.dumps(ref, indent=2) + "\n", encoding="utf-8")
    print(f"written: {(HERE / 'heights-ref.json').relative_to(ROOT)}")

    if "--compare" in sys.argv:
        got = slint_heights()
        bad = 0
        print(f"\n{'variant':<20} {'ref card':>9} {'slint':>9} {'delta':>6} {'ref win':>9} {'slint':>9}")
        for variant in VARIANTS:
            want = ref[variant]["card"]
            have = got[variant]["card"]
            want_win = want + 56
            have_win = got[variant]["win"]
            delta = have - want
            flag = "" if abs(delta) < 0.51 and abs(have_win - want_win) < 0.51 else "  MISMATCH"
            if flag:
                bad += 1
            print(f"{variant:<20} {want:>9.2f} {have:>9.2f} {delta:>+6.2f} {want_win:>9.2f} {have_win:>9.2f}{flag}")
        print(f"\nverdict: {'PASS' if bad == 0 else f'FAIL ({bad} mismatched)'}")
        return 0 if bad == 0 else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
