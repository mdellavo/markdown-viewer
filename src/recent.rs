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

/// The Recent page's Markdown. Items under `root` (what the page is
/// served from) link to their pages; others are listed by path.
pub fn page(list: &[RecentItem], root: &Path) -> String {
    page_at(list, root, now(), dirs_home().as_deref())
}

fn dirs_home() -> Option<std::path::PathBuf> {
    crate::platform::home()
}

fn page_at(list: &[RecentItem], root: &Path, now: u64, home: Option<&Path>) -> String {
    let mut md = String::from("# Recent\n\n");
    let items: Vec<&RecentItem> = list
        .iter()
        .filter(|r| Path::new(&r.path).exists())
        .collect();
    if items.is_empty() {
        md.push_str(
            "Nothing yet. Files and folders you open with markdown-viewer will be listed here.\n",
        );
        return md;
    }
    md.push_str("Files and folders you've opened, newest first.\n\n");
    md.push_str("| Name | Folder | Opened |\n| --- | --- | --- |\n");
    for item in items {
        let path = Path::new(&item.path);
        let is_dir = path.is_dir();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| item.path.clone());
        let shown = if is_dir {
            format!("📁 {name}/")
        } else {
            name
        };
        let label = cell(&shown);
        let name_cell = match path.strip_prefix(root) {
            Ok(rel) => {
                let url: Vec<String> = rel
                    .iter()
                    .map(|seg| utf8_percent_encode(&seg.to_string_lossy(), LINK).to_string())
                    .collect();
                let slash = if is_dir && !url.is_empty() { "/" } else { "" };
                format!("[{label}](/{}{slash})", url.join("/"))
            }
            Err(_) => label,
        };
        let folder = path.parent().map(|p| tilde(p, home)).unwrap_or_default();
        md.push_str(&format!(
            "| {name_cell} | {} | {} |\n",
            cell(&folder),
            ago(now.saturating_sub(item.at))
        ));
    }
    md.push_str("\nTo stop remembering files, set `recent = false` in the settings file.\n");
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

/// Text for a table cell: Markdown and the cell separator escaped.
fn cell(text: &str) -> String {
    crate::docs::escape_md(text)
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
        assert!(
            md.contains(&format!(
                "| [a b\\.md](/{url}/a%20b.md) | \\~/{shown} | just now |"
            )),
            "{md}"
        );
        assert!(
            md.contains(&format!("| [📁 {shown}/](/{url}/) | \\~ | 2 hours ago |")),
            "{md}"
        );
        assert!(md.contains("| o\\.md | "), "{md}");
        assert!(!md.contains("gone"));
        assert!(page_at(&[], &home, 0, None).contains("Nothing yet"));
    }
}
