#!/usr/bin/env python3
"""heights.py -- measure every demo state with a headless Chromium and print a table.

Usage: heights.py
Reads the demo's own #m-card / #m-win readout after JS renders (--dump-dom), so the
numbers come from the browser's layout, not from a hand calculation.
"""
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


def measure(browser: str, page: Path, query: str) -> tuple[int, int]:
    with tempfile.TemporaryDirectory() as profile:
        url = f"file:///{page.resolve().as_posix()}?bare=1&{query}"
        out = subprocess.run(
            [
                browser, "--headless=new", "--disable-gpu", "--no-first-run",
                "--virtual-time-budget=3000", f"--user-data-dir={profile}",
                "--dump-dom", url,
            ],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=90,
        ).stdout
    card = re.search(r'id="m-card"[^>]*>(\d+)px<', out)
    win = re.search(r'id="m-win"[^>]*>(\d+)px<', out)
    if not card or not win:
        raise RuntimeError(f"readout missing for {query}")
    return int(card.group(1)), int(win.group(1))


def main() -> int:
    browser = find_browser()
    base = Path(__file__).resolve().parent
    print(f"{'demo':<4} {'tab':<5} {'state':<8} {'card':>6} {'window':>7}")
    print("-" * 34)
    for name in ("panel-demo-a.html", "panel-demo-b.html", "panel-demo-c.html", "panel-demo-d.html"):
        page = base / name
        demo = name.split("-")[-1].split(".")[0].upper()
        for tab in TABS:
            for state in STATES:
                card, win = measure(browser, page, f"theme=dark&tab={tab}&state={state}")
                print(f"{demo:<4} {tab:<5} {state:<8} {card:>5}px {win:>6}px")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
