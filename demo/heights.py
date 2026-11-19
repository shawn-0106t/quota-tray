#!/usr/bin/env python3
"""heights.py -- measure card/window heights with a headless Chromium.

Usage: heights.py            # the shipped windows, from demo/ui-demo.html
       heights.py --demos    # the frozen A-E demos, every tab x state (history)

The numbers come from the browser's layout, not from a hand calculation: the
generated review page carries a JSON readout (`#m-product`) of every window's card
height, and the frozen demos carry `#m-card` / `#m-win` after JS renders.
"""
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

CANDIDATES = [
    Path(os.environ.get("LOCALAPPDATA", "")) / "Google/Chrome/Application/chrome.exe",
    Path("C:/Program Files/Google/Chrome/Application/chrome.exe"),
    Path("C:/Program Files (x86)/Google/Chrome/Application/chrome.exe"),
    Path("C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"),
    Path("C:/Program Files/Microsoft/Edge/Application/msedge.exe"),
]

STATES = ["normal", "four", "hot", "err", "nocred"]
TABS = ["kimi", "ds"]


def find_browser() -> str:
    for c in CANDIDATES:
        if c.is_file():
            return str(c)
    print("no Chromium browser found", file=sys.stderr)
    raise SystemExit(1)


def dump_dom(browser: str, page: Path, query: str, size: str | None = None) -> str:
    with tempfile.TemporaryDirectory() as profile_dir:
        args = [
            browser, "--headless=new", "--disable-gpu", "--no-first-run",
            "--virtual-time-budget=3000", f"--user-data-dir={profile_dir}",
            "--dump-dom", f"file:///{page.resolve().as_posix()}?{query}",
        ]
        # A viewport is only pinned where `100vh` matters (the settings sheet's
        # `max-height`): at headless's default 800x600 the card reports its cap
        # instead of its content, which is exactly the number this table wants.
        if size:
            args.insert(5, f"--window-size={size}")
        return subprocess.run(
            args,
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=90,
        ).stdout


def measure(browser: str, page: Path, query: str) -> tuple[int, int]:
    out = dump_dom(browser, page, f"bare=1&{query}")
    card = re.search(r'id="m-card"[^>]*>(\d+)px<', out)
    win = re.search(r'id="m-win"[^>]*>(\d+)px<', out)
    if not card or not win:
        raise RuntimeError(f"readout missing for {query}")
    return int(card.group(1)), int(win.group(1))


PRODUCT_ROWS = ("main-kimi", "main-deepseek", "settings", "skills", "menu")
# The same stage size demo/make_readme_shots.py renders at; the settings card is
# `100vh`-capped, so a smaller viewport would report the cap, not the content.
PRODUCT_VIEWPORT = "1480,1400"


def product(browser: str, base: Path) -> int:
    """The shipped windows, read off demo/ui-demo.html (generated from the CSS)."""
    out = dump_dom(browser, base / "ui-demo.html", "theme=dark", size=PRODUCT_VIEWPORT)
    match = re.search(r'id="m-product"[^>]*>(.*?)</script>', out, re.S)
    if not match:
        print("readout missing - run demo/build_ui_demo.py first", file=sys.stderr)
        return 1
    heights = json.loads(match.group(1))
    print(f"{'window':<16} {'card':>6} {'window':>7}")
    print("-" * 31)
    for name in PRODUCT_ROWS:
        row = heights.get(name)
        if row is None:
            print(f"{name:<16} {'-':>6} {'-':>7}")
            continue
        print(f"{name:<16} {row['card']:>5}px {row['window']:>6}px")
    return 0


def demos(browser: str, base: Path) -> int:
    print(f"{'demo':<4} {'tab':<5} {'state':<8} {'card':>6} {'window':>7}")
    print("-" * 34)
    for name in (
        "panel-demo-a.html",
        "panel-demo-b.html",
        "panel-demo-c.html",
        "panel-demo-d.html",
        "panel-demo-e.html",
    ):
        page = base / name
        demo = name.split("-")[-1].split(".")[0].upper()
        for tab in TABS:
            for state in STATES:
                card, win = measure(browser, page, f"theme=dark&tab={tab}&state={state}")
                print(f"{demo:<4} {tab:<5} {state:<8} {card:>5}px {win:>6}px")
    return 0


def main() -> int:
    browser = find_browser()
    base = Path(__file__).resolve().parent
    if "--demos" in sys.argv:
        return demos(browser, base)
    return product(browser, base)


if __name__ == "__main__":
    raise SystemExit(main())
