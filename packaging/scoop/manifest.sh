#!/usr/bin/env bash
# Print the Scoop manifest (bucket/markdown-viewer.json in
# mdellavo/scoop-bucket) for a release.
# Usage: manifest.sh <version> <dir containing markdown-viewer-<target>.zip.sha256 files>
set -euo pipefail

version=$1
dist=$2
repo="https://github.com/mdellavo/markdown-viewer"

sha() { cut -d' ' -f1 "$dist/markdown-viewer-$1.zip.sha256"; }
url() { echo "$repo/releases/download/v$2/markdown-viewer-$1.zip"; }
x64=x86_64-pc-windows-msvc
arm=aarch64-pc-windows-msvc

cat <<JSON
{
    "version": "$version",
    "description": "Render Markdown with GitHub styling and live-reload it in your browser",
    "homepage": "$repo",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "$(url $x64 "$version")",
            "hash": "$(sha $x64)",
            "extract_dir": "markdown-viewer-$x64"
        },
        "arm64": {
            "url": "$(url $arm "$version")",
            "hash": "$(sha $arm)",
            "extract_dir": "markdown-viewer-$arm"
        }
    },
    "bin": "markdown-viewer.exe",
    "notes": "PowerShell completions: . \"\$dir\\\\completions\\\\_markdown-viewer.ps1\" (add to your \$PROFILE)",
    "checkver": {
        "github": "$repo"
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "$(url $x64 '$version')",
                "extract_dir": "markdown-viewer-$x64"
            },
            "arm64": {
                "url": "$(url $arm '$version')",
                "extract_dir": "markdown-viewer-$arm"
            }
        },
        "hash": {
            "url": "\$url.sha256"
        }
    }
}
JSON
