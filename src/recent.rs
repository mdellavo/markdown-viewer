//! Recently opened files and folders: remembered in the state file when
//! you open one, and shown on the Recent page (`--recent`, or running
//! `markdown-viewer` with no arguments in your home folder).

use std::path::Path;

use percent_encoding::{AsciiSet, utf8_percent_encode};

use crate::config::{Prefs, RecentItem};

/// Characters to encode in a path segment of a Markdown link (also inside
/// a table cell).
pub(crate) const LINK: &AsciiSet = &crate::obsidian::SEGMENT
    .add(b'|')
    .add(b'(')
    .add(b')')
    .add(b'[')
    .add(b']')
    .add(b'\\');

/// How many to remember.
pub const MAX: usize = 30;

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Remember that `path` (a file or folder) was opened.
pub fn record(path: &Path) {
    // Best effort: an unwritable state file just means no Recent list.
    let _ = Prefs::update(|prefs| add(&mut prefs.recent, path, now()));
}

fn add(list: &mut Vec<RecentItem>, path: &Path, at: u64) {
    let path = path.to_string_lossy().into_owned();
    list.retain(|r| r.path != path);
    list.insert(0, RecentItem { path, at });
    list.truncate(MAX);
}

/// One remembered item, as the Recent page and the sidebar show it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Shown {
    /// The full path (what removing and opening refer to).
    pub path: String,
    pub name: String,
    /// The folder it's in, with `~` for the home folder.
    pub folder: String,
    /// `folder`, `markdown`, `table` (CSV, TSV) or `file`.
    pub kind: &'static str,
    pub ago: String,
    /// When, roughly: `Past day`, `Past week`, `Past month` or `Older`.
    pub group: &'static str,
    /// Its page in this viewer, if it's inside the served folder.
    pub href: Option<String>,
}

/// The remembered items that still exist, newest first. Those under
/// `root` (what the viewer serves) get a link to their page.
pub fn shown(list: &[RecentItem], root: &Path) -> Vec<Shown> {
    shown_at(list, root, now(), dirs_home().as_deref())
}

fn dirs_home() -> Option<std::path::PathBuf> {
    crate::platform::home()
}

fn shown_at(list: &[RecentItem], root: &Path, now: u64, home: Option<&Path>) -> Vec<Shown> {
    list.iter()
        .filter(|r| Path::new(&r.path).exists())
        .map(|item| {
            let path = Path::new(&item.path);
            let is_dir = path.is_dir();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| item.path.clone());
            let href = path.strip_prefix(root).ok().map(|rel| {
                let url: Vec<String> = rel
                    .iter()
                    .map(|seg| utf8_percent_encode(&seg.to_string_lossy(), LINK).to_string())
                    .collect();
                let slash = if is_dir && !url.is_empty() { "/" } else { "" };
                format!("/{}{slash}", url.join("/"))
            });
            let kind = if is_dir {
                "folder"
            } else if crate::docs::is_markdown(path) {
                "markdown"
            } else if crate::docs::is_viewable(path) {
                "table"
            } else {
                "file"
            };
            let secs = now.saturating_sub(item.at);
            Shown {
                path: item.path.clone(),
                name,
                folder: path.parent().map(|p| tilde(p, home)).unwrap_or_default(),
                kind,
                ago: ago(secs),
                group: match secs {
                    0..86_400 => "Past day",
                    86_400..604_800 => "Past week",
                    604_800..2_592_000 => "Past month",
                    _ => "Older",
                },
                href,
            }
        })
        .collect()
}

/// Remove `path` from the remembered list.
pub fn forget(path: &str) -> anyhow::Result<()> {
    Prefs::update(|prefs| prefs.recent.retain(|r| r.path != path))
}

/// The Recent page's Markdown: a heading, then the list as HTML (rows with
/// an icon, the name and folder, when, and a remove button; ui.js makes
/// the buttons work and opens items outside the served folder).
pub fn page(list: &[RecentItem], root: &Path) -> String {
    page_at(list, root, now(), dirs_home().as_deref())
}

const ICON_FOLDER: &str = include_str!("../assets/octicons/file-directory-16.svg");
const ICON_MARKDOWN: &str = include_str!("../assets/octicons/book-16.svg");
const ICON_TABLE: &str = include_str!("../assets/octicons/table-16.svg");
const ICON_FILE: &str = include_str!("../assets/octicons/file-16.svg");
const ICON_REMOVE: &str = include_str!("../assets/octicons/x-16.svg");

fn svg(icon: &str) -> String {
    icon.trim()
        .replacen("<svg ", "<svg aria-hidden=\"true\" focusable=\"false\" ", 1)
}

fn page_at(list: &[RecentItem], root: &Path, now: u64, home: Option<&Path>) -> String {
    use crate::render::{escape_attr, escape_text};
    let mut md = String::from("# Recent\n\n");
    let items = shown_at(list, root, now, home);
    if items.is_empty() {
        md.push_str(
            "Nothing yet. Files and folders you open with markdown-viewer will be listed here.\n",
        );
        return md;
    }
    md.push_str("Files and folders you've opened, newest first.\n\n");
    // One HTML block: no blank lines inside.
    md.push_str("<div class=\"mdv-recent\">\n");
    let mut group = "";
    for item in &items {
        if item.group != group {
            if !group.is_empty() {
                md.push_str("</div>\n");
            }
            group = item.group;
            md.push_str(&format!(
                "<div class=\"mdv-recent-group\">{group}</div>\n<div class=\"mdv-recent-list\">\n"
            ));
        }
        let icon = match item.kind {
            "folder" => ICON_FOLDER,
            "markdown" => ICON_MARKDOWN,
            "table" => ICON_TABLE,
            _ => ICON_FILE,
        };
        let name = if item.kind == "folder" {
            format!("{}/", item.name)
        } else {
            item.name.clone()
        };
        // Outside the served folder: ui.js asks the viewer to open it.
        let href = match &item.href {
            Some(h) => format!("href=\"{}\"", escape_attr(h)),
            None => "href=\"#\" data-mdv-recent-open".to_string(),
        };
        md.push_str(&format!(
            "<div class=\"mdv-recent-item\" data-path=\"{path}\">\
             <a class=\"mdv-recent-link\" {href} title=\"{path}\">\
             <span class=\"mdv-recent-icon\">{icon}</span>\
             <span class=\"mdv-recent-text\"><span class=\"mdv-recent-name\">{name}</span>\
             <span class=\"mdv-recent-folder\">{folder}</span></span>\
             <span class=\"mdv-recent-when\">{ago}</span></a>\
             <button type=\"button\" class=\"mdv-recent-remove\" title=\"Remove from Recent\" \
             aria-label=\"Remove {name_attr} from Recent\">{remove}</button></div>\n",
            path = escape_attr(&item.path),
            icon = svg(icon),
            name = escape_text(&name),
            name_attr = escape_attr(&name),
            folder = escape_text(&item.folder),
            ago = escape_text(&item.ago),
            remove = svg(ICON_REMOVE),
        ));
    }
    md.push_str("</div>\n</div>\n\n");
    md.push_str(
        "Remove an item with its × button. To stop remembering files, set \
         `recent = false` in the settings file.\n",
    );
    md
}

/// `~/x` for paths in the home folder.
fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rel) if rel.as_os_str().is_empty() => "~".into(),
        Some(rel) => format!("~/{}", rel.display()),
        None => path.display().to_string(),
    }
}

fn ago(secs: u64) -> String {
    let (n, unit) = match secs {
        0..60 => return "just now".into(),
        60..3600 => (secs / 60, "minute"),
        3600..86_400 => (secs / 3600, "hour"),
        86_400..2_592_000 => (secs / 86_400, "day"),
        _ => (secs / 2_592_000, "month"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Canonical;

    #[test]
    fn keeps_the_newest_without_duplicates() {
        let mut list = Vec::new();
        for i in 0..MAX + 5 {
            add(&mut list, Path::new(&format!("/f{i}.md")), i as u64);
        }
        add(&mut list, Path::new("/f10.md"), 99);
        assert_eq!(list.len(), MAX);
        assert_eq!(list[0].path, "/f10.md");
        assert_eq!(list.iter().filter(|r| r.path == "/f10.md").count(), 1);
    }

    #[test]
    fn page_links_what_it_can() {
        // A folder name that needs escaping in both Markdown and URLs
        // (Windows doesn't allow `|` in names).
        let (folder, url, shown) = if cfg!(windows) {
            ("notes [x]", "notes%20%5Bx%5D", "notes \\[x\\]")
        } else {
            ("notes|x", "notes%7Cx", "notes\\|x")
        };
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonical().unwrap();
        std::fs::create_dir_all(home.join(folder)).unwrap();
        std::fs::write(home.join(folder).join("a b.md"), "# A").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let out = outside.path().canonical().unwrap().join("o.md");
        std::fs::write(&out, "# O").unwrap();
        let list = vec![
            RecentItem {
                path: home.join(folder).join("a b.md").to_string_lossy().into(),
                at: 10_000,
            },
            RecentItem {
                path: home.join(folder).to_string_lossy().into(),
                at: 10_000 - 7200,
            },
            RecentItem {
                path: out.to_string_lossy().into(),
                at: 0,
            },
            RecentItem {
                path: home.join("gone.md").to_string_lossy().into(),
                at: 0,
            },
        ];
        let md = page_at(&list, &home, 10_030, Some(&home));
        let items = shown_at(&list, &home, 10_030, Some(&home));
        assert_eq!(items.len(), 3, "the missing file is left out");
        assert_eq!(
            items[0].href.as_deref(),
            Some(format!("/{url}/a%20b.md").as_str())
        );
        assert_eq!(items[0].folder, format!("~/{folder}"));
        assert_eq!(
            (items[0].kind, items[0].ago.as_str()),
            ("markdown", "just now")
        );
        assert_eq!(items[1].href.as_deref(), Some(format!("/{url}/").as_str()));
        assert_eq!((items[1].kind, items[1].folder.as_str()), ("folder", "~"));
        assert_eq!(items[1].ago, "2 hours ago");
        assert_eq!(items[2].href, None, "outside the served folder");
        assert!(md.contains(&format!("href=\"/{url}/a%20b.md\"")), "{md}");
        assert!(md.contains("data-mdv-recent-open"), "{md}");
        assert!(md.contains("class=\"mdv-recent-remove\""));
        assert!(md.contains("<div class=\"mdv-recent-group\">Past day</div>"));
        assert!(!md.contains("gone"));
        // One HTML block, so Markdown leaves it alone.
        let block = &md[md.find("<div").unwrap()..md.rfind("</div>").unwrap()];
        assert!(!block.contains("\n\n"));
        let _ = shown;
        assert!(page_at(&[], &home, 0, None).contains("Nothing yet"));
    }
}
