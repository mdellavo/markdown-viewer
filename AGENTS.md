# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`markdown-viewer` is a Rust CLI that renders a Markdown file to a GitHub-styled
HTML page and opens it in the browser. `--watch` (or giving a folder) serves
it locally and pushes updates to the page when files change. Pages carry a
small viewer UI: toolbar, contents/files sidebar, keyboard shortcuts.

## Layout

- `src/main.rs`: CLI (clap), merging options with the settings file
  (`Settings`), one-shot mode, `-o`, `--pdf` and serving.
- `src/config.rs`: the settings file (`Config`, TOML, unknown keys are
  errors) and remembered viewer preferences (`Prefs`: theme and sidebar,
  JSON state file). `MARKDOWN_VIEWER_CONFIG` / `MARKDOWN_VIEWER_STATE`
  override their paths; the tests always set both.
- `src/theme.rs`: page CSS for all themes at once, switched by
  `<html data-mdv-theme>` (it splits GitHub's stylesheet into shared rules
  and per-scheme variables), the print layout, and `--style` CSS.
- `src/github.rs`: post-processing to match GitHub's HTML: alert icons and
  heading link icons.
- `src/docs.rs`: `Docs`, every document being viewed (Markdown files and
  folder listings) as a `watch` channel of rendered HTML. It watches only
  the folders of viewed documents, non-recursively, and lists the folder's
  Markdown files for the sidebar.
- `src/pdf.rs`: `--pdf` via a headless Chromium-based browser with a
  throwaway profile.
- `src/frontmatter.rs`: YAML front matter as a GitHub-style table.
- `src/obsidian.rs`: resolving `[[wiki links]]` and `![[embeds]]` (by name
  anywhere in the vault, closest first). Obsidian mode turns on inside a
  vault; the served root then becomes the vault so linked notes open.
- `src/editor.rs`: opening a file at a line in the user's editor
  (click-to-source). Commands run without a shell.
- `src/instances.rs`: registry of running viewers (next to the state file),
  so a second launch for the same target reuses the first
  (`/__mdv/whoami`), and `browser::focus` brings its window forward.
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
- `--exit-when-closed`: the server counts connected pages
  (`ServerHandle::abandoned`). It exits 5 s after the last one leaves, or
  after 60 s if none ever connects.
- `assets/`: GitHub CSS, `ui.js` and `ui.css` (the viewer UI), `live.js`,
  `mermaid-init.js`, `mermaid.min.js.gz`, `math-init.js`, `katex/` (KaTeX
  JS, CSS and woff2 fonts) and `octicons/` (GitHub icons, MIT), embedded with
  `include_str!`/`include_bytes!`. Everything ships inside the binary; there
  are no runtime files.
- `build.rs`: inlines KaTeX's woff2 fonts into its CSS as `data:` URIs, so one
  stylesheet works both inlined and served.
- Page scripts cooperate through a few globals and events:
  - `window.mdvEnhancers`: `render(root)` functions (Mermaid, math, copy
    buttons). `live.js` runs them all on new content before swapping it in.
    Follow the same pattern for any new client-side rendering.
  - `window.mdvRetheme`: functions to redraw for a new theme (Mermaid).
  - `mdv:updated`: dispatched by `live.js` after a swap, with
    `detail.changed`, the first changed block. `ui.js` rebuilds the contents
    and word count, and scrolls to and highlights the change.
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
- `examples/example.md`: feature tour (GFM, alerts, raw HTML, math, all
  Mermaid diagram types). Add new features here and to its test.
- `docs/screenshot.md` and `docs/screenshots.sh`: the README screenshots.
  Run `cargo build && docs/screenshots.sh` after visible UI changes and
  check the images before committing them.
- `packaging/package.sh`: packages a release build (tarball with
  completions and man page; `.deb` and `.rpm` on Linux), used by both the
  release workflow and CI.
- `packaging/homebrew/formula.sh`: generates the Homebrew formula for a release.
- `.github/workflows/`: `ci.yml` (fmt, clippy, tests on macOS and Linux) and
  `release.yml` (binaries, GitHub release, tap update).

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
- After changing options, regenerate completions and the man page:
  `UPDATE_GENERATED=1 cargo test --test cli generated_files`.
- If `cargo build` stops picking up source changes ("Fresh" after edits),
  run `cargo clean -p markdown-viewer-rs`.
- Run fmt, clippy and tests before finishing a change.
- Keep `README.md` up to date in the same change: every user-visible
  feature, option or behaviour change must be reflected in its **Features**
  list and, where relevant, Usage, the options table, Building or Security.
  Remove anything that's no longer true.
- Add user-visible changes to `CHANGELOG.md` under `[Unreleased]`.

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
3. Commit, then `git tag vX.Y.Z && git push --tags`.

The release workflow fails if the tag doesn't match `Cargo.toml`. It then:

- builds binaries, `.deb` and `.rpm` packages (`packaging/package.sh`) and
  publishes the GitHub release;
- updates the Homebrew tap using the `HOMEBREW_TAP_DEPLOY_KEY` secret (the
  private half of a deploy key with write access to `mdellavo/homebrew-tap`
  only);
- publishes the crate to crates.io as `markdown-viewer-rs` if the
  `CARGO_REGISTRY_TOKEN` secret is set.

Each step warns and skips if its secret is missing. To update the tap by
hand:

```sh
gh release download vX.Y.Z --pattern '*.sha256' --dir /tmp/dist
packaging/homebrew/formula.sh X.Y.Z /tmp/dist > ../homebrew-tap/Formula/markdown-viewer.rb
# commit and push in homebrew-tap, then check:
brew update && brew upgrade markdown-viewer && brew audit --strict markdown-viewer
```
