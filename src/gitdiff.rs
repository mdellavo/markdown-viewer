//! "Compare to git": which lines of a file changed since the last commit,
//! via the `git` command (`git diff -U0 HEAD`).

use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Added,
    Modified,
    /// Lines were removed just after `start` (`end` == `start`).
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub kind: Kind,
    /// 1-based lines in the current file (inclusive).
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Compared with the last commit.
    Ok,
    /// In a repository but never committed: every line is new.
    Untracked,
    /// Not in a git repository (or git isn't installed).
    NotARepo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diff {
    pub status: Status,
    pub changes: Vec<Change>,
}

/// Changes to `file` since the last commit.
pub fn diff(file: &Path) -> Diff {
    let not_a_repo = Diff {
        status: Status::NotARepo,
        changes: Vec::new(),
    };
    let (Some(dir), Some(name)) = (file.parent(), file.file_name()) else {
        return not_a_repo;
    };
    let git = |args: &[&str]| {
        Command::new("git")
            // English messages: the "not a git repository" check reads them.
            .env("LC_ALL", "C")
            .arg("-C")
            .arg(dir)
            .args(args)
            .arg("--")
            .arg(name)
            .output()
            .ok()
    };
    // Is it tracked? (Also tells us whether this is a repository at all.)
    match git(&["ls-files", "--error-unmatch"]) {
        None => return not_a_repo,
        Some(out) if !out.status.success() => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("not a git repository") {
                return not_a_repo;
            }
            let lines = std::fs::read_to_string(file)
                .map(|t| t.lines().count() as u32)
                .unwrap_or(0);
            return Diff {
                status: Status::Untracked,
                changes: (lines > 0)
                    .then_some(Change {
                        kind: Kind::Added,
                        start: 1,
                        end: lines,
                    })
                    .into_iter()
                    .collect(),
            };
        }
        Some(_) => {}
    }
    match git(&["diff", "--no-color", "--no-ext-diff", "-U0", "HEAD"]) {
        Some(out) if out.status.success() => Diff {
            status: Status::Ok,
            changes: parse(&String::from_utf8_lossy(&out.stdout)),
        },
        // No commits yet, or git failed: nothing to compare with.
        _ => not_a_repo,
    }
}

/// Parse `@@ -a,b +c,d @@` hunk headers from `git diff -U0`.
fn parse(diff: &str) -> Vec<Change> {
    diff.lines()
        .filter_map(|line| {
            let header = line.strip_prefix("@@ ")?;
            let mut parts = header.split_whitespace();
            let old = parts.next()?.strip_prefix('-')?;
            let new = parts.next()?.strip_prefix('+')?;
            let (_, old_len) = range(old)?;
            let (start, len) = range(new)?;
            Some(match (old_len, len) {
                (_, 0) => Change {
                    kind: Kind::Deleted,
                    start,
                    end: start,
                },
                (0, _) => Change {
                    kind: Kind::Added,
                    start,
                    end: start + len - 1,
                },
                _ => Change {
                    kind: Kind::Modified,
                    start,
                    end: start + len - 1,
                },
            })
        })
        .collect()
}

/// "12,3" → (12, 3); "12" → (12, 1)
fn range(s: &str) -> Option<(u32, u32)> {
    match s.split_once(',') {
        Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
        None => Some((s.parse().ok()?, 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hunk_headers() {
        let diff = "diff --git a/x b/x\n@@ -3 +3 @@ ctx\n-old\n+new\n@@ -10,0 +11,2 @@\n+a\n+b\n@@ -20,3 +22,0 @@\n";
        assert_eq!(
            parse(diff),
            [
                Change {
                    kind: Kind::Modified,
                    start: 3,
                    end: 3
                },
                Change {
                    kind: Kind::Added,
                    start: 11,
                    end: 12
                },
                Change {
                    kind: Kind::Deleted,
                    start: 22,
                    end: 22
                },
            ]
        );
    }

    #[test]
    fn tracks_a_real_repository() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap()
        };
        if !git(&["init", "-q"]).status.success() {
            return; // git not available
        }
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "user.name", "Test"]);
        let file = dir.path().join("doc.md");
        std::fs::write(&file, "one\ntwo\nthree\n").unwrap();
        assert_eq!(diff(&file).status, Status::Untracked);
        git(&["add", "doc.md"]);
        git(&["commit", "-q", "-m", "x"]);
        assert_eq!(diff(&file).changes, []);
        std::fs::write(&file, "one\nTWO\nthree\nfour\n").unwrap();
        let d = diff(&file);
        assert_eq!(d.status, Status::Ok);
        assert_eq!(
            d.changes,
            [
                Change {
                    kind: Kind::Modified,
                    start: 2,
                    end: 2
                },
                Change {
                    kind: Kind::Added,
                    start: 4,
                    end: 4
                },
            ]
        );
        let outside = tempfile::tempdir().unwrap();
        let lone = outside.path().join("x.md");
        std::fs::write(&lone, "x").unwrap();
        // A temp dir could sit inside a repository; only check when it doesn't.
        let d = diff(&lone);
        assert!(matches!(d.status, Status::NotARepo | Status::Untracked));
    }
}
