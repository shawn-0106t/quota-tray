#!/usr/bin/env python3
"""Build the release assets for a Quota Tray release.

Outputs, all at the repository root (gitignored):

    QuotaTray.exe                      the portable exe under its asset name
    QuotaTray_<version>_x64-setup.exe  the NSIS installer under its asset name
    QuotaTray-v<version>.zip           the source tree + the portable exe at the root
    SHA256SUMS.txt                     sha256sum format, covering the three above

The zip is the whole source tree as `git ls-files` sees it (tracked plus
untracked-but-not-ignored), so it can never drift out of sync with
.gitignore; a filtered walk is the fallback when git metadata is unavailable
or when `git ls-files` answered for some other enclosing repository.

The version comes from `src-tauri/tauri.conf.json` - one source of truth, so
a tag and the thing it names cannot disagree.

The big outputs are staged as `<name>.part` and renamed only once complete: the
zip last, so a failure there removes the checksum file instead of leaving a
pair that fails `sha256sum -c`. A failed run therefore never advertises a
checksum for a file that is not there.
"""
import hashlib
import json
import os
import subprocess
import sys
import zipfile
from fnmatch import fnmatchcase

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))

# Never shipped even when present, matched case-folded like the rules below
# (all are gitignored as well; belt and braces for the non-git walk).
EXCLUDE_FILES = {"sha256sums.txt", "sha256sums.txt.part", "portable.dat"}
# Release-asset names; keeps a binary dropped at the repo root for
# `sha256sum -c` out of the snapshot on either OS.
EXCLUDE_PATTERNS = ("quotatray*.exe",)
# Archives, the .part intermediates, and bytecode are never snapshot material
EXCLUDE_SUFFIXES = (".zip", ".part", ".pyc")
# When `git ls-files` resolves to a different enclosing repository (e.g. this
# tree unpacked inside someone else's checkout) the answer is not ours.
REPO_SENTINELS = ("AGENTS.md", "scripts/release/make_release_zip.py")
# Fallback walk exclusions, only used when git is unavailable. The walk cannot
# read .gitignore, so every ignored build-output directory is named here.
EXCLUDE_DIRS = {"node_modules", "target", "dist", "gen", ".git", ".vs",
                "__pycache__", ".shots"}


def version():
    with open(os.path.join(ROOT, "src-tauri", "tauri.conf.json"), encoding="utf-8") as f:
        return json.load(f)["version"]


def binaries(ver):
    """(repo-relative source, asset name, goes into the zip) for every exe."""
    return [
        ("src-tauri/target/release/quota-tray.exe", "QuotaTray.exe", True),
        (f"src-tauri/target/release/bundle/nsis/Quota Tray_{ver}_x64-setup.exe",
         f"QuotaTray_{ver}_x64-setup.exe", False),
    ]


def source_files():
    """Repo-relative paths of every file belonging in the source snapshot."""
    files = None
    try:
        out = subprocess.run(
            ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
            cwd=ROOT, capture_output=True, check=True,
        )
        files = [n for n in out.stdout.decode("utf-8", errors="replace").split("\0") if n]
    except (OSError, subprocess.CalledProcessError):
        pass
    if files is not None and not set(REPO_SENTINELS).issubset(files):
        files = None
    if files is None:
        files = []
        for dirpath, dirnames, filenames in os.walk(ROOT):
            dirnames[:] = [d for d in dirnames if d not in EXCLUDE_DIRS]
            for name in filenames:
                rel = os.path.relpath(os.path.join(dirpath, name), ROOT)
                files.append(rel.replace(os.sep, "/"))
    return [f for f in files
            if os.path.basename(f).lower() not in EXCLUDE_FILES
            and not any(fnmatchcase(os.path.basename(f).lower(), p) for p in EXCLUDE_PATTERNS)
            and not f.lower().endswith(EXCLUDE_SUFFIXES)
            # --cached can list a path whose worktree file was deleted but not
            # yet staged; skip it instead of aborting mid-zip.
            and os.path.isfile(os.path.join(ROOT, f))]


def sha256_of(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def stage_copy(src, dst):
    """Copy through `<dst>.part` so a killed run never leaves a short asset."""
    with open(src, "rb") as fin, open(dst + ".part", "wb") as fout:
        for chunk in iter(lambda: fin.read(1 << 20), b""):
            fout.write(chunk)
    os.replace(dst + ".part", dst)


def write_checksums(sums_part, zip_part, zip_name, assets):
    """Stage SHA256SUMS.txt.part for exactly the uploaded assets.

    Hashes the `.part` zip, which already holds exactly the bytes the renamed
    zip will hold.
    """
    lines = [f"{sha256_of(os.path.join(ROOT, asset))}  {asset}"
             for _, asset, _ in assets]
    lines.append(f"{sha256_of(zip_part)}  {zip_name}")
    with open(sums_part, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")


def ensure_replaceable(paths):
    """Fail before building anything if a destination is obviously unusable."""
    for p in paths:
        if os.path.isdir(p):
            raise SystemExit(f"cannot replace {p}: is a directory")
        if os.path.exists(p) and not os.access(p, os.W_OK):
            raise SystemExit(f"cannot replace {p}: not writable")


def main():
    ver = version()
    assets = binaries(ver)
    zip_name = f"QuotaTray-v{ver}.zip"
    out = os.path.join(ROOT, zip_name)
    part = out + ".part"
    sums = os.path.join(ROOT, "SHA256SUMS.txt")
    sums_part = sums + ".part"
    staged = [os.path.join(ROOT, asset) for _, asset, _ in assets]
    try:
        # Inside the try on purpose: a stale .part left by an earlier crash is
        # swept by the cleanup below even when this run fails its pre-flight.
        for src, _, _ in assets:
            if not os.path.isfile(os.path.join(ROOT, src)):
                raise SystemExit(f"missing binary: {src} (run `npx tauri build` first)")
        ensure_replaceable([out, sums, part, sums_part] + staged)
        for src, asset, _ in assets:
            stage_copy(os.path.join(ROOT, src), os.path.join(ROOT, asset))
        rels = source_files()
        if not rels:
            raise SystemExit("no source files found (wrong repo root, or .gitignore too broad)")
        with zipfile.ZipFile(part, "w", zipfile.ZIP_DEFLATED) as zf:
            for rel in rels:
                zf.write(os.path.join(ROOT, rel), rel)
            for _, asset, in_zip in assets:
                if in_zip:
                    zf.write(os.path.join(ROOT, asset), asset)
        write_checksums(sums_part, part, zip_name, assets)
        os.replace(sums_part, sums)  # renamed together with the zip below
        try:
            os.replace(part, out)
        except OSError:
            # The staged exes already replaced the previous ones, so the old
            # checksum file describes files that no longer exist: drop it rather
            # than leave a pair that fails `sha256sum -c`.
            try:
                os.remove(sums)
            except OSError:
                pass  # surface the zip-rename error rather than the cleanup one
            raise
    except BaseException:  # incl. KeyboardInterrupt: never leave a half-product behind
        for tmp in [part, sums_part] + [p + ".part" for p in staged]:
            try:
                os.remove(tmp)
            except OSError:
                pass  # keep the original failure as the surfaced error
        raise
    size_mb = os.path.getsize(out) / 1024 / 1024
    print(f"version: {ver}")
    print(f"written: {zip_name} ({size_mb:.1f} MB, {len(rels)} files)")
    for _, asset, _ in assets:
        print(f"written: {asset}")
    print("written: SHA256SUMS.txt")


if __name__ == "__main__":
    sys.exit(main())
