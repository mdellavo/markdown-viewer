//! The page footer: when the file was last changed in git and by whom, and
//! an "Edit on GitHub" link (as Docusaurus and VitePress show). Only for
//! files committed to a git repository; the link only when its remote is on
//! GitHub and a branch is checked out. Turned off with `last_updated =
//! false` in the settings file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::render::{escape_attr, escape_text};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageMeta {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub author: String,
    pub edit_url: Option<String>,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Characters kept as they are in a path segment of the link.
const SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'.')
    .remove(b'-')
    .remove(b'_')
    .remove(b'~');

/// The footer details for `file`, if it's in git. `repo` is the GitHub
/// address from the settings file, if one is set.
pub fn for_file(file: &Path, repo: Option<&str>) -> Option<PageMeta> {
    let dir = file.parent()?;
    let name = file.file_name()?.to_str()?;
    let log = git(dir, &["log", "-1", "--format=%as%x00%an", "--", name])?;
    let (date, author) = log.split_once('\0')?;
    let edit_url = (|| {
        let base = repo
            .map(str::to_string)
            .or_else(|| crate::references::detect(dir))?;
        let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|b| b != "HEAD")?;
        let path = git(dir, &["ls-files", "--full-name", "--", name])?;
        let path: Vec<String> = path
            .lines()
            .next()?
            .split('/')
            .map(|p| percent_encoding::utf8_percent_encode(p, SEGMENT).to_string())
            .collect();
        // Branch names may have slashes (`feature/x`); GitHub takes them as is.
        let branch: Vec<String> = branch
            .split('/')
            .map(|p| percent_encoding::utf8_percent_encode(p, SEGMENT).to_string())
            .collect();
        let branch = branch.join("/");
        Some(format!("{base}/edit/{branch}/{}", path.join("/")))
    })();
    Some(PageMeta {
        date: date.to_string(),
        author: author.to_string(),
        edit_url,
    })
}

/// How long served pages reuse a file's details (they change on commit,
/// not on save).
const CACHE_FOR: Duration = Duration::from_secs(30);

type Cache = Mutex<HashMap<PathBuf, (Instant, Option<PageMeta>)>>;

/// `for_file`, remembered for a while (served pages ask on every request).
pub fn cached(file: &Path, repo: Option<&str>) -> Option<PageMeta> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some((at, meta)) = cache.lock().unwrap().get(file)
        && at.elapsed() < CACHE_FOR
    {
        return meta.clone();
    }
    let meta = for_file(file, repo);
    cache
        .lock()
        .unwrap()
        .insert(file.to_path_buf(), (Instant::now(), meta.clone()));
    meta
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `2026-11-27` → `27 November 2026`.
fn long_date(date: &str) -> String {
    let mut parts = date.split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return date.to_string();
    };
    match (m.parse::<usize>(), d.parse::<u32>()) {
        (Ok(m @ 1..=12), Ok(d)) => format!("{d} {} {y}", MONTHS[m - 1]),
        _ => date.to_string(),
    }
}

/// The footer shown after the article.
pub fn footer(meta: &PageMeta) -> String {
    let mut html = format!(
        "<footer class=\"mdv-page-meta\"><span>Last updated on <time datetime=\"{}\">{}</time> by {}</span>",
        escape_attr(&meta.date),
        escape_text(&long_date(&meta.date)),
        escape_text(&meta.author)
    );
    if let Some(url) = &meta.edit_url {
        html.push_str(&format!(
            "<a href=\"{}\" target=\"_blank\" rel=\"noopener\">Edit on GitHub</a>",
            escape_attr(url)
        ));
    }
    html.push_str("</footer>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_shows_date_author_and_link() {
        let html = footer(&PageMeta {
            date: "2026-11-27".into(),
            author: "A <B>".into(),
            edit_url: Some("https://github.com/o/r/edit/main/docs/a%20b.md".into()),
        });
        assert_eq!(
            html,
            "<footer class=\"mdv-page-meta\"><span>Last updated on <time datetime=\"2026-11-27\">27 November 2026</time> \
             by A &lt;B&gt;</span><a href=\"https://github.com/o/r/edit/main/docs/a%20b.md\" target=\"_blank\" \
             rel=\"noopener\">Edit on GitHub</a></footer>\n"
        );
        assert_eq!(long_date("bad"), "bad");
    }

    #[test]
    fn details_come_from_git() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(d)
                    .args(args)
                    .env("GIT_AUTHOR_DATE", "2026-01-02T03:04:05Z")
                    .env("GIT_COMMITTER_DATE", "2026-01-02T03:04:05Z")
                    .output()
                    .unwrap()
                    .status
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.name", "Ada Lovelace"]);
        run(&["config", "user.email", "ada@example.com"]);
        run(&["remote", "add", "origin", "git@github.com:o/r.git"]);
        std::fs::create_dir(d.join("docs")).unwrap();
        std::fs::write(d.join("docs/my notes.md"), "# Hi\n").unwrap();
        std::fs::write(d.join("new.md"), "# New\n").unwrap();
        run(&["add", "docs"]);
        run(&["commit", "-q", "-m", "x"]);
        let meta = for_file(&d.join("docs/my notes.md"), None).unwrap();
        assert_eq!(meta.date, "2026-01-02");
        assert_eq!(meta.author, "Ada Lovelace");
        assert_eq!(
            meta.edit_url.as_deref(),
            Some("https://github.com/o/r/edit/main/docs/my%20notes.md")
        );
        assert_eq!(for_file(&d.join("new.md"), None), None, "never committed");
    }
}
