//! Blame on hover (<kbd>B</kbd>): who last changed each line of a file and
//! when, via `git blame --porcelain` (which includes changes not committed
//! yet, as the working file is blamed).

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Commit {
    /// Abbreviated hash; empty for lines not committed yet.
    pub hash: String,
    pub author: String,
    /// `YYYY-MM-DD`, in the author's time zone.
    pub date: String,
    /// Seconds since 1970, to find a block's latest change.
    pub time: i64,
    pub summary: String,
    /// The commit on its hosting site, when that's known.
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Ok,
    /// In a repository but never committed.
    Untracked,
    /// Not in a git repository (or git isn't installed).
    NotARepo,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Blame {
    pub status: Status,
    pub commits: Vec<Commit>,
    /// For each line of the file (the first is line 1), its commit's index
    /// in `commits`.
    pub lines: Vec<u32>,
}

impl Blame {
    fn empty(status: Status) -> Self {
        Self {
            status,
            commits: Vec::new(),
            lines: Vec::new(),
        }
    }
}

pub fn blame(file: &Path) -> Blame {
    let (Some(dir), Some(name)) = (file.parent(), file.file_name()) else {
        return Blame::empty(Status::NotARepo);
    };
    let git = |args: &[&std::ffi::OsStr]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("LC_ALL", "C")
            .output()
            .ok()
    };
    let out = git(&[
        "blame".as_ref(),
        "--porcelain".as_ref(),
        "--".as_ref(),
        name,
    ]);
    match out {
        Some(o) if o.status.success() => {
            let commit_url = commit_url(dir);
            parse(&String::from_utf8_lossy(&o.stdout), commit_url.as_ref())
        }
        _ => {
            let inside = git(&["rev-parse".as_ref(), "--is-inside-work-tree".as_ref()])
                .is_some_and(|o| o.status.success());
            Blame::empty(if inside {
                Status::Untracked
            } else {
                Status::NotARepo
            })
        }
    }
}

/// How to link a commit on the repository's hosting site: the address up
/// to the hash.
fn commit_url(dir: &Path) -> Option<String> {
    use crate::references::Forge;
    let (forge, base) = crate::references::detect_forge(dir)?;
    Some(match forge {
        Forge::GitHub | Forge::Gitea => format!("{base}/commit/"),
        Forge::GitLab => format!("{base}/-/commit/"),
        Forge::Bitbucket => format!("{base}/commits/"),
    })
}

const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

/// `git blame --porcelain` output: a header per line (`HASH ORIG FINAL
/// [COUNT]`), the commit's details the first time it appears, then the line
/// itself after a tab.
fn parse(text: &str, commit_url: Option<&String>) -> Blame {
    #[derive(Default)]
    struct Details {
        author: String,
        time: i64,
        tz: String,
        summary: String,
    }
    let mut details: HashMap<String, Details> = HashMap::new();
    let mut index: HashMap<String, u32> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut lines: Vec<(usize, String)> = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.starts_with('\t') {
            continue;
        }
        let mut words = line.split(' ');
        let first = words.next().unwrap_or("");
        if first.len() == 40 && first.bytes().all(|b| b.is_ascii_hexdigit()) {
            let final_line: usize = words.nth(1).and_then(|n| n.parse().ok()).unwrap_or(0);
            current = first.to_string();
            if !index.contains_key(&current) {
                index.insert(current.clone(), order.len() as u32);
                order.push(current.clone());
            }
            lines.push((final_line, current.clone()));
            continue;
        }
        let d = details.entry(current.clone()).or_default();
        let value = line.split_once(' ').map_or("", |(_, v)| v);
        match first {
            "author" => d.author = value.to_string(),
            "author-time" => d.time = value.parse().unwrap_or(0),
            "author-tz" => d.tz = value.to_string(),
            "summary" => d.summary = value.to_string(),
            _ => {}
        }
    }
    let commits = order
        .iter()
        .map(|hash| {
            let d = details.remove(hash).unwrap_or_default();
            let committed = hash != UNCOMMITTED;
            Commit {
                hash: if committed {
                    hash[..7].to_string()
                } else {
                    String::new()
                },
                author: if committed {
                    d.author
                } else {
                    "You".to_string()
                },
                date: date(d.time + tz_seconds(&d.tz)),
                time: d.time,
                summary: if committed {
                    d.summary
                } else {
                    "Not committed yet".to_string()
                },
                url: commit_url
                    .filter(|_| committed)
                    .map(|base| format!("{base}{hash}")),
            }
        })
        .collect();
    lines.sort_by_key(|(n, _)| *n);
    let lines = lines.into_iter().map(|(_, hash)| index[&hash]).collect();
    Blame {
        status: Status::Ok,
        commits,
        lines,
    }
}

/// `+0130` → 5400.
fn tz_seconds(tz: &str) -> i64 {
    let (sign, digits) = match tz.as_bytes().first() {
        Some(b'-') => (-1, &tz[1..]),
        Some(b'+') => (1, &tz[1..]),
        _ => return 0,
    };
    let n: i64 = digits.parse().unwrap_or(0);
    sign * ((n / 100) * 3600 + (n % 100) * 60)
}

/// Seconds since 1970 → `YYYY-MM-DD` (proleptic Gregorian; Howard
/// Hinnant's `civil_from_days`).
fn date(seconds: i64) -> String {
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        assert_eq!(date(1_798_761_600 - 1), "2026-12-31");
        assert_eq!(tz_seconds("-0130"), -5400);
    }

    #[test]
    fn blames_committed_and_new_lines() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(d)
                    .args(args)
                    .env("GIT_AUTHOR_DATE", "2026-01-02T23:30:00-0200")
                    .env("GIT_COMMITTER_DATE", "2026-01-02T23:30:00-0200")
                    .output()
                    .unwrap()
                    .status
                    .success()
            )
        };
        let file = d.join("a.md");
        assert_eq!(blame(&file).status, Status::NotARepo);
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.name", "Ada Lovelace"]);
        run(&["config", "user.email", "ada@example.com"]);
        run(&["remote", "add", "origin", "https://codeberg.org/o/r.git"]);
        std::fs::write(&file, "# A\n\nOne.\n").unwrap();
        assert_eq!(blame(&file).status, Status::Untracked);
        run(&["add", "a.md"]);
        run(&["commit", "-q", "-m", "First words"]);
        std::fs::write(&file, "# A\n\nOne.\nTwo.\n").unwrap();
        let b = blame(&file);
        assert_eq!(b.status, Status::Ok);
        assert_eq!(b.lines, vec![0, 0, 0, 1]);
        let first = &b.commits[0];
        assert_eq!(
            (
                first.author.as_str(),
                first.date.as_str(),
                first.summary.as_str()
            ),
            ("Ada Lovelace", "2026-01-02", "First words"),
            "dated in the author's time zone"
        );
        assert_eq!(first.hash.len(), 7);
        assert!(
            first
                .url
                .as_deref()
                .unwrap()
                .starts_with("https://codeberg.org/o/r/commit/")
        );
        assert_eq!(b.commits[1].summary, "Not committed yet");
        assert_eq!(b.commits[1].url, None);
    }
}
