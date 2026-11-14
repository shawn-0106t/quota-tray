#!/usr/bin/env bash
# shot.sh -- screenshot a local demo page for design review.
#
# Usage: shot.sh <page.html> <out.png> [query] [width=560] [height=760] [scale=2]
#   query  URL query string without '?', e.g. "bare=1&theme=dark" (may be empty string)
#
# Why this exists next to rationalism-design's headless-shot.sh: that script only
# accepts a plain file path, and the demos need ?theme/?tab/?state presets.
# Browser probing follows the house rule: Chrome -> Edge -> other Chromium.
# Windows browsers cannot write to MSYS paths, so every path handed to the
# browser is converted with cygpath first.
set -euo pipefail

[ $# -ge 2 ] || { echo "usage: shot.sh <page.html> <out.png> [query] [width] [height] [scale]" >&2; exit 1; }

PAGE="$1"; OUT="$2"; QUERY="${3:-}"; WIDTH="${4:-560}"; HEIGHT="${5:-760}"; SCALE="${6:-2}"

[ -f "$PAGE" ] || { echo "page not found: $PAGE" >&2; exit 1; }

win() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
mix() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }

PAGE_ABS="$(cd "$(dirname "$PAGE")" && pwd)/$(basename "$PAGE")"
URL="file:///$(mix "$PAGE_ABS")"
URL="${URL// /%20}"
[ -n "$QUERY" ] && URL="$URL?$QUERY"

BROWSER=""
SEARCHED=()
for cand in \
  "$LOCALAPPDATA/Google/Chrome/Application/chrome.exe" \
  "/c/Program Files/Google/Chrome/Application/chrome.exe" \
  "/c/Program Files (x86)/Google/Chrome/Application/chrome.exe" \
  "/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe" \
  "/c/Program Files/Microsoft/Edge/Application/msedge.exe"; do
  SEARCHED+=("$cand")
  if [ -f "$cand" ]; then BROWSER="$cand"; break; fi
done
if [ -z "$BROWSER" ]; then
  for cmd in chrome google-chrome chromium msedge microsoft-edge; do
    SEARCHED+=("$cmd")
    if resolved="$(command -v "$cmd" 2>/dev/null)"; then BROWSER="$resolved"; break; fi
  done
fi
[ -n "$BROWSER" ] || { printf 'no Chromium browser found. searched:\n'; printf '  %s\n' "${SEARCHED[@]}"; exit 1; }

mkdir -p "$(dirname "$OUT")" .shots
PROFILE="$(mktemp -d .shots/prof-XXXXXX)"
trap 'rm -rf "$PROFILE"' EXIT

# The browser needs a Windows path, so the output has to be anchored to $PWD -
# but only when it is relative; a caller that passed an absolute path would
# otherwise get "C:/repo/C:/elsewhere/x.png".
case "$OUT" in
  /*|[A-Za-z]:/*) OUT_ABS="$OUT" ;;
  *) OUT_ABS="$PWD/$OUT" ;;
esac

"$BROWSER" \
  --headless=new \
  --disable-gpu \
  --hide-scrollbars \
  --no-first-run \
  --no-default-browser-check \
  --disable-extensions \
  --force-device-scale-factor="$SCALE" \
  --window-size="$WIDTH,$HEIGHT" \
  --virtual-time-budget=4000 \
  --user-data-dir="$(win "$PWD/$PROFILE")" \
  --screenshot="$(win "$OUT_ABS")" \
  "$URL" >/dev/null 2>&1 || { echo "browser failed" >&2; exit 1; }

[ -f "$OUT" ] || { echo "no screenshot produced" >&2; exit 1; }
printf '%s\n' "$OUT"
