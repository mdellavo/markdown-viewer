# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project uses [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Faster pages: text shows before diagram, math and chart libraries load;
  diagrams on screen are drawn first and are remembered by the browser, so
  reloads and returning to a page show them at once; the first page is
  rendered while the browser opens; unchanged code blocks and drawings
  aren't redone on each save; live updates follow saves sooner; and served
  pages share their styles and viewer script, which the browser keeps.

### Added

- Tags: front matter `tags:` and, in Obsidian notes, `#tags` in the text
  are shown as chips, and a Tags sidebar tab (<kbd>#</kbd>) lists every tag
  in the folder with the files using it.
- More drawings from code blocks, made when the page is built (offline, in
  exports and PDFs): ```` ```bob ```` (Svgbob ASCII art), ```` ```pikchr ````,
  ```` ```wavedrom ```` timing diagrams, ```` ```qr ```` codes, and
  ```` ```fen ````/```` ```pgn ```` chess boards. Click a diagram to enlarge
  it.
- ```` ```dot ````, ```` ```plantuml ```` and ```` ```d2 ```` diagrams, drawn
  by Graphviz, PlantUML or D2 when installed.
- Vega-Lite charts (```` ```vega-lite ````), sheet music in ABC notation
  (```` ```abc ````) and molecules from SMILES (```` ```smiles ````), with the
  libraries bundled and loaded only on pages that use them.
- GPX, KML and WKT maps, OBJ, PLY and OFF models, and model and map files
  shown as images (`![part](part.stl)`, binary STL too).
- glTF (`.gltf`, `.glb`, ```` ```gltf ````) and 3MF models.
- Molecules: molfiles as structure diagrams; 3D molfiles, XYZ and PDB as
  balls and sticks that turn when dragged.
- Calendars: ```` ```ics ```` blocks and `.ics` files as a list of events.
- AsciiMath in ```` ```asciimath ```` blocks.
- Video and audio linked like images become players.
- Colour swatches after colours in inline code (`` `#0969da` ``), as on
  GitHub.

## [0.13.0] - 2026-11-27

### Added

- GeoJSON and TopoJSON maps (```` ```geojson ````, ```` ```topojson ````)
  and STL 3D models (```` ```stl ````), drawn from the code blocks GitHub
  draws. Maps show points, lines and areas over a latitude and longitude
  grid (no web map tiles), with properties on hover and GitHub's style
  properties; click to enlarge. Models are shaded, and turn when dragged.
  Both are drawn when the page is built, so exports and PDFs have them.
- A rendered diff: `--diff REV` shows a file with what changed since a git
  revision marked, added text in green and removed text struck through in
  red, with a word count summary. It follows the file as you save, and
  works with `-o` and `--pdf`.
- Definition lists, `^superscript^`, `~subscript~` and abbreviations
  (`*[HTML]: HyperText Markup Language`).

### Changed

- A single-tilde `~text~` is now subscript. On GitHub it strikes through;
  `~~text~~` still does.

## [0.12.0] - 2026-11-27

### Added

- A **Recent** tab in the sidebar (<kbd>h</kbd>) lists the files and
  folders you opened lately on every served page. Files outside the
  folder being viewed open in a viewer of their own.
- Remove an item from the recent list with its × button, on the Recent
  page or in the sidebar.

### Changed

- Sidebar tabs are icons, with the selected tab's name beside its icon,
  so all four fit (names show on hover).
- The Recent page is a list of rows grouped by when you opened them, with
  icons for files, folders and tables, and it updates itself when the list
  changes.

## [0.11.0] - 2026-11-26

### Added

- Back, forward and reload buttons at the start of the toolbar (app
  windows have no browser controls), with <kbd>[</kbd>, <kbd>]</kbd> and
  <kbd>r</kbd> as shortcuts.
- Links to web pages, and to local files the viewer doesn't show as pages
  (images, PDFs and so on), open in a new browser window. Links between
  documents and within the page stay in the viewer's window.

### Changed

- The Files sidebar is a folder tree, like an editor's file explorer:
  folders open and close with a click and stay as you left them while you
  move between pages, the current file's folders start open, and a folder
  holding just one folder shows as one row (`.github / workflows`).
  Filtering shows the matches with their folders open.

### Fixed

- Saved preferences, reading positions and the Recent list could be lost
  when two viewers or two requests saved at once. Saves now hold a lock and
  replace the file in one step; the settings file is also replaced in one
  step.
- Folder listings and exported sites linked wrongly to names containing
  `#`, `?` or `%`.
- Compare to git (`D`) marked every line as new outside a repository when
  git spoke a language other than English.
- A folder export with both `index.md` and `README.md` lost `index.md`'s
  page; `index.md` is now the folder's page, and README gets its own.
- `--pdf` deleted an existing PDF before knowing the new one would be
  written; a failed run now leaves it alone.
- Changing the `obsidian` setting in the settings dialog half-applied it
  to the running viewer. It now applies from the next run, as the dialog
  says.

### Security

- A `<base>` tag in a document could make served pages load Mermaid or
  KaTeX from another site, which could then change settings and run the
  editor command. Pages now refuse `<base>` and load the libraries from
  the viewer's own address.
- HTML and SVG files in a served folder ran their scripts on the viewer's
  address, where they could reach its endpoints. Files from the folder are
  now served sandboxed (PDFs excepted, which run no page scripts).
- A request for `//host` on a folder named `host` redirected to that
  site; folder redirects now stay on the viewer.

## [0.10.0] - 2026-11-26

### Added

- View a page's Markdown source (`v`, or the code button in the toolbar):
  syntax highlighted with line numbers, updated as the file changes.
  Switching keeps your place, and double-clicking a line opens the
  editor there. It works for web addresses, standard input and the
  clipboard too, and copying those pages now includes their Markdown. It
  opens at once with the plain source, then highlights it (with a busy
  indicator); `source_highlighting = false` (also in the settings dialog)
  turns highlighting off.

## [0.9.0] - 2026-11-26

### Added

- Web addresses: `markdown-viewer https://…/file.md`, including GitHub and
  GitLab file pages and repository addresses (which show the README).
  Relative links and images point back at the web, and open pages
  re-fetch every 10 seconds. Fetched with `curl`.
- Hide the toolbar's word count and reading time with <kbd>w</kbd> (or by
  clicking it); the choice is remembered. `word_count = false` (also in the
  settings dialog) hides it by default.

- A settings dialog (<kbd>,</kbd> or the gear in the toolbar) that changes
  the settings file and applies the change at once: rendering options
  update the page, the style and custom CSS reload it, and launch options
  are used next time. Changes keep the file's comments and are only saved
  if valid.

- An About section in the help (`?`): version and release notes, commit,
  build target, compiler, bundled Mermaid and KaTeX, links to the
  changelog, issues and license, and (when serving) the settings and state
  files, editor and served folder, with a button to copy it all.
- `markdown-viewer --version` shows the commit and build details (`-V`
  stays short).
- Credits in the help: the projects included in every page (Mermaid,
  KaTeX, GitHub's Markdown CSS, Octicons) and the Rust crates
  markdown-viewer is built with, with versions, licenses and links.
- `examples/slides.md`: an example deck for presentation mode.
- `examples/vault`: an example Obsidian vault with wiki links, embeds,
  callouts, highlights and comments.

### Changed

- Opening a page without `--watch` now serves it from a viewer in the
  background, so the settings dialog, live reload and everything else a
  served page has work there too. The command returns at once, and the
  viewer stops when the window closes. `--no-open` (without `--watch`)
  still writes a standalone file, and `--port` now works without `--watch`.
- Watch mode stops by itself once the window (or tab) it opened is closed:
  the server shuts down and the command exits after 5 seconds without a
  page. `--keep-running` or `exit_when_closed = false` keeps serving until
  Ctrl-C; `--no-open` never stops by itself.
- The `[TOC]` table of contents is a card with a "Contents" title and
  quieter, nested links. It folds: the whole list, or any section with
  sections under it. Folds are kept across live updates and opened for
  printing.

### Fixed

- Diagrams in presentation mode (s) were shrunk to a 16-pixel square by the
  rule for toolbar icons; they now show, sized to fill the slide. Math
  symbols drawn as SVG (such as square roots) were affected too.

## [0.8.0] - 2026-11-26

### Added

- Windows support: release zips for x64 and ARM64, a Scoop bucket
  (`scoop bucket add mdellavo https://github.com/mdellavo/scoop-bucket`),
  and `cargo binstall` downloads. On Windows the default browser comes from
  the registry, `--browser` finds Chrome, Edge, Brave, Vivaldi or Firefox in
  their install folders, `--pdf` uses Chrome or Edge, editors installed as
  `.cmd` files (VS Code, Cursor) work for click-to-source, `--clipboard`
  uses PowerShell, and settings live in `%APPDATA%\markdown-viewer`.
- PowerShell completions (`completions/_markdown-viewer.ps1`).

### Changed

- Line endings are LF on every platform (`.gitattributes`).

## [0.7.1] - 2026-11-26

### Added

- [Writing custom CSS](docs/custom-css.md), a guide to `--css`, and an
  example theme, `examples/custom.css`.

### Fixed

- Custom CSS can override the theme's colour variables (for example
  `.markdown-body { --fgColor-accent: … }`); before, the built-in theme
  rules won.

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

[Unreleased]: https://github.com/mdellavo/markdown-viewer/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.12.0...v0.13.0
[0.12.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.9.0...v0.10.0
[0.9.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.8.0...v0.9.0
[0.8.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.7.1...v0.8.0
[0.7.1]: https://github.com/mdellavo/markdown-viewer/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mdellavo/markdown-viewer/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/mdellavo/markdown-viewer/releases/tag/v0.1.0
