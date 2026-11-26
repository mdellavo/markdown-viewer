#!/usr/bin/env bash
# Regenerate the README demo (docs/demo-{light,dark}.gif): an editor and
# markdown-viewer side by side, the page updating as the file is saved.
# tests/demo.rs records the frames (see docs/demo/ for the stage and the
# text it types); ffmpeg turns them into GIFs, which GitHub and crates.io
# play inline.
#
# Needs Chrome (or Chromium) and ffmpeg:
#   docs/demo.sh
set -euo pipefail

cd "$(dirname "$0")/.."
command -v ffmpeg >/dev/null || { echo "docs/demo.sh needs ffmpeg" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

for theme in light dark; do
  MDV_DEMO_FRAMES="$work/$theme" MDV_DEMO_THEME="$theme" \
    cargo test --quiet --test demo -- --ignored >/dev/null
  # One palette for the whole clip, and only the changed rectangle of each
  # frame stored, keeps the GIF small.
  ffmpeg -loglevel error -y -f concat -i "$work/$theme/frames.txt" \
    -vf "fps=20,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
    -loop 0 "docs/demo-$theme.gif"
  echo "wrote docs/demo-$theme.gif ($(du -h "docs/demo-$theme.gif" | cut -f1))"
done
