# Writing custom CSS

markdown-viewer looks like GitHub by default. Use your own stylesheet to
change fonts, colours, widths, code blocks or the printed page.
[`examples/custom.css`](../examples/custom.css) is a complete example: a
warm, book-like theme with a dark variant.

```sh
markdown-viewer --css examples/custom.css README.md
```

## Using a stylesheet

Give it on the command line, or set it once in the
[settings file](../README.md#settings-file) (`~/` is expanded):

```sh
markdown-viewer --css ~/notes.css notes.md
```

```toml
css = "~/notes.css"
```

It applies everywhere a page is made: the viewer, `-o` exports, static
sites and `--pdf`. It goes after all the built-in styles, so your rules win
over GitHub's when they're equally specific. You can combine it with a
built-in `--style` (`wide`, `book` or `compact`).

While viewing, the file is read again each time the page loads. After
editing it, reload the page (<kbd>⌘</kbd>/<kbd>Ctrl</kbd>-<kbd>R</kbd>);
there's no need to restart.

## How the page is built

```html
<html data-mdv-theme="auto">          <!-- auto, light or dark -->
  <body>
    <article class="markdown-body">   <!-- your document -->
      …
    </article>
    <div class="mdv-ui mdv-toolbar">…</div>   <!-- the viewer's own UI -->
  </body>
</html>
```

Start every rule with `.markdown-body` to style the document without
touching the viewer's toolbar and sidebar.

## Colours

The colours come from GitHub's CSS variables. Change a variable, and
everything that uses it follows, in every part of the page:

```css
.markdown-body {
  --fgColor-accent: #9a4a12;   /* links */
}
```

The variables you're most likely to want:

| Variable | Used for |
| --- | --- |
| `--fgColor-default` | text, and code blocks |
| `--fgColor-muted` | quieter text: quotes, footnotes, `h6` |
| `--fgColor-accent` | links |
| `--bgColor-default` | the document's background |
| `--bgColor-muted` | code blocks, keys (`<kbd>`), striped table rows |
| `--bgColor-neutral-muted` | inline `code` |
| `--borderColor-default` | table borders, quote bars, alert bars, `---` rules |
| `--borderColor-muted` | the rules under `h1` and `h2`, lines between table rows |

Fonts aren't variables; set `font-family` (see [Recipes](#recipes)).

The page outside the document has its own background, so set it too when
you change `--bgColor-default`:

```css
html { background: #f8f1e3; }
```

### Light and dark

A plain `.markdown-body` rule applies in both themes. To set colours for
dark only, cover both ways the page can be dark: dark chosen with
<kbd>t</kbd> or `--theme dark`, and `auto` on a system in dark mode.

```css
:root[data-mdv-theme="dark"] .markdown-body {
  --fgColor-accent: #e0955c;
}
@media (prefers-color-scheme: dark) {
  :root[data-mdv-theme="auto"] .markdown-body {
    --fgColor-accent: #e0955c;
  }
}
```

Printing and `--pdf` always use the light colours.

## Recipes

Width, fonts and spacing:

```css
.markdown-body {
  max-width: 720px;
  font-family: "Iowan Old Style", Georgia, serif;
  font-size: 18px;
  line-height: 1.7;
}
.markdown-body code,
.markdown-body pre { font-family: "JetBrains Mono", Menlo, monospace; }
```

Headings:

```css
.markdown-body h1,
.markdown-body h2 { border-bottom: none; }
.markdown-body h2 { color: var(--fgColor-accent); }
```

Code blocks and syntax highlighting. Highlighted code uses `hl-` classes
named after what the text is: `hl-keyword`, `hl-string`, `hl-comment`,
`hl-constant`, `hl-entity`, and so on.

```css
.markdown-body pre { border-radius: 0; border-left: 3px solid var(--borderColor-default); }
.markdown-body .hl-comment { font-style: normal; opacity: 0.7; }
```

Alerts (`> [!NOTE]`) and Obsidian callouts (`> [!tip]`). A callout's colour
is the `--mdv-callout` variable:

```css
.markdown-body .markdown-alert-note { background: var(--bgColor-muted); }
.markdown-body .callout[data-callout="tip"] { --mdv-callout: #2da44e; }
```

Hiding things:

```css
.markdown-body .anchor { display: none; }            /* heading link icons */
.markdown-body .mdv-front-matter { display: none; }  /* or front_matter = "hide" */
```

## Printing and PDFs

`--pdf` and the browser's Print use print rules, so put changes for paper
in `@media print`. `@page` sets the paper size and margins.

```css
@page { size: A4; margin: 20mm; }
@media print {
  .markdown-body { font-size: 11pt; }
  .markdown-body h2 { break-before: page; }   /* each section on a new page */
}
```

## What else you can style

| Selector | What it is |
| --- | --- |
| `.markdown-body` | the document |
| `.markdown-alert`, `.markdown-alert-note` (`-tip`, `-important`, `-warning`, `-caution`) | GitHub alerts |
| `.callout`, `.callout[data-callout="…"]`, `.callout-title` | Obsidian callouts |
| `.mdv-code-figure`, `.mdv-code-title` | a code block with a `title="…"`, and its title |
| `.mdv-line`, `.mdv-hl` | a line of a numbered or highlighted code block; a highlighted line |
| `.mdv-toc`, `.mdv-toc > details > summary` | a `[TOC]` table of contents, and its "Contents" title |
| `.mdv-toc li > details > summary` | a section in the contents that folds (it has sections under it) |
| `.mdv-front-matter` | the front matter table |
| `table.mdv-data`, `.mdv-data-note` | a CSV or TSV file's table, and the row count under it |
| `div.mermaid` | a Mermaid diagram |
| `.mdv-map`, `.mdv-map-grid`, `.mdv-map-area`, `.mdv-map-line`, `.mdv-map-point` | a map (GeoJSON, TopoJSON, GPX, KML, WKT), its grid, and its areas, lines and points |
| `.mdv-stl`, `.mdv-stl polygon` | a 3D model (STL, OBJ, PLY, OFF), and its triangles (shaded through the `--l` lightness, 0.3 to 1) |
| `.mdv-diagram` | a drawn diagram: with `.mdv-bob`, `.mdv-pikchr`, `.mdv-wavedrom`, `.mdv-qr`, `.mdv-chess`, `.mdv-dot`, `.mdv-plantuml`, `.mdv-d2`, `.mdv-vega`, `.mdv-abc` or `.mdv-smiles` |
| `.mdv-diagram-invalid`, `.mdv-diagram-missing` | a block that couldn't be drawn (with the reason), or whose program isn't installed |
| `.mdv-wave-line`, `.mdv-wave-bus`, `.mdv-wave-2` … `.mdv-wave-9`, `.mdv-wave-x`, `.mdv-wave-name`, `.mdv-wave-data` | WaveDrom: signal lines, bus values and their fill colours, unknown values, lane names and value labels |
| `.mdv-chess-light`, `.mdv-chess-dark`, `.mdv-chess-moved`, `.mdv-chess-white`, `.mdv-chess-black` | chess squares, the last move's squares, and the pieces |
| `.mdv-page-meta` | the footer after the article: when the file last changed in git, by whom, and the "Edit on GitHub" link (outside `.markdown-body`) |
| `a.mdv-tag`, `td.mdv-tags` | a tag chip (`#tag` in Obsidian notes, front matter `tags:`), and the front matter cell holding them |
| `.mdv-color` | the swatch after a colour in inline code |
| `.mdv-molecule`, `.mdv-molecule-3d`, `.mdv-mol-bond`, `.mdv-mol-label`, `.mdv-mol-stick`, `circle.mdv-el-O` (and other elements) | molecules: structure bonds and atom labels, and 3D sticks and atoms by element |
| `.mdv-calendar`, `.mdv-cal-month`, `.mdv-cal-day`, `.mdv-cal-time`, `.mdv-cal-zone`, `.mdv-cal-note`, `.mdv-cal-desc` | a calendar's table, month rows, dates, times, time zones, places and descriptions |
| `video.mdv-video`, `audio.mdv-audio` | players for video and audio linked like images |
| `abbr[title]` | an abbreviation |
| `dl`, `dt`, `dd` | a definition list |
| `.math-display`, `.math-inline` | math |
| `.mdv-ref` | a GitHub reference (`#123`, `@user`) |
| `.mdv-embed-note` | an embedded Obsidian note (`![[Note]]`) |
| `.mdv-wikilink-missing` | a wiki link to a note that doesn't exist |
| `.task-list-item`, `.task-list-item-checkbox` | task list items |
| `:root[data-mdv-number] .markdown-body h2::before` | heading numbers (`--number-headings`) |

The viewer's UI (`.mdv-toolbar`, `.mdv-sidebar` and other `.mdv-ui`
elements) can be styled too, but its structure may change between
versions. It's hidden when printing.

## Limits

Pages work offline and block anything from the web except images, so:

- **Fonts:** use fonts installed on your computer by name. Web fonts
  (`@import` from Google Fonts, `@font-face` with an `https://` URL) are
  blocked. To use a font file, embed it in the stylesheet as a `data:` URL.
- **Images:** `url(…)` works for images on the web and `data:` URLs.
  Relative paths resolve against the page, not the stylesheet, so use
  absolute or `data:` URLs.
- **Scripts:** a stylesheet can't add any.
- **Mermaid diagrams** use Mermaid's own light or dark colours, which
  these variables don't change.
