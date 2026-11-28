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

/// Local images get their size (`width`, `height` and `data-mdv-sized`,
/// which the page styles keep in proportion), so the page holds their
/// space while they load instead of moving down when each arrives. Only
/// images without a size of their own.
pub fn sizes(html: String, dir: &Path) -> String {
    if !html.contains("<img ") {
        return html;
    }
    let mut out = String::with_capacity(html.len() + 256);
    let mut rest = html.as_str();
    while let Some(i) = rest.find("<img ") {
        out.push_str(&rest[..i]);
        let tag_end = rest[i..].find('>').map_or(rest.len(), |e| i + e + 1);
        let tag = &rest[i..tag_end];
        let size = (|| {
            if attr(tag, "width").is_some() || attr(tag, "height").is_some() {
                return None;
            }
            let src = attr(tag, "src")?;
            if src.contains("://") || src.starts_with('/') || src.starts_with("data:") {
                return None;
            }
            let path = src.split(['?', '#']).next()?;
            let path = percent_decode_str(path).decode_utf8().ok()?;
            image_size(&dir.join(path.as_ref()))
        })();
        match size {
            Some((w, h)) => {
                out.push_str("<img");
                let _ = std::fmt::Write::write_fmt(
                    &mut out,
                    format_args!(" width=\"{w}\" height=\"{h}\" data-mdv-sized"),
                );
                out.push_str(&tag[4..]);
            }
            None => out.push_str(tag),
        }
        rest = &rest[tag_end..];
    }
    out.push_str(rest);
    out
}

/// The pixel size of a PNG, GIF, JPEG or WebP file, from its header.
fn image_size(file: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = Vec::new();
    // JPEG sizes come after any metadata, which is usually well under this.
    std::fs::File::open(file)
        .ok()?
        .take(256 * 1024)
        .read_to_end(&mut head)
        .ok()?;
    let size = header_size(&head)?;
    (size.0 > 0 && size.1 > 0).then_some(size)
}

fn header_size(b: &[u8]) -> Option<(u32, u32)> {
    let be16 = |i: usize| Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?) as u32);
    let le16 = |i: usize| Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?) as u32);
    let be32 = |i: usize| Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?));
    let le24 = |i: usize| {
        let x = b.get(i..i + 3)?;
        Some(x[0] as u32 | (x[1] as u32) << 8 | (x[2] as u32) << 16)
    };
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.get(12..16)? == b"IHDR" {
        return Some((be32(16)?, be32(20)?));
    }
    if b.starts_with(b"GIF8") {
        return Some((le16(6)?, le16(8)?));
    }
    if b.starts_with(b"RIFF") && b.get(8..12)? == b"WEBP" {
        return match b.get(12..16)? {
            b"VP8 " => Some((le16(26)? & 0x3fff, le16(28)? & 0x3fff)),
            b"VP8L" => {
                let x = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?);
                Some(((x & 0x3fff) + 1, ((x >> 14) & 0x3fff) + 1))
            }
            b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1)),
            _ => None,
        };
    }
    if b.starts_with(&[0xff, 0xd8]) {
        let mut i = 2;
        loop {
            while *b.get(i)? != 0xff {
                i += 1;
            }
            while *b.get(i)? == 0xff {
                i += 1;
            }
            let marker = *b.get(i)?;
            i += 1;
            if matches!(marker, 0xd8 | 0x01 | 0xd0..=0xd7) {
                continue;
            }
            let len = be16(i)? as usize;
            // Start of frame (not DHT 0xc4, JPG 0xc8 or DAC 0xcc).
            if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                let (h, w) = (be16(i + 3)?, be16(i + 5)?);
                // EXIF orientation may turn the picture, which browsers
                // follow; leave those unsized rather than guess.
                return (!rotated(&b[..i])).then_some((w, h));
            }
            if marker == 0xd9 || len < 2 {
                return None;
            }
            i += len;
        }
    }
    None
}

/// Whether a JPEG's EXIF data says the picture is turned a quarter.
fn rotated(b: &[u8]) -> bool {
    let Some(at) = b.windows(6).position(|w| w == b"Exif\0\0") else {
        return false;
    };
    let t = &b[at + 6..];
    let little = t.starts_with(b"II");
    let u16_at = |i: usize| {
        t.get(i..i + 2).map(|x| {
            if little {
                u16::from_le_bytes([x[0], x[1]])
            } else {
                u16::from_be_bytes([x[0], x[1]])
            }
        })
    };
    let u32_at = |i: usize| {
        t.get(i..i + 4).map(|x| {
            let x = [x[0], x[1], x[2], x[3]];
            if little {
                u32::from_le_bytes(x)
            } else {
                u32::from_be_bytes(x)
            }
        })
    };
    let Some(ifd) = u32_at(4) else { return false };
    let ifd = ifd as usize;
    let Some(count) = u16_at(ifd) else {
        return false;
    };
    (0..count as usize).any(|n| {
        let e = ifd + 2 + n * 12;
        u16_at(e) == Some(0x0112) && u16_at(e + 8).is_some_and(|o| (5..=8).contains(&o))
    })
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
    fn local_images_get_their_size() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(640u32.to_be_bytes());
        png.extend(480u32.to_be_bytes());
        std::fs::write(d.join("a b.png"), &png).unwrap();
        std::fs::write(d.join("g.gif"), b"GIF89a\x20\x00\x10\x00").unwrap();
        // SOI, an APP0 segment, then a baseline frame of 300x200.
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 11, 8, 0, 200, 1, 44, 3,
        ];
        std::fs::write(d.join("p.jpg"), jpeg).unwrap();
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
        webp.extend([99, 0, 0, 49, 0, 0]);
        std::fs::write(d.join("w.webp"), &webp).unwrap();
        let html = "<p><img src=\"a%20b.png\" alt=\"A\" /> <img src=\"g.gif\" alt=\"\" />\
                    <img src=\"p.jpg?x\" alt=\"\" /><img src=\"w.webp\" alt=\"\" />\
                    <img src=\"g.gif\" width=\"5\" alt=\"\" /><img src=\"missing.png\" alt=\"\" />\
                    <img src=\"https://x.test/g.gif\" alt=\"\" /></p>";
        let out = sizes(html.into(), d);
        assert_eq!(
            out,
            "<p><img width=\"640\" height=\"480\" data-mdv-sized src=\"a%20b.png\" alt=\"A\" /> \
             <img width=\"32\" height=\"16\" data-mdv-sized src=\"g.gif\" alt=\"\" />\
             <img width=\"300\" height=\"200\" data-mdv-sized src=\"p.jpg?x\" alt=\"\" />\
             <img width=\"100\" height=\"50\" data-mdv-sized src=\"w.webp\" alt=\"\" />\
             <img src=\"g.gif\" width=\"5\" alt=\"\" /><img src=\"missing.png\" alt=\"\" />\
             <img src=\"https://x.test/g.gif\" alt=\"\" /></p>"
        );
    }

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
