"""Generate the README screenshots from the all-windows demo page.

`demo/ui-demo.html` is itself generated from the shipped markup and stylesheets
(`demo/build_ui_demo.py`), so what lands in `docs/` is the product's own DOM at
real size, not a mock-up. This script owns the crop: the three window cards -
both panel tabs and the settings sheet - in both themes, downscaled to a width
GitHub renders without shrinking.

Run: PYTHONUTF8=1 python demo/make_readme_shots.py
Needs Pillow and a Chromium browser (probed by `demo/shot.sh`: Chrome -> Edge).
"""
import pathlib
import subprocess
import sys

from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
SHOT = ROOT / ".shots" / "readme"
OUT = ROOT / "docs"

# The demo's window grid: page pixel box around the three top cards at 2x.
# Left edge 40 and right edge 2740 keep both cards' shadow; the settings card's
# rightmost ink sits at 2702, so a tighter crop clips its corner radius.
CROP = (40, 430, 2740, 1720)
EXPECT_SIZE = (2960, 3000)  # 1480x1500 CSS at scale 2, as shot.sh renders it
WIDTH = 1200


def main() -> int:
    demo = ROOT / "demo" / "ui-demo.html"
    if not demo.exists():
        print("demo/ui-demo.html is missing - run demo/build_ui_demo.py first")
        return 1
    SHOT.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    for theme in ("light", "dark"):
        raw = SHOT / f"{theme}-2x.png"
        # shot.sh runs under Git Bash: forward slashes, no backslash-escaped paths
        subprocess.run(
            ["bash", (ROOT / "demo" / "shot.sh").as_posix(), demo.as_posix(),
             raw.as_posix(), f"theme={theme}", "1480", "1500", "2"],
            check=True, cwd=ROOT,
        )
        image = Image.open(raw)
        # A browser / font / layout change moves the cards; cropping a fixed box
        # then pads with black instead of failing, so check the canvas first.
        if image.size != EXPECT_SIZE:
            print(f"{raw.name} is {image.width}x{image.height}, expected "
                  f"{EXPECT_SIZE[0]}x{EXPECT_SIZE[1]} - re-measure CROP")
            return 1
        crop = image.crop(CROP)
        height = round(crop.height * WIDTH / crop.width)
        crop = crop.resize((WIDTH, height), Image.LANCZOS)
        target = OUT / f"screenshot-{theme}.png"
        crop.save(target, optimize=True)
        print(f"written: {target.relative_to(ROOT)} ({crop.width}x{crop.height})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
