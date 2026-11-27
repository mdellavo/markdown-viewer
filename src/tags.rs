//! Tags: `#tags` in the text (Obsidian notes) and `tags:` in front matter
//! (every page), shown as chips, and the index of tags across the served
//! folder for the sidebar's Tags tab (`/__mdv/tags`).
//!
//! A tag is `#` and letters, digits, `_`, `-` and `/` (nested tags, as in
//! Obsidian), with at least one character that isn't a digit (`#123` is an
//! issue reference), after the start of a line, a space or an opening
//! bracket. Tags match case-insensitively.

use std::collections::BTreeMap;
use std::path::Path;

use crate::render::{escape_attr, escape_text};

fn tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '/')
}

/// The tag starting at `s` (after its `#`), if it is one.
fn tag_at(s: &str) -> Option<&str> {
    let len = s
        .char_indices()
        .find(|&(_, c)| !tag_char(c))
        .map_or(s.len(), |(i, _)| i);
    let tag = &s[..len];
    let ok = !tag.is_empty()
        && !tag
            .chars()
            .all(|c| c.is_ascii_digit() || c == '/' || c == '-')
        && tag.chars().next().is_some_and(|c| c != '/' && c != '-');
    ok.then_some(tag)
}

fn starts_tag(before: Option<char>) -> bool {
    before.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '[' | '{' | ',' | ';'))
}

/// A tag chip. ui.js makes it open the Tags tab at that tag.
pub fn chip(tag: &str) -> String {
    let tag = tag.trim_start_matches('#');
    format!(
        "<a class=\"mdv-tag\" data-mdv-tag=\"{}\">#{}</a>",
        escape_attr(&tag.to_lowercase()),
        escape_text(tag)
    )
}

/// Elements whose text never holds tags.
const SKIP: &[&str] = &[
    "a", "code", "pre", "kbd", "samp", "script", "style", "svg", "math", "textarea",
];

/// Turn `#tags` in rendered HTML text into chips.
pub fn finish(html: &str) -> String {
    if !html.contains('#') {
        return html.to_string();
    }
    let mut out = String::with_capacity(html.len() + 256);
    let mut skip: Vec<String> = Vec::new();
    let mut rest = html;
    loop {
        let at = rest.find('<').unwrap_or(rest.len());
        let (text, tag_start) = rest.split_at(at);
        if skip.is_empty() {
            push_text(&mut out, text);
        } else {
            out.push_str(text);
        }
        if tag_start.is_empty() {
            break;
        }
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
            if let Some(i) = skip.iter().rposition(|n| *n == name) {
                skip.truncate(i);
            }
        } else if !tag.ends_with("/>")
            && (SKIP.contains(&name.as_str()) || tag.contains("data-math-style"))
        {
            skip.push(name);
        }
        rest = &tag_start[end..];
    }
    out
}

fn push_text(out: &mut String, text: &str) {
    let mut copied = 0;
    for (i, c) in text.char_indices() {
        if c != '#' || i < copied {
            continue;
        }
        // At the start of a text node, look at what was written before it,
        // unless that ends an element (`<p>#tag`).
        let before = text[..i].chars().next_back().or_else(|| {
            if i == 0 {
                out.chars().next_back().filter(|c| *c != '>')
            } else {
                None
            }
        });
        if !starts_tag(before) {
            continue;
        }
        if let Some(tag) = tag_at(&text[i + 1..]) {
            out.push_str(&text[copied..i]);
            out.push_str(&chip(tag));
            copied = i + 1 + tag.len();
        }
    }
    out.push_str(&text[copied..]);
}

/// The `tags:` (or `tag:`) of YAML front matter: a list, or a string of
/// names separated by commas or spaces. Leading `#`s are dropped.
pub fn front_matter_tags(value: &yaml_rust2::Yaml) -> Vec<String> {
    use yaml_rust2::Yaml;
    let names: Vec<String> = match value {
        Yaml::Array(items) => items
            .iter()
            .filter_map(|v| match v {
                Yaml::String(s) => Some(s.clone()),
                Yaml::Integer(i) => Some(i.to_string()),
                _ => None,
            })
            .collect(),
        Yaml::String(s) => s.split([',', ' ']).map(str::to_string).collect(),
        _ => Vec::new(),
    };
    names
        .into_iter()
        .map(|n| n.trim().trim_start_matches('#').to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

pub fn is_tags_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("tags") || key.eq_ignore_ascii_case("tag")
}

/// Tags in a Markdown file, with their lines: front matter tags, and (with
/// `inline`) `#tags` in the text, outside code.
pub fn in_markdown(text: &str, inline: bool) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    let mut body_start = 0;
    if let Some((yaml, end)) = crate::frontmatter::extract(text) {
        body_start = end;
        if let Ok(docs) = yaml_rust2::YamlLoader::load_from_str(yaml)
            && let Some(yaml_rust2::Yaml::Hash(map)) = docs.first()
        {
            for (k, v) in map {
                if k.as_str().is_some_and(is_tags_key) {
                    found.extend(front_matter_tags(v).into_iter().map(|t| (t, 1)));
                }
            }
        }
    }
    if !inline {
        return found;
    }
    let mut fence: Option<String> = None;
    for (n, line) in text.lines().enumerate().skip(body_start) {
        let trimmed = line.trim_start();
        let mark: String = trimmed
            .chars()
            .take_while(|c| *c == '`' || *c == '~')
            .collect();
        if mark.len() >= 3 && mark.chars().all(|c| c == mark.chars().next().unwrap()) {
            match &fence {
                None => fence = Some(mark),
                Some(open) if mark.starts_with(open.as_str()) => fence = None,
                _ => {}
            }
            continue;
        }
        if fence.is_some() || line.starts_with("    ") || line.starts_with('\t') {
            continue;
        }
        // Inline code spans hold no tags.
        let mut in_code = false;
        let mut prev: Option<char> = None;
        for (i, c) in line.char_indices() {
            if c == '`' {
                in_code = !in_code;
            } else if c == '#'
                && !in_code
                && starts_tag(prev)
                && let Some(tag) = tag_at(&line[i + 1..])
            {
                found.push((tag.to_string(), n + 1));
            }
            prev = Some(c);
        }
    }
    found
}

/// One tag in the index: how it's written, and where.
#[derive(serde::Serialize, Debug)]
pub struct Tagged {
    pub tag: String,
    pub files: Vec<TagFile>,
}

#[derive(serde::Serialize, Debug)]
pub struct TagFile {
    pub file: String,
    pub line: usize,
    pub count: usize,
}

/// Every tag in `files` (root-relative Markdown files), sorted by name;
/// each file once per tag, at its first use.
pub fn index(root: &Path, files: &[String], inline: bool) -> Vec<Tagged> {
    let mut map: BTreeMap<String, Tagged> = BTreeMap::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(root.join(file)) else {
            continue;
        };
        for (tag, line) in in_markdown(&text, inline) {
            let entry = map.entry(tag.to_lowercase()).or_insert_with(|| Tagged {
                tag: tag.clone(),
                files: Vec::new(),
            });
            match entry.files.iter_mut().find(|f| f.file == *file) {
                Some(f) => f.count += 1,
                None => entry.files.push(TagFile {
                    file: file.clone(),
                    line,
                    count: 1,
                }),
            }
        }
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_in_text_become_chips() {
        let html = "<p>A #note about #Rust/async, not #123 or C#, a#b, \
                    <a href=\"#x\">#link</a> <code>#code</code> &#39;q&#39; (#paren) #trail-.</p>";
        let out = finish(html);
        assert_eq!(out.matches("class=\"mdv-tag\"").count(), 4, "{out}");
        assert!(out.contains("<a class=\"mdv-tag\" data-mdv-tag=\"rust/async\">#Rust/async</a>"));
        assert!(out.contains("(<a class=\"mdv-tag\" data-mdv-tag=\"paren\">#paren</a>)"));
        assert!(out.contains("not #123 or C#, a#b"));
        assert!(out.contains("<code>#code</code>") && out.contains(">#link</a>"));
        assert!(out.contains("&#39;q&#39;"));
        assert!(
            out.contains("data-mdv-tag=\"trail-\">#trail-</a>."),
            "{out}"
        );
    }

    #[test]
    fn tags_in_markdown_files() {
        let md = "---\ntitle: T\ntags: [Project, \"#idea\"]\n---\n# Heading\n\nSome #topic text.\n\n```\n#not-in-code\n```\n\n`#nor-here` but #here\n";
        let tags = in_markdown(md, true);
        assert_eq!(
            tags,
            vec![
                ("Project".into(), 1),
                ("idea".into(), 1),
                ("topic".into(), 7),
                ("here".into(), 13)
            ]
        );
        assert_eq!(in_markdown(md, false).len(), 2, "front matter only");
        let one = yaml_rust2::YamlLoader::load_from_str("tags: a, b c").unwrap();
        assert_eq!(front_matter_tags(&one[0]["tags"]), vec!["a", "b", "c"]);
    }

    #[test]
    fn index_groups_files_by_tag() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "#Rust and #rust again, #web\n").unwrap();
        std::fs::write(dir.path().join("b.md"), "---\ntags: rust\n---\nText\n").unwrap();
        let files = vec!["a.md".to_string(), "b.md".to_string()];
        let idx = index(dir.path(), &files, true);
        assert_eq!(idx.len(), 2);
        assert_eq!(idx[0].tag, "Rust");
        assert_eq!(idx[0].files.len(), 2);
        assert_eq!(idx[0].files[0].count, 2);
        assert_eq!(idx[1].tag, "web");
    }
}
