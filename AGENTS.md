# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`markdown-viewer` is a Rust CLI that renders a Markdown file to a GitHub-styled
HTML page and opens it in the browser. `--watch` (or giving a folder) serves
it locally and pushes updates to the page when files change. Pages carry a
small viewer UI: toolbar, contents/files sidebar, keyboard shortcuts.

## Layout

- `src/main.rs`: CLI (clap), merging options with the settings file
  (`Settings`), `-o`, `--pdf` and serving. Opening a page without
  `--watch` re-runs the command detached as a background viewer
  (`launch_in_background`: `--watch --exit-when-closed` plus the hidden
  `--background`), relays its first line (the address) and returns; its
  stderr goes to `viewer.log` next to the state file. `--no-open` without
  `--watch` still writes a standalone page to the temp dir.
- `src/config.rs`: the settings file (`Config`, TOML, unknown keys are
  errors) and remembered viewer preferences (`Prefs`: theme and sidebar,
  JSON state file). `MARKDOWN_VIEWER_CONFIG` / `MARKDOWN_VIEWER_STATE`
  override their paths; the tests always set both.
- `src/theme.rs`: page CSS for all themes at once, switched by
  `<html data-mdv-theme>` (it splits GitHub's stylesheet into shared rules
  and per-scheme variables), the print layout, and `--style` CSS. Theme
  selectors are wrapped in `:where()` so `--css` stylesheets can override
  the variables with plain `.markdown-body` rules; keep it that way (a
  browser test checks `examples/custom.css`).
- `docs/custom-css.md`: the guide to `--css`. Update its class and variable
  tables when page markup or styles change.
- `src/github.rs`: post-processing to match GitHub's HTML: alert icons and
  heading link icons.
- `src/docs.rs`: `Docs`, every document being viewed (Markdown files and
  folder listings) as a `watch` channel of rendered HTML. It watches only
  the folders of viewed documents, non-recursively, and lists the folder's
  Markdown files for the sidebar.
- `src/pdf.rs`: `--pdf` via a headless Chromium-based browser with a
  throwaway profile.
- `src/frontmatter.rs`: YAML front matter as a GitHub-style table.
- `src/codeblocks.rs`: code block extras from the info string (`title`,
  `{ranges}`, `showLineNumbers`); splits highlighted code into
  `.mdv-line` spans, closing and reopening spans across line breaks.
- `src/toc.rs`: `[TOC]` / `<!-- TOC [maxN] -->` markers.
- `src/gitdiff.rs`: lines changed since `HEAD`, via `git diff -U0`.
- `src/tables.rs`: CSV and TSV files as HTML tables (`docs::is_viewable`
  is Markdown or these; `markdown_files` stays Markdown-only for search,
  links and wiki links, `document_files` adds them for the file list and
  sites).
- `src/references.rs`: GitHub references (`#123`, `@user`, commits),
  linked in rendered HTML outside links, code and math. The repository
  comes from the git remote (`RenderOptions::github`).
- `src/lint.rs`: `--lint` rules (`RULES`), run on HTML rendered with source
  positions; shares `check()` in main.rs with `--check-links`.
- `src/desktop.rs`: `--install-desktop` (freedesktop entry and icon in
  `$XDG_DATA_HOME`). `packaging/linux/markdown-viewer.desktop` is the
  packaged copy; a test keeps it equal to `desktop::entry`.
- `src/recent.rs`: recently opened files (`Prefs::recent`, recorded on
  launch, never changed by pages) and the Recent page, a virtual document
  served from the home folder.
- Sortable tables live in `ui.js`; like callout folds, `live.js` asks it to
  restore the file's row order before an update (`mdvSort.save`) and
  re-sorts after, so source positions stay right.
- `src/remote.rs`: web addresses: GitHub/GitLab page and repository
  addresses to raw files (`Remote::new`), fetching with `curl`, and
  `absolutize` (relative links against the page, images against the raw
  file). In main.rs a `Source::Url` is a virtual document served from an
  empty temp folder (`web_root`), re-fetched every 10 s (`follow_url`).
- `src/tasks.rs`: ticking task boxes (`POST /__mdv/task`): changes the
  `[ ]` at the item's source position, only if it is still a task item in
  the state the page showed.
- `src/site.rs`: `-o DIR` for a folder. Pages are standalone pages with
  `PageOptions::site_root` set: libraries load from `_mdv/` at the site
  root, and `ui.js` gets a Files sidebar from `_mdv/files.js`. Links are
  rewritten (`.md` → `.html`, folders → `index.html`) and linked files
  copied. Pages must keep working from `file://` (a browser test checks).
- `src/links.rs`: `--check-links` (local files, anchors, wiki links;
  web links through `curl` with `--online`) and the `/__mdv/check` endpoint.
- `src/obsidian.rs`: resolving `[[wiki links]]` and `![[embeds]]` (by name
  anywhere in the vault, closest first). Obsidian mode turns on inside a
  vault; the served root then becomes the vault so linked notes open.
- `src/platform.rs`: everything OS-specific: `Canonical::canonical()`
  (use it instead of `canonicalize()`: Windows' `\\?\` paths break
  comparisons and URLs), `file_url`, `home()` (`HOME`, else
  `USERPROFILE`), `detach` for launched programs, `find_program` (finds
  `.cmd` editors on Windows; Rust's std quotes their arguments safely), and
  Windows browser lookup (registry and install folders).
- `src/editor.rs`: opening a file at a line in the user's editor
  (click-to-source). Commands run without a shell.
- `src/instances.rs`: registry of running viewers (next to the state file),
  so a second launch for the same target reuses the first
  (`/__mdv/whoami`), and `browser::focus` brings its window forward.
  `--goto` finds a viewer serving the file or a folder containing it and
  posts to `/__mdv/goto`; the server broadcasts a `goto` event to pages.
- `editors/`: Zed tasks and key binding, and a dependency-free VS Code
  extension (tested with a stand-in `vscode` module:
  `node editors/vscode/test/extension.test.js`, also run in CI).
- `src/browser.rs`: opens the page as an app window (`--app=` for Chromium),
  a new window (`--new-window` for Chromium/Firefox, AppleScript for Safari)
  or a tab, falling back in that order. Finds the default browser via
  LaunchServices (macOS) or `xdg-settings` (Linux). Chrome ignores
  `--window-size` when already running, so window size isn't configurable.
- `src/render.rs`: `Renderer` (comrak with GFM and math options, syntect
  highlighting via CSS classes), the ```` ```mermaid ```` fence plugin, and
  the page template (`PageOptions`), including the CSP. `RenderOptions`:
  `sourcepos` (served pages only: `data-sourcepos` drives click-to-source,
  reading positions and `#mdv-line-N` links), `obsidian`, `front_matter`.
- `src/server.rs`: axum server: pages for any Markdown file or folder under
  the root (folders redirect to a trailing slash, then show their README or
  a listing), `/__mdv/events?doc=` live-update stream per document
  (Server-Sent Events), `POST /__mdv/prefs` (needs an `X-MDV` header),
  `/__mdv/files`, `/__mdv/{mermaid,katex}.js`, `/__mdv/katex.css`, and
  static files.
- Settings dialog (`/__mdv/settings`): `config::update_file` edits one key
  with `toml_edit` (keeping comments) and writes only if the file still
  parses and `ServeOptions::reconfigure` (built in main.rs: the new file
  merged with the run's command-line options, as at startup) succeeds.
  The result goes into `Live` (page template, custom CSS, editor) and, if
  the render options changed, `Docs::set_renderer`, which re-renders open
  pages. Page-level changes answer `reload: true`. New settings need a
  field in `Config`, `Settings`, `overridden_settings` and the dialog's
  `SETTINGS` list in ui.js.
- Shutdown: on Ctrl-C, `main.rs` calls `ServerHandle::announce_shutdown`,
  the event stream sends a `close` event, and `live.js` closes the window if
  the page was rendered with `close_on_exit` (only for windows we opened),
  otherwise it shows a banner. The server waits 300 ms for this before
  exiting. Never kill the browser process: it may hold the user's other
  windows.
- `src/finder.rs` (macOS only): `--install-finder` builds
  `~/Applications/Markdown Viewer.app` with `osacompile` (an AppleScript
  droplet that runs `markdown-viewer --watch --exit-when-closed <file>` in
  the background), then swaps in our icon, sets Markdown document types with
  `plutil`, re-signs ad hoc and registers it with `lsregister`. It also
  writes a Quick Action workflow to `~/Library/Services` that calls
  `open -a "Markdown Viewer"`. The app's icon source is
  `assets/macos/AppIcon.svg`; `AppIcon.icns` is generated from it (render at
  1024px, then `sips` and `iconutil`).
- Stopping when the window closes (`exit_policy` in main.rs): the server
  counts connected pages (`ServerHandle::abandoned`) and exits 5 s after
  the last one leaves. On by default when we opened the page; off with
  `--no-open`, `--keep-running` or `exit_when_closed = false`.
  `--exit-when-closed` forces it (even with `--no-open`) and also gives up
  after 60 s if no page ever connects.
- `assets/`: GitHub CSS, `ui.js` and `ui.css` (the viewer UI), `live.js`,
  `mermaid-init.js`, `mermaid.min.js.gz`, `math-init.js`, `katex/` (KaTeX
  JS, CSS and woff2 fonts) and `octicons/` (GitHub icons, MIT), embedded with
  `include_str!`/`include_bytes!`. Everything ships inside the binary; there
  are no runtime files.
- `build.rs`: inlines KaTeX's woff2 fonts into its CSS as `data:` URIs, so one
  stylesheet works both inlined and served. Also sets `MDV_*` build
  details (git commit and date, target, profile, rustc; the commit comes
  from `.cargo_vcs_info.json` in crates.io builds).
- `src/about.rs`: `--version` (long form) and the help's About section:
  `build_json()` goes into every UI page (`data-about`; nothing local), and
  `/__mdv/about` adds local details for served pages only. `docs_command`
  drops the long version so the man page doesn't change per commit.
- `assets/credits.json`: the direct Rust dependencies (and a total count)
  for the help's Credits, generated from `cargo metadata` by the
  `generated_files` test. After changing dependencies:
  `UPDATE_GENERATED=1 cargo test --test cli generated_files`.
- Page scripts cooperate through a few globals and events:
  - `window.mdvEnhancers`: `render(root)` functions (Mermaid, math, copy
    buttons). `live.js` runs them all on new content before swapping it in.
    Follow the same pattern for any new client-side rendering.
  - `window.mdvRetheme`: functions to redraw for a new theme (Mermaid).
  - `mdv:updated`: dispatched by `live.js` after an update, with
    `detail.changed`, the first changed block. `ui.js` rebuilds the contents
    and word count, refreshes git marks, and scrolls to the change.
  - `mdv:goto`: `live.js` received a `--goto` for this page.
- Live updates are block patches: `live.js` keeps the server's raw blocks
  (keys without `data-sourcepos`), matches them from both ends, enhances and
  replaces only the changed range, and re-syncs positions below it. This
  needs page blocks to map 1:1 to the server's top-level elements, so
  enhancers must replace a top-level element with exactly one element.
- `window.mdvInitial`: promises for the page's own first render;
  `live.js` waits for them before comparing, and never jumps on the first
  sync after connecting.
- Callout folding is reader state: `ui.js` records the rendered fold in
  `data-mdv-fold-orig`, and `live.js` ignores and then restores it around
  updates.
- Library checks must test for the library's functions (e.g.
  `typeof window.mermaid.initialize`), never just `window.mermaid`: an
  element with that id also appears as a global.
- `ui.js` loads before the enhancer scripts, so a remembered theme is set
  before diagrams are drawn. Its elements live outside `.markdown-body` (live
  updates replace that) and carry the `mdv-ui` class (hidden when printing).
- `tests/cli.rs`: end-to-end tests against the built binary, including an
  insta snapshot of `tests/fixtures/gfm.md` and a check that every feature in
  `examples/example.md` still renders.
- `examples/`: try-it-yourself files, each with a render test in
  `tests/cli.rs`:
  - `example.md`: the feature tour (GFM, alerts, raw HTML, math, all
    Mermaid diagram types, code block extras, `[TOC]`, GitHub references).
    It links to the others.
  - `slides.md`: a deck for presentation mode.
  - `vault/`: an Obsidian vault (`.obsidian/app.json` marks it) with wiki
    links, embeds, callouts, highlights and comments.
  - `cities.csv` (tables), `other.md` (linked pages), `custom.css`
    (`--css`).
- `docs/screenshot.md` and `docs/screenshots.sh`: the README screenshots.
  Run `cargo build && docs/screenshots.sh` after visible UI changes and
  check the images before committing them.
- `docs/demo.sh`: the README demo GIFs (`docs/demo-{light,dark}.gif`), an
  editor and the viewer side by side. `tests/demo.rs` (an ignored test)
  records the frames: it types `docs/demo/typed.md` into a copy of
  `docs/demo/notes.md` on the stage `docs/demo/desktop.html`, saving at
  each `<!-- save -->`; ffmpeg makes the GIFs. Needs Chrome and ffmpeg.
- `packaging/package.sh`: packages a release build (tarball with
  completions and man page; `.deb` and `.rpm` on Linux), used by both the
  release workflow and CI.
- `packaging/homebrew/formula.sh`: generates the Homebrew formula for a release.
- `packaging/package.ps1`: the Windows counterpart of `package.sh` (a zip).
- `packaging/scoop/manifest.sh`: generates the Scoop manifest for
  `mdellavo/scoop-bucket`.
- `packaging/aur/pkgbuild.sh`: generates the AUR package
  (`markdown-viewer-bin`: PKGBUILD and .SRCINFO) from a release's
  archives; `packaging/aur/test.sh` builds and installs it in an
  `archlinux` container (CI does this for the latest release).
- `.github/workflows/`: `ci.yml` (fmt, clippy, tests on macOS, Linux and
  Windows, release builds, the AUR package) and `release.yml` (binaries,
  GitHub release, Homebrew tap, crates.io, AUR, Scoop). Steps run in bash,
  Git Bash on Windows.
- Windows: check locally with
  `cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings`;
  behaviour is only tested in CI. Test helpers `fake_editor` and `file_url`
  (tests/common) keep tests cross-platform.

## Commands

```sh
cargo build
cargo test                       # unit + end-to-end tests
cargo clippy --all-targets -- -D warnings
cargo fmt
cargo insta review               # after an intended rendering change
cargo run -- --watch tests/fixtures/gfm.md
```

## Rules

- Keep it offline: no CDN links or network fetches in generated pages.
- The only scripts in pages are `ui.js` (not in `-o`/`--pdf` exports),
  `live.js` (watch mode), Mermaid and KaTeX (each an init script plus the
  library, only on pages that need them). Any script must carry the per-page
  CSP nonce. Standalone pages inline the libraries; served pages fetch them
  from `/__mdv/`.
- Watch folders, not files, because editors often save by renaming a temp
  file over the original. Keep it that way.
- `server::resolve` must never serve a path outside the root; extend its
  test when touching it. The file list must not follow symlinked folders.
- End-to-end and browser tests must use the `mdv()` helper (tests/common),
  so they never read or write the user's settings and preferences.
- Browser tests (`tests/browser.rs`) cover the viewer UI; add one for any new
  interaction. They skip without Chrome, except in CI
  (`MARKDOWN_VIEWER_BROWSER_TESTS=required`).
- After changing options or dependencies, regenerate completions, the man
  page and the credits:
  `UPDATE_GENERATED=1 cargo test --test cli generated_files`.
- If `cargo build` stops picking up source changes ("Fresh" after edits),
  run `cargo clean -p markdown-viewer-rs`.
- Run fmt, clippy and tests before finishing a change.
- Keep `README.md` up to date in the same change: every user-visible
  feature, option or behaviour change must be reflected in its **Features**
  list and, where relevant, Usage, the options table, Building or Security.
  Remove anything that's no longer true.
- Add user-visible changes to `CHANGELOG.md` under `[Unreleased]`.
- Show every new rendering feature or file type in `examples/`, and
  extend that example's render test: add it to `example.md`, or to the
  example it belongs with (`slides.md` for presentation, `vault/` for
  Obsidian). If none fits, add a new example, link it from `example.md`
  and the README, and give it a test. Change or remove examples when a
  feature changes.

## Updating Mermaid

```sh
v=X.Y.Z
curl -sSL https://registry.npmjs.org/mermaid/-/mermaid-$v.tgz | tar xz -C /tmp
gzip -9 -n -c /tmp/package/dist/mermaid.min.js > assets/mermaid.min.js.gz
cp /tmp/package/LICENSE assets/LICENSE-mermaid
echo $v > assets/MERMAID_VERSION
cargo test   # includes a check that the bundle is safe to inline
```

Check the tarball against the `integrity` hash from
`https://registry.npmjs.org/mermaid/$v` before using it.

## Updating KaTeX

```sh
v=X.Y.Z
curl -sSL https://registry.npmjs.org/katex/-/katex-$v.tgz | tar xz -C /tmp
cp /tmp/package/dist/katex.min.js /tmp/package/dist/katex.min.css assets/katex/
rm assets/katex/fonts/*.woff2 && cp /tmp/package/dist/fonts/*.woff2 assets/katex/fonts/
cp /tmp/package/LICENSE assets/LICENSE-katex
echo $v > assets/katex/VERSION
cargo test   # build.rs fails if a font isn't inlined; tests check inline safety
```

Check the tarball against the `integrity` hash from
`https://registry.npmjs.org/katex/$v` before using it.

## Releasing

1. Move `[Unreleased]` entries in `CHANGELOG.md` to a new version section.
2. Bump `version` in `Cargo.toml`, run `cargo build` to update `Cargo.lock`.
3. Refresh the README images: `cargo build && docs/screenshots.sh &&
   docs/demo.sh`, and look at them before committing.
4. Commit, then `git tag vX.Y.Z && git push --tags`.

The release workflow fails if the tag doesn't match `Cargo.toml`. It then:

- builds binaries, `.deb` and `.rpm` packages (`packaging/package.sh`) and
  publishes the GitHub release;
- updates the Homebrew tap using the `HOMEBREW_TAP_DEPLOY_KEY` secret (the
  private half of a deploy key with write access to `mdellavo/homebrew-tap`
  only);
- publishes the crate to crates.io as `markdown-viewer-rs` if the
  `CARGO_REGISTRY_TOKEN` secret is set;
- installs the release with Scoop on a Windows runner, then updates
  `bucket/markdown-viewer.json` in `mdellavo/scoop-bucket` if the
  `SCOOP_BUCKET_DEPLOY_KEY` secret is set (a deploy key with write access
  to that repository only);
- tests the AUR package on Arch and pushes it to
  `aur.archlinux.org/markdown-viewer-bin.git` if the `AUR_SSH_PRIVATE_KEY`
  secret is set (a key on the maintaining AUR account). The optional
  `AUR_MAINTAINER` repository variable adds a Maintainer line.

Each step warns and skips if its secret is missing. To update the tap by
hand:

```sh
gh release download vX.Y.Z --pattern '*.sha256' --dir /tmp/dist
packaging/homebrew/formula.sh X.Y.Z /tmp/dist > ../homebrew-tap/Formula/markdown-viewer.rb
# commit and push in homebrew-tap, then check:
brew update && brew upgrade markdown-viewer && brew audit --strict markdown-viewer
```
