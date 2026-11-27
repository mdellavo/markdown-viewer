//! `-o DIR` for a folder: a static site. Every Markdown file becomes an
//! HTML page at the same relative path (`.md` → `.html`), with links
//! between them rewritten. Every folder gets an `index.html` (its README, or
//! a listing). Images and other files the pages link to are copied. The
//! viewer UI works as in a one-shot page, plus a Files sidebar listing
//! the site. Mermaid and KaTeX are written once, under `_mdv/`.
//!
//! The pages work from any web server (GitHub Pages included: a
//! `.nojekyll` file stops Jekyll hiding `_mdv/`) and opened straight from
//! disk.

use crate::platform::Canonical;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use percent_encoding::{percent_decode_str, utf8_percent_encode};

use crate::docs;
use crate::obsidian::{LinkContext, SEGMENT};
use crate::render::{PageOptions, Renderer, SITE_DIR, escape_attr};

/// What an export wrote.
pub struct Summary {
    pub pages: usize,
    pub files: usize,
    /// Only the first files were exported (the folder has too many).
    pub truncated: bool,
}

/// Export the Markdown under `root` to `out`. `page` gives the page
/// options (theme, style, CSS) for a title.
pub fn export(
    renderer: &Renderer,
    obsidian: bool,
    root: &Path,
    out: &Path,
    page: &dyn Fn(String) -> PageOptions,
) -> Result<Summary> {
    let out = prepare(root, out)?;
    let (files, truncated) = docs::document_files(root);
    let markdown: HashSet<&str> = files.iter().map(String::as_str).collect();
    // Wiki links resolve to notes only, as in the viewer.
    let notes: Vec<String> = files
        .iter()
        .filter(|f| docs::is_markdown(Path::new(f.as_str())))
        .cloned()
        .collect();
    // Folders with Markdown somewhere inside, each getting an index.html.
    let mut folders = BTreeSet::from([String::new()]);
    for f in &files {
        let mut dir = parent(f);
        while !dir.is_empty() {
            folders.insert(dir.to_string());
            dir = parent(dir);
        }
    }
    let site = Site {
        renderer,
        obsidian,
        root,
        out: &out,
        notes: &notes,
        markdown,
        folders: &folders,
        page,
    };

    let mut assets = BTreeSet::new();
    let mut libs = Libs::default();
    let mut pages = 0;
    for rel in &files {
        let text = std::fs::read_to_string(root.join(rel))
            .with_context(|| format!("cannot read {}", root.join(rel).display()))?;
        site.write(rel, &html_name(rel), &text, &mut assets, &mut libs)?;
        pages += 1;
    }
    let page_names: BTreeSet<String> = files.iter().map(|f| html_name(f)).collect();
    for dir in &folders {
        let index = join(dir, "index.html");
        // An index.md already became this folder's index.html; its README
        // still has a page of its own (README.html).
        if page_names.contains(&index) {
            continue;
        }
        let path = root.join(dir);
        match docs::readme(&path) {
            Some(readme) => {
                let rel = join(
                    dir,
                    &readme.file_name().unwrap_or_default().to_string_lossy(),
                );
                let text = std::fs::read_to_string(&readme)
                    .with_context(|| format!("cannot read {}", readme.display()))?;
                site.write(&rel, &index, &text, &mut assets, &mut libs)?;
            }
            None => {
                let listing = docs::listing_markdown(root, &path);
                site.write(dir, &index, &listing, &mut assets, &mut libs)?;
                pages += 1;
            }
        }
    }

    let copied = copy_assets(root, &out, &assets)?;
    write_support(&out, &files, &libs)?;
    Ok(Summary {
        pages,
        files: copied,
        truncated,
    })
}

/// Create the output folder, refusing one that holds anything but an
/// earlier export, or that is the folder being exported.
fn prepare(root: &Path, out: &Path) -> Result<PathBuf> {
    if out.exists() {
        if !out.is_dir() {
            bail!(
                "{} is a file; give a folder to write the site to",
                out.display()
            );
        }
        let empty = std::fs::read_dir(out)?.next().is_none();
        if !empty && !out.join(SITE_DIR).is_dir() {
            bail!(
                "{} isn't empty; use a new folder (or one an earlier export wrote)",
                out.display()
            );
        }
    }
    std::fs::create_dir_all(out).with_context(|| format!("cannot create {}", out.display()))?;
    let out = out.canonical()?;
    if out == root || root.starts_with(&out) {
        bail!("the site can't be written over the folder being exported");
    }
    Ok(out)
}

/// Which libraries any page needs.
#[derive(Default)]
struct Libs {
    math: bool,
    mermaid: bool,
    /// Vega-Lite, abcjs, SmilesDrawer (libraries.rs ids).
    others: std::collections::BTreeSet<&'static str>,
}

struct Site<'a> {
    renderer: &'a Renderer,
    obsidian: bool,
    root: &'a Path,
    out: &'a Path,
    notes: &'a [String],
    markdown: HashSet<&'a str>,
    folders: &'a BTreeSet<String>,
    page: &'a dyn Fn(String) -> PageOptions,
}

impl Site<'_> {
    /// Render `text` (the document `doc`, a file or folder relative to the
    /// root) to the page `to`, in the same folder as `doc`'s links expect.
    fn write(
        &self,
        doc: &str,
        to: &str,
        text: &str,
        assets: &mut BTreeSet<String>,
        libs: &mut Libs,
    ) -> Result<()> {
        let is_folder = to.ends_with("index.html") && !docs::is_viewable(Path::new(doc));
        let dir = if is_folder { doc } else { parent(doc) };
        let body = if let Some(delim) = crate::tables::delimiter(Path::new(doc)) {
            crate::tables::to_html(text, delim)
        } else if self.obsidian {
            let abs = self.root.join(dir);
            let ctx = LinkContext {
                root: self.root,
                dir: &abs,
                files: self.notes,
                served: false,
                depth: 0,
            };
            self.renderer.render_doc(text, Some(&ctx))
        } else {
            self.renderer
                .render_in(text, Some(&self.root.join(dir)), None)
        };
        let body = self.rewrite(&body, dir, assets);

        let title = if is_folder {
            if doc.is_empty() {
                self.root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "/".into())
            } else {
                doc.to_string()
            }
        } else {
            crate::server::file_title(&self.root.join(doc))
        };
        let depth = to.matches('/').count();
        let options = PageOptions {
            ui: true,
            doc: (!is_folder).then(|| doc.to_string()),
            site_root: Some("../".repeat(depth)),
            ..(self.page)(title)
        };
        let html = self.renderer.wrap_page(&body, &options);
        libs.math |= html.contains(&format!("{SITE_DIR}/katex.js"));
        libs.mermaid |= html.contains(&format!("{SITE_DIR}/mermaid.js"));
        for lib in crate::libraries::used(&body) {
            libs.others.insert(lib.id);
        }
        let path = self.out.join(to);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&path, html).with_context(|| format!("cannot write {}", path.display()))
    }

    /// Point links at the site's pages: `x.md` → `x.html`, a folder →
    /// its `index.html`, root-relative paths → relative ones. Records the
    /// other local files linked to, to copy them.
    fn rewrite(&self, html: &str, dir: &str, assets: &mut BTreeSet<String>) -> String {
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        while let Some(at) = rest.find('<') {
            let (before, tag_start) = rest.split_at(at);
            out.push_str(before);
            let end = tag_start.find('>').map_or(tag_start.len(), |e| e + 1);
            let mut tag = tag_start[..end].to_string();
            let name = tag[1..]
                .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let attrs: &[&str] = match name.as_str() {
                "a" => &["href"],
                "img" | "source" => &["src", "srcset"],
                "video" => &["src", "poster"],
                "audio" => &["src"],
                _ => &[],
            };
            for attr in attrs {
                tag = self.rewrite_attr(&tag, attr, dir, assets);
            }
            out.push_str(&tag);
            rest = &tag_start[end..];
        }
        out.push_str(rest);
        out
    }

    /// One attribute of a tag; `srcset` holds several URLs.
    fn rewrite_attr(
        &self,
        tag: &str,
        attr: &str,
        dir: &str,
        assets: &mut BTreeSet<String>,
    ) -> String {
        let key = format!(" {attr}=\"");
        let Some(start) = tag.find(&key).map(|i| i + key.len()) else {
            return tag.to_string();
        };
        let Some(len) = tag[start..].find('"') else {
            return tag.to_string();
        };
        let value = crate::links::unescape(&tag[start..start + len]);
        let new = if attr == "srcset" {
            value
                .split(',')
                .map(|item| {
                    let item = item.trim();
                    let (url, descriptor) = item.split_once(' ').unwrap_or((item, ""));
                    let url = self
                        .link(url, dir, assets)
                        .unwrap_or_else(|| url.to_string());
                    format!("{url} {descriptor}").trim_end().to_string()
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            match self.link(&value, dir, assets) {
                Some(new) => new,
                None => return tag.to_string(),
            }
        };
        format!(
            "{}{}{}",
            &tag[..start],
            escape_attr(&new),
            &tag[start + len..]
        )
    }

    /// The new URL for a link from a page in `dir`, if it changes.
    fn link(&self, href: &str, dir: &str, assets: &mut BTreeSet<String>) -> Option<String> {
        let (rel, suffix) = local_target(href, dir)?;
        let abs = self.root.join(&rel);
        let target = if self.folders.contains(&rel) && abs.is_dir() {
            join(&rel, "index.html")
        } else if self.markdown.contains(rel.as_str()) {
            html_name(&rel)
        } else if abs.is_file() && !docs::is_markdown(&abs) {
            assets.insert(rel.clone());
            rel
        } else {
            return None;
        };
        Some(relative_url(dir, &target) + suffix)
    }
}

/// For a link to a local path: (its path relative to the root, the
/// `?query#fragment` after it). `None` for web links, fragments and paths
/// that leave the root.
fn local_target<'h>(href: &'h str, dir: &str) -> Option<(String, &'h str)> {
    if href.is_empty() || href.starts_with('#') || href.starts_with("//") || has_scheme(href) {
        return None;
    }
    let split = href.find(['?', '#']).unwrap_or(href.len());
    let (path, suffix) = href.split_at(split);
    let path = percent_decode_str(path).decode_utf8().ok()?;
    let joined = match path.strip_prefix('/') {
        Some(abs) => abs.to_string(),
        None => join(dir, &path),
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some((parts.join("/"), suffix))
}

fn has_scheme(href: &str) -> bool {
    match href.find(':') {
        Some(colon) => {
            let scheme = &href[..colon];
            scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        }
        None => false,
    }
}

/// A URL for `to` (relative to the root) from a page in `dir`.
fn relative_url(dir: &str, to: &str) -> String {
    let from: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let to: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut url = "../".repeat(from.len() - common);
    let rest: Vec<String> = to[common..]
        .iter()
        .map(|s| utf8_percent_encode(s, SEGMENT).to_string())
        .collect();
    url.push_str(&rest.join("/"));
    // "a:b.html" would read as a URL scheme.
    if url.split('/').next().is_some_and(|s| s.contains(':')) {
        url.insert_str(0, "./");
    }
    if url.is_empty() { "./".into() } else { url }
}

fn copy_assets(root: &Path, out: &Path, assets: &BTreeSet<String>) -> Result<usize> {
    let mut copied = 0;
    for rel in assets {
        // Never copy from outside the folder (through a symlink) or from
        // the output itself.
        let Ok(src) = root.join(rel).canonical() else {
            continue;
        };
        if !src.starts_with(root) || src.starts_with(out) {
            continue;
        }
        let dest = out.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(&src, &dest).with_context(|| format!("cannot copy {}", src.display()))?;
        copied += 1;
    }
    Ok(copied)
}

/// `_mdv/`: the file list for the sidebar and the libraries pages need.
fn write_support(out: &Path, files: &[String], libs: &Libs) -> Result<()> {
    let dir = out.join(SITE_DIR);
    std::fs::create_dir_all(&dir)?;
    let list = serde_json::to_string(files)?;
    std::fs::write(
        dir.join("files.js"),
        format!("window.mdvSiteFiles = {list};\n"),
    )?;
    if libs.math {
        std::fs::write(dir.join("katex.js"), crate::render::KATEX_JS)?;
        std::fs::write(dir.join("katex.css"), crate::render::KATEX_CSS)?;
    }
    if libs.mermaid {
        std::fs::write(dir.join("mermaid.js"), crate::render::mermaid_js())?;
    }
    for id in &libs.others {
        if let Some(lib) = crate::libraries::by_id(id) {
            std::fs::write(dir.join(format!("{id}.js")), crate::libraries::js(lib))?;
        }
    }
    // GitHub Pages runs Jekyll, which hides folders starting with "_".
    std::fs::write(out.join(".nojekyll"), "")?;
    Ok(())
}

/// `a/b.md` → `a/b.html`; other pages keep their extension, so they
/// can't clash (`data.csv` → `data.csv.html`).
fn html_name(rel: &str) -> String {
    if !docs::is_markdown(Path::new(rel)) {
        return format!("{rel}.html");
    }
    let stem = match rel.rfind('.') {
        Some(dot) if dot > rel.rfind('/').map_or(0, |s| s + 1) => &rel[..dot],
        _ => rel,
    };
    format!("{stem}.html")
}

/// The folder part of a root-relative path (`""` at the root).
fn parent(rel: &str) -> &str {
    rel.rfind('/').map_or("", |s| &rel[..s])
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_targets_and_relative_urls() {
        assert_eq!(local_target("b.md#x", "a"), Some(("a/b.md".into(), "#x")));
        assert_eq!(
            local_target("../c%20d.md", "a"),
            Some(("c d.md".into(), ""))
        );
        assert_eq!(
            local_target("/img/p.png?v=1", "a/b"),
            Some(("img/p.png".into(), "?v=1"))
        );
        assert_eq!(local_target("../../x.md", "a"), None);
        assert_eq!(local_target("https://x.org/a.md", ""), None);
        assert_eq!(local_target("mailto:a@b.c", ""), None);
        assert_eq!(local_target("#top", ""), None);
        assert_eq!(relative_url("a/b", "a/c.html"), "../c.html");
        assert_eq!(relative_url("", "a b/index.html"), "a%20b/index.html");
        assert_eq!(relative_url("a", "a/x.html"), "x.html");
        assert_eq!(relative_url("", "c:d.html"), "./c:d.html");
        assert_eq!(html_name("a.b/README.markdown"), "a.b/README.html");
        assert_eq!(html_name("a.b/notes.csv"), "a.b/notes.csv.html");
    }

    #[test]
    fn exports_pages_folders_and_assets() {
        let tmp = std::env::temp_dir().join(format!("mdv-site-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let src = tmp.join("src");
        std::fs::create_dir_all(src.join("guide/deep")).unwrap();
        std::fs::create_dir_all(src.join("img")).unwrap();
        std::fs::write(
            src.join("README.md"),
            "# Home\n\n[Guide](guide/) · [Intro](guide/intro.md#start) · \
             [web](https://example.com/a.md) · [missing](nope.md)\n\n![logo](img/logo.png)\n\n<picture><source srcset=\"img/dark.png 2x, /img/logo.png\"><img src=\"img/logo.png\"></picture>\n",
        )
        .unwrap();
        std::fs::write(
            src.join("guide/intro.md"),
            "# Intro\n\n[Home](../README.md) [Deep](/guide/deep/x.md)\n",
        )
        .unwrap();
        std::fs::write(src.join("guide/deep/x.md"), "# X\n\n$x^2$\n").unwrap();
        std::fs::write(src.join("img/logo.png"), "png").unwrap();
        std::fs::write(src.join("img/unused.png"), "png").unwrap();
        std::fs::write(src.join("img/dark.png"), "png").unwrap();
        let src = src.canonical().unwrap();
        let out = tmp.join("site");

        let renderer = Renderer::default();
        let page = |title: String| PageOptions {
            title,
            ..Default::default()
        };
        let summary = export(&renderer, false, &src, &out, &page).unwrap();
        assert_eq!((summary.pages, summary.files), (5, 2));
        let read = |p: &str| std::fs::read_to_string(out.join(p)).unwrap();

        let home = read("index.html");
        assert!(read("README.html").contains("href=\"guide/intro.html#start\""));
        assert!(home.contains("href=\"guide/index.html\""), "{home}");
        assert!(home.contains("href=\"guide/intro.html#start\""));
        assert!(home.contains("href=\"https://example.com/a.md\""));
        assert!(home.contains("href=\"nope.md\""));
        assert!(home.contains("src=\"img/logo.png\""));
        assert!(
            home.contains("srcset=\"img/dark.png 2x, img/logo.png\""),
            "{home}"
        );
        assert!(home.contains("data-site-root=\"\""));
        let intro = read("guide/intro.html");
        assert!(intro.contains("href=\"../README.html\""), "{intro}");
        assert!(intro.contains("href=\"deep/x.html\""));
        assert!(intro.contains("src=\"../_mdv/files.js\""));
        // A generated listing for folders without a README.
        assert!(read("guide/index.html").contains("href=\"intro.html\""));
        assert!(read("guide/deep/index.html").contains("href=\"x.html\""));
        // Math is loaded from the shared copy; no Mermaid is needed.
        assert!(read("guide/deep/x.html").contains("data-katex-js=\"../../_mdv/katex.js\""));
        assert!(out.join("_mdv/katex.js").is_file());
        assert!(!out.join("_mdv/mermaid.js").exists());
        assert!(read("_mdv/files.js").contains("\"guide/deep/x.md\""));
        assert!(out.join("img/logo.png").is_file() && !out.join("img/unused.png").exists());
        assert!(out.join(".nojekyll").is_file());

        // Exporting again over the earlier export works; a folder with
        // other things in it, or the source itself, is refused.
        export(&renderer, false, &src, &out, &page).unwrap();
        assert!(export(&renderer, false, &src, &src.join("img"), &page).is_err());
        assert!(export(&renderer, false, &src, &src, &page).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
