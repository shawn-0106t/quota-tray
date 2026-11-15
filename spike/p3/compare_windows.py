#!/usr/bin/env python3
"""Compare a P3 window snapshot with the shipped DOM, in CSS pixels.

Same method as `spike/p2/compare_bands.py`: render the real markup + CSS in a
headless Chromium at DPR 2, convert both sides' text bands to CSS px (the Slint
window runs at the live 225% scale), and diff the band tops. Pixel-diff numbers
are meaningless across DPRs and are not reported.

The reference page is the one `spike/p3/ref_windows.py` builds, so the numbers
this script checks are the same numbers the Slint layout constants came from —
but the check is independent: it reads the two rendered images, not the JSON.

Usage:
  slint/target/debug/quota-tray-slint.exe --shots --theme light --out .shots/p3/light
  PYTHONUTF8=1 python spike/p3/compare_windows.py --shot .shots/p3/light/settings.png \
      --window settings --theme light

Windows: settings (404), skills (404), menu (188). States: settings default /
manual / env; the other two have one state.
"""
import argparse
import base64
import io
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SHOTS = ROOT / ".shots" / "p3"
REF_SCALE = 2

WINDOWS = {
    "settings": {"w": 404, "page": "ref-settings.html"},
    "skills": {"w": 404, "page": "ref-skills.html"},
    "menu": {"w": 188, "page": "ref-menu.html"},
}

ARGS = argparse.ArgumentParser()
ARGS.add_argument("--shot", required=True, help="Slint window snapshot (.png)")
ARGS.add_argument("--window", required=True, choices=sorted(WINDOWS))
ARGS.add_argument("--state", default="default", choices=["default", "manual", "env"])
ARGS.add_argument("--theme", default="light", choices=["light", "dark"])
ARGS.add_argument("--out", default=str(SHOTS))


def load(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB")).astype(np.int16)


def bands(
    image: np.ndarray, scale: float, win_w: float, win_h: float, min_height: float = 4
) -> list[tuple[float, float]]:
    """Rows carrying ink inside the card, as (top, height) in CSS px.

    The x window is the card's content area: it excludes the 28px shadow room
    (which is a gradient, not ink) and the right 44px, where the settings /
    skills scrollbars live — a 4px thumb spans the whole scroll range and would
    merge every band into one.
    """
    grey = (image @ np.array([0.299, 0.587, 0.114])).astype(np.int16)
    x0 = int(round(40 * scale))
    x1 = int(round((win_w - 80) * scale))
    grey = grey[:, x0:x1]
    runs: list[tuple[int, int, int]] = []  # (top, height, widest ink extent)
    start = None
    widest = 0
    for y in range(grey.shape[0]):
        row = grey[y]
        background = np.bincount(row).argmax()
        # 2, not 4: a one-character line (several skills describe themselves
        # with the bare ">" their folded YAML leaves behind) paints two or three
        # device pixels per row.
        ink = np.where(np.abs(row - background) > 12)[0]
        if len(ink) > 2:
            if start is None:
                start = y
                widest = 0
            widest = max(widest, int(ink.max() - ink.min() + 1))
        elif start is not None:
            runs.append((start, y - start, widest))
            start = None
    if start is not None:
        runs.append((start, grey.shape[0] - start, widest))
    merged: list[list[float]] = []
    for top, height, extent in runs:
        if merged and (top - (merged[-1][0] + merged[-1][1])) / scale < 2.0:
            merged[-1][1] = top + height - merged[-1][0]
            merged[-1][2] = max(merged[-1][2], extent)
        else:
            merged.append([top, height, extent])
    # Only the card's interior: the 28px shadow room above and below is a
    # gradient, and on a clamped sheet the last half-row the browser clips at
    # the card's bottom edge is not a layout difference.
    #
    # A run narrower than 5 CSS px is a 1px border or a lone ">" — its band top
    # is AA-threshold luck, not a layout position. Both sides drop them, so they
    # cannot desynchronise the comparison.
    card_top = 28.0
    card_bottom = win_h - 28.0
    return [
        (top / scale, height / scale)
        for top, height, extent in merged
        if height / scale >= min_height
        and extent / scale >= 5.0
        and card_top <= top / scale < card_bottom
    ]


def data_uri(path: Path) -> str:
    buffer = io.BytesIO()
    Image.open(path).convert("RGB").save(buffer, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")


def main() -> int:
    args = ARGS.parse_args()
    shot_path = Path(args.shot)
    if not shot_path.is_file():
        raise SystemExit(f"snapshot not found: {shot_path}")
    spec = WINDOWS[args.window]
    win_w = float(spec["w"])

    shot = Image.open(shot_path).convert("RGB")
    shot_scale = shot.width / win_w
    win_h = round(shot.height / shot_scale)
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    # The reference needs the same viewport height; the settings sheet also
    # needs the same card cap (its own 90% clamp) or the content lengths differ.
    query = f"theme={args.theme}"
    if args.state in ("manual", "env"):
        query += f"&{args.state}=1"
    if args.window == "settings":
        query += f"&cap={win_h - 56}"
    ref_raw = out_dir / f"ref-{args.window}-{args.state}-{args.theme}-raw.png"
    subprocess.run(
        [
            "bash",
            (ROOT / "demo" / "shot.sh").as_posix(),
            (HERE / spec["page"]).as_posix(),
            ref_raw.as_posix(),
            query,
            str(int(win_w)),
            str(win_h),
            str(REF_SCALE),
        ],
        check=True,
        cwd=ROOT,
    )
    ref = Image.open(ref_raw).convert("RGB")
    ref_path = out_dir / f"ref-{args.window}-{args.state}-{args.theme}.png"
    ref.save(ref_path)

    ref_bands = bands(load(ref_path), REF_SCALE, win_w, win_h)
    shot_bands = bands(load(shot_path), shot_scale, win_w, win_h)
    pairs = min(len(ref_bands), len(shot_bands))
    offsets = [shot_bands[i][0] - ref_bands[i][0] for i in range(pairs)]

    print(
        f"{args.window}/{args.state}/{args.theme}  slint DPR {shot_scale:.2f}  ref DPR {REF_SCALE}  "
        f"window {win_w:g}x{win_h} logical"
    )
    print(f"bands ref={len(ref_bands)} slint={len(shot_bands)} compared={pairs}")
    print()
    print(f"{'#':>2} {'ref y':>7} {'slint y':>8} {'dy(css)':>8}  {'height':>6}")
    worst = 0.0
    worst_text = 0.0
    for i in range(pairs):
        dy = offsets[i]
        worst = max(worst, abs(dy))
        # A band at least 8px tall is a text line; thinner ones are hairlines
        # and button borders, whose first antialiased row is a DPR artifact
        # (the one 1.56px outlier is the Save button's bottom border).
        if shot_bands[i][1] >= 8.0:
            worst_text = max(worst_text, abs(dy))
        print(
            f"{i:>2} {ref_bands[i][0]:>7.1f} {shot_bands[i][0]:>8.1f} {dy:>+8.2f}  {shot_bands[i][1]:>6.1f}"
        )
    if len(ref_bands) != len(shot_bands):
        print("\nband count mismatch:")
        print("  ref  ", [round(b[0], 1) for b in ref_bands])
        print("  slint", [round(b[0], 1) for b in shot_bands])
    verdict = (
        "PASS"
        if worst_text <= 1.0 and worst <= 1.75 and len(ref_bands) == len(shot_bands)
        else "FAIL"
    )
    print(
        f"\nworst band offset {worst:.2f} css px (text bands {worst_text:.2f})  verdict: {verdict}"
    )

    overlay = Image.blend(ref.resize(shot.size, Image.LANCZOS), shot, 0.5)
    overlay_path = out_dir / f"overlay-{args.window}-{args.state}-{args.theme}.png"
    overlay.save(overlay_path)
    html = out_dir / f"compare-{args.window}-{args.state}-{args.theme}.html"
    html.write_text(
        f"""<!doctype html><meta charset="utf-8"><title>{args.window}</title>
<style>body{{font:13px 'Segoe UI',sans-serif;margin:16px}}
figure{{display:inline-block;margin:0 12px 12px 0}}img{{display:block;height:520px}}</style>
<h3>{args.window} / {args.state} / {args.theme} — HTML (DPR {REF_SCALE}) · Slint (DPR {shot_scale:g}) · 50% overlay</h3>
<figure><img src="{data_uri(ref_path)}"><figcaption>HTML</figcaption></figure>
<figure><img src="{data_uri(shot_path)}"><figcaption>Slint</figcaption></figure>
<figure><img src="{data_uri(overlay_path)}"><figcaption>overlay (ref scaled to the shot)</figcaption></figure>
""",
        encoding="utf-8",
    )
    print(f"written: {html.relative_to(ROOT)}")
    return 0 if verdict == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
