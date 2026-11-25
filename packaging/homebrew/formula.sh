#!/usr/bin/env bash
# Print the Homebrew formula for a release.
# Usage: formula.sh <version> <dir containing markdown-viewer-<target>.tar.gz.sha256 files>
set -euo pipefail

version=$1
dist=$2
repo="https://github.com/mdellavo/markdown-viewer"

sha() { cut -d' ' -f1 "$dist/markdown-viewer-$1.tar.gz.sha256"; }
url() { echo "$repo/releases/download/v$version/markdown-viewer-$1.tar.gz"; }

cat <<RUBY
class MarkdownViewer < Formula
  desc "Render a Markdown file with GitHub styling and live-reload it in your browser"
  homepage "$repo"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$(url aarch64-apple-darwin)"
      sha256 "$(sha aarch64-apple-darwin)"
    end
    on_intel do
      url "$(url x86_64-apple-darwin)"
      sha256 "$(sha x86_64-apple-darwin)"
    end
  end

  on_linux do
    on_arm do
      url "$(url aarch64-unknown-linux-gnu)"
      sha256 "$(sha aarch64-unknown-linux-gnu)"
    end
    on_intel do
      url "$(url x86_64-unknown-linux-gnu)"
      sha256 "$(sha x86_64-unknown-linux-gnu)"
    end
  end

  head do
    url "$repo.git", branch: "main"
    depends_on "rust" => :build
  end

  def install
    if build.head?
      system "cargo", "install", *std_cargo_args
    else
      bin.install "markdown-viewer"
    end
  end

  test do
    (testpath/"test.md").write "# Hello"
    assert_match "<h1", shell_output("#{bin}/markdown-viewer #{testpath}/test.md -o -")
  end
end
RUBY
