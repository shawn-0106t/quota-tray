#!/usr/bin/env python3
"""check_tokens.py -- fail when slint/ui/tokens.slint drifts from src/theme.css.

The Slint shell cannot read CSS custom properties, so `Tokens` re-declares every
colour token in `slint/ui/tokens.slint`. Two copies drift: for example the
`--ink-3` contrast fix (2026-11-20, WCAG 1.4.11) landed in `src/theme.css` and
the Slint copy kept the old value until a review found it. This gate compares
both themes token by token, plus the card shadow's geometry (the CSS value and
every `drop-shadow-*` declaration in `slint/ui/`).

Usage (from the repository root):
    PYTHONUTF8=1 python scripts/parity/check_tokens.py

Exit 1 when any token, shadow colour or shadow geometry disagrees; the table
shows every compared token either way.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CSS = ROOT / "src" / "theme.css"
SLINT = ROOT / "slint" / "ui" / "tokens.slint"
SLINT_UI = ROOT / "slint" / "ui"

THEMES = ("light", "dark")

CSS_BLOCK = r":root\[data-theme='{theme}'\]\s*\{{(.*?)\}}"
CSS_DECL = re.compile(r"^\s*(--[a-z0-9-]+)\s*:\s*([^;]+);", re.M)
SLINT_COLOR = re.compile(
    r"out property <color>\s+([a-z0-9-]+)\s*:\s*dark\s*\?\s*(#[0-9a-fA-F]+)\s*"
    r":\s*(#[0-9a-fA-F]+)\s*;"
)
SHADOW = re.compile(
    r"^\s*(-?\d+(?:\.\d+)?)(?:px)?\s+(-?\d+(?:\.\d+)?)(?:px)?\s+"
    r"(-?\d+(?:\.\d+)?)(?:px)?\s+(#[0-9a-fA-F]{6,8})\s*$"
)
DROP_SHADOW = re.compile(
    r"drop-shadow-(offset-x|offset-y|blur|spread|color)\s*:\s*([^;]+);"
)


def token_name(css_name: str) -> str:
    """`--ink-1` is `ink1` in Slint; `--hair-soft` keeps its hyphen."""
    return re.sub(r"-(?=\d)", "", css_name.removeprefix("--"))


def css_tokens() -> dict[str, dict[str, str]]:
    text = CSS.read_text(encoding="utf-8")
    tokens: dict[str, dict[str, str]] = {}
    for theme in THEMES:
        blocks = list(re.finditer(CSS_BLOCK.format(theme=theme), text, re.S))
        if len(blocks) != 1:
            raise SystemExit(
                f"expected exactly one {theme} theme block in {CSS}, found {len(blocks)}"
            )
        found = {}
        for name, value in CSS_DECL.findall(blocks[0].group(1)):
            if name in found:
                raise SystemExit(f"duplicate token {name} in the {theme} block of {CSS}")
            # Trailing comments are written on their own line in theme.css,
            # but strip one defensively.
            value = value.split("/*")[0].strip()
            found[name] = value
        tokens[theme] = found
    return tokens


def slint_tokens() -> tuple[dict[str, tuple[str, str]], dict[str, int], list[str]]:
    colors: dict[str, tuple[str, str]] = {}
    lines: dict[str, int] = {}
    duplicates: list[str] = []
    for number, line in enumerate(SLINT.read_text(encoding="utf-8").splitlines(), 1):
        match = SLINT_COLOR.search(line)
        if match:
            name, dark, light = match.groups()
            if name in colors:
                duplicates.append(f"{name} at tokens.slint:{number}")
            colors[name] = (light.lower(), dark.lower())
            lines[name] = number
    return colors, lines, duplicates


def shadow_geometry(css: dict[str, dict[str, str]]) -> dict[str, tuple[float, float, float, str]]:
    geometry = {}
    for theme in THEMES:
        raw = css[theme].get("--shadow")
        if raw is None:
            raise SystemExit("--shadow missing from src/theme.css")
        match = SHADOW.match(raw)
        if not match:
            raise SystemExit(f"--shadow is not `x y blur colour`: {raw!r}")
        x, y, blur, colour = match.groups()
        geometry[theme] = (float(x), float(y), float(blur), colour.lower())
    return geometry


def slint_shadows() -> list[tuple[str, int, str, str]]:
    """Every `drop-shadow-*` declaration in the Slint UI: file, line, key, value."""
    found = []
    for path in sorted(SLINT_UI.glob("*.slint")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            # A commented-out declaration is not a declaration: without this,
            # `// drop-shadow-…` still counted as a live card shadow.
            code = line.split("//", 1)[0]
            for key, value in DROP_SHADOW.findall(code):
                found.append((path.name, number, key, value.strip()))
    return found


def main() -> int:
    css = css_tokens()
    colors, lines, duplicates = slint_tokens()
    geometry = shadow_geometry(css)

    rows: list[tuple[str, str, str, str, str]] = []
    problems = 0

    for duplicate in duplicates:
        problems += 1
        rows.append((f"(slint) {duplicate}", "-", "-", "-", "DUPLICATE token"))

    for css_name in sorted(css[THEMES[0]]):
        if css_name == "--shadow":
            continue
        name = token_name(css_name)
        light = css["light"][css_name].lower()
        dark = css["dark"].get(css_name, "<missing>").lower()
        if css_name not in css["dark"]:
            problems += 1
            rows.append((css_name, light, dark, "-", "MISSING in dark"))
            continue
        if name not in colors:
            problems += 1
            rows.append((css_name, light, dark, "-", "MISSING in tokens.slint"))
            continue
        slint_light, slint_dark = colors[name]
        if (light, dark) == (slint_light, slint_dark):
            rows.append((css_name, light, dark, name, "ok"))
        else:
            problems += 1
            rows.append(
                (css_name, light, dark, name, f"DIFFERS: slint {slint_light} / {slint_dark}")
            )

    accounted = {token_name(name) for name in css[THEMES[0]]}
    for name, where in sorted(lines.items()):
        if name not in accounted:
            problems += 1
            rows.append((f"(slint) {name}", "-", "-", name, f"EXTRA at tokens.slint:{where}"))

    light_only = set(css["light"]) - set(css["dark"])
    dark_only = set(css["dark"]) - set(css["light"])
    for name in sorted(light_only | dark_only):
        problems += 1
        rows.append((name, "-", "-", "-", "MISSING from one CSS theme"))

    # The shadow colour is a token; the geometry lives in every card's
    # drop-shadow declaration, so both halves are checked.
    if "shadow" not in colors:
        problems += 1
        rows.append(("--shadow", "-", "-", "-", "MISSING in tokens.slint"))
    else:
        slint_light, slint_dark = colors["shadow"]
        if (geometry["light"][3], geometry["dark"][3]) != (slint_light, slint_dark):
            problems += 1
            rows.append(
                (
                    "--shadow",
                    geometry["light"][3],
                    geometry["dark"][3],
                    "shadow",
                    f"DIFFERS: slint {slint_light} / {slint_dark}",
                )
            )
        else:
            rows.append(("--shadow", geometry["light"][3], geometry["dark"][3], "shadow", "ok"))

    shadows = slint_shadows()
    # The panel card and every other card carry 5 declarations each; a shortfall
    # means a card lost its shadow (the gate used to compare only what it found).
    if len(shadows) < 10:
        problems += 1
        print(
            f"expected at least 10 live drop-shadow-* declarations, found {len(shadows)}",
            file=sys.stderr,
        )
    if geometry["light"][:3] != geometry["dark"][:3]:
        problems += 1
        print(
            f"--shadow geometry differs between themes ({geometry['light'][:3]} vs "
            f"{geometry['dark'][:3]}); Tokens can only switch the colour",
            file=sys.stderr,
        )
    expected = {
        "offset-x": f"{geometry['light'][0]:g}px",
        "offset-y": f"{geometry['light'][1]:g}px",
        "blur": f"{geometry['light'][2]:g}px",
        "spread": "0px",
        "color": "Tokens.shadow",
    }
    for file, line, key, value in shadows:
        if value != expected[key]:
            problems += 1
            print(
                f"{file}:{line}: drop-shadow-{key}: {value} != {expected[key]} "
                f"(from {CSS.name})",
                file=sys.stderr,
            )

    width = max(len(row[0]) for row in rows) + 2
    print(f"{'token':<{width}} {'light':<10} {'dark':<10} {'slint':<10} status")
    print("-" * (width + 42))
    for css_name, light, dark, name, status in rows:
        print(f"{css_name:<{width}} {light:<10} {dark:<10} {name:<10} {status}")
    print()
    print(
        f"{len(rows)} tokens, {len(shadows)} drop-shadow declarations, {problems} mismatch(es) "
        f"({CSS.name} vs {SLINT.name})"
    )
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())
