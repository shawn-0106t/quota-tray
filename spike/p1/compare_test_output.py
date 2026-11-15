#!/usr/bin/env python3
"""Compare two captures of the headless self-checks (HANDOFF §5, phase P1).

`--test-fetch`, `--test-deepseek` and `--test-usage` print live data, so the
timestamps and scan durations can never repeat between two processes. Those
four occurrences are masked before comparing; every other byte — key order,
serde's number formatting, error strings, the scan counters — must match
exactly. That is what proves the `quota-core` extraction changed no behaviour.

Usage:
    PYTHONUTF8=1 python spike/p1/compare_test_output.py BEFORE_DIR AFTER_DIR

Both directories must hold the eight files a full capture writes:
fetch.out/.err, deepseek.out/.err, deepseek-verbose.out/.err, usage.out/.err.
Exits 1 on any difference that is not one of the masked fields.
"""

import difflib
import re
import sys
from pathlib import Path

FILES = [
    "fetch.out",
    "fetch.err",
    "deepseek.out",
    "deepseek.err",
    "deepseek-verbose.out",
    "deepseek-verbose.err",
    "usage.out",
    "usage.err",
]

# Live values: the fetched/scanned timestamp, the scan duration field and the
# `ms=` numbers on the two scan summary lines.
MASKED = [
    (re.compile(r'("(?:fetchedAt|scannedAt)": )"[^"]*"'), r'\1"<time>"'),
    (re.compile(r'("scanMs": )\d+'), r"\g<1>0"),
    (re.compile(r"\bms=\d+"), "ms=<n>"),
]


def normalize(text: str) -> str:
    for pattern, replacement in MASKED:
        text = pattern.sub(replacement, text)
    return text


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2
    before = Path(sys.argv[1])
    after = Path(sys.argv[2])
    failed = False
    for name in FILES:
        old = before / name
        new = after / name
        if not old.is_file() or not new.is_file():
            print(f"MISSING {name}")
            failed = True
            continue
        a = normalize(old.read_text(encoding="utf-8"))
        b = normalize(new.read_text(encoding="utf-8"))
        if a == b:
            print(f"OK      {name}")
            continue
        failed = True
        print(f"DIFF    {name}")
        for line in difflib.unified_diff(
            a.splitlines(), b.splitlines(), str(old), str(new), lineterm=""
        ):
            print(line)
    print("result: " + ("FAIL" if failed else "identical modulo the four live fields"))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
