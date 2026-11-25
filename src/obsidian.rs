//! Obsidian notes: resolving `[[wiki links]]` and `![[embeds]]` the way
//! Obsidian does (by note name anywhere in the vault, preferring the
//! closest match). Callouts, `==highlights==` and `%%comments%%` are parsed
//! by comrak; callouts are styled in theme.rs.

use std::path::{Component, Path, PathBuf};

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

use crate::render::{escape_attr, escape_text};

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "svg", "webp", "avif", "bmp"];

const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'/');

/// Renders an embedded note: (root-relative path, optional heading) → HTML.
pub type EmbedFn<'a> = dyn Fn(&str, Option<&str>) -> Option<String> + 'a;

/// Where a document is, for resolving its links.
#[derive(Clone, Copy)]
pub struct LinkContext<'a> {
    /// The vault (or served folder) root.
    pub root: &'a Path,
    /// The document's own folder.
    pub dir: &'a Path,
    /// Markdown files under the root, `/`-separated and root-relative.
    pub files: &'a [String],
    /// Served pages link root-relative (`/notes/a.md`); standalone pages
    /// link relative to the document's folder.
    pub served: bool,
    /// How many notes deep we are in `![[embeds]]` (the page itself is 0).
    pub depth: usize,
}

/// Embedded notes go at most this deep (so notes embedding each other stop).
pub const MAX_EMBED_DEPTH: usize = 2;

/// Is this inside an Obsidian vault (a `.obsidian` folder here or above)?
pub fn in_vault(path: &Path) -> bool {
    path.ancestors().any(|dir| dir.join(".obsidian").is_dir())
}

/// The vault root containing `path`, if any.
pub fn vault_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|dir| dir.join(".obsidian").is_dir())
        .map(Path::to_path_buf)
}

/// The Markdown of `text` under the heading `heading` (up to the next
/// heading of the same or a higher level), for `![[Note#Heading]]`.
pub fn section(text: &str, heading: &str) -> Option<String> {
    let wanted = comrak::Anchorizer::new().anchorize(heading);
    let mut out = Vec::new();
    let mut level = None;
    for line in text.lines() {
        let hashes = line.chars().take_while(|c| *c == '#').count();
        let is_heading = (1..=6).contains(&hashes) && line[hashes..].starts_with(' ');
        if let Some(l) = level {
            if is_heading && hashes <= l {
                break;
            }
            out.push(line);
        } else if is_heading && comrak::Anchorizer::new().anchorize(line[hashes..].trim()) == wanted
        {
            level = Some(hashes);
            out.push(line);
        }
    }
    level.map(|_| out.join("\n"))
}

/// Drop `data-sourcepos` attributes (an embedded note's line numbers would
/// point into the wrong file).
pub fn strip_sourcepos(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find(" data-sourcepos=\"") {
        out.push_str(&rest[..at]);
        let after = &rest[at + " data-sourcepos=\"".len()..];
        rest = after.find('"').map_or("", |end| &after[end + 1..]);
    }
    out.push_str(rest);
    out
}

/// Backlinks: lines in other notes that link to `target` (root-relative),
/// by `[[wiki link]]` or a Markdown link.
pub fn backlinks(root: &Path, files: &[String], target: &str) -> Vec<(String, usize, String)> {
    let mut hits = Vec::new();
    for file in files {
        if file == target {
            continue;
        }
        let path = root.join(file);
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let dir = path.parent().unwrap_or(root);
        let ctx = LinkContext {
            root,
            dir,
            files,
            served: true,
            depth: 0,
        };
        for (i, line) in text.lines().enumerate() {
            if links_in(line).iter().any(|l| links_to(l, &ctx, target)) {
                hits.push((file.clone(), i + 1, line.trim().chars().take(160).collect()));
            }
        }
    }
    hits
}

/// Link targets on a line: `[[target|…]]` (true) and `](target)` (false).
fn links_in(line: &str) -> Vec<(bool, String)> {
    let mut found = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find("[[") {
        let after = &rest[at + 2..];
        let Some(end) = after.find("]]") else { break };
        let target = after[..end].split(['|', '#']).next().unwrap_or("").trim();
        if !target.is_empty() {
            found.push((true, target.to_string()));
        }
        rest = &after[end + 2..];
    }
    let mut rest = line;
    while let Some(at) = rest.find("](") {
        let after = &rest[at + 2..];
        let Some(end) = after.find(')') else { break };
        let target = after[..end].split_whitespace().next().unwrap_or("");
        let target = target
            .trim_matches(['<', '>'])
            .split('#')
            .next()
            .unwrap_or("");
        if !target.is_empty() && !target.contains("://") && !target.starts_with("mailto:") {
            found.push((
                false,
                percent_decode_str(target).decode_utf8_lossy().into_owned(),
            ));
        }
        rest = &after[end + 1..];
    }
    found
}

fn links_to((wiki, target): &(bool, String), ctx: &LinkContext, wanted: &str) -> bool {
    if *wiki {
        return find_note(target, ctx).as_deref() == Some(wanted);
    }
    let path = if target.starts_with('/') {
        ctx.root.join(target.trim_start_matches('/'))
    } else {
        ctx.dir.join(target)
    };
    normalize(&path)
        .strip_prefix(ctx.root)
        .is_ok_and(|rel| rel.to_string_lossy().replace('\\', "/") == wanted)
}

/// Resolve `.` and `..` without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Resolve wiki links and embeds in rendered HTML.
pub fn finish(html: String, ctx: &LinkContext, embed: &EmbedFn) -> String {
    let html = if html.contains("data-wikilink=\"true\"") {
        resolve_links(&html, ctx)
    } else {
        html
    };
    if html.contains("![[") {
        resolve_embeds(&html, ctx, embed)
    } else {
        html
    }
}

/// Rewrite the href of every `<a … data-wikilink="true">`.
fn resolve_links(html: &str, ctx: &LinkContext) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(marker) = rest.find("data-wikilink=\"true\"") {
        let tag_start = rest[..marker].rfind("<a ").unwrap_or(marker);
        let tag = &rest[tag_start..marker];
        let Some(href_at) = tag.find("href=\"") else {
            out.push_str(&rest[..marker + 1]);
            rest = &rest[marker + 1..];
            continue;
        };
        let value_start = tag_start + href_at + 6;
        let value_end = value_start + rest[value_start..].find('"').unwrap_or(0);
        let target = percent_decode_str(&rest[value_start..value_end]).decode_utf8_lossy();
        out.push_str(&rest[..value_start]);
        match resolve(&target, ctx) {
            Some(url) => {
                out.push_str(&escape_attr(&url));
                out.push_str(&rest[value_end..marker]);
            }
            None => {
                out.push_str(&rest[value_start..value_end]);
                out.push_str(&rest[value_end..marker]);
                out.push_str("class=\"mdv-wikilink-missing\" title=\"Note not found\" ");
            }
        }
        out.push_str("data-wikilink=\"true\"");
        rest = &rest[marker + "data-wikilink=\"true\"".len()..];
    }
    out.push_str(rest);
    out
}

/// Turn `![[name]]` / `![[name|300]]` text into images (or links, for notes).
fn resolve_embeds(html: &str, ctx: &LinkContext, embed: &EmbedFn) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("![[") {
        let Some(len) = rest[start + 3..].find("]]") else {
            break;
        };
        let inner = &rest[start + 3..start + 3 + len];
        if inner.contains('<') || inner.contains('\n') {
            out.push_str(&rest[..start + 3]);
            rest = &rest[start + 3..];
            continue;
        }
        out.push_str(&rest[..start]);
        let (target, label) = match inner.split_once('|') {
            Some((t, l)) => (t.trim(), Some(l.trim())),
            None => (inner.trim(), None),
        };
        let target = html_unescape(target);
        let url = resolve(&target, ctx);
        // A note: show it inline. If the embed is a paragraph of its own,
        // replace the paragraph, since a block can't sit inside one.
        let (note_path, heading) = match target.split_once('#') {
            Some((p, h)) => (p.trim(), Some(h.trim())),
            None => (target.trim(), None),
        };
        let note = (!note_path.is_empty())
            .then(|| find_note(note_path, ctx))
            .flatten()
            .and_then(|rel| embed(&rel, heading));
        if let (Some(note_html), Some(url)) = (&note, &url) {
            let embed_html = format!(
                "<div class=\"mdv-embed-note\"><div class=\"mdv-embed-title\"><a href=\"{}\">{}</a></div>{}</div>",
                escape_attr(url),
                escape_text(&target),
                note_html
            );
            let p_open = out.rfind("<p").filter(|&p| {
                let tail = &out[p..];
                tail.ends_with('>') && tail.matches('<').count() == 1
            });
            let after = &rest[start + 3 + len + 2..];
            if let (Some(p), Some(stripped)) = (p_open, after.strip_prefix("</p>")) {
                out.truncate(p);
                out.push_str(&embed_html);
                rest = stripped;
            } else {
                out.push_str(&embed_html);
                rest = after;
            }
            continue;
        }
        let is_image = Path::new(target.split('#').next().unwrap_or(""))
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
        match (url, is_image) {
            (Some(url), true) => {
                let width = label
                    .and_then(|l| l.split('x').next())
                    .filter(|w| w.chars().all(|c| c.is_ascii_digit()) && !w.is_empty())
                    .map(|w| format!(" width=\"{w}\""))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"{width}>",
                    escape_attr(&url),
                    escape_attr(&target)
                ));
            }
            (Some(url), false) => out.push_str(&format!(
                "<a class=\"mdv-embed\" href=\"{}\">{}</a>",
                escape_attr(&url),
                escape_text(&target)
            )),
            (None, _) => out.push_str(&format!(
                "<span class=\"mdv-wikilink-missing\" title=\"Not found\">{}</span>",
                escape_text(&target)
            )),
        }
        rest = &rest[start + 3 + len + 2..];
    }
    out.push_str(rest);
    out
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

/// Resolve a wiki-link target (`Note`, `dir/Note`, `Note#Heading`,
/// `image.png`) to a URL.
fn resolve(target: &str, ctx: &LinkContext) -> Option<String> {
    let (path, anchor) = match target.split_once('#') {
        Some((p, a)) => (p.trim(), Some(a.trim())),
        None => (target.trim(), None),
    };
    let fragment = anchor
        .map(|a| {
            if a.starts_with('^') {
                format!("#{a}")
            } else {
                format!("#{}", comrak::Anchorizer::new().anchorize(a))
            }
        })
        .unwrap_or_default();
    if path.is_empty() {
        return Some(fragment);
    }
    let rel = find_note(path, ctx).or_else(|| find_file(path, ctx))?;
    Some(format!("{}{fragment}", url_for(&rel, ctx)))
}

/// A Markdown note by name or partial path, Obsidian-style: an exact
/// root-relative path wins, then notes in the current folder, then the
/// shortest path anywhere.
pub fn find_note(path: &str, ctx: &LinkContext) -> Option<String> {
    let wanted = if crate::docs::is_markdown(Path::new(path)) {
        path.to_string()
    } else {
        format!("{path}.md")
    };
    let wanted_lc = wanted.to_lowercase();
    let suffix = format!("/{wanted_lc}");
    let here = ctx
        .dir
        .strip_prefix(ctx.root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let mut matches: Vec<&String> = ctx
        .files
        .iter()
        .filter(|f| {
            let lc = f.to_lowercase();
            lc == wanted_lc || lc.ends_with(&suffix)
        })
        .collect();
    matches.sort_by_key(|f| {
        let in_here = Path::new(f.as_str())
            .parent()
            .is_some_and(|p| p.to_string_lossy() == here);
        let exact = f.to_lowercase() == wanted_lc;
        (!exact, !in_here, f.matches('/').count(), f.len())
    });
    matches.first().map(|f| f.to_string())
}

/// Any other file (images, PDFs): next to the document, root-relative, or
/// (as Obsidian does for attachments) by file name anywhere in the vault.
fn find_file(path: &str, ctx: &LinkContext) -> Option<String> {
    for base in [ctx.dir, ctx.root] {
        let candidate = base.join(path);
        if candidate.is_file()
            && let Ok(rel) = candidate.strip_prefix(ctx.root)
            && !rel.components().any(|c| matches!(c, Component::ParentDir))
        {
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    if path.contains('/') {
        return None;
    }
    let mut found = Vec::new();
    let mut budget = 5000;
    find_by_name(ctx.root, &path.to_lowercase(), 0, &mut budget, &mut found);
    found.sort_by_key(|p| (p.components().count(), p.as_os_str().len()));
    let rel = found.first()?.strip_prefix(ctx.root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

/// Files named `name` (lowercase) under `dir`, skipping hidden folders and
/// symlinks, within a budget of directory entries.
fn find_by_name(dir: &Path, name: &str, depth: usize, budget: &mut usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 || depth > 8 {
            return;
        }
        *budget -= 1;
        let file_name = entry.file_name().to_string_lossy().to_lowercase();
        if file_name.starts_with('.') {
            continue;
        }
        match entry.file_type() {
            Ok(t) if t.is_dir() => find_by_name(&entry.path(), name, depth + 1, budget, out),
            Ok(t) if t.is_file() && file_name == name => out.push(entry.path()),
            _ => {}
        }
    }
}

/// URL for a root-relative path, from this document.
fn url_for(rel: &str, ctx: &LinkContext) -> String {
    let encode = |p: &str| {
        p.split('/')
            .map(|seg| utf8_percent_encode(seg, SEGMENT).to_string())
            .collect::<Vec<_>>()
            .join("/")
    };
    if ctx.served {
        return format!("/{}", encode(rel));
    }
    // Relative to the document's folder: up to the root, then down.
    let here = ctx.dir.strip_prefix(ctx.root).unwrap_or(Path::new(""));
    let ups = here.components().count();
    format!("{}{}", "../".repeat(ups), encode(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        root: &'a Path,
        dir: &'a Path,
        files: &'a [String],
        served: bool,
    ) -> LinkContext<'a> {
        LinkContext {
            root,
            dir,
            files,
            served,
            depth: 0,
        }
    }

    fn no_embed(_: &str, _: Option<&str>) -> Option<String> {
        None
    }

    fn files() -> Vec<String> {
        [
            "Home.md",
            "notes/My Note.md",
            "notes/deep/Other.md",
            "archive/My Note.md",
        ]
        .map(String::from)
        .to_vec()
    }

    #[test]
    fn resolves_notes_by_name_preferring_closest() {
        let f = files();
        let root = Path::new("/vault");
        let c = ctx(root, Path::new("/vault/notes"), &f, true);
        assert_eq!(
            resolve("My Note", &c).as_deref(),
            Some("/notes/My%20Note.md")
        );
        assert_eq!(
            resolve("other", &c).as_deref(),
            Some("/notes/deep/Other.md")
        );
        assert_eq!(
            resolve("archive/My Note", &c).as_deref(),
            Some("/archive/My%20Note.md")
        );
        assert_eq!(
            resolve("Home#Getting Started", &c).as_deref(),
            Some("/Home.md#getting-started")
        );
        assert_eq!(resolve("#Local", &c).as_deref(), Some("#local"));
        assert_eq!(resolve("Missing", &c), None);
    }

    #[test]
    fn standalone_links_are_relative() {
        let f = files();
        let c = ctx(
            Path::new("/vault"),
            Path::new("/vault/notes/deep"),
            &f,
            false,
        );
        assert_eq!(resolve("Home", &c).as_deref(), Some("../../Home.md"));
    }

    #[test]
    fn rewrites_links_and_marks_missing_ones() {
        let f = files();
        let c = ctx(Path::new("/vault"), Path::new("/vault"), &f, true);
        let html = "<p><a href=\"My%20Note\" data-wikilink=\"true\">My Note</a> \
                    <a data-sourcepos=\"1:1-1:2\" href=\"Nope\" data-wikilink=\"true\">Nope</a></p>";
        let out = finish(html.into(), &c, &no_embed);
        assert!(
            out.contains("href=\"/notes/My%20Note.md\" data-wikilink"),
            "{out}"
        );
        assert!(out.contains("class=\"mdv-wikilink-missing\""), "{out}");
    }

    #[test]
    fn embeds_images_with_size() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pic.png"), b"x").unwrap();
        let f = files();
        let c = ctx(dir.path(), dir.path(), &f, true);
        let out = finish(
            "<p>![[pic.png|300]] and ![[Home]] and ![[gone.png]]</p>".into(),
            &c,
            &no_embed,
        );
        assert!(
            out.contains("<img src=\"/pic.png\" alt=\"pic.png\" width=\"300\">"),
            "{out}"
        );
        assert!(
            out.contains("<a class=\"mdv-embed\" href=\"/Home.md\">Home</a>"),
            "{out}"
        );
        assert!(out.contains("mdv-wikilink-missing"), "{out}");
    }

    #[test]
    fn finds_attachments_anywhere_in_the_vault() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("attachments/img")).unwrap();
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        std::fs::write(dir.path().join("attachments/img/Pic.PNG"), b"x").unwrap();
        let f = files();
        let notes = dir.path().join("notes");
        let c = ctx(dir.path(), &notes, &f, true);
        let out = finish("<p>![[pic.png]]</p>".into(), &c, &no_embed);
        assert!(
            out.contains("<img src=\"/attachments/img/Pic.PNG\""),
            "{out}"
        );
    }

    #[test]
    fn embeds_notes_in_place_of_their_paragraph() {
        let f = files();
        let c = ctx(Path::new("/vault"), Path::new("/vault"), &f, true);
        let embed =
            |rel: &str, heading: Option<&str>| Some(format!("<p>BODY of {rel} {heading:?}</p>"));
        let out = finish(
            "<p data-sourcepos=\"1:1-1:9\">![[Home]]</p>\n<p>Inline ![[Home#Intro]] here</p>"
                .into(),
            &c,
            &embed,
        );
        assert!(out.starts_with("<div class=\"mdv-embed-note\"><div class=\"mdv-embed-title\"><a href=\"/Home.md\">Home</a></div><p>BODY of Home.md None</p></div>\n"), "{out}");
        assert!(
            out.contains("<p>Inline <div class=\"mdv-embed-note\">"),
            "inline embeds stay put: {out}"
        );
        assert!(out.contains("Some(\"Intro\")"));
    }

    #[test]
    fn extracts_sections() {
        let text = "# A\nintro\n## B\nb text\n### B1\nmore\n## C\nc text";
        assert_eq!(section(text, "B").unwrap(), "## B\nb text\n### B1\nmore");
        assert_eq!(section(text, "missing"), None);
    }

    #[test]
    fn strips_source_positions() {
        assert_eq!(
            strip_sourcepos(
                "<p data-sourcepos=\"1:1-2:3\">x <a data-sourcepos=\"1:3-1:4\" href=\"y\">y</a></p>"
            ),
            "<p>x <a href=\"y\">y</a></p>"
        );
    }

    #[test]
    fn finds_backlinks_from_wiki_and_markdown_links() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        std::fs::create_dir_all(r.join("notes/sub")).unwrap();
        std::fs::write(r.join("notes/Target.md"), "# Target").unwrap();
        std::fs::write(r.join("notes/a.md"), "intro\nsee [[Target|it]] here").unwrap();
        std::fs::write(r.join("notes/sub/b.md"), "[link](../Target.md#x)").unwrap();
        std::fs::write(r.join("c.md"), "[[Other]] and [web](https://x/Target.md)").unwrap();
        let files: Vec<String> = ["c.md", "notes/Target.md", "notes/a.md", "notes/sub/b.md"]
            .map(String::from)
            .to_vec();
        let hits = backlinks(r, &files, "notes/Target.md");
        let found: Vec<_> = hits.iter().map(|(f, l, _)| (f.as_str(), *l)).collect();
        assert_eq!(found, [("notes/a.md", 2), ("notes/sub/b.md", 1)]);
    }

    #[test]
    fn finds_vaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".obsidian")).unwrap();
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();
        assert!(in_vault(&dir.path().join("notes")));
        assert_eq!(vault_root(&dir.path().join("notes")).unwrap(), dir.path());
    }
}
