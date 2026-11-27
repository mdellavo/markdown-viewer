//! `--diff REV`: the page as it is now, with what changed since a git
//! revision marked on it: added text in green, removed text struck through
//! in red.
//!
//! Both versions are rendered, split into top-level blocks, and the blocks
//! compared. Changed paragraphs, headings, lists, quotes and tables are
//! compared word by word: the result keeps the new version's tags, with
//! added words wrapped in `<ins class="mdv-ins">` and removed words (text only, so the
//! markup stays balanced) in `<del class="mdv-del">`. Other changed blocks (code, diagrams,
//! maps) are shown whole: the old one struck through, the new one marked.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::render::{RenderOptions, Renderer, escape_text};

/// The file's text at `rev` (empty if it didn't exist then).
pub fn old_text(file: &Path, rev: &str) -> Result<String> {
    let dir = file.parent().context("file has no parent folder")?;
    let name = file
        .file_name()
        .context("not a file")?
        .to_string_lossy()
        .into_owned();
    if rev.starts_with('-') {
        bail!("{rev} isn't a git revision");
    }
    let out = Command::new("git")
        // English messages: the checks below read them.
        .env("LC_ALL", "C")
        .arg("-C")
        .arg(dir)
        .arg("show")
        .arg(format!("{rev}:./{name}"))
        .output()
        .context("--diff needs git, which couldn't be run")?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("not a git repository") {
        bail!("{} isn't in a git repository", file.display());
    }
    // A file added since then: everything is new.
    if err.contains("exists on disk, but not in") || err.contains("does not exist in") {
        return Ok(String::new());
    }
    bail!(
        "git couldn't find {rev}: {}",
        err.trim().trim_start_matches("fatal: ")
    )
}

/// Render `file`'s text `new` with what changed since `old` marked.
/// `options` are the viewer's; source positions are left out (they'd
/// differ between the two).
pub fn render(options: &RenderOptions, file: &Path, old: &str, new: &str, rev: &str) -> String {
    let renderer = Renderer::with_options(RenderOptions {
        sourcepos: false,
        ..options.clone()
    });
    let dir = file.parent();
    compare(
        &renderer.render_in(old, dir, None),
        &renderer.render_in(new, dir, None),
        rev,
    )
}

fn compare(old_html: &str, new_html: &str, rev: &str) -> String {
    let old = blocks(old_html);
    let new = blocks(new_html);
    let mut out = String::new();
    let mut words = Count::default();
    let mut ops = diff(&old, &new, |a, b| a == b);
    // Pair removed and added blocks between unchanged ones, in order.
    let mut i = 0;
    while i < ops.len() {
        match ops[i] {
            Op::Same(_, n) => {
                out.push_str(new[n]);
                out.push('\n');
                i += 1;
            }
            _ => {
                let start = i;
                while i < ops.len() && !matches!(ops[i], Op::Same(..)) {
                    i += 1;
                }
                let run = &mut ops[start..i];
                let dels: Vec<usize> = run
                    .iter()
                    .filter_map(|o| if let Op::Del(o) = o { Some(*o) } else { None })
                    .collect();
                let adds: Vec<usize> = run
                    .iter()
                    .filter_map(|o| if let Op::Add(n) = o { Some(*n) } else { None })
                    .collect();
                for k in 0..dels.len().max(adds.len()) {
                    match (dels.get(k), adds.get(k)) {
                        (Some(&o), Some(&n)) if comparable(old[o], new[n]) => {
                            out.push_str(&inline(old[o], new[n], &mut words));
                        }
                        (o, n) => {
                            if let Some(&o) = o {
                                words.removed += count_words(old[o]);
                                out.push_str(&format!(
                                    "<div class=\"mdv-diff-block mdv-diff-del\">{}</div>",
                                    old[o]
                                ));
                            }
                            if let Some(&n) = n {
                                words.added += count_words(new[n]);
                                out.push_str(&format!(
                                    "<div class=\"mdv-diff-block mdv-diff-ins\">{}</div>",
                                    new[n]
                                ));
                            }
                        }
                    }
                    out.push('\n');
                }
            }
        }
    }
    let summary = if words.added == 0 && words.removed == 0 {
        format!("No changes since <code>{}</code>.", escape_text(rev))
    } else {
        format!(
            "Changes since <code>{}</code>: <span class=\"mdv-diff-added\">{} added</span>, \
             <span class=\"mdv-diff-removed\">{} removed</span>.",
            escape_text(rev),
            plural(words.added, "word"),
            plural(words.removed, "word")
        )
    };
    format!("<div class=\"mdv-diff-summary\">{summary}</div>\n{out}")
}

fn plural(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

#[derive(Default)]
struct Count {
    added: usize,
    removed: usize,
}

/// Blocks whose words are compared (others are replaced whole).
fn comparable(old: &str, new: &str) -> bool {
    let (a, b) = (tag_name(old), tag_name(new));
    a == b
        && (matches!(
            a.as_str(),
            "p" | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "ul"
                | "ol"
                | "blockquote"
                | "table"
                | "dl"
        ) || (a == "div" && old.contains("markdown-alert") && new.contains("markdown-alert")))
}

fn tag_name(block: &str) -> String {
    block
        .strip_prefix('<')
        .unwrap_or("")
        .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// The top-level elements of rendered HTML.
fn blocks(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = html.trim_start();
    let mut offset = html.len() - rest.len();
    while !rest.is_empty() {
        let len = block_len(rest);
        out.push(&html[offset..offset + len]);
        let after = &rest[len..];
        let trimmed = after.trim_start();
        offset += len + (after.len() - trimmed.len());
        rest = trimmed;
    }
    out
}

/// The length of the element (or text run, or comment) `s` starts with.
fn block_len(s: &str) -> usize {
    if !s.starts_with('<') {
        return s.find('<').unwrap_or(s.len());
    }
    if s.starts_with("<!--") {
        return s.find("-->").map_or(s.len(), |e| e + 3);
    }
    let mut depth = 0usize;
    let mut i = 0;
    while let Some(at) = s[i..].find('<') {
        let start = i + at;
        let end = s[start..].find('>').map_or(s.len(), |e| start + e + 1);
        let tag = &s[start..end];
        if tag.starts_with("<!--") {
            i = s[start..].find("-->").map_or(s.len(), |e| start + e + 3);
            continue;
        }
        let name = tag_name(&format!(
            "<{}",
            tag.trim_start_matches("</").trim_start_matches('<')
        ));
        if tag.starts_with("</") {
            depth = depth.saturating_sub(1);
        } else if !tag.ends_with("/>") && !VOID.contains(&name.as_str()) {
            depth += 1;
        }
        i = end;
        if depth == 0 {
            return end;
        }
    }
    s.len()
}

const PUNCT: &[u8] = b".,;:!?()[]\"'";

/// Tags, whitespace runs, punctuation and words.
fn tokens(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = html.as_bytes();
    while i < b.len() {
        let start = i;
        if b[i] == b'<' {
            i = html[i..].find('>').map_or(b.len(), |e| i + e + 1);
        } else if b[i].is_ascii_whitespace() {
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
        } else if PUNCT.contains(&b[i]) {
            // Punctuation on its own, so "fox." and "fox" share a word.
            i += 1;
        } else {
            while i < b.len()
                && b[i] != b'<'
                && !b[i].is_ascii_whitespace()
                && !PUNCT.contains(&b[i])
            {
                if b[i] == b'&' {
                    // An entity (&amp;) stays whole.
                    let end = html[i..]
                        .find(';')
                        .filter(|e| *e <= 10)
                        .map_or(1, |e| e + 1);
                    i += end;
                } else {
                    i += 1;
                }
            }
        }
        out.push(&html[start..i]);
    }
    out
}

fn is_tag(t: &str) -> bool {
    t.starts_with('<')
}

fn is_word(t: &str) -> bool {
    !is_tag(t) && !t.trim().is_empty()
}

/// A word for the summary's counts (not punctuation).
fn counts(t: &str) -> bool {
    !is_tag(t) && t.chars().any(char::is_alphanumeric)
}

fn count_words(html: &str) -> usize {
    tokens(html).into_iter().filter(|t| counts(t)).count()
}

/// One block, compared word by word. The new version's tags are all kept
/// (so the result is balanced); removed tags are dropped, their text kept.
fn inline(old: &str, new: &str, words: &mut Count) -> String {
    let a = tokens(old);
    let b = tokens(new);
    let mut out = String::with_capacity(new.len() + 64);
    // Text waiting to be wrapped: (`ins` or `del`, text).
    let mut pending: Option<(&str, String)> = None;
    let flush = |out: &mut String, pending: &mut Option<(&str, String)>| {
        if let Some((kind, text)) = pending.take() {
            // Spaces at the edges stay outside the mark.
            let trimmed = text.trim_end();
            out.push_str(&format!(
                "<{kind} class=\"mdv-{kind}\">{trimmed}</{kind}>{}",
                &text[trimmed.len()..]
            ));
        }
    };
    for op in diff(&a, &b, |x, y| x == y) {
        let (kind, token) = match op {
            Op::Same(_, n) => {
                flush(&mut out, &mut pending);
                out.push_str(b[n]);
                continue;
            }
            Op::Add(n) => ("ins", b[n]),
            Op::Del(o) => ("del", a[o]),
        };
        if is_tag(token) {
            flush(&mut out, &mut pending);
            if kind == "ins" {
                out.push_str(token);
            }
            continue;
        }
        if counts(token) {
            match kind {
                "ins" => words.added += 1,
                _ => words.removed += 1,
            }
        }
        match &mut pending {
            Some((k, text)) if *k == kind => text.push_str(token),
            _ => {
                flush(&mut out, &mut pending);
                if is_word(token) {
                    pending = Some((kind, token.to_string()));
                } else if kind == "ins" {
                    // Whitespace on its own: keep the new version's.
                    out.push_str(token);
                }
            }
        }
    }
    flush(&mut out, &mut pending);
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Same(usize, usize),
    Del(usize),
    Add(usize),
}

/// The longest common subsequence of `a` and `b`, as edit operations
/// (removals before additions within a change). Common ends are trimmed
/// first; a very large middle is treated as replaced whole.
fn diff<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool) -> Vec<Op> {
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && eq(&a[pre], &b[pre]) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre
        && suf < b.len() - pre
        && eq(&a[a.len() - 1 - suf], &b[b.len() - 1 - suf])
    {
        suf += 1;
    }
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    let mut ops: Vec<Op> = (0..pre).map(|i| Op::Same(i, i)).collect();
    let (n, m) = (am.len(), bm.len());
    if n * m > 4_000_000 {
        ops.extend((0..n).map(|i| Op::Del(pre + i)));
        ops.extend((0..m).map(|j| Op::Add(pre + j)));
    } else {
        // lcs[i][j]: the LCS length of am[i..] and bm[j..].
        let mut lcs = vec![0u32; (n + 1) * (m + 1)];
        let at = |i: usize, j: usize| i * (m + 1) + j;
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[at(i, j)] = if eq(&am[i], &bm[j]) {
                    lcs[at(i + 1, j + 1)] + 1
                } else {
                    lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        let mut adds = Vec::new();
        while i < n || j < m {
            if i < n && j < m && eq(&am[i], &bm[j]) {
                ops.append(&mut adds);
                ops.push(Op::Same(pre + i, pre + j));
                i += 1;
                j += 1;
            } else if j < m && (i == n || lcs[at(i, j + 1)] >= lcs[at(i + 1, j)]) {
                adds.push(Op::Add(pre + j));
                j += 1;
            } else {
                ops.push(Op::Del(pre + i));
                i += 1;
            }
        }
        ops.append(&mut adds);
    }
    ops.extend((0..suf).map(|k| Op::Same(a.len() - suf + k, b.len() - suf + k)));
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(old: &str, new: &str) -> String {
        super::render(
            &RenderOptions::default(),
            Path::new("doc.md"),
            old,
            new,
            "HEAD~3",
        )
    }

    #[test]
    fn words_are_marked_in_changed_paragraphs() {
        let html = render(
            "# Title\n\nThe quick brown fox.\n\nSame.\n",
            "# Title\n\nThe quick red fox jumps.\n\nSame.\n",
        );
        assert!(
            html.contains("<p>The quick <del class=\"mdv-del\">brown</del><ins class=\"mdv-ins\">red</ins> fox <ins class=\"mdv-ins\">jumps</ins>.</p>"),
            "{html}"
        );
        assert!(
            html.contains("<h1 id=\"title\">"),
            "unchanged blocks as they were: {html}"
        );
        assert!(html.contains("<p>Same.</p>"));
        assert!(
            html.contains("Changes since <code>HEAD~3</code>: <span class=\"mdv-diff-added\">"),
            "{html}"
        );
    }

    #[test]
    fn markup_stays_balanced() {
        let html = render("A **bold** claim.\n", "A plain claim, *now* emphasised.\n");
        let opens = html.matches("<em>").count() + html.matches("<strong>").count();
        let closes = html.matches("</em>").count() + html.matches("</strong>").count();
        assert_eq!(opens, closes, "{html}");
        assert!(
            !html.contains("<strong>"),
            "removed tags are dropped: {html}"
        );
        assert!(
            html.contains("<del class=\"mdv-del\">bold</del>")
                || html.contains("<del class=\"mdv-del\">bold"),
            "{html}"
        );
    }

    #[test]
    fn other_blocks_are_replaced_whole() {
        let html = render(
            "```rust\nlet a = 1;\n```\n",
            "```rust\nlet a = 2;\n```\n\nNew paragraph.\n",
        );
        assert!(
            html.contains("<div class=\"mdv-diff-block mdv-diff-del\"><pre"),
            "{html}"
        );
        assert!(
            html.contains("<div class=\"mdv-diff-block mdv-diff-ins\"><pre"),
            "{html}"
        );
        assert!(
            html.contains("<div class=\"mdv-diff-block mdv-diff-ins\"><p>New paragraph.</p></div>"),
            "{html}"
        );
    }

    #[test]
    fn list_items_added_and_no_changes() {
        let html = render("- one\n- two\n", "- one\n- two\n- three\n");
        assert!(
            html.contains("<li><ins class=\"mdv-ins\">three</ins></li>"),
            "{html}"
        );
        assert!(html.contains("1 word added"), "{html}");
        let same = render("Same.\n", "Same.\n");
        assert!(same.contains("No changes since"), "{same}");
    }

    #[test]
    fn blocks_split_at_the_top_level() {
        let b = blocks(
            "<p>a <em>b</em></p>\n<hr />\n<ul>\n<li><p>x</p></li>\n</ul>\n<!-- c -->\n<img src=\"x\">",
        );
        assert_eq!(b.len(), 5, "{b:?}");
        assert_eq!(b[2], "<ul>\n<li><p>x</p></li>\n</ul>");
    }
}
