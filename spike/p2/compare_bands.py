#!/usr/bin/env python3
"""Compare a Slint panel snapshot with the shipped DOM, in CSS pixels.

The P2 panel is no longer absolutely positioned: blocks stack from `Metrics`
constants, the card height is data-derived and the card's interior can scroll.
This script is the layout check that the rewrite kept the pixel-verified
geometry: it renders `index.html` (real markup + CSS, same variant data) with a
headless Chromium and compares the row bands (text / hairlines) with the Slint
snapshot's own bands.

The two sides are rendered at DIFFERENT device pixel ratios on purpose:

- the HTML reference at DPR 2, the DPR `spike/slint-demo/ref/rects-*.json` was
  measured at (Chrome snaps `line-height: normal` to the device grid, so its
  layout is only comparable at the DPR it was verified at — at DPR 2.25 the same
  card measures 501px instead of 504px),
- the Slint panel at the live display scale (225% on this machine), because that
  is the only scale its window can render at.

Both band lists are therefore converted to CSS px (device px / DPR) and compared
in that space. Pixel-diff numbers are not meaningful across DPRs and are not
reported.

Usage:
  slint/target/debug/quota-tray-slint.exe --hold --variant kimi-full --theme light \
      --out .shots/p2/kimi-full
  PYTHONUTF8=1 python spike/p2/compare_bands.py --shot .shots/p2/kimi-full/panel.png \
      --variant kimi-full --theme light
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
PAGE = HERE / "ref-variants.html"
SHOTS = ROOT / ".shots" / "p2"
REF_SCALE = 2

ARGS = argparse.ArgumentParser()
ARGS.add_argument("--shot", required=True, help="Slint panel snapshot (.png)")
ARGS.add_argument("--variant", default="kimi-full")
ARGS.add_argument("--theme", default="light")
ARGS.add_argument("--out", default=str(SHOTS))


def load(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB")).astype(np.int16)


def bands(image: np.ndarray, scale: float, min_height: int = 5) -> list[tuple[float, float]]:
    """Rows carrying ink inside the card, as (top, height) in CSS px.

    Only the card interior is scanned: the stage and the shadow gradient differ
    from the paper by more than the ink threshold.
    """
    grey = (image @ np.array([0.299, 0.587, 0.114])).astype(np.int16)
    # 28px shadow room + the 12px corner radius: the rounded corners let the
    # stage/shadow show inside a row and would register as a phantom band at the
    # card's top edge.
    margin = int(round(40 * scale))
    grey = grey[:, margin:grey.shape[1] - margin]
    runs: list[tuple[int, int]] = []
    start = None
    for y in range(grey.shape[0]):
        row = grey[y]
        background = np.bincount(row).argmax()
        ink = int((np.abs(row - background) > 12).sum())
        if ink > 4:
            if start is None:
                start = y
        elif start is not None:
            runs.append((start, y - start))
            start = None
    if start is not None:
        runs.append((start, grey.shape[0] - start))
    # Merge runs separated by less than 2 CSS px: whether a 3px gap (the footer's
    # icon/label gap) resolves as one band or two depends on the DPR, not on the
    # layout. Merging keeps the comparison about positions.
    merged: list[list[float]] = []
    for top, height in runs:
        if merged and (top - (merged[-1][0] + merged[-1][1])) / scale < 2.0:
            merged[-1][1] = top + height - merged[-1][0]
        else:
            merged.append([top, height])
    return [
        (top / scale, height / scale)
        for top, height in merged
        if height / scale >= min_height
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
    shot = Image.open(shot_path).convert("RGB")
    shot_scale = shot.width / 424.0
    win_h = round(shot.height / shot_scale)
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    ref_raw = out_dir / f"ref-{args.variant}-raw.png"
    subprocess.run(
        [
            "bash",
            (ROOT / "demo" / "shot.sh").as_posix(),
            PAGE.as_posix(),
            ref_raw.as_posix(),
            f"variant={args.variant}&theme={args.theme}&h={win_h}",
            "424",
            str(win_h),
            str(REF_SCALE),
        ],
        check=True,
        cwd=ROOT,
    )
    ref = Image.open(ref_raw).convert("RGB")
    ref = ref.crop((0, 0, int(round(424 * REF_SCALE)), int(round(win_h * REF_SCALE))))
    shot = shot.crop((0, 0, int(round(424 * shot_scale)), int(round(win_h * shot_scale))))
    ref_path = out_dir / f"ref-{args.variant}-{args.theme}.png"
    ref.save(ref_path)

    ref_bands = bands(load(ref_path), REF_SCALE)
    shot_bands = bands(load(shot_path), shot_scale)
    pairs = min(len(ref_bands), len(shot_bands))
    offsets = [shot_bands[i][0] - ref_bands[i][0] for i in range(pairs)]

    print(
        f"variant {args.variant}  slint DPR {shot_scale:.2f}  ref DPR {REF_SCALE}  "
        f"window 424x{win_h} logical"
    )
    print(f"bands ref={len(ref_bands)} slint={len(shot_bands)} compared={pairs}")
    print()
    print(f"{'#':>2} {'ref y':>7} {'slint y':>8} {'dy(css)':>8}")
    worst = 0.0
    for i in range(pairs):
        dy = offsets[i]
        worst = max(worst, abs(dy))
        print(f"{i:>2} {ref_bands[i][0]:>7.1f} {shot_bands[i][0]:>8.1f} {dy:>+8.2f}")
    if len(ref_bands) != len(shot_bands):
        print(f"\nband count mismatch:\n  ref   {ref_bands}\n  slint {shot_bands}")
    verdict = "PASS" if worst <= 1.0 and len(ref_bands) == len(shot_bands) else "FAIL"
    print(f"\nworst band offset {worst:.2f} css px  verdict: {verdict}")

    overlay = Image.blend(
        ref.resize(shot.size, Image.LANCZOS), shot, 0.5
    )
    overlay_path = out_dir / f"overlay-{args.variant}-{args.theme}.png"
    overlay.save(overlay_path)
    html = out_dir / f"compare-{args.variant}-{args.theme}.html"
    html.write_text(
        f"""<!doctype html><meta charset="utf-8"><title>{args.variant}</title>
<style>body{{font:13px 'Segoe UI',sans-serif;margin:16px}}
figure{{display:inline-block;margin:0 12px 12px 0}}img{{display:block;height:420px}}</style>
<h3>{args.variant} / {args.theme} — HTML (DPR {REF_SCALE}) · Slint (DPR {shot_scale:g}) · 50% overlay</h3>
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
