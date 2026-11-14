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

# The demo's window grid, in page pixels at 2x: the three top cards with a ~30 px
# margin of empty stage around them. Derived from the live DOM (`.win` boxes via
# getBoundingClientRect — main 26..450, deepseek 480..904, settings 934..1338 wide,
# all 267..873 tall at dpr 2) plus the shadow spill below (ink reaches y≈1787) and
# the empty band before the second row (starts y≈1880). Re-measure with the same
# probe after any stage-padding or layout change; the border check is the alarm.
CROP = (30, 500, 2682, 1810)
EXPECT_SIZE = (2960, 3000)  # 1480x1500 CSS at scale 2, as shot.sh renders it
WIDTH = 1200


def border_is_clean(image: Image.Image) -> bool:
    """True when the crop's outer 1 px ring is one flat colour.

    The stage behind the cards is a flat colour per theme, so a clean ring means
    the crop ends on empty page. Anything else - a card edge, a line of caption
    text - means the layout moved and the crop is cutting content; the old fixed
    box padded with black and looked plausible, which is how a clipped heading
    shipped once already.
    """
    left, top, right, bottom = CROP
    ring = [image.getpixel((x, y)) for x in range(left, right) for y in (top, bottom - 1)]
    ring += [image.getpixel((x, y)) for y in range(top, bottom) for x in (left, right - 1)]
    return len(set(ring)) == 1


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
        if not border_is_clean(image):
            print(f"{raw.name}: crop {CROP} does not end on empty stage - "
                  "re-measure CROP (a card or a caption line is being cut)")
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
