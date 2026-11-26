//! The Markdown source of a page, syntax highlighted (the viewer's source
//! view, `v`): the same `hl-` classes as code blocks, so the theme colours
//! apply, with one `<span class="mdv-line" data-line="N">` per line.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::render::escape_text;
use crate::theme::HL_PREFIX;

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Load the syntax definitions ahead of the first source view.
pub fn warm_up() {
    syntaxes();
}

/// The source view's HTML: highlighted (cached by content, since
/// highlighting a long file takes a while), or plain.
pub fn render(text: &str, highlighted: bool) -> Arc<String> {
    if !highlighted {
        return Arc::new(plain(text));
    }
    static CACHE: OnceLock<Mutex<HashMap<u64, Arc<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let mut h = DefaultHasher::new();
    text.hash(&mut h);
    let key = h.finish();
    if let Some(html) = cache.lock().unwrap().get(&key) {
        return html.clone();
    }
    let html = Arc::new(highlight(text));
    let mut cache = cache.lock().unwrap();
    // A few documents' worth; older versions aren't needed.
    if cache.len() >= 8 {
        cache.clear();
    }
    cache.insert(key, html.clone());
    html
}

/// Lines per chunk. Chunks are `content-visibility: auto`, so the browser
/// only lays out the ones on screen: long files open quickly.
const CHUNK: usize = 200;

/// Wrap rendered lines (HTML, one per line) into the source view's markup.
fn wrap(lines: impl Iterator<Item = String>) -> String {
    let mut out = String::from("<pre class=\"mdv-source-code\"><code>");
    for (i, line) in lines.enumerate() {
        if i % CHUNK == 0 {
            if i > 0 {
                out.push_str("</span>");
            }
            out.push_str("<span class=\"mdv-chunk\">");
        }
        out.push_str(&format!(
            "<span class=\"mdv-line\" data-line=\"{}\">{line}</span>",
            i + 1
        ));
    }
    if out.ends_with("</span>") {
        out.push_str("</span>");
    }
    out.push_str("</code></pre>\n");
    out
}

/// The source as escaped lines, without highlighting (fast).
pub fn plain(text: &str) -> String {
    wrap(text.lines().map(escape_text))
}

/// Highlighted HTML for Markdown source.
pub fn highlight(text: &str) -> String {
    let set = syntaxes();
    let syntax = set
        .find_syntax_by_extension("md")
        .unwrap_or_else(|| set.find_syntax_plain_text());
    let mut generator = ClassedHTMLGenerator::new_with_class_style(
        syntax,
        set,
        ClassStyle::SpacedPrefixed { prefix: HL_PREFIX },
    );
    let mut highlighted = true;
    for line in LinesWithEndings::from(text) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            highlighted = false;
            break;
        }
    }
    let html = if highlighted {
        generator.finalize()
    } else {
        escape_text(text)
    };
    // Highlighting spans can cross line breaks; split_lines closes and
    // reopens them so each line stands alone.
    let mut lines = crate::codeblocks::split_lines(&html);
    // A trailing newline doesn't start another line.
    if text.ends_with('\n') && lines.last().is_some_and(|l| !has_text(l)) {
        lines.pop();
    }
    wrap(lines.into_iter())
}

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
        c => !in_tag && !c.is_whitespace(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_sources_are_chunked() {
        let text: String = (0..450).map(|i| format!("line {i}\n")).collect();
        let html = plain(&text);
        assert_eq!(html.matches("class=\"mdv-chunk\"").count(), 3);
        assert_eq!(html.matches("class=\"mdv-line\"").count(), 450);
        assert_eq!(
            html.matches("<span").count(),
            html.matches("</span>").count()
        );
    }

    #[test]
    fn plain_matches_highlighted_lines() {
        let text = "# Title\n\n<b>x</b> & y\n";
        let plain = plain(text);
        assert_eq!(plain.matches("class=\"mdv-line\"").count(), 3);
        assert!(plain.contains("&lt;b&gt;x&lt;/b&gt; &amp; y"));
        assert!(!plain.contains("hl-"));
        assert_eq!(
            render(text, true).matches("class=\"mdv-line\"").count(),
            highlight(text).matches("class=\"mdv-line\"").count()
        );
        assert!(
            Arc::ptr_eq(&render(text, true), &render(text, true)),
            "cached"
        );
    }

    #[test]
    fn highlights_markdown_one_span_per_line() {
        let html =
            highlight("# Title\n\nSome **bold** <b>text</b>\n\n```rust\nfn main() {}\n```\n");
        assert!(html.starts_with("<pre class=\"mdv-source-code\"><code>"));
        assert_eq!(html.matches("class=\"mdv-line\"").count(), 7, "{html}");
        assert!(html.contains("data-line=\"1\""));
        assert!(html.contains("data-line=\"7\""));
        assert!(!html.contains("data-line=\"8\""));
        assert!(html.contains("hl-"), "highlighted: {html}");
        // Raw HTML in the source is shown, not run.
        assert!(
            html.contains("&lt;b&gt;") || html.contains("&lt;"),
            "{html}"
        );
        assert!(!html.contains("<b>text"));
    }
}
