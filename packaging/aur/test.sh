#!/usr/bin/env bash
# Check a generated AUR package on Arch Linux (run as root in an archlinux
# container): .SRCINFO matches makepkg's, namcap is happy, and the package
# builds, installs and runs.
# Usage: test.sh <dir with PKGBUILD and .SRCINFO>
set -euo pipefail
pacman -Syu --noconfirm --needed base-devel namcap >/dev/null
useradd -m builder 2>/dev/null || true
work=/home/builder/pkg
rm -rf "$work" && cp -r "$1" "$work" && chown -R builder "$work"
cd "$work"
su builder -c "makepkg --printsrcinfo" | diff -u - .SRCINFO
namcap PKGBUILD
su builder -c "makepkg --noconfirm --cleanbuild"
pacman -U --noconfirm ./markdown-viewer-bin-*.pkg.tar.zst
markdown-viewer --version
# The archlinux image skips man pages when installing, so check the package.
files=$(pacman -Qlpq ./markdown-viewer-bin-*.pkg.tar.zst)
for f in /usr/bin/markdown-viewer /usr/share/man/man1/markdown-viewer.1.gz \
         /usr/share/zsh/site-functions/_markdown-viewer \
         /usr/share/bash-completion/completions/markdown-viewer \
         /usr/share/fish/vendor_completions.d/markdown-viewer.fish \
         /usr/share/licenses/markdown-viewer-bin/LICENSE; do
  grep -qx "$f" <<<"$files" || { echo "missing from package: $f"; exit 1; }
done
printf '# Hello\n\n- [x] done\n' | markdown-viewer - -o - | grep -q '<h1'
echo "AUR package OK"
