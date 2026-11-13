#!/usr/bin/env python3
"""check_contrast.py -- WCAG ratios for the demo token pairs.

Usage: check_contrast.py
Composites alpha colors over their ground, then reports sRGB contrast for the
text pairs that actually appear in the demos (body/muted/accent/badge/status).
"""
from itertools import product


def hex_rgb(h: str) -> tuple[float, float, float]:
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))  # type: ignore[return-value]


def over(fg: tuple[float, float, float], alpha: float, bg: tuple[float, float, float]):
    return tuple(f * alpha + b * (1 - alpha) for f, b in zip(fg, bg))


def lum(c: tuple[float, float, float]) -> float:
    def ch(v: float) -> float:
        return v / 12.92 if v <= 0.04045 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = (ch(v) for v in c)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def ratio(a, b) -> float:
    la, lb = lum(a), lum(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def main() -> int:
    black, white = hex_rgb("#000000"), hex_rgb("#ffffff")
    rows: list[tuple[str, float, str]] = []

    # Demo A, light: paper #fbfaf9
    a_light = hex_rgb("#fbfaf9")
    rows += [
        ("A light body   ink-1 90%", ratio(over(black, 0.90, a_light), a_light), "4.5"),
        ("A light muted  ink-2 60%", ratio(over(black, 0.60, a_light), a_light), "4.5"),
        ("A light badge  #0a5fc0 on 10% blue", ratio(hex_rgb("#0a5fc0"), over(hex_rgb("#1783ff"), 0.10, a_light)), "4.5"),
        ("A light warn   #9a5c00", ratio(hex_rgb("#9a5c00"), a_light), "4.5"),
        ("A light danger #b42318", ratio(hex_rgb("#b42318"), a_light), "4.5"),
        ("A light ok     #0f7b52", ratio(hex_rgb("#0f7b52"), a_light), "4.5"),
    ]
    # Demo A, dark: #181817
    a_dark = hex_rgb("#181817")
    rows += [
        ("A dark  body   ink-1 84%", ratio(over(white, 0.84, a_dark), a_dark), "4.5"),
        ("A dark  muted  ink-2 56%", ratio(over(white, 0.56, a_dark), a_dark), "4.5"),
        ("A dark  badge  #6fb4ff on 15% blue", ratio(hex_rgb("#6fb4ff"), over(hex_rgb("#1a88ff"), 0.15, a_dark)), "4.5"),
        ("A dark  warn   #e0a54b", ratio(hex_rgb("#e0a54b"), a_dark), "4.5"),
        ("A dark  danger #ff6b6b", ratio(hex_rgb("#ff6b6b"), a_dark), "4.5"),
        ("A dark  ok     #4cc38a", ratio(hex_rgb("#4cc38a"), a_dark), "4.5"),
    ]
    # Demo B, dark: #141414 / light: #ffffff
    b_dark, b_light = hex_rgb("#141414"), hex_rgb("#ffffff")
    rows += [
        ("B dark  ink    #ffffff", ratio(white, b_dark), "4.5"),
        ("B dark  ink-2  #d8d8d8", ratio(hex_rgb("#d8d8d8"), b_dark), "4.5"),
        ("B dark  gray   #9a9a9a", ratio(hex_rgb("#9a9a9a"), b_dark), "4.5"),
        ("B dark  red-ink#ff5c6c", ratio(hex_rgb("#ff5c6c"), b_dark), "4.5"),
        ("B dark  ok     #3fbf7f", ratio(hex_rgb("#3fbf7f"), b_dark), "4.5"),
        ("B dark  warn   #e0a54b", ratio(hex_rgb("#e0a54b"), b_dark), "4.5"),
        ("B dark  tab    #fff on #d5001c", ratio(white, hex_rgb("#d5001c")), "4.5"),
        ("B light ink    #0b0b0b", ratio(hex_rgb("#0b0b0b"), b_light), "4.5"),
        ("B light gray   #6f6f6f", ratio(hex_rgb("#6f6f6f"), b_light), "4.5"),
        ("B light red-ink#b30018", ratio(hex_rgb("#b30018"), b_light), "4.5"),
        ("B light ok     #0f7b4f", ratio(hex_rgb("#0f7b4f"), b_light), "4.5"),
        ("B light warn   #9a5c00", ratio(hex_rgb("#9a5c00"), b_light), "4.5"),
        ("B light tab    #fff on #d5001c", ratio(white, hex_rgb("#d5001c")), "4.5"),
    ]

    # Demo C: paper #faf9f5 / #141413, Kimi-blue accent
    c_light, c_dark = hex_rgb("#faf9f5"), hex_rgb("#141413")
    c_field = hex_rgb("#f5f4ed")
    rows += [
        ("C light ink    #141413", ratio(hex_rgb("#141413"), c_light), "4.5"),
        ("C light ink-2  #5e5d59", ratio(hex_rgb("#5e5d59"), c_light), "4.5"),
        ("C light ink-3  #6b6a64 (AA step-down)", ratio(hex_rgb("#6b6a64"), c_light), "4.5"),
        ("C light ink-3 on field #f5f4ed", ratio(hex_rgb("#6b6a64"), c_field), "4.5"),
        ("C light error  #b53333", ratio(hex_rgb("#b53333"), c_light), "4.5"),
        ("C light chip   #0a5fc0 on blue 10% tint", ratio(hex_rgb("#0a5fc0"), over(hex_rgb("#1783ff"), 0.10, c_light)), "4.5"),
        ("C dark  ink    #faf9f5", ratio(hex_rgb("#faf9f5"), c_dark), "4.5"),
        ("C dark  ink-2  #b0aea5", ratio(hex_rgb("#b0aea5"), c_dark), "4.5"),
        ("C dark  ink-3  #9a988f", ratio(hex_rgb("#9a988f"), c_dark), "4.5"),
        ("C dark  error  #d9655a (AA step-up)", ratio(hex_rgb("#d9655a"), c_dark), "4.5"),
        ("C dark  chip   #6fb4ff on blue 15% tint", ratio(hex_rgb("#6fb4ff"), over(hex_rgb("#1a88ff"), 0.15, c_dark)), "4.5"),
        ("C light bar    #0d6fe8 vs track #e8e6dc", ratio(hex_rgb("#0d6fe8"), hex_rgb("#e8e6dc")), "3.0"),
        ("C dark  bar    #1a88ff vs track #30302e", ratio(hex_rgb("#1a88ff"), hex_rgb("#30302e")), "3.0"),
        ("C light focus  #0d6fe8 vs paper", ratio(hex_rgb("#0d6fe8"), c_light), "3.0"),
    ]

    # Demo D: A's paper/ink ladder + single Kimi-blue accent + one danger red
    d_light, d_dark = hex_rgb("#fbfaf9"), hex_rgb("#181817")
    d_track_l = over(black, 0.07, d_light)
    d_track_d = over(white, 0.07, d_dark)
    rows += [
        ("D light ink-1 90%", ratio(over(black, 0.90, d_light), d_light), "4.5"),
        ("D light ink-2 60% on paper", ratio(over(black, 0.60, d_light), d_light), "4.5"),
        ("D light chip ink on blue 10% tint", ratio(hex_rgb("#0a5fc0"), over(hex_rgb("#1783ff"), 0.10, d_light)), "4.5"),
        ("D light danger #b42318", ratio(hex_rgb("#b42318"), d_light), "4.5"),
        ("D light bar #0d6fe8 vs track", ratio(hex_rgb("#0d6fe8"), d_track_l), "3.0"),
        ("D dark  ink-1 84%", ratio(over(white, 0.84, d_dark), d_dark), "4.5"),
        ("D dark  ink-2 56%", ratio(over(white, 0.56, d_dark), d_dark), "4.5"),
        ("D dark  chip ink on blue 15% tint", ratio(hex_rgb("#6fb4ff"), over(hex_rgb("#1a88ff"), 0.15, d_dark)), "4.5"),
        ("D dark  danger #ff6b6b", ratio(hex_rgb("#ff6b6b"), d_dark), "4.5"),
        ("D dark  bar #1a88ff vs track", ratio(hex_rgb("#1a88ff"), d_track_d), "3.0"),
        ("D light tab inactive on track", ratio(over(black, 0.60, hex_rgb("#f1f0ec")), hex_rgb("#f1f0ec")), "4.5"),
        ("D light tab active on tile", ratio(over(black, 0.90, hex_rgb("#ffffff")), hex_rgb("#ffffff")), "4.5"),
        ("D dark  tab inactive on track", ratio(over(white, 0.56, hex_rgb("#262624")), hex_rgb("#262624")), "4.5"),
        ("D dark  tab active on tile", ratio(over(white, 0.84, hex_rgb("#3a3a37")), hex_rgb("#3a3a37")), "4.5"),
        ("D light switch track ink-2 60%", ratio(over(black, 0.60, d_light), d_light), "3.0"),
        ("D dark  switch track ink-2 56%", ratio(over(white, 0.56, d_dark), d_dark), "3.0"),
    ]

    bad = 0
    for label, r, need in rows:
        ok = r >= float(need)
        bad += 0 if ok else 1
        print(f"{'PASS' if ok else 'FAIL'}  {r:5.2f}  (>= {need})  {label}")
    print(f"\n{len(rows) - bad}/{len(rows)} pairs pass")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
