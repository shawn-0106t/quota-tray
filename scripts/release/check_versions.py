#!/usr/bin/env python3
"""Assert that every version carrier in this repository agrees.

A release is one version written in five places, and only `tauri.conf.json` is
read by the build:

    package.json                 npm package version
    package-lock.json            ... and both of its own fields (root entry)
    src-tauri/tauri.conf.json    the canonical one: the exe's file properties,
                                 the asset names and make_release_zip.py all
                                 read this
    src-tauri/Cargo.toml         the crate version
    src-tauri/Cargo.lock         follows Cargo.toml, but is committed

The 1.0.1 bump left `package-lock.json` stale until a review caught it: `npm ci`
does not care, the build does not read it, and only the source zip kept the
discrepancy. Run with `--tag vX.Y.Z` (the release workflow does) to also assert
the pushed tag equals the carriers.

Needs Python 3.11+ for `tomllib` (the runners and this machine have it).
Exit code is 1 on any mismatch, 0 otherwise.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
TAG_RE = re.compile(r"^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")


def text(rel: str) -> str:
    return (ROOT / rel).read_text(encoding="utf-8")


def cargo_toml_version(rel: str) -> str | None:
    # `version.workspace = true` (and the like) parses to no string here and
    # reports the file as missing the key rather than guessing from a
    # dependency's own `version =` line.
    package = tomllib.loads(text(rel)).get("package", {})
    version = package.get("version")
    return version if isinstance(version, str) else None


def cargo_lock_version(rel: str) -> str | None:
    for package in tomllib.loads(text(rel)).get("package", []):
        if package.get("name") == "quota-tray":
            version = package.get("version")
            return version if isinstance(version, str) else None
    return None


def carriers() -> list[tuple[str, str | None]]:
    package = json.loads(text("package.json"))
    lock = json.loads(text("package-lock.json"))
    tauri = json.loads(text("src-tauri/tauri.conf.json"))
    root_entry = lock.get("packages", {}).get("", {})
    return [
        ("package.json", package.get("version")),
        ("package-lock.json", lock.get("version")),
        ('package-lock.json (packages[""])', root_entry.get("version")),
        ("src-tauri/tauri.conf.json", tauri.get("version")),
        ("src-tauri/Cargo.toml", cargo_toml_version("src-tauri/Cargo.toml")),
        ("src-tauri/Cargo.lock", cargo_lock_version("src-tauri/Cargo.lock")),
    ]


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--tag", help="also require this tag (vX.Y.Z) to equal the carriers")
    args = parser.parse_args()

    try:
        found = carriers()
    except (OSError, ValueError) as exc:
        print(f"could not read the version carriers: {exc}", file=sys.stderr)
        return 1

    canonical = dict(found).get("src-tauri/tauri.conf.json")
    if not canonical:
        print("src-tauri/tauri.conf.json carries no version", file=sys.stderr)
        return 1

    width = max(len(name) for name, _ in found)
    mismatched = []
    for name, version in found:
        ok = version == canonical
        print(f"{name:<{width}}  {version or '(missing)'}  {'ok' if ok else 'MISMATCH'}", flush=True)
        if not ok:
            mismatched.append(name)

    if mismatched:
        print(
            f"\ncanonical version is {canonical} (src-tauri/tauri.conf.json); "
            f"mismatched: {', '.join(mismatched)}\n"
            "the npm lockfile follows `npm install --package-lock-only`, "
            "Cargo.lock follows a `cargo` build.",
            file=sys.stderr,
        )
        return 1

    if args.tag is not None:
        if not TAG_RE.match(args.tag):
            print(f"\n--tag {args.tag!r} is not vX.Y.Z", file=sys.stderr)
            return 1
        expected = f"v{canonical}"
        if args.tag != expected:
            print(f"\ntag {args.tag} does not match {expected}", file=sys.stderr)
            return 1
        print(f"{'tag':<{width}}  {args.tag}  ok", flush=True)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
