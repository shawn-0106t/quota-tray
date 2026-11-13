#!/usr/bin/env python3
"""crop_panel.py -- crop a demo screenshot down to the 424px window and trim
the uniform backdrop above/below the panel.

Usage: crop_panel.py <in.png> <out.png> [width_css=424] [scale=2]
"""
import sys
from PIL import Image


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    src, dst = sys.argv[1], sys.argv[2]
    width_css = int(sys.argv[3]) if len(sys.argv) > 3 else 424
    scale = int(sys.argv[4]) if len(sys.argv) > 4 else 2

    img = Image.open(src).convert("RGB")
    w = min(img.width, width_css * scale)
    img = img.crop((0, 0, w, img.height))

    bg = img.getpixel((2, 2))
    tol = 6

    def row_is_backdrop(y: int) -> bool:
        row = [img.getpixel((x, y)) for x in range(0, img.width, 3)]
        return all(abs(p[0] - bg[0]) <= tol and abs(p[1] - bg[1]) <= tol and abs(p[2] - bg[2]) <= tol
                   for p in row)

    top, bottom = 0, img.height - 1
    while top < bottom and row_is_backdrop(top):
        top += 1
    while bottom > top and row_is_backdrop(bottom):
        bottom -= 1

    img.crop((0, top, img.width, bottom + 1)).save(dst)
    print(f"{dst} {img.width}x{bottom + 1 - top}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
