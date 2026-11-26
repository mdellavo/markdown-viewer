#!/usr/bin/env bash
# Package a release build for one target into dist/:
#   markdown-viewer-<target>.tar.gz   (binary, docs, completions, man page,
#   and on Linux the desktop entry and icon)
#   and on Linux also a .deb and an .rpm,
# each with a .sha256 checksum next to it.
#
# Usage: packaging/package.sh <target>   (after cargo build --release --target <target>)
# Linux packages need cargo-deb and cargo-generate-rpm installed.
set -euo pipefail

target=$1
cd "$(dirname "$0")/.."
mkdir -p dist

name=markdown-viewer-$target
rm -rf "$name"
mkdir "$name"
cp "target/$target/release/markdown-viewer" README.md LICENSE CHANGELOG.md "$name"/
cp -R completions man "$name"/
# The desktop entry and icon (used by the AUR package; Linux only).
case "$target" in
  *-linux-*)
    mkdir "$name/desktop"
    cp packaging/linux/markdown-viewer.desktop "$name/desktop/"
    cp assets/macos/AppIcon.svg "$name/desktop/markdown-viewer.svg"
    ;;
esac
tar czf "dist/$name.tar.gz" "$name"
rm -rf "$name"

case "$target" in
  *-linux-*)
    version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
    arch=${target%%-*}
    deb=$(cargo deb --target "$target" --no-build --no-strip --output "dist/" | tail -1)
    debarch=$([ "$arch" = x86_64 ] && echo amd64 || echo arm64)
    mv "$deb" "dist/markdown-viewer_${version}_${debarch}.deb"
    cargo generate-rpm --target "$target" -o "dist/markdown-viewer-${version}-1.${arch}.rpm"
    ;;
esac

cd dist
for f in *; do
  case "$f" in *.sha256) continue ;; esac
  shasum -a 256 "$f" > "$f.sha256"
done
ls -l
