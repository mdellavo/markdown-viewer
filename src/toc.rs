//! `[TOC]` (on a line of its own) or `<!-- TOC -->` inserts a table of
//! contents built from the document's headings. `<!-- TOC max2 -->` limits
//! it to two levels (the default is three). A single h1 is treated as the
//! title and left out, as in the viewer's sidebar.

/// Replace TOC markers in rendered HTML.
pub fn finish(html: String) -> String {
    let lower = html.to_ascii_lowercase();
    if !lower.contains("[toc]") && !lower.contains("toc") {
        return html;
    }
    let markers = find_markers(&html);
    if markers.is_empty() {
        return html;
    }
    let headings = headings(&html);
    let mut out = String::with_capacity(html.len() + 1024);
    let mut last = 0;
    for m in markers {
        out.push_str(&html[last..m.start]);
        out.push_str(&render(&headings, m.depth, m.sourcepos.as_deref()));
        last = m.end;
    }
    out.push_str(&html[last..]);
    out
}

struct Marker {
    start: usize,
    end: usize,
    depth: usize,
    sourcepos: Option<String>,
}

/// `<p …>[TOC]</p>` and `<!-- TOC [maxN] -->`, in document order.
fn find_markers(html: &str) -> Vec<Marker> {
    let mut markers = Vec::new();
    let mut i = 0;
    while let Some(at) = html[i..].find('<').map(|p| i + p) {
        let rest = &html[at..];
        if rest.starts_with("<p") {
            let Some(open_end) = rest.find('>') else {
                break;
            };
            let after = &rest[open_end + 1..];
            if after.len() >= 9 && after[..9].eq_ignore_ascii_case("[TOC]</p>") {
                let tag = &rest[..open_end];
                markers.push(Marker {
                    start: at,
                    end: at + open_end + 1 + 9,
                    depth: 3,
                    sourcepos: attr(tag, "data-sourcepos"),
                });
            }
        } else if let Some(comment) = rest.strip_prefix("<!--")
            && let Some(close) = comment.find("-->")
        {
            let words: Vec<&str> = comment[..close].split_whitespace().collect();
            if words.first().is_some_and(|w| w.eq_ignore_ascii_case("toc")) {
                let depth = words
                    .get(1)
                    .and_then(|w| w.to_ascii_lowercase().strip_prefix("max")?.parse().ok())
                    .filter(|d: &usize| (1..=6).contains(d))
                    .unwrap_or(3);
                markers.push(Marker {
                    start: at,
                    end: at + 4 + close + 3,
                    depth,
                    sourcepos: None,
                });
            }
        }
        i = at + 1;
    }
    markers
}

/// (level, id, inner HTML text) for every heading with an id.
fn headings(html: &str) -> Vec<(usize, String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(at) = html[i..].find("<h").map(|p| i + p) {
        let rest = &html[at..];
        let level = rest.as_bytes().get(2).copied().unwrap_or(0);
        if (b'1'..=b'6').contains(&level) && matches!(rest.as_bytes().get(3), Some(b' ' | b'>')) {
            let level = (level - b'0') as usize;
            let open_end = rest.find('>').unwrap_or(0);
            let close = format!("</h{level}>");
            if let (Some(id), Some(end)) = (attr(&rest[..open_end], "id"), rest.find(&close)) {
                out.push((level, id, text_of(&rest[open_end + 1..end])));
            }
        }
        i = at + 2;
    }
    out
}

/// The heading's text: its HTML without tags (and without the hover anchor).
fn text_of(inner: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// A heading in the contents, with the headings under it.
struct Node<'a> {
    id: &'a str,
    text: &'a str,
    children: Vec<Node<'a>>,
}

/// Nest headings by level (a skipped level just nests one deeper).
fn tree<'a>(items: &[&'a (usize, String, String)]) -> Vec<Node<'a>> {
    let mut roots = Vec::new();
    let mut stack: Vec<(usize, Node<'a>)> = Vec::new();
    let attach =
        |node: Node<'a>, stack: &mut Vec<(usize, Node<'a>)>, roots: &mut Vec<Node<'a>>| match stack
            .last_mut()
        {
            Some((_, parent)) => parent.children.push(node),
            None => roots.push(node),
        };
    for (level, id, text) in items.iter().map(|i| (i.0, &i.1, &i.2)) {
        while stack.last().is_some_and(|(l, _)| *l >= level) {
            let (_, done) = stack.pop().unwrap();
            attach(done, &mut stack, &mut roots);
        }
        stack.push((
            level,
            Node {
                id,
                text,
                children: Vec::new(),
            },
        ));
    }
    while let Some((_, done)) = stack.pop() {
        attach(done, &mut stack, &mut roots);
    }
    roots
}

/// A list of headings; one with headings under it can be folded (its
/// chevron folds it, its text jumps to it).
fn write_list(nodes: &[Node], out: &mut String) {
    out.push_str("<ul>");
    for node in nodes {
        let link = format!("<a href=\"#{}\">{}</a>", node.id, node.text);
        if node.children.is_empty() {
            out.push_str(&format!("<li>{link}</li>"));
        } else {
            out.push_str(&format!("<li><details open><summary>{link}</summary>"));
            write_list(&node.children, out);
            out.push_str("</details></li>");
        }
    }
    out.push_str("</ul>");
}

/// The contents: a `<details>` (so it folds, with or without scripts)
/// holding the nested list.
fn render(headings: &[(usize, String, String)], depth: usize, sourcepos: Option<&str>) -> String {
    let h1s = headings.iter().filter(|(l, _, _)| *l == 1).count();
    let top = if h1s <= 1 {
        headings
            .iter()
            .map(|(l, _, _)| *l)
            .filter(|l| *l > 1)
            .min()
            .unwrap_or(2)
    } else {
        1
    };
    let items: Vec<_> = headings
        .iter()
        .filter(|(l, _, _)| *l >= top && *l < top + depth)
        .collect();
    let pos = sourcepos
        .map(|p| format!(" data-sourcepos=\"{p}\""))
        .unwrap_or_default();
    let mut out = format!(
        "<nav class=\"mdv-toc\" aria-label=\"Contents\"{pos}><details open><summary>Contents</summary>"
    );
    let nodes = tree(&items);
    if !nodes.is_empty() {
        write_list(&nodes, &mut out);
    }
    out.push_str("</details></nav>\n");
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = at + tag[at..].find('"')?;
    Some(tag[at..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "<h1 id=\"title\">Title</h1>\n<p data-sourcepos=\"3:1-3:5\">[TOC]</p>\n\
        <h2 id=\"a\">A<a class=\"anchor\"></a></h2>\n<h3 id=\"a1\">A &amp; 1</h3>\n\
        <h4 id=\"deep\">Deep</h4>\n<h5 id=\"deeper\">Deeper</h5>\n<h2 id=\"b\">B</h2>\n";

    #[test]
    fn replaces_the_marker_with_nested_contents() {
        let out = finish(DOC.into());
        assert!(!out.contains("[TOC]"));
        assert!(out.contains(
            "<nav class=\"mdv-toc\" aria-label=\"Contents\" data-sourcepos=\"3:1-3:5\">\
             <details open><summary>Contents</summary><ul>"
        ));
        // Headings with headings under them fold; the rest are plain items.
        assert!(
            out.contains(
                "<li><details open><summary><a href=\"#a\">A</a></summary>\
                 <ul><li><details open><summary><a href=\"#a1\">A &amp; 1</a></summary>\
                 <ul><li><a href=\"#deep\">Deep</a></li></ul></details></li></ul></details></li>"
            ),
            "{out}"
        );
        assert!(
            out.contains("<li><a href=\"#b\">B</a></li></ul></details></nav>"),
            "{out}"
        );
        assert!(!out.contains("#title\""), "a lone h1 is the title");
        assert!(!out.contains("#deeper\""), "three levels by default");
    }

    #[test]
    fn comment_marker_with_depth() {
        let doc = DOC.replace(
            "<p data-sourcepos=\"3:1-3:5\">[TOC]</p>",
            "<!-- TOC max1 -->",
        );
        let out = finish(doc);
        assert!(out.contains("href=\"#a\"") && out.contains("href=\"#b\""));
        assert!(!out.contains("href=\"#a1\""), "{out}");
    }

    #[test]
    fn leaves_other_text_alone() {
        let doc = "<p>The [TOC] marker goes on its own line.</p>";
        assert_eq!(finish(doc.into()), doc);
    }
}
