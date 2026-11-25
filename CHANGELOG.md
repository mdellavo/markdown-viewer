# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-11-25

### Added

- Viewer toolbar and keyboard shortcuts (press `?`): switch theme with `t`
  (auto, light, dark) without reloading. Diagrams redraw to match.
- Keyboard navigation: `j`/`k`, `d`/`u`, `g`/`G`, and `n`/`p` for headings.
- Contents sidebar (`o`) that follows your place as you scroll.
- Folder mode: `markdown-viewer folder/` (or no argument) shows the folder's
  README or a file listing, and every Markdown file opened from it is live.
  A file list with a filter (`f`) in the sidebar.
- Live updates scroll to the first change and briefly highlight it.
- GitHub details: alert icons, the heading link icon, a copy button on code
  blocks, and `#gh-dark-mode-only` / `#gh-light-mode-only` images.
- Word count and reading time in the toolbar.
- `--pdf PATH` writes a PDF through a Chromium-based browser, and a print
  layout for printing from the browser.
- `--style github|wide|book|compact` and `--css PATH`.
- A settings file, `~/.config/markdown-viewer/config.toml`, for default
  options.
- The theme and sidebar you choose in the viewer are remembered between runs.
- README screenshots, in light and dark.

### Changed

- `--port` and `--exit-when-closed` no longer require `--watch` when serving
  a folder.

## [0.2.0] - 2026-11-25

### Added

- macOS Finder integration: `--install-finder` adds a "Markdown Viewer" app
  (Open With, double-click, drag and drop) and an "Open in Markdown Viewer"
  Quick Action for the current user; `--uninstall-finder` removes them.
- `--exit-when-closed`: in watch mode, exit once the page has been closed for
  5 seconds. Used by the Finder integration.
- The Homebrew formula explains how to turn on the Finder integration.

## [0.1.0] - 2026-11-25

### Added

- Render a Markdown file to a self-contained HTML page with GitHub styling
  (light, dark, or following the OS setting via `--theme`).
- GitHub Flavored Markdown: tables, task lists, strikethrough, autolinks,
  footnotes, alerts (`> [!NOTE]`), emoji shortcodes, heading anchors, and
  hidden YAML front matter.
- Syntax highlighting for fenced code blocks, done at render time (no JS).
- `--watch`: local server on 127.0.0.1 that live-reloads the page when the
  file changes, keeping the scroll position.
- Images and linked files next to the Markdown file are served in watch mode;
  linked `.md` files render as pages.
- `-o PATH` (or `-o -` for stdout) to write the HTML instead of opening it.
- `--browser`, `--port` and `--no-open` options.
- Content-Security-Policy that blocks scripts embedded in Markdown, and a Host
  header check against DNS rebinding.
- Release workflow publishing macOS and Linux binaries and a Homebrew formula.
- Mermaid diagrams in ```` ```mermaid ```` code blocks, drawn with a bundled
  copy of mermaid.js (12.3.0, works offline) in light or dark to match the
  page. Diagrams update in place in watch mode without flicker; a diagram
  with errors stays visible as source with the error on hover.
- Pages open as a minimal app window (no tabs or address bar) in Chrome,
  Edge, Brave, Chromium and Vivaldi, and in a new window in Firefox, Opera
  and Safari. Other browsers fall back to a new tab. Choose with `--app`
  (default), `--window` or `--tab`.
- `examples/example.md`, a tour of every supported feature.
- Math, typeset with a bundled copy of KaTeX (0.19.1, works offline):
  `$…$`, `$$…$$`, `` $`…`$ `` and ```` ```math ```` blocks, as on GitHub.
  Invalid TeX is shown in red; dollar amounts are left alone.
- README with a Features section listing everything supported.
- Stopping `--watch` with Ctrl-C closes the window markdown-viewer opened.
  Tabs, and pages opened by hand, stay open with a "stopped" banner.

[Unreleased]: https://github.com/mdellavo/markdown-viewer/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/mdellavo/markdown-viewer/releases/tag/v0.1.0
