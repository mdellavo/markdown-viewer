//! The documents being viewed, kept rendered and up to date.
//!
//! A "document" is either a Markdown file or a directory (rendered as a
//! listing of its Markdown files and subfolders). Each one viewed gets a
//! `watch` channel of rendered HTML. Only the folders of viewed documents
//! are watched, non-recursively, so viewing a file in a huge tree is cheap.
//!
//! Folders are watched rather than files because many editors save by
//! writing a temp file and renaming it over the original, which would
//! silently end a watch placed on the file itself.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use tokio::sync::{mpsc, watch};

use crate::render::Renderer;

const DEBOUNCE: Duration = Duration::from_millis(100);
/// Limits for the file list, so a huge folder can't stall the viewer.
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 6;
/// Folders never worth listing.
const SKIP_DIRS: &[&str] = &["node_modules", "target", "vendor", "__pycache__"];

pub struct Docs {
    root: PathBuf,
    renderer: Arc<Renderer>,
    channels: Mutex<HashMap<PathBuf, watch::Sender<String>>>,
    watched: Mutex<HashSet<PathBuf>>,
    debouncer: Mutex<Debouncer<RecommendedWatcher, RecommendedCache>>,
}

impl Docs {
    /// Track documents under `root` (canonical). Must be called inside a
    /// Tokio runtime: changes are processed on a background task.
    pub fn new(root: PathBuf, renderer: Arc<Renderer>) -> Result<Arc<Self>> {
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<PathBuf>>();
        let debouncer = new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| {
            if let Ok(events) = res {
                let paths = events.into_iter().flat_map(|e| e.event.paths).collect();
                let _ = tx.send(paths);
            }
        })
        .context("failed to start file watcher")?;
        let docs = Arc::new(Self {
            root,
            renderer,
            channels: Mutex::default(),
            watched: Mutex::default(),
            debouncer: Mutex::new(debouncer),
        });
        let weak = Arc::downgrade(&docs);
        tokio::spawn(async move {
            while let Some(paths) = rx.recv().await {
                let Some(docs) = weak.upgrade() else { break };
                // Rendering is synchronous; keep it off the async workers.
                let _ = tokio::task::spawn_blocking(move || docs.changed(&paths)).await;
            }
        });
        Ok(docs)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Live, rendered HTML for a document (canonical path under the root).
    pub fn subscribe(&self, doc: &Path) -> Result<watch::Receiver<String>> {
        let mut channels = self.channels.lock().unwrap();
        if let Some(tx) = channels.get(doc) {
            return Ok(tx.subscribe());
        }
        let html = self.render(doc)?;
        let (tx, rx) = watch::channel(html);
        channels.insert(doc.to_path_buf(), tx);
        drop(channels);
        let dir = if doc.is_dir() {
            doc
        } else {
            doc.parent().unwrap_or(doc)
        };
        self.watch_dir(dir)?;
        Ok(rx)
    }

    fn watch_dir(&self, dir: &Path) -> Result<()> {
        let mut watched = self.watched.lock().unwrap();
        if watched.insert(dir.to_path_buf()) {
            self.debouncer
                .lock()
                .unwrap()
                .watch(dir, RecursiveMode::NonRecursive)
                .with_context(|| format!("failed to watch {}", dir.display()))?;
        }
        Ok(())
    }

    /// Re-render every tracked document affected by these changed paths: a
    /// file itself, or the folder it's in (for listings).
    fn changed(&self, paths: &[PathBuf]) {
        let mut affected = HashSet::new();
        for path in paths {
            let Some(parent) = path.parent().and_then(|p| p.canonicalize().ok()) else {
                continue;
            };
            if let Some(name) = path.file_name() {
                affected.insert(parent.join(name));
            }
            affected.insert(parent);
        }
        let channels = self.channels.lock().unwrap();
        for doc in affected {
            let Some(tx) = channels.get(&doc) else {
                continue;
            };
            // The file may briefly not exist mid-save; keep the last good render.
            if let Ok(html) = self.render(&doc) {
                tx.send_if_modified(|current| {
                    if *current == html {
                        false
                    } else {
                        *current = html;
                        true
                    }
                });
            }
        }
    }

    fn render(&self, doc: &Path) -> Result<String> {
        if doc.is_dir() {
            return Ok(self
                .renderer
                .render_body(&listing_markdown(&self.root, doc)));
        }
        let text = std::fs::read_to_string(doc)
            .with_context(|| format!("cannot read {}", doc.display()))?;
        Ok(self.renderer.render_body(&text))
    }

    /// Markdown files under the root, as `/`-separated relative paths: the
    /// root's own files first, then each folder's, alphabetically.
    pub fn list_files(&self) -> (Vec<String>, bool) {
        let mut files = Vec::new();
        let truncated = collect(&self.root, &self.root, 0, &mut files);
        files.sort_by(|a, b| {
            let depth = |s: &str| s.matches('/').count().min(1);
            (depth(a), a.to_lowercase()).cmp(&(depth(b), b.to_lowercase()))
        });
        (files, truncated)
    }
}

/// Recursively gather Markdown files; returns true if a limit was hit.
fn collect(root: &Path, dir: &Path, depth: usize, out: &mut Vec<String>) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= MAX_FILES {
            return true;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        // file_type() doesn't follow symlinks, so linked folders (which
        // could loop or leave the root) are skipped.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            if depth + 1 >= MAX_DEPTH || SKIP_DIRS.contains(&name.as_ref()) {
                continue;
            }
            if collect(root, &path, depth + 1, out) {
                return true;
            }
        } else if is_markdown(&path)
            && let Ok(rel) = path.strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    false
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "md" | "markdown" | "mdown" | "mkd" | "mkdn"
        )
    })
}

/// A folder's README, if it has one.
pub fn readme(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut names: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_markdown(p))
        .collect();
    names.sort();
    for wanted in ["readme", "index"] {
        if let Some(p) = names.iter().find(|p| {
            p.file_stem()
                .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(wanted))
        }) {
            return Some(p.clone());
        }
    }
    None
}

/// A generated page listing a folder's subfolders and Markdown files.
fn listing_markdown(root: &Path, dir: &Path) -> String {
    let title = if dir == root {
        dir.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into())
    } else {
        dir.strip_prefix(root)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            match entry.file_type() {
                Ok(t) if t.is_dir() && !SKIP_DIRS.contains(&name.as_str()) => dirs.push(name),
                Ok(t) if t.is_file() && is_markdown(&entry.path()) => files.push(name),
                _ => {}
            }
        }
    }
    dirs.sort_by_key(|n| n.to_lowercase());
    files.sort_by_key(|n| n.to_lowercase());

    let mut md = format!("# {}\n\n", escape_md(&title));
    if dir != root {
        md.push_str("[↑ Parent folder](../)\n\n");
    }
    if dirs.is_empty() && files.is_empty() {
        md.push_str("No Markdown files here.\n");
    }
    for d in &dirs {
        md.push_str(&format!("- 📁 [{}/](<{}/>)\n", escape_md(d), d));
    }
    for f in &files {
        md.push_str(&format!("- [{}](<{}>)\n", escape_md(f), f));
    }
    md
}

/// Escape characters with meaning in Markdown link text and headings.
fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\`*_{}[]()<>#+-.!|~$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        std::fs::write(r.join("README.md"), "# Root").unwrap();
        std::fs::write(r.join("notes.markdown"), "x").unwrap();
        std::fs::write(r.join("image.png"), "x").unwrap();
        std::fs::create_dir_all(r.join("docs/deep")).unwrap();
        std::fs::write(r.join("docs/guide.md"), "x").unwrap();
        std::fs::write(r.join("docs/deep/more.md"), "x").unwrap();
        std::fs::create_dir_all(r.join("node_modules/pkg")).unwrap();
        std::fs::write(r.join("node_modules/pkg/README.md"), "x").unwrap();
        std::fs::create_dir_all(r.join(".git")).unwrap();
        std::fs::write(r.join(".git/x.md"), "x").unwrap();
        dir
    }

    #[test]
    fn lists_markdown_files_root_first_skipping_noise() {
        let dir = tree();
        let mut files = Vec::new();
        let root = dir.path().canonicalize().unwrap();
        assert!(!collect(&root, &root, 0, &mut files));
        files.sort_by(|a, b| {
            let depth = |s: &str| s.matches('/').count().min(1);
            (depth(a), a.to_lowercase()).cmp(&(depth(b), b.to_lowercase()))
        });
        assert_eq!(
            files,
            [
                "notes.markdown",
                "README.md",
                "docs/deep/more.md",
                "docs/guide.md"
            ]
        );
    }

    #[test]
    fn finds_readme_case_insensitively() {
        let dir = tree();
        assert_eq!(
            readme(dir.path()).unwrap().file_name().unwrap(),
            "README.md"
        );
        assert_eq!(readme(&dir.path().join("docs")), None);
    }

    #[test]
    fn listing_links_folders_and_files() {
        let dir = tree();
        let root = dir.path();
        let md = listing_markdown(root, &root.join("docs"));
        assert!(md.starts_with("# docs\n"), "{md}");
        assert!(md.contains("[↑ Parent folder](../)"));
        assert!(md.contains("- 📁 [deep/](<deep/>)"));
        assert!(md.contains("- [guide\\.md](<guide.md>)"));
    }
}
