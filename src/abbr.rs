//! Abbreviations, as in PHP Markdown Extra and MultiMarkdown:
//!
//! ```markdown
//! The HTML spec is maintained by the W3C.
//!
//! *[HTML]: Hyper Text Markup Language
//! *[W3C]: World Wide Web Consortium
//! ```
//!
//! Definition lines are blanked before rendering (keeping line numbers for
//! source positions), and every whole-word use in the text becomes
//! `<abbr title="…">`. Text in code, links' addresses, math and diagrams is
//! left alone.

/// Abbreviation definitions, longest first (so `HTML5` wins over `HTML`).
pub type Defs = Vec<(String, String)>;

/// Take the definitions out of `markdown`. `None` if it has none (the
/// common case, which then costs nothing).
pub fn extract(markdown: &str) -> Option<(String, Defs)> {
    if !markdown.contains("*[") {
        return None;
    }
    let mut defs: Defs = Vec::new();
    let mut out = String::with_capacity(markdown.len());
    let mut fence: Option<(char, usize)> = None;
    for line in markdown.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let ending = &line[body.len()..];
        let indent = body.len() - body.trim_start_matches(' ').len();
        let trimmed = body.trim_start_matches(' ');
        // Fenced code: definitions inside are just text.
        if indent < 4 {
            let mark = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            if let Some(c) = mark {
                let run = trimmed.chars().take_while(|x| *x == c).count();
                if run >= 3 {
                    match fence {
                        None => fence = Some((c, run)),
                        Some((open, len))
                            if open == c && run >= len && trimmed[run..].trim().is_empty() =>
                        {
                            fence = None
                        }
                        _ => {}
                    }
                }
            }
        }
        if fence.is_none()
            && indent < 4
            && let Some((abbr, title)) = definition(trimmed)
        {
            defs.retain(|(a, _)| *a != abbr);
            defs.push((abbr, title));
            out.push_str(ending);
            continue;
        }
        out.push_str(line);
    }
    if defs.is_empty() {
        return None;
    }
    defs.sort_by_key(|d| std::cmp::Reverse(d.0.len()));
    Some((out, defs))
}

/// `*[ABBR]: Full text` → (`ABBR`, `Full text`).
fn definition(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("*[")?;
    let (abbr, rest) = rest.split_once("]:")?;
    let abbr = abbr.trim();
    if abbr.is_empty() || abbr.contains(['[', ']']) {
        return None;
    }
    Some((abbr.to_string(), rest.trim().to_string()))
}

/// Elements whose text is never marked.
const SKIP: &[&str] = &[
    "abbr", "code", "pre", "kbd", "samp", "script", "style", "svg", "math", "textarea",
];

/// Wrap the uses of `defs` in rendered HTML in `<abbr>`.
pub fn finish(html: &str, defs: &Defs) -> String {
    // Rendered text is HTML-escaped, so match the escaped forms.
    let keys: Vec<(String, String)> = defs
        .iter()
        .map(|(a, t)| (crate::render::escape_text(a), crate::render::escape_attr(t)))
        .collect();
    let mut out = String::with_capacity(html.len() + 256);
    let mut open: Vec<String> = Vec::new();
    let mut rest = html;
    loop {
        let Some(at) = rest.find('<') else {
            push_text(&mut out, rest, open.is_empty(), &keys);
            break;
        };
        let (text, tag_start) = rest.split_at(at);
        push_text(&mut out, text, open.is_empty(), &keys);
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

/// Letters, digits and `_` join words; anything else separates them.
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn push_text(out: &mut String, text: &str, mark: bool, keys: &[(String, String)]) {
    if !mark || text.is_empty() {
        out.push_str(text);
        return;
    }
    let mut i = 0;
    let mut copied = 0;
    while i < text.len() {
        let starts_word = text[..i].chars().next_back().is_none_or(|c| !word_char(c));
        if starts_word {
            let found = keys.iter().find(|(key, _)| {
                text[i..].starts_with(key.as_str())
                    && text[i + key.len()..]
                        .chars()
                        .next()
                        .is_none_or(|c| !word_char(c))
            });
            if let Some((key, title)) = found {
                out.push_str(&text[copied..i]);
                out.push_str(&format!("<abbr title=\"{title}\">{key}</abbr>"));
                i += key.len();
                copied = i;
                continue;
            }
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.push_str(&text[copied..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_are_taken_out_keeping_lines() {
        let md = "The HTML spec.\n\n*[HTML]: Hyper Text Markup Language\n*[HTML5]: The fifth\n\n```\n*[NOT]: code\n```\n";
        let (out, defs) = extract(md).unwrap();
        assert_eq!(out.lines().count(), md.lines().count());
        assert!(!out.contains("Hyper Text"));
        assert!(out.contains("*[NOT]: code"), "{out}");
        assert_eq!(defs[0].0, "HTML5", "longest first");
        assert_eq!(defs.len(), 2);
        assert!(extract("no definitions").is_none());
    }

    #[test]
    fn whole_words_outside_code_are_marked() {
        let defs: Defs = vec![
            ("HTML5".into(), "The fifth".into()),
            ("HTML".into(), "Hyper \"Text\"".into()),
            ("R&D".into(), "Research".into()),
        ];
        let html = "<p>HTML and HTML5, not HTMLX; <code>HTML</code> \
                    <a href=\"/HTML\">HTML</a> R&amp;D</p>";
        let out = finish(html, &defs);
        assert!(
            out.starts_with("<p><abbr title=\"Hyper &quot;Text&quot;\">HTML</abbr> and <abbr title=\"The fifth\">HTML5</abbr>, not HTMLX;"),
            "{out}"
        );
        assert!(out.contains("<code>HTML</code>"));
        assert!(out.contains("href=\"/HTML\""), "attributes untouched");
        assert!(
            out.contains("<a href=\"/HTML\"><abbr"),
            "link text is marked"
        );
        assert!(
            out.contains("<abbr title=\"Research\">R&amp;D</abbr>"),
            "{out}"
        );
    }
}
