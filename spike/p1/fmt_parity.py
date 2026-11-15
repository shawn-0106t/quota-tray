#!/usr/bin/env python3
"""Cross-check `quota-core::format` against the TypeScript it replaces.

While both shells exist, the Tauri frontend still renders from `src/common.ts`
and the Rust port is what the Slint shell will use, so the two must agree string
for string (HANDOFF §6.3). This harness:

  1. copies `src/common.ts` with its `@tauri-apps` imports stripped,
  2. runs `spike/p1/fmt_cases.ts` under node (native type stripping),
  3. runs `quota-core`'s `fmt_probe` example over the same case list,
  4. diffs the two TSV streams.

Usage (from the repository root):
    PYTHONUTF8=1 python spike/p1/fmt_parity.py

Exits 1 on the first case where the two implementations disagree.
"""

import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORK = ROOT / ".shots" / "p1" / ".fmt"
QUOTA_CORE = ROOT / "quota-core"


def main() -> int:
    WORK.mkdir(parents=True, exist_ok=True)
    source = (ROOT / "src" / "common.ts").read_text(encoding="utf-8")
    stripped = "".join(
        line for line in source.splitlines(keepends=True) if not line.startswith("import ")
    )
    (WORK / "common_probe.ts").write_text(stripped, encoding="utf-8")
    shutil.copy(ROOT / "spike" / "p1" / "fmt_cases.ts", WORK / "fmt_cases.ts")

    node = subprocess.run(
        ["node", "fmt_cases.ts"], cwd=WORK, capture_output=True, text=True, encoding="utf-8"
    )
    if node.returncode != 0:
        print(node.stderr, file=sys.stderr)
        return 1

    rust = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--example",
            "fmt_probe",
            "--",
            str(WORK / "cases.json"),
        ],
        cwd=QUOTA_CORE,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if rust.returncode != 0:
        print(rust.stderr, file=sys.stderr)
        return 1

    js_lines = (WORK / "js.tsv").read_text(encoding="utf-8").splitlines()
    rust_lines = rust.stdout.splitlines()
    if js_lines == rust_lines:
        print(f"format parity OK: {len(js_lines)} cases identical (TS == Rust)")
        return 0
    print(f"format parity FAIL: {len(js_lines)} TS cases vs {len(rust_lines)} Rust cases")
    for i, (a, b) in enumerate(zip(js_lines, rust_lines)):
        if a != b:
            print(f"  case {i}:\n    TS:   {a}\n    Rust: {b}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
