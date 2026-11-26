//! `--check-links`: find broken links and images in Markdown files.
//!
//! Checked offline: relative files (including other Markdown files),
//! `#anchors` in the same or another document, and Obsidian wiki links.
//! With `--online`, http(s) links are also requested (through `curl`).

use crate::platform::Canonical;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use percent_encoding::percent_decode_str;
use serde::Serialize;

use crate::render::Renderer;

/// A link found in rendered HTML.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub target: String,
    pub line: u32,
    pub image: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Broken {
    pub line: u32,
    pub target: String,
    pub reason: String,
}

/// Every `href` and `src` in rendered HTML, with the source line of the
/// nearest element carrying a position (needs `sourcepos` rendering).
pub fn extract(html: &str) -> Vec<Link> {
    let mut links = Vec::new();
    let mut line = 1;
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        let after = &rest[at + 1..];
        let end = after.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        if let Some(pos) = attr(tag, "data-sourcepos")
            && let Some(l) = pos.split(':').next().and_then(|l| l.parse().ok())
        {
            line = l;
        }
        let name = tag.split_whitespace().next().unwrap_or("");
        let wanted = match name {
            "a" => Some(("href", false)),
            "img" => Some(("src", true)),
            _ => None,
        };
        if let Some((attr_name, image)) = wanted
            && let Some(target) = attr(tag, attr_name)
            && !tag.contains("class=\"anchor\"")
        {
            links.push(Link {
                target: unescape(&target),
                line,
                image,
            });
        }
        if tag.contains("mdv-wikilink-missing") {
            let text = after[end + 1..].split('<').next().unwrap_or("").to_string();
            links.push(Link {
                target: format!("[[{}]]", unescape(&text)),
                line,
                image: false,
            });
        }
        rest = &after[end.min(after.len())..];
    }
    links
}

/// Checks links across many files, caching each document's anchors.
pub struct Checker {
    renderer: Arc<Renderer>,
    root: PathBuf,
    files: Vec<String>,
    anchors: Mutex<HashMap<PathBuf, Option<HashSet<String>>>>,
}

impl Checker {
    pub fn new(renderer: Arc<Renderer>, root: PathBuf) -> Self {
        let (files, _) = crate::docs::markdown_files(&root);
        Self {
            renderer,
            root,
            files,
            anchors: Mutex::default(),
        }
    }

    /// Render a file the way the viewer would (for its links and anchors).
    pub fn render(&self, file: &Path) -> Option<String> {
        let text = std::fs::read_to_string(file).ok()?;
        let dir = file.parent()?;
        let ctx = crate::obsidian::LinkContext {
            root: &self.root,
            dir,
            files: &self.files,
            served: false,
            depth: 0,
        };
        Some(self.renderer.render_doc(&text, Some(&ctx)))
    }

    /// Broken local links in one file, the web links (checked separately,
    /// with `--online`), and how many links there were in all.
    pub fn check(&self, file: &Path) -> (Vec<Broken>, Vec<Link>, usize) {
        let Some(html) = self.render(file) else {
            return (Vec::new(), Vec::new(), 0);
        };
        self.anchors
            .lock()
            .unwrap()
            .insert(file.to_path_buf(), Some(ids(&html)));
        let dir = file.parent().unwrap_or(&self.root);
        let mut broken = Vec::new();
        let mut external = Vec::new();
        let links = extract(&html);
        let total = links.len();
        for link in links {
            let t = link.target.as_str();
            let lower = t.to_ascii_lowercase();
            if let Some(name) = t.strip_prefix("[[") {
                broken.push(Broken {
                    line: link.line,
                    target: t.to_string(),
                    reason: format!("no note named {:?}", name.trim_end_matches("]]")),
                });
            } else if lower.starts_with("http://") || lower.starts_with("https://") {
                external.push(link);
            } else if ["mailto:", "tel:", "data:", "javascript:", "ftp:", "//"]
                .iter()
                .any(|p| lower.starts_with(p))
                || t.is_empty()
            {
                continue;
            } else if let Some(reason) = self.check_local(file, dir, t) {
                broken.push(Broken {
                    line: link.line,
                    target: t.to_string(),
                    reason,
                });
            }
        }
        (broken, external, total)
    }

    /// Why a relative link is broken, or None if it's fine.
    fn check_local(&self, file: &Path, dir: &Path, target: &str) -> Option<String> {
        let (path, fragment) = match target.split_once('#') {
            Some((p, f)) => (p, Some(f)),
            None => (target, None),
        };
        let path = path.split('?').next().unwrap_or("");
        let resolved = if path.is_empty() {
            file.to_path_buf()
        } else {
            let decoded = percent_decode_str(path).decode_utf8_lossy();
            if let Some(abs) = decoded.strip_prefix('/') {
                self.root.join(abs)
            } else {
                dir.join(decoded.as_ref())
            }
        };
        if !resolved.exists() {
            return Some("file not found".into());
        }
        let fragment = fragment.filter(|f| !f.is_empty() && !f.starts_with("mdv-line-"))?;
        if resolved.is_dir() || !crate::docs::is_markdown(&resolved) {
            return None;
        }
        let fragment = percent_decode_str(fragment)
            .decode_utf8_lossy()
            .into_owned();
        let anchors = self.anchors_of(&resolved)?;
        (!anchors.contains(&fragment)).then(|| format!("no heading or anchor #{fragment}"))
    }

    fn anchors_of(&self, file: &Path) -> Option<HashSet<String>> {
        let key = file.canonical().unwrap_or_else(|_| file.to_path_buf());
        if let Some(cached) = self.anchors.lock().unwrap().get(&key) {
            return cached.clone();
        }
        let ids = self.render(&key).map(|html| ids(&html));
        self.anchors.lock().unwrap().insert(key, ids.clone());
        ids
    }
}

/// Every `id` and `name` in rendered HTML.
fn ids(html: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for marker in [" id=\"", " name=\""] {
        let mut rest = html;
        while let Some(at) = rest.find(marker) {
            let after = &rest[at + marker.len()..];
            let end = after.find('"').unwrap_or(after.len());
            out.insert(unescape(&after[..end]));
            rest = &after[end..];
        }
    }
    out
}

/// Request each URL once (in parallel) with curl; returns url → error for
/// the broken ones.
pub fn check_online(urls: &BTreeSet<String>) -> Result<HashMap<String, String>, String> {
    if Command::new("curl").arg("--version").output().is_err() {
        return Err("--online needs curl".into());
    }
    let queue = Arc::new(Mutex::new(urls.iter().cloned().collect::<Vec<_>>()));
    let failures = Arc::new(Mutex::new(HashMap::new()));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let queue = queue.clone();
            let failures = failures.clone();
            std::thread::spawn(move || {
                loop {
                    let Some(url) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    if let Some(err) = fetch_status(&url) {
                        failures.lock().unwrap().insert(url, err);
                    }
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    Ok(Arc::try_unwrap(failures)
        .map(|m| m.into_inner().unwrap())
        .unwrap_or_default())
}

/// None if the URL answers with a success or redirect; else why not. Tries
/// a HEAD request, then GET (some servers refuse HEAD).
fn fetch_status(url: &str) -> Option<String> {
    let mut last = String::new();
    // HEAD first; then GET, for servers that refuse HEAD.
    for head in [true, false] {
        let out = Command::new("curl")
            .args([
                "-sS",
                "-L",
                "--max-time",
                "15",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
            ])
            .args(if head { &["-I"][..] } else { &[] })
            .arg("-A")
            .arg("markdown-viewer link checker")
            .arg(url)
            .output()
            .ok()?;
        let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if code.starts_with('2') || code.starts_with('3') {
            return None;
        }
        last = if code == "000" {
            String::from_utf8_lossy(&out.stderr)
                .trim()
                .trim_start_matches("curl: ")
                .to_string()
        } else {
            format!("HTTP {code}")
        };
    }
    Some(last)
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = at + tag[at..].find('"')?;
    Some(tag[at..end].to_string())
}

pub fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::RenderOptions;

    #[test]
    fn extracts_links_with_lines() {
        let html = "<h1 data-sourcepos=\"1:1-1:3\">T<a href=\"#t\" class=\"anchor\"></a></h1>\n\
                    <p data-sourcepos=\"3:1-3:9\">x <a data-sourcepos=\"3:3-3:9\" href=\"a.md#b&amp;c\">a</a></p>\n\
                    <p data-sourcepos=\"5:1-5:5\"><img src=\"pic.png\" alt=\"\"></p>";
        assert_eq!(
            extract(html),
            [
                Link {
                    target: "a.md#b&c".into(),
                    line: 3,
                    image: false
                },
                Link {
                    target: "pic.png".into(),
                    line: 5,
                    image: true
                },
            ]
        );
    }

    #[test]
    fn finds_broken_files_and_anchors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonical().unwrap();
        std::fs::write(root.join("pic.png"), b"x").unwrap();
        std::fs::write(root.join("other.md"), "# Other\n\n## Real section\n").unwrap();
        std::fs::write(
            root.join("doc.md"),
            "# Doc\n\n[ok](other.md#real-section) [bad file](missing.md)\n\n\
             [bad anchor](other.md#nope) [here](#doc) [gone](#nowhere)\n\n\
             ![img](pic.png) ![missing](no.png) [web](https://example.com) [mail](mailto:a@b.c)\n",
        )
        .unwrap();
        let renderer = Arc::new(Renderer::with_options(RenderOptions {
            sourcepos: true,
            ..Default::default()
        }));
        let checker = Checker::new(renderer, root.clone());
        let (broken, external, _) = checker.check(&root.join("doc.md"));
        let found: Vec<_> = broken.iter().map(|b| (b.line, b.target.as_str())).collect();
        assert_eq!(
            found,
            [
                (3, "missing.md"),
                (5, "other.md#nope"),
                (5, "#nowhere"),
                (7, "no.png")
            ],
            "{broken:?}"
        );
        assert_eq!(external.len(), 1);
    }
}
