# markdown-viewer

Render a Markdown file with GitHub styling and view it in your browser, with
live reload while you edit. A small, fast, offline alternative to
[grip](https://github.com/joeyespo/grip) and Marked 2.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/screenshot-dark.png">
  <img alt="markdown-viewer showing a document with a contents sidebar, a tip alert, a Mermaid flowchart, a math formula and highlighted Rust code" src="docs/screenshot-light.png">
</picture>

```sh
markdown-viewer --watch README.md
markdown-viewer                     # browse the current folder
```

For a tour of every feature, run:

```sh
markdown-viewer --watch examples/example.md
```

## Features

**Rendering**

- GitHub's look, in light, dark, or following your OS setting. Switch any
  time with <kbd>t</kbd>.
- Built-in styles: `github` (default), `wide` (full window width), `book`
  (serif type, for long reading) and `compact`. Add your own CSS with `--css`.
- GitHub Flavored Markdown:
  - Headings with link anchors (hover a heading for its link icon).
  - Bold, italic and strikethrough.
  - Tables with column alignment. In the viewer, click a column header to
    sort by it (numbers as numbers); click again to reverse, and a third
    time for the file's order.
  - Task lists. When watching, tick a box to update the file: the `[ ]`
    in the Markdown becomes `[x]`.
  - Autolinked URLs and email addresses.
  - Footnotes.
  - Emoji shortcodes such as `:rocket:`.
- GitHub alerts, with GitHub's icons: `> [!NOTE]`, `[!TIP]`, `[!IMPORTANT]`,
  `[!WARNING]` and `[!CAUTION]`.
- Syntax highlighting for fenced code blocks, done when the page is built
  (no JavaScript), with copy and wrap buttons on each block.
- Code block extras, set after the language (GitHub ignores them):
  ```` ```rust title="main.rs" {3-5} showLineNumbers ```` adds a file-name
  title, highlights lines 3 to 5 and numbers the lines. `--line-numbers`
  numbers every block.
- A `[TOC]` line, or `<!-- TOC -->`, becomes a table of contents built from
  the headings (`<!-- TOC max2 -->` for two levels).
- Theme-aware images: `#gh-light-mode-only` and `#gh-dark-mode-only` image
  URLs show only in that theme, as on GitHub.
- Math, typeset with [KaTeX](https://katex.org/):
  - Inline `$…$` and `` $`…`$ ``.
  - Display `$$…$$` and ```` ```math ```` blocks.
  - Invalid TeX is shown in red rather than breaking the page.
  - Dollar amounts like "$5 or $10" are left alone.
- [Mermaid](https://mermaid.js.org/) diagrams in ```` ```mermaid ```` blocks.
  All diagram types are supported: flowchart, sequence, class, state,
  entity-relationship, Gantt, pie, git graph, mindmap, timeline and more. A
  diagram with errors stays visible as source, with the error on hover.
- Raw HTML, as READMEs often use it: centered images, `<details>`, `<kbd>`,
  `<sub>`/`<sup>`.
- YAML front matter shown as a table, as on GitHub (or hidden with
  `--front-matter hide`).
- Numbered headings (1, 1.1, 1.2, …) with `--number-headings`, in the page
  and the contents.
- Obsidian notes, on automatically inside a vault (a `.obsidian` folder):
  - `[[wiki links]]`, including `[[Note|label]]` and `[[Note#Heading]]`,
    found by name anywhere in the vault. Missing notes are marked.
  - `![[image.png]]` embeds (with an optional width, `![[image.png|300]]`).
  - `![[Note]]` shows the note inline, and `![[Note#Heading]]` just that
    section.
  - A **Backlinks** tab listing the notes that link to this one (it also
    works outside a vault, for ordinary Markdown links).
  - Callouts (`> [!tip] Title`), including folded ones (`> [!faq]-`).
  - `==highlights==`, with `%%comments%%` hidden.

- CSV and TSV files are shown as tables, as on GitHub: open one directly,
  or from a folder listing. Columns of numbers are right-aligned, and
  columns sort like any table.
- GitHub references, as GitHub links them: `#123` and `GH-123` (issues and
  pull requests), `owner/repo#123`, `@user` and commit hashes. The
  repository comes from the folder's git remote (GitHub or GitHub
  Enterprise), or the `github_repo` setting. Text in code and links is left
  alone.

**Viewing**

- Opens as a minimal app window (no tabs or address bar), a new window, or a
  tab, in your default browser or the one you choose (see
  [Opening the page](#opening-the-page)).
- Watch mode (`--watch`) updates the page whenever you save:
  - It scrolls to the first thing you changed and briefly highlights it.
    Otherwise your scroll position is kept.
  - Diagrams and math are drawn before the update is shown, so nothing
    flickers.
  - Works with editors that save by replacing the file.
- Folder mode: `markdown-viewer some/folder` (or no argument, for the
  current folder) shows the folder's README, or a list of its files. Every
  Markdown file you open from there updates live too.
- A sidebar with the document's contents, which follows your place as you
  scroll, and, when serving, a list of the folder's Markdown files with a
  filter that also searches their text.
- Double-click a paragraph (or press <kbd>e</kbd>) to open the source in your
  editor at that line (see [Editor](#editor)). The other way round,
  `markdown-viewer --goto file.md:42` scrolls the preview to a line; the
  [Zed and VS Code integrations](editors/) use it to follow your cursor.
- Reopening a file takes you back to where you were reading.
- Recent files: `markdown-viewer --recent`, or `markdown-viewer` with no
  arguments in your home folder, lists the files and folders you opened
  lately, newest first, each a click away.
- Opening a file that's already being viewed brings its window to the front
  instead of opening another.
- Click an image or diagram to zoom; scroll to zoom further and drag to pan.
- Presentation mode (<kbd>s</kbd>): the document as full-screen slides,
  split at `---` rules (or at headings, if there are none).
- Preview standard input (`markdown-viewer -`) or the clipboard
  (`--clipboard`; with `--watch` the page follows the clipboard as it
  changes).
- Larger or smaller text (<kbd>+</kbd> / <kbd>-</kbd>), remembered between
  runs.
- Show what changed since the last git commit (<kbd>D</kbd>): changed blocks
  get a bar in the margin, like an editor's gutter.
- Copy the whole page (<kbd>c</kbd>): rich text for Docs, Slack or email, with
  the Markdown as plain text.
- Check the page's links (<kbd>l</kbd>), or every file's with
  `--check-links` (see [Checking links](#checking-links)).
- Large files stay fast: an edit updates only the blocks that changed.
- Word count and reading time in the toolbar.
- Keyboard shortcuts for everything (see [In the viewer](#in-the-viewer)).
- Your theme and sidebar choices are remembered between runs.
- In watch mode, relative images load, and links to other `.md` files open as
  rendered pages.
- Ctrl-C stops watch mode and closes the window it opened.

**Exporting**

- `-o` writes a single self-contained HTML file (or `-o -` for stdout), with
  any diagrams and math included.
- For a folder, `-o site/` writes a static site: a page for every Markdown
  file, with links between them working, an index for every folder (its
  README or a listing), the images they use, and the viewer UI with a Files
  sidebar. Put it on GitHub Pages or any web server, or open it straight
  from disk.
- `--pdf` writes a PDF using Chrome, Chromium, Edge, Brave or Vivaldi, with
  diagrams and math rendered, in light colours, and without page breaks
  inside code blocks, tables or diagrams.
- Printing from the browser uses the same print layout.

**Finder (macOS)**

- A "Markdown Viewer" app for Markdown files: Open With, double-click (once
  set as the default), or drop files on its icon. See
  [Finder integration](#finder-integration-macos).
- An "Open in Markdown Viewer" Quick Action in Finder's right-click menu.
- Each file opens in its own live-reloading window, and the viewer exits
  when you close the window.

**Linux desktops**

- A "Markdown Viewer" application for Markdown, CSV and TSV files: Open With
  in your file manager, or the default if you choose. The `.deb`, `.rpm` and
  AUR packages include it; otherwise `--install-desktop` adds it (see
  [Linux desktop integration](#linux-desktop-integration)).

**Offline and safe**

- Everything ships inside one binary: stylesheets, fonts, KaTeX and
  mermaid.js. Pages never contact the network, though images that link to
  the web still load. KaTeX and Mermaid are only added to pages that use them.
- Scripts inside the Markdown are blocked (see [Security](#security)).

## Install

With Homebrew (macOS and Linux), including shell completions and a man page:

```sh
brew install mdellavo/tap/markdown-viewer
```

Debian, Ubuntu and Fedora packages (x86_64 and ARM64) are attached to each
[release](https://github.com/mdellavo/markdown-viewer/releases):

```sh
sudo apt install ./markdown-viewer_*_amd64.deb      # Debian, Ubuntu
sudo dnf install ./markdown-viewer-*.x86_64.rpm     # Fedora, RHEL
```

On Arch Linux, from the AUR (prebuilt, x86_64 and ARM64):

```sh
yay -S markdown-viewer-bin    # or paru, or makepkg by hand
```

With Rust's tools (the crate is `markdown-viewer-rs`; the command is still
`markdown-viewer`):

```sh
cargo binstall markdown-viewer-rs    # prebuilt binary
cargo install markdown-viewer-rs     # build from source (see Building)
```

## Usage

```sh
markdown-viewer README.md                # render once and open in your browser
markdown-viewer --watch README.md        # live-reload as the file changes
markdown-viewer README.md -o out.html    # write the HTML to a file
markdown-viewer README.md -o - | less    # ...or to stdout
markdown-viewer README.md --pdf out.pdf  # write a PDF
markdown-viewer docs/ -o site/           # write a folder as a static site
markdown-viewer docs/                    # browse a folder (live)
git show HEAD~1:README.md | markdown-viewer -   # preview standard input
markdown-viewer --clipboard --watch      # follow the clipboard as it changes
markdown-viewer --check-links docs/      # report broken links (exit status 1)
markdown-viewer --lint docs/             # report writing problems (exit status 1)
markdown-viewer data.csv                 # a CSV or TSV file as a table
markdown-viewer --recent                 # files and folders you opened lately
markdown-viewer --goto README.md:42      # scroll a running preview to line 42
```

| Option | Description |
|---|---|
| `-w`, `--watch` | Serve on `127.0.0.1` and reload the page when the file changes |
| `-o`, `--output <PATH>` | Write HTML to `PATH` (`-` for stdout) instead of opening it; for a folder, a static site into the folder `PATH` |
| `--pdf <PATH>` | Write a PDF to `PATH` (needs a Chromium-based browser) |
| `-p`, `--port <N>` | Port for `--watch` (default: any free port) |
| `--browser <NAME>` | Open a specific browser, e.g. `"Google Chrome"` or `firefox` |
| `--app` | Open as a minimal app window with no tabs or address bar (default) |
| `--window` | Open in a normal new browser window |
| `--tab` | Open in a tab of an existing window |
| `--no-open` | Don't open a browser |
| `--theme <auto\|light\|dark>` | Colour theme (default: the one you last chose, else auto) |
| `--style <github\|wide\|book\|compact>` | Page style (default: `github`) |
| `--css <PATH>` | Extra CSS file, applied after the built-in styles |
| `--number-headings` | Number headings (1, 1.1, 1.2, …) |
| `--front-matter <table\|hide>` | Show YAML front matter as a table (default) or hide it |
| `--obsidian`, `--no-obsidian` | Force Obsidian notes on or off (default: on inside a vault) |
| `--editor <COMMAND>` | Editor for double-click-to-source (see [Editor](#editor)) |
| `--clipboard` | Preview the clipboard instead of a file |
| `--line-numbers` | Number the lines of every code block |
| `--goto <FILE:LINE>` | Scroll a running viewer to a line, or open one there |
| `--if-running` | With `--goto`: do nothing if no viewer is showing the file |
| `--check-links` | Check the links in a file or folder and report broken ones |
| `--online` | With `--check-links`: also check web links (needs `curl`) |
| `--lint` | Report writing problems in a file or folder (see [Linting](#linting)) |
| `--recent` | Show the files and folders you opened recently |
| `--exit-when-closed` | With `--watch`: exit once the page has been closed for 5 seconds |
| `--install-finder` | macOS: add the Finder integration for your user |
| `--uninstall-finder` | macOS: remove the Finder integration |
| `--install-desktop` | Linux: add the desktop integration for your user |
| `--uninstall-desktop` | Linux: remove the desktop integration |

Without `--watch`, the page is written to your temp directory and opened; no
server keeps running. Giving a folder (or nothing) always serves it with live
reload, except that with nothing in your home folder you get the Recent
page instead of all of your home folder.

### In the viewer

The toolbar in the top-right corner shows the word count and reading time,
and has buttons for the contents, the file list (when serving), editing the
source, presenting, the theme and help. Hovering a code block shows its copy
button.

| Key | Action |
|---|---|
| <kbd>t</kbd> | Switch theme: auto, light, dark |
| <kbd>o</kbd> | Show or hide the contents |
| <kbd>f</kbd> | Find a file (when serving): type to filter, Enter to open |
| <kbd>/</kbd> | Search the text of every file (when serving) |
| <kbd>e</kbd> | Open the source in your editor here (or double-click a paragraph) |
| <kbd>s</kbd> | Present as slides: arrows or space to move, <kbd>Esc</kbd> to stop |
| <kbd>+</kbd> / <kbd>-</kbd> / <kbd>0</kbd> | Larger / smaller / normal text |
| <kbd>c</kbd> | Copy the page |
| <kbd>b</kbd> | Backlinks: other files that link here (when serving) |
| <kbd>l</kbd> | Check this page's links (when serving) |
| <kbd>D</kbd> | Show what changed since the last git commit (when serving) |
| <kbd>j</kbd> / <kbd>k</kbd> | Scroll down / up |
| <kbd>d</kbd> / <kbd>u</kbd> | Half a page down / up |
| <kbd>g</kbd> / <kbd>G</kbd> | Top / bottom |
| <kbd>n</kbd> / <kbd>p</kbd> | Next / previous heading |
| <kbd>?</kbd> | Show the shortcuts |
| <kbd>Esc</kbd> | Close |

The theme, whether the sidebar is open, and where you were in each file are
remembered for next time. `--theme` overrides the remembered theme for that
run. Pages opened without `--watch` remember them in the browser instead.

### Editor

Double-clicking a paragraph, or pressing <kbd>e</kbd>, opens the file in
your editor at that line (when serving). The editor is the first of:

1. `--editor`, or `editor` in the settings file, e.g. `"zed"` or
   `"code -g {file}:{line}"`.
2. `$MARKDOWN_VIEWER_EDITOR`.
3. `$VISUAL` or `$EDITOR`, if it's a known graphical editor.
4. The first one found of Zed, VS Code, Cursor, Windsurf, VSCodium, Sublime
   Text, TextMate or IntelliJ IDEA.
5. On macOS, the default text editor (without jumping to the line).

Terminal editors such as vim can't be used, as there's no terminal to run
them in.

### Settings file

Put defaults for any of these options in
`~/.config/markdown-viewer/config.toml` (or `$XDG_CONFIG_HOME/markdown-viewer/config.toml`).
Command-line options override it, and a theme you pick in the viewer
overrides its `theme`.

```toml
theme = "auto"          # auto, light or dark
style = "book"          # github, wide, book or compact
css = "~/notes.css"     # extra stylesheet
open = "window"         # app, window or tab
browser = "Firefox"
port = 6419
editor = "zed"          # or "code -g {file}:{line}"
front_matter = "table"  # table or hide
number_headings = true
line_numbers = true     # number every code block
obsidian = false        # default: on inside an Obsidian vault
github_references = false   # default: on in a checkout with a GitHub remote
github_repo = "owner/repo"  # if it isn't found from the git remote
lint_ignore = ["multiple-h1", "duplicate-heading"]   # --lint rules to skip
recent = false          # don't remember opened files for the Recent page
```

Unknown keys are reported as errors, so typos don't go unnoticed. Set
`MARKDOWN_VIEWER_CONFIG` to use a different file.

### Opening the page

By default the page opens as a minimal app window, with no tabs, address bar
or toolbar, so the preview can sit next to your editor. It uses your default
browser, or the one given with `--browser`.

| Browser | Default (`--app`) | `--window` |
|---|---|---|
| Chrome, Edge, Brave, Chromium, Vivaldi | app window | new window |
| Firefox, Opera, Safari | new window (no app mode) | new window |
| Others | new tab | new tab |

When you stop `--watch` with Ctrl-C, a window that markdown-viewer opened
closes itself. Some pages stay open instead, showing a "markdown-viewer
stopped" banner:

- A tab, whether from `--tab` or a page you opened yourself.
- A window you've used to visit another page.

The window size is up to the browser. Chrome ignores size requests when it's
already running.

### Finder integration (macOS)

```sh
markdown-viewer --install-finder
```

This adds two things for your user only, with no admin rights needed:

- `~/Applications/Markdown Viewer.app`, which you can use to open Markdown
  files:
  - **Open With:** right-click a `.md` file > Open With > Markdown Viewer.
  - **Double-click:** to make it the default, use Get Info on a `.md` file >
    Open with > Markdown Viewer > Change All….
  - **Drag and drop:** drop files on the app's icon.
  - **Launched on its own:** it asks you to choose a file.
- A right-click > Quick Actions > **Open in Markdown Viewer** menu item. It
  doesn't change which app opens `.md` files. If it's missing, turn it on in
  Finder's right-click > Quick Actions > Customize…, or in System Settings >
  Keyboard > Keyboard Shortcuts > Services.

Each file opens with `--watch --exit-when-closed`, so it updates as you edit
and quits by itself when you close its window. Output goes to
`~/Library/Logs/markdown-viewer.log`.

The app runs the `markdown-viewer` you installed it from. For Homebrew that's
the stable `bin` link, so upgrades keep working. Remove it with
`markdown-viewer --uninstall-finder`.

### Linux desktop integration

The `.deb`, `.rpm` and AUR packages install a "Markdown Viewer" application
for everyone. Otherwise (Homebrew, a tarball or `cargo install`), add it
for your user:

```sh
markdown-viewer --install-desktop
```

This writes `~/.local/share/applications/markdown-viewer.desktop` and its
icon. Markdown, CSV and TSV files then offer Markdown Viewer under **Open
With** in your file manager. To open Markdown files with it by default:

```sh
xdg-mime default markdown-viewer.desktop text/markdown
```

Each file opens with `--watch --exit-when-closed`, so it updates as you edit
and quits by itself when you close its window. The entry runs the
`markdown-viewer` you installed it from (for Homebrew, the stable `bin`
link). Remove it with `markdown-viewer --uninstall-desktop`.

### Checking links

```sh
markdown-viewer --check-links README.md          # one file
markdown-viewer --check-links docs/              # every Markdown file in a folder
markdown-viewer --check-links docs/ --online     # also request web links
```

It reports each broken link as `file:line: target (reason)` and exits with
status 1 if there are any, so it works in CI. It checks relative files and
images, `#anchors` (in the same file or another one), and wiki links in an
Obsidian vault. Web links are only checked with `--online`, which uses
`curl`.

### Linting

```sh
markdown-viewer --lint README.md                 # one file
markdown-viewer --lint docs/                     # every Markdown file in a folder
markdown-viewer --lint --check-links docs/       # both at once
```

Like `--check-links`, it reports `file:line: problem (rule)` and exits with
status 1 if there are any. The rules:

| Rule | Checks that |
| --- | --- |
| `heading-increment` | heading levels go down one at a time (no `#` then `###`) |
| `multiple-h1` | there's one top-level (`#`) heading |
| `duplicate-heading` | no two headings under the same heading have the same text (the second's link would end in `-1`) |
| `image-alt` | images have alt text, for screen readers |
| `empty-link` | links have text and a target |
| `code-language` | fenced code blocks say their language, for highlighting |

Skip rules with `lint_ignore` in the [settings file](#settings-file).

### Editors

`markdown-viewer --goto file.md:42` scrolls a running viewer to line 42, or
opens one there. Zed tasks and a VS Code extension that use it are in
[`editors/`](editors/): a key to show the cursor's line in Zed, and
following the cursor in VS Code.

## Building

You need Rust 1.91 or newer. Install it with [rustup](https://rustup.rs/), or
run `rustup update` to upgrade. No other tools or system libraries are needed:
the stylesheets, fonts, icons, KaTeX and mermaid.js are built into the binary.

```sh
git clone https://github.com/mdellavo/markdown-viewer
cd markdown-viewer
cargo build --release
```

The binary is `target/release/markdown-viewer`. To put it on your `PATH`
(in `~/.cargo/bin`):

```sh
cargo install --path .
```

To run it straight from the source tree without installing:

```sh
cargo run -- --watch examples/example.md
```

### Packages

Release packages are built by `packaging/package.sh`, the script the
release workflow uses. Build for a target first, then package it. The output
goes to `dist/`, with a `.sha256` checksum next to each file.

```sh
target=$(rustc -vV | sed -n 's/^host: //p')    # e.g. aarch64-apple-darwin
cargo build --release --locked --target "$target"
packaging/package.sh "$target"
```

This always makes `markdown-viewer-<target>.tar.gz`, which holds the binary,
README, LICENSE, CHANGELOG, shell completions and man page. On Linux it also
makes a `.deb` and an `.rpm`, which need two more tools:

```sh
cargo install cargo-deb cargo-generate-rpm
```

Their contents are set in `Cargo.toml`, under `[package.metadata.deb]` and
`[package.metadata.generate-rpm]`. To check a `.deb`:

```sh
sudo dpkg -i dist/markdown-viewer_*.deb && markdown-viewer --version
```

The Homebrew formula and the AUR package download a published GitHub
release, so they're generated from that release's checksums:

```sh
v=0.5.0
gh release download v$v --pattern '*.sha256' --dir /tmp/sums

# Homebrew: the formula for mdellavo/homebrew-tap
packaging/homebrew/formula.sh $v /tmp/sums > markdown-viewer.rb

# AUR: PKGBUILD and .SRCINFO for markdown-viewer-bin, then build and
# install it in an Arch Linux container to check it (needs Docker)
packaging/aur/pkgbuild.sh $v /tmp/sums dist/aur
docker run --rm -v "$PWD/dist/aur:/pkg:ro" -v "$PWD/packaging/aur/test.sh:/test.sh:ro" \
  archlinux:latest bash /test.sh /pkg
```

For crates.io, `cargo package --locked` builds and checks the crate that
`cargo publish` would upload. It needs everything committed; add
`--allow-dirty` to try it with uncommitted changes.

### Development

```sh
cargo test                                  # unit, end-to-end and browser tests
cargo clippy --all-targets -- -D warnings   # lints, as CI runs them
cargo fmt                                   # format
```

The browser tests drive the viewer in headless Chrome. They're skipped if no
Chrome-family browser is installed, except in CI, where they must run.

The shell completions (`completions/`) and man page (`man/`) are generated
from the command-line options, and a test fails if they're out of date.
Regenerate them with:

```sh
UPDATE_GENERATED=1 cargo test --test cli generated_files
```

If a change alters rendered output on purpose, the snapshot test fails.
Review and accept the new snapshot with
[cargo-insta](https://insta.rs/docs/cli/) (`cargo install cargo-insta`):

```sh
cargo insta review
```

Releases are built by GitHub Actions when a `v*` tag is pushed; see
[AGENTS.md](AGENTS.md#releasing).

## Security

- **Raw HTML:** allowed, because READMEs often use it. The page's
  Content-Security-Policy blocks any script that comes from the Markdown,
  and KaTeX and Mermaid run in their strict modes.
- **Watch server:** listens on loopback only and serves only files inside
  the Markdown file's folder (or the folder you gave it). The file list
  skips hidden folders and doesn't follow symlinked folders.
- **Other websites:** requests addressed to any other hostname are rejected,
  so a website can't read the files being served. Saving preferences also
  needs a custom header, which other websites can't send to it.
- **Editor and reading positions:** the page can ask the server to open a
  file in your editor or to remember a position, but only for Markdown files
  inside the served folder, and only with a custom header other websites
  can't send. The editor runs without a shell.
- **Ticking task boxes:** the one way the page changes your files. The
  server only changes a `[ ]` or `[x]` in a Markdown file inside the served
  folder, only if that line still holds a task item in the state the page
  showed, and only with the same custom header. If the file is open in an
  editor with unsaved changes, the editor will see it changed on disk.
- **Static sites:** `-o DIR` only writes into an empty folder or one an
  earlier export wrote, and only copies files inside the exported folder
  that a page links to.
- **Recent files:** the list stays on your computer, in the state file
  next to your reading positions. The Recent page is served from your home
  folder, the same as browsing it would be. Set `recent = false` to stop
  recording.
- **GitHub references:** only change links on the page; nothing is fetched
  from GitHub.
- **PDF export:** runs the browser headless with a throwaway profile, so it
  never touches your own browser profile.

## License

MIT. Includes:

- [github-markdown-css](https://github.com/sindresorhus/github-markdown-css)
  (MIT, see `assets/LICENSE-github-markdown-css`).
- [KaTeX](https://github.com/KaTeX/KaTeX) (MIT, see `assets/LICENSE-katex`).
- [Primer Octicons](https://github.com/primer/octicons) (MIT, see
  `assets/LICENSE-octicons`).
- [Mermaid](https://github.com/mermaid-js/mermaid) (MIT, see
  `assets/LICENSE-mermaid`).
