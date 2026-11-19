"""Generate demo/ui-demo.html: every window of the app on one page.

The markup and CSS are lifted from the shipped files - `index.html`,
`settings.html`, `skills.html`, `menu.html` and their stylesheets - so the demo
cannot drift from the product. What the generator adds is the demo frame (stage,
labels, theme switch), per-window CSS scoping (each window's rules are prefixed
with its wrapper class, because e.g. `.pane` means something different in the
panel and in the settings sheet) and a paint script that fills the real DOM ids
with representative values.

Run: PYTHONUTF8=1 python demo/build_ui_demo.py
"""

import base64
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent


def read(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


def body_of(html: str) -> str:
    """Inner body HTML, minus the module scripts and the root id."""
    match = re.search(r"<body[^>]*>(.*)</body>", html, re.S | re.I)
    assert match, "no <body>"
    # Case-insensitive on purpose (HTML tag names are), and both tags accept
    # anything up to '>' the way a parser does: CodeQL's py/bad-tag-filter
    # rejects a strip that misses <SCRIPT> or a junk-filled </script \n>.
    inner = re.sub(r"<script\b[^>]*>.*?</script[^>]*>", "", match.group(1), flags=re.S | re.I)
    # A page can host several windows; duplicate ids would be invalid, and the
    # card styling only ever needs `.root`.
    inner = inner.replace('<div id="root">', '<div class="root">')
    return inner.strip()


def prefix_selectors(css: str, scope: str) -> str:
    """Prefix every selector in a stylesheet with `scope`.

    Handles one level of nesting (@media) and leaves @keyframes alone, which is
    all these stylesheets need.
    """
    css = re.sub(r"/\*.*?\*/", "", css, flags=re.S)
    out = []
    i = 0
    while i < len(css):
        brace = css.find("{", i)
        if brace < 0:
            break
        head = css[i:brace].strip()
        if head.startswith("@keyframes") or head.startswith("@font-face"):
            close = matching_brace(css, brace)
            out.append(css[brace - len(head) - 1 : close + 1] if False else css[i : close + 1])
            i = close + 1
            continue
        if head.startswith("@media"):
            close = matching_brace(css, brace)
            inner = css[brace + 1 : close]
            out.append(f"{head}{{{prefix_selectors(inner, scope)}}}")
            i = close + 1
            continue
        close = matching_brace(css, brace)
        body = css[brace + 1 : close]
        selectors = [s.strip() for s in head.split(",") if s.strip()]
        scoped = ", ".join(f"{scope} {s}" for s in selectors)
        out.append(f"{scoped}{{{body}}}")
        i = close + 1
    return "\n".join(out)


def matching_brace(css: str, open_index: int) -> int:
    depth = 0
    for index in range(open_index, len(css)):
        char = css[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("unbalanced braces")


def tray_icon() -> str:
    data = (ROOT / "src-tauri" / "icons" / "tray.png").read_bytes()
    return "data:image/png;base64," + base64.b64encode(data).decode("ascii")


def build() -> str:
    theme = read("src/theme.css").replace("#root", ".root")
    # The panel's show/hide animation is app behaviour, not design: drop it.
    main_css = read("src/main.css").replace("#root", ".root")
    main_css = re.sub(r"body\.enter \.root \{[^}]*\}", "", main_css)
    main_css = re.sub(r"body\.leave \.root \{[^}]*\}", "", main_css)
    main_css = re.sub(r"\.root \{\s*opacity: 0;\s*transform: translateY\(16px\);\s*\}", "", main_css)
    # The panel's card is bottom-anchored only in the real window: the rule hangs
    # off `body.panel`, and `body_of()` keeps the inner HTML only, so the demo's
    # <body> has no class and these selectors can never match there. Drop them
    # instead of shipping dead rules. `[^{}*/]*` keeps the pattern out of
    # comments, which quote `body.panel > #root` themselves.
    main_css = re.sub(r"body\.panel > \.root[^{}*/]*\{[^}]*\}", "", main_css)
    settings_css = read("src/settings.css").replace("#root", ".root")
    skills_css = read("src/skills.css").replace("#root", ".root")
    menu_css = read("src/menu.css").replace("#root", ".root")

    panel = body_of(read("index.html"))
    settings = body_of(read("settings.html"))
    skills = body_of(read("skills.html"))
    menu = body_of(read("menu.html"))

    # Two panel instances: the provider tab is a radio group, so the second one
    # needs its own name or the two instances would steal the selection.
    panel_kimi = (
        panel.replace('name="provider"', 'name="provider-kimi"')
        .replace('id="pane-deepseek" hidden>', 'id="pane-deepseek" hidden>')
    )
    panel_ds = (
        panel.replace('name="provider"', 'name="provider-ds"')
        .replace('<section class="pane" id="pane-kimi">', '<section class="pane" id="pane-kimi" hidden>')
        .replace('<section class="pane" id="pane-deepseek" hidden>', '<section class="pane" id="pane-deepseek">')
    )
    # Check the right radio in each instance.
    panel_kimi = panel_kimi.replace('value="kimi" />', 'value="kimi" checked />', 1)
    panel_ds = panel_ds.replace('value="deepseek" />', 'value="deepseek" checked />', 1)

    paint = """
    <script>
      (function () {
        // Theme switch: the buttons had aria-pressed but no listener (the page
        // only read ?theme=), so clicking Dark did nothing. Both paths must work.
        const params = new URLSearchParams(location.search);
        function applyTheme(value, remember) {
          const theme = value === 'dark' ? 'dark' : 'light';
          document.documentElement.dataset.theme = theme;
          for (const name of ['light', 'dark']) {
            const button = document.getElementById('t-' + name);
            if (button) button.setAttribute('aria-pressed', String(theme === name));
          }
          if (remember) {
            const url = new URL(location.href);
            url.searchParams.set('theme', theme);
            history.replaceState(null, '', url);
          }
        }
        applyTheme(params.get('theme'), false);
        for (const name of ['light', 'dark']) {
          const button = document.getElementById('t-' + name);
          if (button) button.addEventListener('click', () => applyTheme(name, true));
        }

        const money = (yuan, currency) => (currency === 'USD' ? '$' : '\\u00a5') + yuan.toFixed(2);
        const tokens = (n) => (n >= 1e6 ? (n / 1e6).toFixed(2).replace(/\\.?0+$/, '') + 'M'
          : n >= 1e3 ? (n / 1e3).toFixed(1).replace(/\\.?0+$/, '') + 'K' : String(n));
        const count = (n) => n.toLocaleString('en-US');

        function set(scope, id, text) {
          const node = scope.querySelector('#' + id);
          if (node) node.textContent = text;
        }
        function show(scope, id, visible) {
          const node = scope.querySelector('#' + id);
          if (node) node.hidden = !visible;
        }

        // ---- main panel: Kimi ----
        const kimi = document.querySelector('.win-kimi');
        if (kimi) {
          set(kimi, 'last-updated', 'Updated 16:42');
          set(kimi, 'week-pct', '12');
          kimi.querySelector('#week-fill').style.width = '12%';
          set(kimi, 'week-reset', 'Resets in 2d 18h');
          set(kimi, 'five-pct', '31');
          kimi.querySelector('#five-fill').style.width = '31%';
          set(kimi, 'five-reset', 'Resets in 3h 33m');
          show(kimi, 'kimi-cache', true);
          show(kimi, 'kimi-speed', true);
          set(kimi, 'cache-week', '99.5');
          kimi.querySelector('#cache-week-fill').style.width = '99.5%';
          set(kimi, 'cache-week-note', count(3169) + ' requests');
          set(kimi, 'cache-today', '99.7');
          kimi.querySelector('#cache-today-fill').style.width = '99.7%';
          set(kimi, 'cache-today-note', count(807) + ' requests');
          set(kimi, 'speed-value', '135.8 tok/s');
          set(kimi, 'speed-note', 'First token 1.04 s \\u00b7 ' + count(3166) + ' calls');
          set(kimi, 'extra-balance', 'No data');
          set(kimi, 'cli-version', '0.39.1');
          show(kimi, 'badge', true);
        }

        // ---- main panel: DeepSeek ----
        const ds = document.querySelector('.win-deepseek');
        if (ds) {
          set(ds, 'last-updated', 'Updated 16:42');
          set(ds, 'ds-status', 'Available');
          set(ds, 'ds-total', money(45.44, 'CNY'));
          set(ds, 'ds-detail', 'Granted ' + money(0, 'CNY') + ' \\u00b7 Topped up ' + money(45.44, 'CNY'));
          show(ds, 'ds-detail', true);
          const models = [
            ['DeepSeek-V4.1-Flash', 665797039, 28.50316864, 99.6, 4.531104],
            ['DeepSeek-V4.1-Pro', 184200311, 12.88204117, 71.3, 1.902244],
            ['DeepSeek-V3.2', 41203311, 2.13409922, 58.4, 0.301188],
            ['DeepSeek-V4.1-Reasoner-Pro-Max-Extended', 9033112, 0.88201133, null, 0.112004],
          ];
          const maxTokens = models[0][1];
          const list = ds.querySelector('#ds-models');
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
            cost.textContent = money(monthCost, 'CNY');
            main.append(label, cost);
            const bar = document.createElement('div');
            bar.className = 'bar share';
            const fill = document.createElement('i');
            fill.style.width = Math.min(100, (monthTokens / maxTokens) * 100) + '%';
            bar.append(fill);
            const meta = document.createElement('div');
            meta.className = 'meta';
            const used = document.createElement('span');
            used.textContent = tokens(monthTokens) + ' tok \\u00b7 ';
            const hit = document.createElement('span');
            hit.className = hitRate != null && hitRate < 60 ? 'hit is-bad' : 'hit';
            hit.textContent = hitRate == null ? 'cache --' : 'cache ' + String(hitRate).replace(/\\.0$/, '') + '%';
            used.append(hit);
            const today = document.createElement('span');
            today.textContent = 'today ' + money(todayCost, 'CNY');
            meta.append(used, today);
            row.append(main, bar, meta);
            list.append(row);
          }
          set(ds, 'ds-sum', 'This month ' + money(44.40, 'CNY') + ' \\u00b7 897M tok');
          set(ds, 'ds-sum-today', 'today ' + money(6.85, 'CNY'));
          show(ds, 'ds-summary', true);
        }

        // ---- settings ----
        const settings = document.querySelector('.win-settings');
        if (settings) {
          settings.querySelector('input[name="theme"][value="system"]').checked = true;
          settings.querySelector('input[name="interval"][value="5"]').checked = true;
          settings.querySelector('#autostart').checked = true;
          set(settings, 'kimi-version', '0.39.1 \\u2192 0.40.0');
          set(settings, 'kimi-credential', 'OAuth token \\u00b7 expires 17:20');
          set(settings, 'ds-api-key', 'Configured');
          set(settings, 'ds-user-token', 'Configured');
        }

        // ---- skills ----
        const skills = document.querySelector('.win-skills');
        if (skills) {
          const items = [
            ['code-review', 'Review code changes for security, performance and correctness.'],
            ['check-kimi-code-docs', 'Answer Kimi Code product questions from the official documentation.'],
            ['skill-creator', 'Create new skills and measure their performance.'],
            ['update-config', 'Inspect or edit config.toml and tui.toml.'],
            ['write-goal', 'Turn a rough intention into a completion contract for goal mode.'],
          ];
          set(skills, 'summary-text', items.length + ' skills \\u00b7 from ~/.kimi-code/skills');
          const list = skills.querySelector('#list');
          list.replaceChildren();
          const group = document.createElement('div');
          group.className = 'group-title';
          group.textContent = 'USER';
          list.append(group);
          for (const [name, description] of items) {
            const item = document.createElement('div');
            item.className = 'skill';
            item.title = description;
            const head = document.createElement('div');
            head.className = 'skill-head';
            const label = document.createElement('span');
            label.className = 'skill-name';
            label.textContent = name;
            head.append(label);
            const desc = document.createElement('div');
            desc.className = 'skill-desc';
            desc.textContent = description;
            item.append(head, desc);
            list.append(item);
          }
        }

        // Machine-readable card/window heights for demo/heights.py: the review
        // page is built from the shipped CSS, so its heights are the product's.
        // `window` adds the 56px shadow room the app reports to its backend.
        // A <script type="application/json"> keeps the page's layout untouched.
        const heights = {};
        for (const [key, sel] of [
          ['main-kimi', '.win-kimi'],
          ['main-deepseek', '.win-deepseek'],
          ['settings', '.win-settings'],
          ['skills', '.win-skills'],
          ['menu', '.win-menu'],
        ]) {
          const card = document.querySelector(sel + ' .root');
          if (!card) continue;
          const h = Math.round(card.getBoundingClientRect().height);
          heights[key] = { card: h, window: h + 56 };
        }
        const readout = document.createElement('script');
        readout.type = 'application/json';
        readout.id = 'm-product';
        readout.textContent = JSON.stringify(heights);
        document.body.append(readout);
      })();
    </script>
    """

    def window(title: str, note: str, cls: str, markup: str, width: int) -> str:
        return f"""
      <section class="card">
        <h2>{title}</h2>
        <p>{note}</p>
        <div class="win {cls}" style="width: {width}px">{markup}</div>
      </section>"""

    tray = tray_icon()
    return f"""<!doctype html>
<html lang="en" data-theme="light">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Quota Tray - every window</title>
  <!--
    ALL WINDOWS ON ONE PAGE. Generated by `demo/build_ui_demo.py` from the shipped
    markup and stylesheets, so this page cannot drift from the product: the CSS
    below is theme.css / main.css / settings.css / skills.css / menu.css with each
    window's rules scoped to its wrapper, and the markup is the real body of each
    HTML file. Values are a representative snapshot, not a live fetch.

    ?theme=dark switches the tokens.
    Heights are content-driven in the app (panel_height / settings_height); here
    each card simply grows with its content, and the skills window keeps its fixed
    404x520 because that one is fixed in tauri.conf.json.
  -->
  <style>
    /* ---------- demo stage (not part of the product) ---------- */
    .top {{ max-width: 1420px; margin: 0 auto 26px; }}
    .top h1 {{ font-size: 18px; font-weight: 600; margin: 0 0 6px; color: var(--ink-1); }}
    .top p {{ font-size: 12px; line-height: 1.6; color: var(--ink-2); margin: 0; max-width: 760px; }}
    .theme-switch {{ display: flex; gap: 6px; margin-top: 14px; }}
    .theme-switch button {{
      font-family: inherit; font-size: 11.5px; font-weight: 500;
      padding: 5px 12px; border-radius: 7px; cursor: pointer;
      border: 1px solid var(--hair); background: none; color: var(--ink-1);
    }}
    .theme-switch button[aria-pressed='true'] {{ background: var(--seg-track); border-color: var(--seg-ring); }}
    .stack {{ display: flex; flex-wrap: wrap; gap: 34px 30px; align-items: flex-start; max-width: 1420px; margin: 0 auto; }}
    .card {{ margin: 0; }}
    .card h2 {{ font-size: 12.5px; font-weight: 600; margin: 0 0 4px; color: var(--ink-1); }}
    .card p {{ font-size: 11px; color: var(--ink-2); margin: 0 0 8px; max-width: 424px; line-height: 1.5; }}
    .win {{ position: relative; }}
    /* The app's windows are 424/404/188 wide including 28px shadow room per side;
       the card inside is 368/348/132. Same here. */

    /* ---------- theme.css (shared tokens + primitives) ---------- */
    {theme}

    /* ---------- main.css, scoped to the panel ---------- */
    {prefix_selectors(main_css, '.win-main')}

    /* ---------- settings.css ---------- */
    {prefix_selectors(settings_css, '.win-settings')}

    /* ---------- skills.css ---------- */
    {prefix_selectors(skills_css, '.win-skills')}

    /* ---------- menu.css ---------- */
    {prefix_selectors(menu_css, '.win-menu')}

    /* ---------- demo-only label for the sign-in window ---------- */
    .tray {{ width: 424px; }}
    .tray-bar {{
      display: flex; align-items: center; gap: 12px; height: 40px;
      padding: 0 14px; border-radius: 8px; margin-bottom: 8px;
    }}
    .tray-bar.dark {{ background: #202020; }}
    .tray-bar.light {{ background: var(--light, #f3f3f3); border: 1px solid var(--hair); }}
    .tray-bar .sys {{ font-size: 11px; color: #ffffffb0; }}
    .tray-bar .clock {{ margin-left: auto; font-size: 11.5px; color: #ffffffb0; }}
    .tray-bar.light .sys, .tray-bar.light .clock {{ color: #00000090; }}
    .tray-tip {{
      display: grid; gap: 1px; width: max-content;
      padding: 7px 10px; border-radius: 7px; border: 1px solid var(--hair);
      background: var(--paper); font-size: 11px; color: var(--ink-1);
      box-shadow: var(--shadow);
    }}
    .tray-tip span:first-child {{ font-weight: 500; }}
    .tray-tip span:not(:first-child) {{ color: var(--ink-2); }}

    .signin {{ width: 520px; }}
    .signin .shell {{
      border: 1px solid var(--hair); border-radius: 10px; background: var(--paper);
      box-shadow: var(--shadow); overflow: hidden;
    }}
    .signin .titlebar {{
      display: flex; align-items: center; justify-content: space-between;
      gap: 12px; height: 32px; padding: 0 10px 0 12px;
      background: var(--seg-track); border-bottom: 1px solid var(--hair);
      font-size: 11.5px; color: var(--ink-1);
    }}
    .signin .titlebar span.win-title {{ font-weight: 500; }}
    .signin .titlebar .sys {{ color: var(--ink-2); letter-spacing: 2px; }}
    .signin .body {{
      height: 150px; display: flex; align-items: center; justify-content: center;
      background: repeating-linear-gradient(135deg, var(--hair-soft) 0 8px, transparent 8px 16px);
      font-size: 11px; color: var(--ink-2);
    }}
    .signin .states {{ margin-top: 10px; display: grid; gap: 6px; }}
    .signin .state {{ font-size: 10.5px; color: var(--ink-2); }}
    .signin .state code {{ font-family: inherit; color: var(--ink-1); }}

    /* ---------- stage overrides, last on purpose ----------
       theme.css resets html/body (padding 0, transparent, overflow hidden) and
       skills.css gives the (already scoped) .root a viewport height. The stage
       rules tie on specificity with those and win only by coming last, so they
       must stay at the end of the block: .win-skills .root collides with the
       scoped prefix_selectors() copy, and the later declaration takes it. */
    html, body {{
      padding: 0;
      background: #e7e7e4;
      overflow: auto;
    }}
    :root[data-theme='dark'] body {{ background: #0f0f12; }}
    body {{
      margin: 0;
      padding: 30px 26px 60px;
      user-select: text;
      -webkit-user-select: text;
    }}
    .win-skills .root {{ height: 464px; }}
  </style>
</head>
<body>
  <header class="top">
    <h1>Quota Tray - every window</h1>
    <p>
      Generated from the shipped markup and stylesheets by
      <code>demo/build_ui_demo.py</code>. Both panel tabs, the settings sheet, the
      skills window, the tray menu and the sign-in window's title bar, at real
      size. Values are a snapshot ({money_note}); the windows are otherwise the
      product's own DOM.
    </p>
    <div class="theme-switch">
      <button id="t-light" aria-pressed="true">Light</button>
      <button id="t-dark" aria-pressed="false">Dark</button>
    </div>
  </header>

  <main class="stack">
    {window('Main panel - Kimi', 'Provider tab on Kimi: quota readout, the local cache/decode block, Extra usage and the CLI row.', 'win-main win-kimi', panel_kimi, 424)}
    {window('Main panel - DeepSeek', 'Provider tab on DeepSeek: balance, four model rows (token-share bars) and the month total.', 'win-main win-deepseek', panel_ds, 424)}
    {window('Settings', 'General preferences, the read-only Kimi section and the DeepSeek credentials block with its manual fallback.', 'win-settings', settings, 404)}
    {window('Skills', 'Kimi skills, read-only and lazy-loaded. Fixed 404x520 window.', 'win-skills', skills, 404)}
    {window('Tray menu', 'Right click on the tray icon; 188px wide, height follows the five rows.', 'win-menu', menu, 188)}
    <section class="card">
      <h2>Tray</h2>
      <p>Notification area at 40px, icon at 16px. The tray embeds the 48px cut so
        every DPI scale downsamples instead of stretching (PLAN §9).</p>
      <div class="tray">
        <div class="tray-bar dark"><span class="sys">^</span><span class="sys">v</span>
          <span class="sys">|</span><img src="{tray}" width="16" height="16" alt="" />
          <span class="clock">16:42</span></div>
        <div class="tray-bar light"><span class="sys">^</span><span class="sys">v</span>
          <span class="sys">|</span><img src="{tray}" width="16" height="16" alt="" />
          <span class="clock">16:42</span></div>
        <div class="tray-tip"><span>Quota Tray</span><span>Kimi 5h 31% · week 12%</span><span>DeepSeek ¥45.44</span></div>
      </div>
    </section>

    <section class="card">
      <h2>Sign-in window</h2>
      <p>
        Temporary window: the console page inside is DeepSeek's, so the app adds
        only the native title bar - and the title is where the status lives
        (PLAN §4). The console body is not drawn here.
      </p>
      <div class="signin">
        <div class="shell">
          <div class="titlebar">
            <span class="win-title">DeepSeek sign-in - verifying\u2026</span>
            <span class="sys">- &#9633; &#10005;</span>
          </div>
          <div class="body">platform.deepseek.com (third-party content)</div>
        </div>
        <div class="states">
          <div class="state">while verifying: <code>DeepSeek sign-in - verifying\u2026</code></div>
          <div class="state">after saving: <code>DeepSeek sign-in - verified, saved</code></div>
          <div class="state">rejected: <code>DeepSeek sign-in - token not accepted - keep signing in</code></div>
        </div>
      </div>
    </section>
  </main>

  {paint}
</body>
</html>
"""


money_note = "16:42 on 2026-11-14"


def main() -> None:
    out = ROOT / "demo" / "ui-demo.html"
    html = build().replace("{money_note}", money_note)
    # The template is a format string: an un-doubled brace in a comment gets
    # interpolated (a `<function ... at 0x...>` repr once shipped this way) and
    # makes the output differ on every run.
    if "<function" in html or " at 0x" in html:
        raise SystemExit("ui-demo.html contains a Python repr - escape the braces in the template")
    out.write_text(html, encoding="utf-8", newline="\n")
    print("wrote", out, out.stat().st_size, "bytes")


if __name__ == "__main__":
    main()
