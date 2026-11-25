#!/usr/bin/env bash
# Regenerate the README screenshots (docs/screenshot-{light,dark}.png) from
# docs/screenshot.md, with the contents sidebar open.
#
# Needs a built binary and a Chromium-based browser:
#   cargo build && docs/screenshots.sh
# Set CHROME to the browser's path if it isn't Google Chrome on macOS.
set -euo pipefail

cd "$(dirname "$0")/.."
bin=${BIN:-target/debug/markdown-viewer}
chrome=${CHROME:-"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

for theme in light dark; do
  # Isolated settings and state: the theme and open sidebar come from here,
  # never from your own preferences.
  printf '{"theme":"%s","sidebar":"contents"}' "$theme" > "$work/state.json"
  page=$(MARKDOWN_VIEWER_STATE="$work/state.json" MARKDOWN_VIEWER_CONFIG="$work/none.toml" \
    "$bin" --no-open docs/screenshot.md)
  # A standalone page (not --watch): headless Chrome can then wait until
  # diagrams and math have finished drawing. Note: a #fragment in the URL
  # makes headless Chrome capture a blank page.
  "$chrome" --headless --disable-gpu --hide-scrollbars \
    --user-data-dir="$work/profile-$theme" \
    --window-size=1280,800 --force-device-scale-factor=2 \
    --virtual-time-budget=8000 \
    --screenshot="docs/screenshot-$theme.png" "file://$page" >/dev/null 2>&1
  echo "wrote docs/screenshot-$theme.png"
done
