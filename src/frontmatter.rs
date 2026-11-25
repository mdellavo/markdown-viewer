//! YAML front matter, shown as a table the way GitHub does: keys across
//! the top, values underneath, nested maps and lists as nested tables.

use yaml_rust2::{Yaml, YamlLoader};

use crate::render::escape_text;

/// The front matter's YAML and the line its closing `---` is on, if the
/// document starts with front matter.
pub fn extract(markdown: &str) -> Option<(&str, usize)> {
    let body = markdown
        .strip_prefix("---\n")
        .or_else(|| markdown.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for (i, line) in body.split_inclusive('\n').enumerate() {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" || trimmed == "..." {
            return Some((&body[..offset], i + 2));
        }
        offset += line.len();
    }
    None
}

/// GitHub-style table for the front matter, tagged with its source lines
/// (so click-to-source works on it). Invalid YAML is shown as-is.
pub fn table(yaml: &str, end_line: usize, sourcepos: bool) -> String {
    let pos = if sourcepos {
        format!(" data-sourcepos=\"1:1-{end_line}:3\"")
    } else {
        String::new()
    };
    match YamlLoader::load_from_str(yaml) {
        Ok(docs) => match docs.first() {
            None => String::new(),
            Some(doc @ Yaml::Hash(_)) => {
                format!(
                    "<div class=\"mdv-front-matter\" {pos}>{}</div>\n",
                    value_html(doc)
                )
            }
            Some(other) => format!(
                "<div class=\"mdv-front-matter\" {pos}><table><tbody><tr><td>{}</td></tr></tbody></table></div>\n",
                value_html(other)
            ),
        },
        Err(e) => format!(
            "<div class=\"mdv-front-matter mdv-front-matter-error\" {pos} \
             title=\"{}\"><pre><code>{}</code></pre></div>\n",
            crate::render::escape_attr(&format!("Invalid YAML front matter: {e}")),
            escape_text(yaml)
        ),
    }
}

fn value_html(value: &Yaml) -> String {
    match value {
        Yaml::Hash(map) => {
            let mut head = String::new();
            let mut row = String::new();
            for (k, v) in map {
                head.push_str(&format!("<th>{}</th>", scalar(k)));
                row.push_str(&format!("<td>{}</td>", value_html(v)));
            }
            format!("<table><thead><tr>{head}</tr></thead><tbody><tr>{row}</tr></tbody></table>")
        }
        Yaml::Array(items) => {
            let cells: String = items
                .iter()
                .map(|v| format!("<td>{}</td>", value_html(v)))
                .collect();
            format!("<table><tbody><tr>{cells}</tr></tbody></table>")
        }
        other => format!("<div>{}</div>", scalar(other)),
    }
}

fn scalar(value: &Yaml) -> String {
    let text = match value {
        Yaml::String(s) | Yaml::Real(s) => s.clone(),
        Yaml::Integer(i) => i.to_string(),
        Yaml::Boolean(b) => b.to_string(),
        Yaml::Null | Yaml::BadValue => String::new(),
        Yaml::Alias(_) | Yaml::Hash(_) | Yaml::Array(_) => String::new(),
    };
    escape_text(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_front_matter() {
        let md = "---\ntitle: X\ntags: [a, b]\n---\n# Body\n";
        let (yaml, end) = extract(md).unwrap();
        assert_eq!(yaml, "title: X\ntags: [a, b]\n");
        assert_eq!(end, 4);
        assert!(extract("# No front matter\n---\n").is_none());
        assert!(extract("---\nnever closed\n").is_none());
    }

    #[test]
    fn renders_like_github() {
        let html = table(
            "title: A <b>\ntags: [x, y]\nmeta:\n  draft: true\n",
            6,
            true,
        );
        assert!(html.contains("data-sourcepos=\"1:1-6:3\""));
        assert!(
            html.contains("<th>title</th><th>tags</th><th>meta</th>"),
            "{html}"
        );
        assert!(html.contains("<div>A &lt;b&gt;</div>"), "escaped: {html}");
        assert!(
            html.contains("<td><div>x</div></td><td><div>y</div></td>"),
            "{html}"
        );
        assert!(html.contains("<th>draft</th>") && html.contains("<div>true</div>"));
    }

    #[test]
    fn invalid_yaml_is_shown_raw() {
        let html = table("title: [unclosed\n", 3, false);
        assert!(html.contains("mdv-front-matter-error"));
        assert!(html.contains("title: [unclosed"));
    }
}
