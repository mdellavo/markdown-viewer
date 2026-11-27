//! Video and audio linked as images (`![demo](demo.mp4)`) become players,
//! as GitHub and Obsidian show them; see `media`.
//!
//! Model and map files linked as images, drawn like their code blocks:
//! `![part](part.stl)` (binary STL too), `.obj`, `.ply`, `.off`, and
//! `![ride](ride.gpx)`, `.kml`, `.geojson`, `.topojson`, `.wkt`. GitHub
//! shows STL and GeoJSON files this way when browsing a repository.
//!
//! Only an image alone in its paragraph is drawn (the drawing replaces the
//! paragraph, keeping its source position), and only for a relative path,
//! not a web address.

use std::path::Path;

use percent_encoding::percent_decode_str;

/// Biggest file read.
const MAX_BYTES: u64 = 20 * 1024 * 1024;

enum Kind {
    Model(crate::stl::Format),
    Map(crate::geo::Format),
    Molecule(String),
    Calendar,
}

fn kind(src: &str) -> Option<Kind> {
    let path = src.split(['?', '#']).next()?;
    let ext = path.rsplit_once('.')?.1;
    if crate::molecules::from_extension(ext) {
        return Some(Kind::Molecule(ext.to_string()));
    }
    if ext.eq_ignore_ascii_case("ics") {
        return Some(Kind::Calendar);
    }
    crate::stl::Format::from_extension(ext)
        .map(Kind::Model)
        .or_else(|| crate::geo::Format::from_extension(ext).map(Kind::Map))
}

const VIDEO: &[&str] = &["mp4", "m4v", "webm", "mov", "ogv"];
const AUDIO: &[&str] = &["mp3", "m4a", "wav", "ogg", "oga", "opus", "flac", "aac"];

/// `<img>` tags for video and audio files become `<video>`/`<audio>`
/// players (anywhere, local or on the web). The alt text becomes the
/// player's label.
pub fn media(html: String) -> String {
    if !html.contains("<img ") {
        return html;
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(i) = rest.find("<img ") {
        out.push_str(&rest[..i]);
        let tag_end = rest[i..].find('>').map_or(rest.len(), |e| i + e + 1);
        let tag = &rest[i..tag_end];
        let player = attr(tag, "src").and_then(|src| {
            let path = src.split(['?', '#']).next()?.to_ascii_lowercase();
            let ext = path.rsplit_once('.')?.1.to_string();
            let element = if VIDEO.contains(&ext.as_str()) {
                "video"
            } else if AUDIO.contains(&ext.as_str()) {
                "audio"
            } else {
                return None;
            };
            let label = attr(tag, "alt")
                .filter(|a| !a.is_empty())
                .or_else(|| attr(tag, "title"))
                .map(|a| format!(" aria-label=\"{}\" title=\"{}\"", escape_attr(&a), escape_attr(&a)))
                .unwrap_or_default();
            Some(format!(
                "<{element} class=\"mdv-{element}\" src=\"{}\" controls preload=\"metadata\"{label}></{element}>",
                escape_attr(&src)
            ))
        });
        match player {
            Some(p) => out.push_str(&p),
            None => out.push_str(tag),
        }
        rest = &rest[tag_end..];
    }
    out.push_str(rest);
    out
}

use crate::render::escape_attr;

/// Draw the model and map images in `html`, reading files relative to `dir`.
pub fn finish(html: String, dir: &Path) -> String {
    if !html.contains("<img") {
        return html;
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(i) = rest.find("<p") {
        let (before, para) = rest.split_at(i);
        out.push_str(before);
        let Some(end) = para.find("</p>") else {
            out.push_str(para);
            rest = "";
            break;
        };
        let whole = &para[..end + 4];
        match draw_paragraph(whole, dir) {
            Some(drawn) => {
                out.push_str(drawn.trim_end_matches('\n'));
                rest = &para[end + 4..];
            }
            // Not one (`<pre>`, text): carry on just after this "<p".
            None => {
                out.push_str("<p");
                rest = &para[2..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `<p …><img src="…" …></p>`, drawn; `None` to leave it as it is.
fn draw_paragraph(p: &str, dir: &Path) -> Option<String> {
    let open_end = p.find('>')?;
    let open = &p[..open_end];
    // `<p>` or `<p data-sourcepos="…">`, not `<pre>` or `<picture>`.
    if !(open == "<p" || open.starts_with("<p ")) {
        return None;
    }
    let inner = p[open_end + 1..p.len() - 4].trim();
    if !inner.starts_with("<img ") || !inner.ends_with('>') || inner[1..].contains('<') {
        return None;
    }
    let src = attr(inner, "src")?;
    let kind = kind(&src)?;
    if src.contains("://") || src.starts_with('/') || src.starts_with("data:") {
        return None;
    }
    let path = src.split(['?', '#']).next()?;
    let path = percent_decode_str(path).decode_utf8().ok()?;
    let file = dir.join(path.as_ref());
    let meta = std::fs::metadata(&file).ok()?;
    if !meta.is_file() {
        return None;
    }
    let sourcepos = open[2..].trim();
    let sourcepos = if sourcepos.is_empty() {
        String::new()
    } else {
        format!(" {sourcepos}")
    };
    let drawn = if meta.len() > MAX_BYTES {
        Err(format!("it's over {} MB", MAX_BYTES / 1024 / 1024))
    } else {
        let bytes = std::fs::read(&file).ok()?;
        match &kind {
            Kind::Model(f) => crate::stl::file(*f, &bytes, file.parent(), &sourcepos),
            Kind::Map(f) => crate::geo::file(*f, &bytes, &sourcepos),
            Kind::Molecule(ext) => crate::molecules::file(ext, &bytes, &sourcepos),
            Kind::Calendar => crate::calendar::file(&bytes, &sourcepos),
        }
    };
    Some(drawn.unwrap_or_else(|why| {
        let class = match kind {
            Kind::Model(_) => "mdv-stl mdv-stl-invalid",
            Kind::Map(_) => "mdv-map mdv-map-invalid",
            Kind::Molecule(_) | Kind::Calendar => "mdv-diagram mdv-diagram-invalid",
        };
        format!(
            "<figure class=\"{class}\"{sourcepos}><figcaption>Can't draw {}: {}</figcaption></figure>",
            crate::render::escape_text(&path),
            crate::render::escape_text(&why)
        )
    }))
}

/// An attribute's value, with comrak's escaping undone.
fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let value = &tag[start..start + tag[start..].find('"')?];
    Some(
        value
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_and_audio_become_players() {
        let html = "<p><img src=\"clips/demo.MP4?t=1\" alt=\"A &quot;demo&quot;\" /> and \
                    <img src=\"https://x.test/song.mp3\" alt=\"\" /> <img src=\"a.png\" alt=\"x\" /></p>";
        let out = media(html.into());
        assert_eq!(
            out,
            "<p><video class=\"mdv-video\" src=\"clips/demo.MP4?t=1\" controls preload=\"metadata\" \
             aria-label=\"A &quot;demo&quot;\" title=\"A &quot;demo&quot;\"></video> and \
             <audio class=\"mdv-audio\" src=\"https://x.test/song.mp3\" controls preload=\"metadata\"></audio> \
             <img src=\"a.png\" alt=\"x\" /></p>"
        );
    }

    #[test]
    fn model_and_map_images_are_drawn() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("tri angle.obj"),
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("p.wkt"), "POINT (1 2)").unwrap();
        std::fs::write(dir.path().join("bad.stl"), "nope").unwrap();
        let html = "<p data-sourcepos=\"1:1-1:20\"><img src=\"tri%20angle.obj\" alt=\"T\" /></p>\n\
                    <p><img src=\"p.wkt\" alt=\"\" /></p>\n<p>text <img src=\"p.wkt\" alt=\"\" /></p>\n\
                    <p><img src=\"bad.stl\" alt=\"\" /></p>\n<p><img src=\"missing.stl\" alt=\"\" /></p>\n\
                    <p><img src=\"https://x.test/a.stl\" alt=\"\" /></p>\n<pre><code>x</code></pre>\n\
                    <p><img src=\"p.wkt\" alt=\"\" /></p>\n";
        let out = finish(html.into(), dir.path());
        assert!(
            out.starts_with("<figure class=\"mdv-stl\" data-sourcepos=\"1:1-1:20\" data-mdv-mesh="),
            "{out}"
        );
        assert!(out.contains("<figure class=\"mdv-map\""));
        assert!(
            out.contains("<p>text <img src=\"p.wkt\""),
            "not alone: left"
        );
        assert!(out.contains("Can't draw bad.stl: an ASCII STL file starts with"));
        assert!(out.contains("<img src=\"missing.stl\""));
        assert!(out.contains("<img src=\"https://x.test/a.stl\""));
        assert!(out.contains("</pre>\n<figure class=\"mdv-map\">"), "{out}");
        assert_eq!(out.lines().count(), html.lines().count());
    }
}
