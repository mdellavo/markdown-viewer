//! Code block extras, set in the fence's info string:
//!
//! ````markdown
//! ```rust title="main.rs" {3-5,8} showLineNumbers
//! ````
//!
//! - `title="…"` adds a caption with the file name.
//! - `{3-5,8}` highlights lines.
//! - `showLineNumbers` (or `linenos`) numbers the lines; `--line-numbers`
//!   numbers every block.
//!
//! GitHub ignores these, so the same Markdown still looks right there.

use crate::render::{escape_attr, escape_text};

/// What a fence's info string asked for.
#[derive(Debug, Default, PartialEq)]
struct Meta {
    title: Option<String>,
    highlight: Vec<(usize, usize)>,
    numbers: bool,
}

/// Apply code block extras to rendered HTML.
pub fn finish(html: String, number_all: bool) -> String {
    if !html.contains("<pre") {
        return html;
    }
    let mut out = String::with_capacity(html.len() + html.len() / 4);
    let mut rest = html.as_str();
    while let Some(start) = rest.find("<pre") {
        out.push_str(&rest[..start]);
        let block = &rest[start..];
        let Some(end) = block.find("</pre>") else {
            out.push_str(block);
            return out;
        };
        let block_html = &block[..end + "</pre>".len()];
        out.push_str(&process_block(block_html, number_all).unwrap_or_else(|| block_html.into()));
        rest = &block[end + "</pre>".len()..];
    }
    out.push_str(rest);
    out
}

/// Rewrite one `<pre>…</pre>`, or `None` to leave it as it is.
fn process_block(block: &str, number_all: bool) -> Option<String> {
    let tag_end = block.find('>')?;
    let pre_tag = &block[..tag_end];
    // Diagrams and math are drawn by scripts; leave them alone.
    if pre_tag.contains("mermaid") || pre_tag.contains("data-math-style") {
        return None;
    }
    let meta = attr(pre_tag, "data-meta")
        .map(|m| parse_meta(&unescape(&m)))
        .unwrap_or_default();
    let numbers = meta.numbers || number_all;
    if meta.title.is_none() && meta.highlight.is_empty() && !numbers {
        return None;
    }

    let code_open = block.find("<code")?;
    let code_body_start = code_open + block[code_open..].find('>')? + 1;
    let code_body_end = block.rfind("</code>")?;
    let lines = split_lines(&block[code_body_start..code_body_end]);

    let mut classes = vec!["mdv-lines"];
    if numbers {
        classes.push("mdv-numbered");
    }
    let mut pre_open = add_class(pre_tag, &classes.join(" "));
    pre_open.push('>');
    let mut body = String::with_capacity(block.len() + lines.len() * 32);
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        let lit = meta.highlight.iter().any(|(a, b)| (*a..=*b).contains(&n));
        body.push_str(if lit {
            "<span class=\"mdv-line mdv-hl\">"
        } else {
            "<span class=\"mdv-line\">"
        });
        body.push_str(line);
        body.push_str("</span>");
    }
    let pre = format!(
        "{pre_open}{}{body}</code></pre>",
        &block[pre_tag.len() + 1..code_body_start]
    );
    Some(match &meta.title {
        Some(title) => format!(
            "<figure class=\"mdv-code-figure\"><figcaption class=\"mdv-code-title\">{}</figcaption>{pre}</figure>",
            escape_text(title)
        ),
        None => pre,
    })
}

/// Split highlighted HTML into lines, closing any `<span>`s open at a line
/// break and reopening them on the next line, so each line stands alone.
fn split_lines(html: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut open: Vec<&str> = Vec::new();
    let mut line = String::new();
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < html.len() {
        if bytes[i] == b'<' {
            let end = html[i..].find('>').map_or(html.len(), |e| i + e + 1);
            let tag = &html[i..end];
            if tag.starts_with("</") {
                open.pop();
            } else if !tag.ends_with("/>") {
                open.push(tag);
            }
            line.push_str(tag);
            i = end;
        } else if bytes[i] == b'\n' {
            for _ in &open {
                line.push_str("</span>");
            }
            lines.push(std::mem::take(&mut line));
            for tag in &open {
                line.push_str(tag);
            }
            i += 1;
        } else {
            let next = html[i..].find(['<', '\n']).map_or(html.len(), |n| i + n);
            line.push_str(&html[i..next]);
            i = next;
        }
    }
    // Code ends with a newline; don't count the empty (tags-only) line
    // after it.
    if has_text(&line) {
        lines.push(line);
    }
    lines
}

/// Does this HTML contain any text outside tags?
fn has_text(html: &str) -> bool {
    let mut in_tag = false;
    html.chars().any(|c| match c {
        '<' => {
            in_tag = true;
            false
        }
        '>' => {
            in_tag = false;
            false
        }
        _ => !in_tag,
    })
}

fn parse_meta(meta: &str) -> Meta {
    let mut m = Meta::default();
    let mut rest = meta.trim();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("title=") {
            let (value, remaining) = quoted(after);
            m.title = Some(value.to_string()).filter(|t| !t.is_empty());
            rest = remaining;
        } else if let Some(after) = rest.strip_prefix('{') {
            let close = after.find('}').unwrap_or(after.len());
            m.highlight = parse_ranges(&after[..close]);
            rest = after.get(close + 1..).unwrap_or("");
        } else {
            let word_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let word = &rest[..word_end];
            if matches!(
                word.to_ascii_lowercase().as_str(),
                "showlinenumbers" | "linenos" | "linenums" | "numbered" | "numbers"
            ) {
                m.numbers = true;
            }
            rest = &rest[word_end..];
        }
        rest = rest.trim_start();
    }
    m
}

/// A value that may be quoted with " or ', and what follows it.
fn quoted(s: &str) -> (&str, &str) {
    for q in ['"', '\''] {
        if let Some(inner) = s.strip_prefix(q) {
            let end = inner.find(q).unwrap_or(inner.len());
            return (&inner[..end], inner.get(end + 1..).unwrap_or(""));
        }
    }
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    (&s[..end], &s[end..])
}

/// "1,3-5" → [(1,1), (3,5)]
fn parse_ranges(s: &str) -> Vec<(usize, usize)> {
    s.split(',')
        .filter_map(|part| {
            let part = part.trim();
            match part.split_once('-') {
                Some((a, b)) => Some((a.trim().parse().ok()?, b.trim().parse().ok()?)),
                None => part.parse().ok().map(|n| (n, n)),
            }
        })
        .filter(|(a, b)| a <= b && *a > 0)
        .collect()
}

/// The value of attribute `name` in an opening tag.
fn attr(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = at + tag[at..].find('"')?;
    Some(tag[at..end].to_string())
}

/// Add classes to an opening tag (without its closing `>`).
fn add_class(tag: &str, classes: &str) -> String {
    match tag.find(" class=\"") {
        Some(at) => {
            let insert = at + " class=\"".len();
            format!("{}{} {}", &tag[..insert], classes, &tag[insert..])
        }
        None => format!("{tag} class=\"{}\"", escape_attr(classes)),
    }
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_info_string_extras() {
        let m = parse_meta("title=\"src/main.rs\" {1,3-5} showLineNumbers");
        assert_eq!(m.title.as_deref(), Some("src/main.rs"));
        assert_eq!(m.highlight, [(1, 1), (3, 5)]);
        assert!(m.numbers);
        assert_eq!(parse_meta("title=x.py").title.as_deref(), Some("x.py"));
        assert_eq!(parse_meta("{5-2,0,x}").highlight, []);
    }

    #[test]
    fn splits_lines_keeping_spans_balanced() {
        let html = "<span class=\"a\">one\n<span class=\"b\">two</span>\nthree</span>\n";
        let lines = split_lines(html);
        assert_eq!(
            lines,
            [
                "<span class=\"a\">one</span>",
                "<span class=\"a\"><span class=\"b\">two</span></span>",
                "<span class=\"a\">three</span>",
            ]
        );
    }

    #[test]
    fn no_empty_line_after_a_trailing_newline_inside_a_span() {
        let lines = split_lines("<span class=\"s\">a\nb\n</span>");
        assert_eq!(lines.len(), 2, "{lines:?}");
    }

    #[test]
    fn plain_blocks_are_untouched() {
        let html = "<pre lang=\"rust\"><code>fn x() {}\n</code></pre>";
        assert_eq!(finish(html.into(), false), html);
    }

    #[test]
    fn numbers_highlights_and_titles() {
        let html = "<pre data-meta=\"title=&quot;a.rs&quot; {2}\" lang=\"rust\" class=\"syntax-highlighting\"><code>one\ntwo\n</code></pre>";
        let out = finish(html.into(), true);
        assert!(out.starts_with("<figure class=\"mdv-code-figure\"><figcaption class=\"mdv-code-title\">a.rs</figcaption>"), "{out}");
        assert!(
            out.contains("class=\"mdv-lines mdv-numbered syntax-highlighting\""),
            "{out}"
        );
        assert!(
            out.contains(
                "<span class=\"mdv-line\">one</span><span class=\"mdv-line mdv-hl\">two</span>"
            ),
            "{out}"
        );
    }

    #[test]
    fn diagrams_and_math_are_skipped() {
        let html = "<pre class=\"mermaid\">graph TD\nA</pre>";
        assert_eq!(finish(html.into(), true), html);
    }
}
