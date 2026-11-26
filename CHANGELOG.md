# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.7.0] - 2026-11-26

### Added

- CSV and TSV files are shown as tables.
- Click a table's column header to sort by it.
- GitHub references (`#123`, `GH-123`, `owner/repo#123`, `@user`, commit
  hashes) link to the repository found from the git remote, or set with
  `github_repo`; `github_references = false` turns them off.
- `--lint` reports writing problems (skipped heading levels, several
  top-level headings, duplicate headings, images without alt text, empty
  links, code fences without a language); `lint_ignore` skips rules.
- Linux desktop integration: `--install-desktop` adds a "Markdown Viewer"
  application for Open With; the `.deb`, `.rpm` and AUR packages include it.
- Recent files: `--recent`, or no arguments in your home folder, lists the
  files and folders you opened lately (`recent = false` turns it off).

### Changed

- Running `markdown-viewer` with no arguments in your home folder shows the
  Recent page instead of browsing the whole home folder.

## [0.6.0] - 2026-11-25

### Added

- Tick task list boxes in the viewer to update the file.
- `-o DIR` for a folder writes a static site: linked pages, folder
  indexes, the images they use, and the viewer UI with a Files sidebar.
- An Arch Linux package, `markdown-viewer-bin` in the AUR, updated by the
  release workflow.

### Changed

- The workflows use the Node.js 24 versions of the GitHub actions
  (`checkout` v7, `upload-artifact` v7, `download-artifact` v8).

## [0.5.0] - 2026-11-25

### Added

- Text size keys (`+`, `-`, `0`), remembered between runs.
- `--goto FILE:LINE` scrolls a running viewer to a line (or opens one
  there); `--if-running` only scrolls. Zed tasks and a VS Code extension in
  `editors/` use it to show the cursor's line and follow the cursor.
- Code block extras in the info string: `title="…"`, highlighted lines
  (`{3-5}`), `showLineNumbers`; `--line-numbers` numbers every block; a wrap
  button on each block.
- Copy the page (`c`): rich text, with the Markdown as plain text.
- `--check-links` (and `--online` for web links), plus `l` in the viewer.
- Obsidian: `![[Note]]` and `![[Note#Heading]]` embed notes inline; a
  Backlinks tab (`b`) lists the files linking to the page.
- Show what changed since the last git commit (`D`).
- `[TOC]` and `<!-- TOC -->` markers insert a table of contents.

### Changed

- Live updates replace only the blocks that changed, so large files stay
  fast; source positions below an edit are updated in place.
- In an Obsidian vault, the whole vault is watched, and pages update when a
  note they embed or that links to them changes.

## [0.4.0] - 2026-11-25

### Added

- Click to jump to source: double-click a paragraph (or press `e`) to open
  the file in your editor at that line. `--editor` / `editor` setting, with
  Zed, VS Code, Cursor, Sublime Text and others detected automatically.
- Reading positions are remembered per file.
- YAML front matter is shown as a table, like GitHub (`--front-matter hide`
  to hide it, as before).
- Search the text of every file in the Files sidebar (`/`), with results
  linking to the matching line.
- Image and diagram zoom: click to enlarge, scroll to zoom, drag to pan.
- Presentation mode (`s`): full-screen slides split at `---` or headings.
- `--number-headings`.
- Opening a file that's already being viewed brings its window to the front
  instead of starting another viewer.
- Preview standard input (`markdown-viewer -`) and the clipboard
  (`--clipboard`, following it live with `--watch`).
- Obsidian notes, automatically inside a vault: wiki links, embeds,
  callouts (including folded ones), highlights and comments.
- Shell completions (bash, zsh, fish) and a man page, installed by Homebrew
  and the Linux packages.
- Debian and RPM packages for x86_64 and ARM64 on each release.
- Published to crates.io as `markdown-viewer-rs`, with `cargo binstall`
  support.
- Releases update the Homebrew tap automatically.

### Changed

- Every pull request now also builds the release binaries and packages.
- Browser tests of the viewer UI run in CI.

### Fixed

- Pages with many diagrams no longer jump away from the top while loading.
- Diagrams and math failed to load on served pages that had a heading named
  "Mermaid" or "KaTeX".
- Alert icons were missing on served pages.

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

[Unreleased]: https://github.com/mdellavo/markdown-viewer/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/mdellavo/markdown-viewer/releases/tag/v0.1.0
