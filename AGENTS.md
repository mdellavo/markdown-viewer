# AGENTS.md

Guidance for coding agents working in this repository.

## What this is

`markdown-viewer` is a Rust CLI that renders a Markdown file to a GitHub-styled
HTML page and opens it in the browser. `--watch` serves it locally and pushes
updates to the page when the file changes.

## Layout

- `src/main.rs`: CLI (clap), one-shot mode, `-o` output.
- `src/browser.rs`: opens the page as an app window (`--app=` for Chromium),
  a new window (`--new-window` for Chromium/Firefox, AppleScript for Safari)
  or a tab, falling back in that order. Finds the default browser via
  LaunchServices (macOS) or `xdg-settings` (Linux). Chrome ignores
  `--window-size` when already running, so window size isn't configurable.
- `src/render.rs`: `Renderer` (comrak with GFM and math options, syntect
  highlighting via CSS classes), the ```` ```mermaid ```` fence plugin, and
  the page template, including the CSP.
- `src/server.rs`: axum server: `/` page, `/__mdv/events` live-reload stream
  (Server-Sent Events), `/__mdv/{mermaid,katex}.js` and `/__mdv/katex.css`,
  and static files from the Markdown file's directory.
- Shutdown: on Ctrl-C, `main.rs` calls `ServerHandle::announce_shutdown`,
  the event stream sends a `close` event, and `live.js` closes the window if
  the page was rendered with `close_on_exit` (only for windows we opened),
  otherwise it shows a banner. The server waits 300 ms for this before
  exiting. Never kill the browser process: it may hold the user's other
  windows.
- `src/watcher.rs`: debounced file watcher that re-renders into a
  `tokio::sync::watch` channel read by the server.
- `assets/`: GitHub CSS (auto/light/dark), `live.js`, `mermaid-init.js`,
  `mermaid.min.js.gz`, `math-init.js` and `katex/` (KaTeX JS, CSS and woff2
  fonts), embedded with `include_str!`/`include_bytes!`. Everything ships
  inside the binary; there are no runtime files.
- `build.rs`: inlines KaTeX's woff2 fonts into its CSS as `data:` URIs, so one
  stylesheet works both inlined and served.
- Page enhancers: `mermaid-init.js` and `math-init.js` each push a
  `render(root)` function onto `window.mdvEnhancers`; `live.js` runs them all
  on new content before swapping it in. Follow the same pattern for any new
  client-side rendering.
- `tests/cli.rs`: end-to-end tests against the built binary, including an
  insta snapshot of `tests/fixtures/gfm.md` and a check that every feature in
  `examples/example.md` still renders.
- `examples/example.md`: feature tour (GFM, alerts, raw HTML, math, all
  Mermaid diagram types). Add new features here and to its test.
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
- The only scripts in pages are `live.js` (watch mode), Mermaid and KaTeX
  (each an init script plus the library, only on pages that need them). Any
  script must carry the per-page CSP nonce. Standalone pages inline the
  libraries; served pages fetch them from `/__mdv/`.
- The watcher watches the parent directory, not the file, because editors
  often save by renaming a temp file over the original. Keep it that way.
- `server::resolve` must never serve a path outside the Markdown file's
  directory; extend its test when touching it.
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

The release workflow fails if the tag doesn't match `Cargo.toml`. Updating the
tap needs the `HOMEBREW_TAP_TOKEN` secret (a token with write access to
`mdellavo/homebrew-tap`).
