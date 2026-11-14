"""Generate the Quota Tray icon set and the review page that shows it.

The shipped mark is the panel's own progress-bar motif: two quota bars, the
taller one on the left, in the PLAN §3.2 accent violet on a transparent field.
Design grid is 24 units; every size is drawn at 8x that size and downsampled, so
the 16px cut keeps clean edges and 256 stays sharp.

Outputs (all overwritten):
  src-tauri/icons/icon.png   256x256 RGBA, the bundle / exe icon
  src-tauri/icons/icon.ico   16/20/24/32/48/64/128/256, Windows bundle icon
  src-tauri/icons/tray.png   48x48 RGBA, what `tray.rs` embeds for the tray
                             (one RGBA buffer, so pick a size that covers the
                             common 100-300% DPI range by downscaling: 48 does,
                             a native 16 would be upscaled at 225% and soften)
  demo/icon-demo.html        the review page (shipped mark + the rejected cuts)

Run: PYTHONUTF8=1 python demo/make_icon.py
"""

import base64
import io
import pathlib

from PIL import Image, ImageDraw

ACCENT = (140, 116, 177, 255)  # #8c74b1, PLAN §3.2
PAPER = (251, 250, 249, 255)  # #fbfaf9
GRID = 24.0
SUPERSAMPLE = 8

# Shipped geometry, in grid units, bottom-aligned at y=19.5; radius 1.6 keeps the
# bars solid at 16px without turning them into pills at 256.
SHIPPED_BARS = [(3.5, 4.5, 10.5, 19.5), (12.5, 10.0, 19.5, 19.5)]
RADIUS = 1.6

# Rejected cuts, kept in the review page for comparison only (§9).
REJECTED = [
    ("tile", [(7.6, 6.4, 11.6, 17.6), (13.0, 11.2, 17.0, 17.6)], 0.86, True),
    ("Q", None, 0.0, False),
    ("bars-6u", [(4, 5, 10, 19), (13, 10, 19, 19)], 2.0, False),
]


def render(size: int, bars, radius: float, on_tile: bool = False) -> Image.Image:
    """One size, drawn at 8x and downscaled - never resampled from another size."""
    hi = size * SUPERSAMPLE
    scale = hi / GRID
    img = Image.new("RGBA", (hi, hi), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    if on_tile:
        draw.rounded_rectangle(
            [0.6 * scale, 0.6 * scale, 23.4 * scale, 23.4 * scale],
            radius=5.6 * scale,
            fill=ACCENT,
        )
    for x0, y0, x1, y1 in bars or []:
        draw.rounded_rectangle(
            [x0 * scale, y0 * scale, x1 * scale, y1 * scale],
            radius=radius * scale,
            fill=PAPER if on_tile else ACCENT,
        )
    return img.resize((size, size), Image.LANCZOS)


def render_q(size: int) -> Image.Image:
    hi = size * SUPERSAMPLE
    scale = hi / GRID
    img = Image.new("RGBA", (hi, hi), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    outer, inner = 8.6 * scale, 4.3 * scale
    c = 12.0 * scale
    draw.ellipse([c - outer, c - outer, c + outer, c + outer], fill=ACCENT)
    draw.ellipse([c - inner, c - inner, c + inner, c + inner], fill=(0, 0, 0, 0))
    draw.line(
        [14.6 * scale, 14.6 * scale, 19.4 * scale, 19.4 * scale],
        fill=ACCENT,
        width=int(2.6 * scale),
    )
    for x, y in ((14.6, 14.6), (19.4, 19.4)):
        r = 1.3 * scale
        draw.ellipse([x * scale - r, y * scale - r, x * scale + r, y * scale + r], fill=ACCENT)
    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    root = pathlib.Path(__file__).resolve().parent.parent
    icons = root / "src-tauri" / "icons"
    icons.mkdir(parents=True, exist_ok=True)

    render(256, SHIPPED_BARS, RADIUS).save(icons / "icon.png", optimize=True)
    render(48, SHIPPED_BARS, RADIUS).save(icons / "tray.png", optimize=True)

    ico_sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    tmp = []
    for size in ico_sizes:
        path = icons / f".ico-{size}.png"
        render(size, SHIPPED_BARS, RADIUS).save(path)
        tmp.append(path)
    render(256, SHIPPED_BARS, RADIUS).save(
        icons / "icon.ico",
        sizes=[(s, s) for s in ico_sizes],
        append_images=[Image.open(p) for p in tmp],
    )
    for path in tmp:
        path.unlink()

    for name in ("icon.png", "tray.png", "icon.ico"):
        print(f"{name}: {(icons / name).stat().st_size} bytes")

    (root / "demo" / "icon-demo.html").write_text(review_page(), encoding="utf-8", newline="\n")
    print("icon-demo.html written")


def b64(img: Image.Image, size: int) -> str:
    buf = io.BytesIO()
    img.resize((size, size), Image.LANCZOS).save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")


def review_page() -> str:
    sizes = [16, 20, 24, 32, 48]
    cuts = [
        ("shipped", "two bars, 7u wide, radius 1.6", render(256, SHIPPED_BARS, RADIUS), True),
        ("rejected: tile", "same bars on a violet tile", render(256, REJECTED[0][1], REJECTED[0][2], True), False),
        ("rejected: Q", "ring plus tail", render_q(256), False),
        ("rejected: 6u bars", "thinner bars, radius 2", render(256, REJECTED[2][1], REJECTED[2][2]), False),
    ]

    rows = []
    for code, note, icon, shipped in cuts:
        cells = "".join(
            f'<figure><img src="{b64(icon, s)}" width="{s}" height="{s}" alt="">'
            f"<figcaption>{s}</figcaption></figure>"
            for s in sizes
        )
        zoom = b64(icon, 16)
        badge = '<span class="badge">shipped</span>' if shipped else ""
        rows.append(
            f"""
      <section class="cand{' chosen' if shipped else ''}">
        <header><span class="code">{code}</span>{badge}
          <span class="note">{note}</span></header>
        <div class="strip light">{cells}<figure class="zoom">
            <img src="{zoom}" width="96" height="96" alt=""><figcaption>16 x6</figcaption></figure></div>
        <div class="strip dark">{cells}<figure class="zoom">
            <img src="{zoom}" width="96" height="96" alt=""><figcaption>16 x6</figcaption></figure></div>
      </section>"""
        )

    tray = b64(render(48, SHIPPED_BARS, RADIUS), 16)
    tray_bars = "".join(
        f'<div class="tray-bar {kind}"><span class="sys">^</span><span class="sys">v</span>'
        f'<span class="sys">|</span><img src="{tray}" width="16" height="16" alt="">'
        f'<span class="clock">15:47</span></div>'
        for kind in ("dark", "light")
    )

    big = "".join(
        f'<figure><img src="{b64(icon, 128)}" width="128" height="128" alt="">'
        f"<figcaption>{code}</figcaption></figure>"
        for code, _, icon, _ in cuts
    )

    return f"""<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Quota Tray - icon set</title>
    <style>
      :root {{
        --paper: #fbfaf9; --ink-1: #000000e6; --ink-2: #00000099;
        --hair: #00000021; --light: #f3f3f3; --dark: #202020;
      }}
      * {{ box-sizing: border-box; }}
      body {{
        margin: 0; padding: 32px 28px 48px; background: #e7e7e4; color: var(--ink-1);
        font-family: 'Segoe UI', system-ui, -apple-system, 'Microsoft YaHei UI', sans-serif;
        font-variant-numeric: tabular-nums;
      }}
      .sheet {{ max-width: 880px; margin: 0 auto; }}
      h1 {{ font-size: 18px; font-weight: 600; margin: 0 0 4px; }}
      .lede {{ font-size: 12.5px; color: var(--ink-2); margin: 0 0 24px; line-height: 1.55; }}
      .cand {{ background: var(--paper); border: 1px solid var(--hair); border-radius: 12px;
               padding: 14px 16px 16px; margin-bottom: 14px; }}
      .cand.chosen {{ box-shadow: inset 0 0 0 1px #7d61a64d; }}
      .cand header {{ display: flex; align-items: baseline; gap: 10px; margin-bottom: 10px; }}
      .code {{ font-size: 13px; font-weight: 600; }}
      .badge {{ font-size: 9.5px; font-weight: 500; color: #6c5293; background: #8c74b12e;
                border-radius: 4px; padding: 2px 8px; }}
      .note {{ font-size: 11px; color: var(--ink-2); margin-left: auto; }}
      .strip {{ display: flex; align-items: center; gap: 18px; padding: 10px 14px;
                border-radius: 8px; margin-bottom: 6px; }}
      .strip.light {{ background: var(--light); }}
      .strip.dark {{ background: var(--dark); }}
      figure {{ margin: 0; display: flex; flex-direction: column; align-items: center; gap: 4px; }}
      figcaption {{ font-size: 9.5px; color: var(--ink-2); }}
      .strip.dark figcaption {{ color: #ffffff8f; }}
      .zoom {{ margin-left: auto; }}
      .zoom img {{ image-rendering: pixelated; }}
      .tray-bar {{ display: flex; align-items: center; gap: 12px; height: 40px;
                   padding: 0 14px; border-radius: 8px; margin-bottom: 6px; }}
      .tray-bar.dark {{ background: var(--dark); }}
      .tray-bar.light {{ background: var(--light); }}
      .sys {{ font-size: 11px; color: #ffffffb0; }}
      .tray-bar.light .sys {{ color: #00000090; }}
      .clock {{ margin-left: auto; font-size: 11.5px; color: #ffffffb0; }}
      .tray-bar.light .clock {{ color: #00000090; }}
      .big {{ display: flex; gap: 26px; align-items: flex-end; margin: 10px 0 0; }}
      footer {{ margin-top: 26px; font-size: 11px; color: var(--ink-2); line-height: 1.6; }}
    </style>
  </head>
  <body>
    <div class="sheet">
      <h1>Quota Tray - icon set</h1>
      <p class="lede">
        Generated by <code>demo/make_icon.py</code>: every size is drawn at 8x and
        downsampled, never resampled from another size. The x6 column is the 16px cut
        magnified with nearest-neighbour - the actual pixels Windows shows at 100%.
      </p>

      {''.join(rows)}

      <div class="cand">
        <header><span class="code">in context</span>
          <span class="note">mock notification area, 40px bar, icon at 16px</span></header>
        {tray_bars}
      </div>

      <div class="cand">
        <header><span class="code">all cuts, 128px</span>
          <span class="note">left to right: shipped, then the three rejected ones</span></header>
        <div class="big">{big}</div>
      </div>

      <footer>
        Violet is #8c74b1: 3.54:1 on #f3f3f3 and 4.06:1 on #202020, so the silhouette
        clears the 3:1 WCAG 1.4.11 asks of meaningful graphics on both taskbars. The
        tray embeds the 32px cut; the exe and the bundle use the 256px / ICO sets.
      </footer>
    </div>
  </body>
</html>
"""


if __name__ == "__main__":
    main()
