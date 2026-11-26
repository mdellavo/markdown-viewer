//! GitHub references, linked the way GitHub shows them: `#123` and
//! `GH-123` (issues and pull requests), `owner/repo#123`, `@user`, and
//! commit hashes. They need a repository, found from the folder's git
//! remote (GitHub or GitHub Enterprise) or set in the settings file.
//!
//! Text inside links, code, math and diagrams is left alone.

use std::path::Path;
use std::process::Command;

/// The repository's web address (`https://github.com/owner/repo`) for a
/// folder inside a git checkout whose remote is on GitHub.
pub fn detect(dir: &Path) -> Option<String> {
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let url = git(&["remote", "get-url", "origin"]).or_else(|| {
        let first = git(&["remote"])?.lines().next()?.to_string();
        git(&["remote", "get-url", &first])
    })?;
    web_url(&url)
}

/// `git@github.com:o/r.git`, `ssh://git@github.com/o/r`,
/// `https://github.com/o/r.git` → `https://github.com/o/r`. Only GitHub
/// hosts (github.com, or a name with "github" in it for Enterprise).
pub fn web_url(remote: &str) -> Option<String> {
    let rest = remote
        .strip_prefix("https://")
        .or_else(|| remote.strip_prefix("http://"))
        .or_else(|| remote.strip_prefix("ssh://"))
        .or_else(|| remote.strip_prefix("git://"))
        .unwrap_or(remote);
    let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
    // scp-like `host:owner/repo`, or `host[:port]/owner/repo`.
    let (host, path) = match (rest.find(':'), rest.find('/')) {
        (Some(c), Some(s)) if c < s => {
            let after = &rest[c + 1..];
            // A port number: `host:22/owner/repo`.
            match after.split_once('/') {
                Some((port, path)) if port.chars().all(|ch| ch.is_ascii_digit()) => {
                    (&rest[..c], path)
                }
                _ => (&rest[..c], after),
            }
        }
        (_, Some(s)) => (&rest[..s], &rest[s + 1..]),
        _ => return None,
    };
    let host = host.to_ascii_lowercase();
    if host != "github.com" && !host.contains("github") {
        return None;
    }
    let path = path.trim_end_matches('/').trim_end_matches(".git");
    let (owner, repo) = path.split_once('/')?;
    let ok = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    };
    (ok(owner) && ok(repo)).then(|| format!("https://{host}/{owner}/{repo}"))
}

/// The setting `github_repo`: `owner/repo`, or a repository's web address.
pub fn from_setting(value: &str) -> Option<String> {
    if value.contains("://") {
        web_url(value)
    } else {
        web_url(&format!("https://github.com/{value}"))
    }
}

/// Elements whose text is never linked.
const SKIP: &[&str] = &[
    "a", "code", "pre", "kbd", "samp", "script", "style", "svg", "math", "textarea",
];

/// Link the references in rendered HTML, for the repository at `repo`
/// (its web address).
pub fn finish(html: &str, repo: &str) -> String {
    // Links to other repositories and users use the same host.
    let host = repo.split('/').take(3).collect::<Vec<_>>().join("/");
    let mut out = String::with_capacity(html.len() + 256);
    let mut open: Vec<String> = Vec::new();
    let mut rest = html;
    loop {
        let Some(at) = rest.find('<') else {
            push_text(&mut out, rest, open.is_empty(), repo, &host);
            break;
        };
        let (text, tag_start) = rest.split_at(at);
        push_text(&mut out, text, open.is_empty(), repo, &host);
        if tag_start.starts_with("<!--") {
            let end = tag_start.find("-->").map_or(tag_start.len(), |e| e + 3);
            out.push_str(&tag_start[..end]);
            rest = &tag_start[end..];
            continue;
        }
        let end = tag_start.find('>').map_or(tag_start.len(), |e| e + 1);
        let tag = &tag_start[..end];
        out.push_str(tag);
        let closing = tag.starts_with("</");
        let name = tag
            .trim_start_matches("</")
            .trim_start_matches('<')
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if closing {
            if let Some(i) = open.iter().rposition(|n| *n == name) {
                open.truncate(i);
            }
        } else if !tag.ends_with("/>")
            && (SKIP.contains(&name.as_str()) || tag.contains("data-math-style"))
        {
            open.push(name);
        }
        rest = &tag_start[end..];
    }
    out
}

fn push_text(out: &mut String, text: &str, link: bool, repo: &str, host: &str) {
    if !link || !text.contains(['#', '@', '-']) && !text.bytes().any(|b| b.is_ascii_hexdigit()) {
        out.push_str(text);
        return;
    }
    let b = text.as_bytes();
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        let starts_word = i == 0 || !joins(b[i - 1]);
        if starts_word && let Some((len, url)) = reference(&text[i..], repo, host) {
            out.push_str(&text[copied..i]);
            out.push_str(&format!(
                "<a class=\"mdv-ref\" href=\"{url}\">{}</a>",
                &text[i..i + len]
            ));
            i += len;
            copied = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
}

/// Characters that make a reference part of a longer word (so `a#1`,
/// `x@y`, `&#39;` and `v1.2-abc1234` aren't references).
fn joins(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c >= 0x80 || b"&#@/._-:".contains(&c)
}

/// The end of a reference: not followed by more of a word.
fn ends(s: &str, at: usize) -> bool {
    s.as_bytes()
        .get(at)
        .is_none_or(|&c| !(c.is_ascii_alphanumeric() || c >= 0x80 || c == b'_'))
}

/// A reference at the start of `s`: (its length, its URL).
fn reference(s: &str, repo: &str, host: &str) -> Option<(usize, String)> {
    let b = s.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    match b[0] {
        // #123
        b'#' => {
            let n = digits(1);
            (n > 0 && b[1] != b'0' && ends(s, 1 + n))
                .then(|| (1 + n, format!("{repo}/issues/{}", &s[1..1 + n])))
        }
        // GH-123
        b'G' if s.starts_with("GH-") => {
            let n = digits(3);
            (n > 0 && b[3] != b'0' && ends(s, 3 + n))
                .then(|| (3 + n, format!("{repo}/issues/{}", &s[3..3 + n])))
        }
        // @user (not @org/team, not an email address: see `joins`)
        b'@' => {
            let name = b[1..]
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric() || **c == b'-')
                .count();
            let user = &s[1..1 + name];
            let valid = (1..=39).contains(&name)
                && b[1].is_ascii_alphanumeric()
                && !user.ends_with('-')
                && !user.contains("--")
                && b.get(1 + name) != Some(&b'/')
                && ends(s, 1 + name);
            valid.then(|| (1 + name, format!("{host}/{user}")))
        }
        _ => owner_repo_issue(s, host).or_else(|| commit(s, repo)),
    }
}

/// `owner/repo#123`.
fn owner_repo_issue(s: &str, host: &str) -> Option<(usize, String)> {
    let part = |from: usize| {
        s.as_bytes()[from..]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || b"-_.".contains(c))
            .count()
    };
    let owner = part(0);
    if owner == 0
        || !s.as_bytes()[0].is_ascii_alphanumeric()
        || s.as_bytes().get(owner) != Some(&b'/')
    {
        return None;
    }
    let repo = part(owner + 1);
    let hash = owner + 1 + repo;
    if repo == 0 || s.as_bytes().get(hash) != Some(&b'#') {
        return None;
    }
    let n = s.as_bytes()[hash + 1..]
        .iter()
        .take_while(|c| c.is_ascii_digit())
        .count();
    if n == 0 || s.as_bytes()[hash + 1] == b'0' || !ends(s, hash + 1 + n) {
        return None;
    }
    Some((
        hash + 1 + n,
        format!(
            "{host}/{}/issues/{}",
            &s[..hash],
            &s[hash + 1..hash + 1 + n]
        ),
    ))
}

/// A commit hash: 7 to 40 lowercase hex digits with both a digit and a
/// letter (so plain numbers and words like "decade" aren't).
fn commit(s: &str, repo: &str) -> Option<(usize, String)> {
    let n = s
        .bytes()
        .take_while(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
        .count();
    let hash = &s[..n];
    let mixed =
        hash.bytes().any(|c| c.is_ascii_digit()) && hash.bytes().any(|c| c.is_ascii_alphabetic());
    ((7..=40).contains(&n) && mixed && ends(s, n)).then(|| (n, format!("{repo}/commit/{hash}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = "https://github.com/o/r";

    #[test]
    fn remotes_to_web_addresses() {
        for remote in [
            "git@github.com:o/r.git",
            "ssh://git@github.com/o/r.git",
            "ssh://git@github.com:22/o/r",
            "https://github.com/o/r",
            "https://user@github.com/o/r.git/",
        ] {
            assert_eq!(web_url(remote).as_deref(), Some(REPO), "{remote}");
        }
        assert_eq!(
            web_url("git@github.example.com:team/tool.git").as_deref(),
            Some("https://github.example.com/team/tool")
        );
        assert_eq!(web_url("https://gitlab.com/o/r.git"), None);
        assert_eq!(web_url("/srv/git/r.git"), None);
        assert_eq!(from_setting("o/r").as_deref(), Some(REPO));
        assert_eq!(from_setting("not a repo"), None);
    }

    #[test]
    fn links_references_in_text_only() {
        let html = "<p>Fixes #12 and GH-3, see a/b#4 by @octo-cat in 1a2b3c4d.</p>\n\
                    <p><code>#5 @x 1a2b3c4</code> <a href=\"#6\">#6</a> \
                    <span data-math-style=\"inline\">#7</span> #8</p>";
        let out = finish(html, REPO);
        assert!(
            out.contains(
                "Fixes <a class=\"mdv-ref\" href=\"https://github.com/o/r/issues/12\">#12</a>"
            ),
            "{out}"
        );
        assert!(out.contains("href=\"https://github.com/o/r/issues/3\">GH-3</a>"));
        assert!(out.contains("href=\"https://github.com/a/b/issues/4\">a/b#4</a>"));
        assert!(out.contains("href=\"https://github.com/octo-cat\">@octo-cat</a>"));
        assert!(out.contains("href=\"https://github.com/o/r/commit/1a2b3c4d\">1a2b3c4d</a>."));
        assert!(out.contains("<code>#5 @x 1a2b3c4</code>"));
        assert!(out.contains("<a href=\"#6\">#6</a>"));
        assert!(out.contains(">#7</span> <a class=\"mdv-ref\""));
    }

    #[test]
    fn leaves_lookalikes_alone() {
        for text in [
            "a@b.com",
            "x#1",
            "#0",
            "#12abc",
            "&#39;",
            "@org/team",
            "@-x",
            "@a--b",
            "1234567",
            "decade",
            "facade1",
            "v1.2-abc1234",
            "1a2b3c",
            "GH-x",
            "https://x.org/#12",
            "C#7",
        ] {
            let html = format!("<p>{text}</p>");
            let out = finish(&html, REPO);
            if text == "facade1" {
                // Hex with a digit: GitHub would link it too, if it were a commit.
                assert!(out.contains("commit/facade1"));
                continue;
            }
            assert_eq!(out, html, "{text}");
        }
    }
}
