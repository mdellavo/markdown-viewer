//! `--lint`: writing problems that GitHub renders without complaint but
//! readers (or screen readers) trip over. Works on the rendered HTML with
//! source positions, plus the source for code fences.

use std::collections::HashSet;

/// Every rule: (name, what it checks). Names are used in the output and in
/// the `lint_ignore` setting.
pub const RULES: &[(&str, &str)] = &[
    ("heading-increment", "heading levels go down one at a time"),
    ("multiple-h1", "one top-level heading per document"),
    (
        "duplicate-heading",
        "no two headings with the same text under the same heading",
    ),
    ("image-alt", "images have alt text"),
    ("empty-link", "links have text and a target"),
    ("code-language", "fenced code blocks say their language"),
];

#[derive(Debug, PartialEq, Eq)]
pub struct Problem {
    pub line: u32,
    pub rule: &'static str,
    pub message: String,
}

/// Lint a document: its Markdown and its HTML rendered with source
/// positions (and without Obsidian embeds, so other files' problems don't
/// show up here).
pub fn check(source: &str, html: &str, ignore: &[String]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut report = |line: u32, rule: &'static str, message: String| {
        if !ignore.iter().any(|r| r == rule) {
            problems.push(Problem {
                line,
                rule,
                message,
            });
        }
    };
    let lines: Vec<&str> = source.lines().collect();

    let mut line = 1;
    let mut last_level = 0;
    let mut h1s = 0;
    // Enclosing headings (level, text), for "same text under the same heading".
    let mut parents: Vec<(u8, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        let after = &rest[at + 1..];
        let end = after.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        let own_pos = attr(tag, "data-sourcepos");
        if let Some(pos) = &own_pos {
            let mut parts = pos.split([':', '-']);
            line = parts.next().and_then(|l| l.parse().ok()).unwrap_or(line);
        }
        let name = tag.split_whitespace().next().unwrap_or("");
        let body = &after[end.min(after.len())..];
        match name {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" if own_pos.is_some() => {
                let level = name.as_bytes()[1] - b'0';
                let text = text_until(body, &format!("</{name}>"));
                if last_level > 0 && level > last_level + 1 {
                    report(
                        line,
                        "heading-increment",
                        format!("heading level jumps from {last_level} to {level}"),
                    );
                }
                last_level = level;
                if level == 1 {
                    h1s += 1;
                    if h1s == 2 {
                        report(
                            line,
                            "multiple-h1",
                            "more than one top-level (#) heading".into(),
                        );
                    }
                }
                while parents.last().is_some_and(|(l, _)| *l >= level) {
                    parents.pop();
                }
                let parent = parents.last().map(|(_, t)| t.clone()).unwrap_or_default();
                let key = (parent, level, text.to_lowercase());
                if !text.is_empty() && !seen.insert(key) {
                    report(
                        line,
                        "duplicate-heading",
                        format!("another heading here is also \"{text}\" (its link gets a -1)"),
                    );
                }
                parents.push((level, text));
            }
            "img" => {
                if attr(tag, "alt").is_none_or(|a| a.trim().is_empty()) {
                    let src = attr(tag, "src").unwrap_or_default();
                    report(line, "image-alt", format!("image without alt text: {src}"));
                }
            }
            // (An <a> with no href at all is a named anchor, not a link.)
            "a" if !tag.contains("class=\"anchor\"") => {
                if let Some(href) = attr(tag, "href") {
                    let inner = &body[..body.find("</a>").unwrap_or(body.len())];
                    let has_text = !strip_tags(inner).trim().is_empty() || inner.contains("<img");
                    if href.trim().is_empty() {
                        report(line, "empty-link", "link without a target".into());
                    } else if !has_text {
                        report(line, "empty-link", format!("link without text: {href}"));
                    }
                }
            }
            "pre" if own_pos.is_some() => {
                if let Some(pos) = &own_pos
                    && fence_without_language(&lines, pos)
                {
                    report(
                        line,
                        "code-language",
                        "code fence without a language".into(),
                    );
                }
            }
            _ => {}
        }
        rest = body;
    }
    problems
}

/// Is the block starting at `pos` (`line:col-…`) a ``` or ~~~ fence with
/// nothing after the fence characters?
fn fence_without_language(lines: &[&str], pos: &str) -> bool {
    let mut parts = pos.split([':', '-']);
    let (Some(line), Some(col)) = (
        parts.next().and_then(|l| l.parse::<usize>().ok()),
        parts.next().and_then(|c| c.parse::<usize>().ok()),
    ) else {
        return false;
    };
    let Some(text) = lines.get(line.wrapping_sub(1)) else {
        return false;
    };
    let text = text.get(col.saturating_sub(1)..).unwrap_or("").trim_start();
    let fence = text.chars().take_while(|&c| c == '`' || c == '~').count();
    fence >= 3 && text[fence..].trim().is_empty()
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let at = tag.find(&key)? + key.len();
    let end = at + tag[at..].find('"')?;
    Some(crate::links::unescape(&tag[at..end]))
}

fn text_until(html: &str, close: &str) -> String {
    let inner = &html[..html.find(close).unwrap_or(html.len())];
    crate::links::unescape(strip_tags(inner).trim())
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{RenderOptions, Renderer};

    fn lint(md: &str, ignore: &[&str]) -> Vec<(u32, &'static str)> {
        let renderer = Renderer::with_options(RenderOptions {
            sourcepos: true,
            ..Default::default()
        });
        let ignore: Vec<String> = ignore.iter().map(|s| s.to_string()).collect();
        check(md, &renderer.render_body(md), &ignore)
            .into_iter()
            .map(|p| (p.line, p.rule))
            .collect()
    }

    #[test]
    fn finds_each_problem() {
        let md = "# Title\n\n### Skipped\n\n## A\n\n### Notes\n\n## B\n\n### Notes\n\n### Notes\n\n\
                  # Second\n\n![](x.png) ![ok](y.png)\n\n[](https://x.org) [text]() [ok](z.md)\n\n\
                  ```\nplain\n```\n\n```rust\nfn main() {}\n```\n\n    indented\n\n<a name=\"here\"></a>\n";
        assert_eq!(
            lint(md, &[]),
            vec![
                (3, "heading-increment"),
                (13, "duplicate-heading"),
                (15, "multiple-h1"),
                (17, "image-alt"),
                (19, "empty-link"),
                (19, "empty-link"),
                (21, "code-language"),
            ]
        );
        assert_eq!(lint(md, &["empty-link", "image-alt"]).len(), 4);
        assert!(lint("# Clean\n\n## Section\n\nText with a [link](a.md).\n", &[]).is_empty());
    }
}
