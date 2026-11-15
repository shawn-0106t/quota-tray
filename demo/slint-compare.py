#!/usr/bin/env python3
"""Compare the Slint spike's snapshots with the HTML reference.

`spike/slint-demo/ref/build_ref.py` renders the shipped DOM (same markup, CSS and
sample data) at scale 2; `slint-demo` snapshots its own window through one of its
renderers. This script pairs each renderer with the reference, writes a
side-by-side + 50 % overlay + amplified diff, and prints:

- differing-pixel share and mean channel delta,
- the vertical position of every text band (the baseline-drift proxy),
- the ink width of a few known strings (shaping, not rasterization: both
  renderers report the same widths).

Usage:
  PYTHONUTF8=1 python demo/slint-compare.py
      -> .shots/slint-demo/compare.html, images embedded (opens offline)
  PYTHONUTF8=1 python demo/slint-compare.py --link --out spike/evidence/compare.html
      -> a small page next to the copies of the shots, for the repository
"""
import base64
import io
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SHOTS = ROOT / ".shots" / "slint-demo"
RECTS = ROOT / "spike" / "slint-demo" / "ref"
CASES = [("light", "kimi"), ("light", "ds"), ("dark", "kimi"), ("dark", "ds")]
RENDERERS = {"femtovg": "slint-{theme}-{tab}.png", "skia": "skia-{theme}-{tab}.png"}
# Elements the ink-width probe reads, by key in rects-<theme>-<tab>.json.
PROBES = [("brand", "span.brand"), ("model4", "span.mname#4"), ("money", "span.mcost")]

ARGS = sys.argv[1:]
LINK = "--link" in ARGS
OUT = Path(ARGS[ARGS.index("--out") + 1]) if "--out" in ARGS else SHOTS / "compare.html"


def load(path: Path) -> np.ndarray:
    return np.asarray(Image.open(path).convert("RGB")).astype(np.int16)


def diff_stats(ref: np.ndarray, cur: np.ndarray) -> tuple[float, float, np.ndarray]:
    delta = np.abs(ref - cur).max(axis=2)
    return float((delta > 8).mean() * 100), float(delta.mean()), delta


def bands(image: np.ndarray, min_height: int = 5) -> list[tuple[int, int]]:
    """Rows that carry content, as (top, height) in device pixels.

    Only the card interior is scanned: the stage (and the shadow gradient over
    it) differs from the paper by more than the ink threshold and would make
    every row a band. 28 px of shadow room at scale 2.
    """
    grey = (image @ np.array([0.299, 0.587, 0.114])).astype(np.int16)
    margin = 28 * (grey.shape[1] // 424)
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
    return [(top, height) for top, height in runs if height >= min_height]


def ink_width(image: np.ndarray, rect: list[float]) -> int:
    """Width of the ink inside a CSS-px rect taken from the reference dump."""
    scale = image.shape[1] // 424
    x, y, w, h = (int(round(v * scale)) for v in rect)
    grey = (image @ np.array([0.299, 0.587, 0.114])).astype(np.int16)
    window = grey[max(0, y - 2):y + h + 2, max(0, x - 2):x + w + 12]
    frame = np.concatenate([window[0], window[-1], window[:, 0], window[:, -1]])
    background = np.bincount(frame).argmax()
    cols = np.where((np.abs(window - background) > 20).any(axis=0))[0]
    return int(cols[-1] - cols[0] + 1) if len(cols) else 0


def data_uri(path: Path) -> str:
    buffer = io.BytesIO()
    Image.open(path).convert("RGB").save(buffer, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")


def src(path: Path) -> str:
    """Page source: the file's own name next to the page, or embedded bytes."""
    return path.name if LINK else data_uri(path)


def main() -> int:
    rows = []
    cards = []
    for theme, tab in CASES:
        reference = SHOTS / f"ref-{theme}-{tab}.png"
        if not reference.is_file():
            print(f"missing reference {reference.name} - run spike/slint-demo/ref/build_ref.py")
            return 1
        ref = load(reference)
        rects = json.loads((RECTS / f"rects-{theme}-{tab}.json").read_text(encoding="utf-8"))
        ref_probe = {name: ink_width(ref, rects[key]) for name, key in PROBES if key in rects}

        card = {"case": f"{theme}/{tab}", "ref": reference, "renderers": []}
        for renderer, pattern in RENDERERS.items():
            path = SHOTS / pattern.format(theme=theme, tab=tab)
            if not path.is_file():
                print(f"missing {renderer} shot {path.name} - render it first")
                return 1
            ref_image = Image.open(reference).convert("RGB")
            cur_image = Image.open(path).convert("RGB")
            if cur_image.size != ref_image.size:
                print(f"{theme}/{tab} {renderer}: size {cur_image.size} != {ref_image.size}")
                return 1
            cur = load(path)
            share, mean, delta = diff_stats(ref, cur)
            ref_bands, cur_bands = bands(ref), bands(cur)
            pairs = min(len(ref_bands), len(cur_bands))
            offsets = [cur_bands[i][0] - ref_bands[i][0] for i in range(pairs)]
            probe = {name: ink_width(cur, rects[key]) for name, key in PROBES if key in rects}

            overlay_path = SHOTS / f"compare-{theme}-{tab}-{renderer}-overlay.png"
            diff_path = SHOTS / f"compare-{theme}-{tab}-{renderer}-diff.png"
            Image.blend(ref_image, cur_image, 0.5).save(overlay_path)
            Image.fromarray(np.clip(delta * 8, 0, 255).astype(np.uint8)).convert("RGB").save(diff_path)

            rows.append(
                {
                    "case": f"{theme}/{tab}",
                    "renderer": renderer,
                    "share": share,
                    "mean": mean,
                    "bands": f"{len(ref_bands)}/{len(cur_bands)}",
                    "maxdy": max((abs(v) for v in offsets), default=0),
                    "dy": ", ".join(f"{v:+d}" for v in offsets),
                    "probe": "  ".join(f"{name} {probe[name] - ref_probe[name]:+d}" for name in ref_probe),
                }
            )
            card["renderers"].append(
                {
                    "name": renderer,
                    "shot": path,
                    "overlay": overlay_path,
                    "diff": diff_path,
                    "probe": [(name, ref_probe[name], probe[name]) for name in ref_probe],
                }
            )
        cards.append(card)

    print(f"{'case':<12}{'renderer':<10}{'diff>8':>8}{'meanD':>7}{'bands':>8}{'max|dy|':>8}  probes (device px vs ref)")
    for row in rows:
        print(
            f"{row['case']:<12}{row['renderer']:<10}{row['share']:>7.2f}%{row['mean']:>7.2f}"
            f"{row['bands']:>8}{row['maxdy']:>8}  {row['probe']}"
        )

    probe_names = [name for name, _ in PROBES]
    html = [
        "<!doctype html><meta charset='utf-8'><title>Slint spike - HTML vs femtovg vs Skia</title>",
        "<style>body{font:13px/1.5 'Segoe UI',sans-serif;margin:24px;background:#0f0f12;color:#e9e9e6}",
        "h1{font-size:18px}h2{font-size:14px;margin:28px 0 8px}",
        "table{border-collapse:collapse;font-variant-numeric:tabular-nums}",
        "td,th{border:1px solid #3a3a37;padding:2px 8px;text-align:left}",
        ".row{display:flex;gap:16px;flex-wrap:wrap}",
        "figure{margin:0}figcaption{color:#a9a9a4;font-size:11px;margin-bottom:4px}",
        "img{width:424px;display:block;border:1px solid #3a3a37}",
        "</style>",
        "<h1>Slint spike - HTML reference vs femtovg vs Skia</h1>",
        "<p>Left: the shipped DOM (index.html + theme.css + main.css, painted by "
        "spike/slint-demo/ref/build_ref.py). Middle and right: Slint 1.18 with the same "
        "absolute coordinates, one image per renderer. All at scale 2, shown at 1 CSS px = 1 px.</p>",
        "<table><tr><th>case</th><th>renderer</th><th>pixels differing &gt;8</th><th>mean |d|</th>"
        "<th>bands ref/slint</th><th>max |dy|</th><th>ink-width delta</th></tr>",
    ]
    for row in rows:
        html.append(
            f"<tr><td>{row['case']}</td><td>{row['renderer']}</td><td>{row['share']:.2f}%</td>"
            f"<td>{row['mean']:.2f}</td><td>{row['bands']}</td><td>{row['maxdy']}</td>"
            f"<td>{row['probe']}</td></tr>"
        )
    html.append("</table>")

    for card in cards:
        html.append(f"<h2>{card['case']}</h2><div class='row'>")
        html.append(f"<figure><figcaption>HTML</figcaption><img src='{src(card['ref'])}' alt='ref'></figure>")
        for renderer in card["renderers"]:
            html.append(
                f"<figure><figcaption>{renderer['name']}</figcaption>"
                f"<img src='{src(renderer['shot'])}' alt='{renderer['name']}'></figure>"
            )
        if not LINK:
            for renderer in card["renderers"]:
                html.append(
                    f"<figure><figcaption>{renderer['name']} &middot; 50% over HTML</figcaption>"
                    f"<img src='{src(renderer['overlay'])}' alt='overlay'></figure>"
                )
            html.append(
                f"<figure><figcaption>|d| x8 &middot; {card['renderers'][-1]['name']}</figcaption>"
                f"<img src='{src(card['renderers'][-1]['diff'])}' alt='diff'></figure>"
            )
        html.append("</div>")
        html.append("<table><tr><th>string</th><th>ref ink (device px)</th><th>ink per renderer</th></tr>")
        names = [k for k, _, _ in card["renderers"][0]["probe"]]
        for name in names:
            ref_value = next(v for k, v, _ in card["renderers"][0]["probe"] if k == name)
            values = " / ".join(
                f"{r['name']} {next(v for k, _, v in r['probe'] if k == name)}" for r in card["renderers"]
            )
            html.append(f"<tr><td>{name}</td><td>{ref_value}</td><td>{values}</td></tr>")
        html.append("</table>")

    settings = [("femtovg", SHOTS / "slint-light-settings.png"), ("skia", SHOTS / "skia-light-settings.png")]
    if any(path.is_file() for _, path in settings):
        html.append("<h2>settings sheet (qualitative)</h2><div class='row'>")
        for name, path in settings:
            if path.is_file():
                html.append(
                    f"<figure><figcaption>{name}</figcaption>"
                    f"<img src='{src(path)}' alt='settings'></figure>"
                )
        html.append("</div>")
    html.append("")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(html), encoding="utf-8")
    print(f"written: {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
