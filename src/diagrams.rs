//! Diagrams drawn as SVG when the page is built, like maps and models: so
//! they need no script, work offline and appear in exports and PDFs.
//!
//! - ```` ```bob ```` (Svgbob: ASCII art to lines and arrows) and
//!   ```` ```pikchr ```` (the PIC-like language from SQLite), both built in;
//! - ```` ```qr ````: the block's text as a QR code;
//! - ```` ```dot ````/```` ```graphviz ````, ```` ```plantuml ```` and
//!   ```` ```d2 ````, through the program when it's installed (`dot`,
//!   `plantuml`, `d2`), with a time limit. Results are cached by content, as
//!   pages are re-rendered on every save. Without the program the block
//!   stays as code, with a note saying what to install.
//!
//! Each is one top-level `<figure class="mdv-diagram …">` (live updates
//! replace top-level elements one for one).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::render::escape_text;

/// A drawn diagram.
pub fn figure(kind: &str, sourcepos: &str, svg: &str) -> String {
    format!("<figure class=\"mdv-diagram mdv-{kind}\"{sourcepos}>{svg}</figure>\n")
}

/// A block that couldn't be drawn: the source, with what went wrong.
pub fn invalid(name: &str, lang: &str, code: &str, sourcepos: &str, why: &str) -> String {
    format!(
        "<figure class=\"mdv-diagram mdv-diagram-invalid\"{sourcepos}><figcaption>Can't draw this {name}: {}</figcaption>\
         <pre lang=\"{lang}\"><code>{}</code></pre></figure>\n",
        escape_text(why),
        escape_text(code)
    )
}

/// ```bob: ASCII-art diagrams.
pub fn bob(code: &str, sourcepos: &str) -> String {
    let settings = svgbob::Settings {
        stroke_color: "currentColor".into(),
        fill_color: "currentColor".into(),
        // Used for "open" shapes, so they look hollow on either theme.
        background: "var(--bgColor-default, #fff)".into(),
        include_backdrop: false,
        font_family: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace".into(),
        ..Default::default()
    };
    let svg = svgbob::to_svg_with_settings(code, &settings);
    figure("bob", sourcepos, &svg)
}

/// ```pikchr diagrams.
pub fn pikchr(code: &str, sourcepos: &str) -> String {
    match pikchr::Pikchr::render(code, None, pikchr::PikchrFlags::default()) {
        // Black is the default colour; follow the theme's text instead.
        // At its own size, not stretched to the page's width.
        Ok(svg) => figure(
            "pikchr",
            sourcepos,
            &svg.replace("rgb(0,0,0)", "currentColor").replacen(
                "<svg",
                &format!("<svg width=\"{}\"", svg.width()),
                1,
            ),
        ),
        Err(e) => invalid("Pikchr", "pikchr", code, sourcepos, e.to_string().trim()),
    }
}

/// ```qr: the text (trimmed) as a QR code.
pub fn qr(code: &str, sourcepos: &str) -> String {
    let text = code.trim();
    match qrcode::QrCode::new(text.as_bytes()) {
        Ok(q) => {
            let svg = q
                .render::<qrcode::render::svg::Color>()
                .min_dimensions(160, 160)
                .quiet_zone(true)
                .build();
            // Drop the XML declaration; it's inline in HTML.
            let svg = svg.find("<svg").map_or(svg.as_str(), |i| &svg[i..]);
            let svg = svg.replacen(
                "<svg",
                &format!(
                    "<svg role=\"img\" aria-label=\"QR code: {}\"",
                    crate::render::escape_attr(text)
                ),
                1,
            );
            figure("qr", sourcepos, &svg)
        }
        Err(e) => invalid("QR code", "qr", code, sourcepos, &e.to_string()),
    }
}

/// A diagram program: what it's called, how to run it, how long to wait.
struct Program {
    /// For messages ("Graphviz").
    name: &'static str,
    /// The command (`dot`).
    command: &'static str,
    lang: &'static str,
    /// Class suffix: `mdv-{kind}`.
    kind: &'static str,
    args: &'static [&'static str],
    /// Reads a file instead of standard input (`in.ext`, writing `out.svg`).
    file_ext: Option<&'static str>,
    timeout: Duration,
}

const GRAPHVIZ: Program = Program {
    name: "Graphviz",
    command: "dot",
    lang: "dot",
    kind: "dot",
    // A transparent background, so it sits on either theme (colours are
    // switched to the text colour below).
    args: &["-Tsvg", "-Gbgcolor=transparent"],
    file_ext: None,
    timeout: Duration::from_secs(10),
};

const PLANTUML: Program = Program {
    name: "PlantUML",
    command: "plantuml",
    lang: "plantuml",
    kind: "plantuml",
    args: &["-tsvg", "-pipe", "-charset", "UTF-8"],
    file_ext: None,
    // Java starts slowly.
    timeout: Duration::from_secs(30),
};

const D2: Program = Program {
    name: "D2",
    command: "d2",
    lang: "d2",
    kind: "d2",
    // Don't fetch icons from the web; just link them.
    args: &["--bundle=false"],
    file_ext: Some("d2"),
    timeout: Duration::from_secs(20),
};

/// ```dot / ```graphviz.
pub fn graphviz(code: &str, sourcepos: &str) -> String {
    run_program(&GRAPHVIZ, code, sourcepos)
}

/// ```plantuml / ```puml.
pub fn plantuml(code: &str, sourcepos: &str) -> String {
    run_program(&PLANTUML, code, sourcepos)
}

/// ```d2.
pub fn d2(code: &str, sourcepos: &str) -> String {
    run_program(&D2, code, sourcepos)
}

/// Most cached drawings kept (a few pages' worth).
const CACHE_SIZE: usize = 200;

type Cache = Mutex<HashMap<(&'static str, String), Result<String, String>>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn run_program(p: &Program, code: &str, sourcepos: &str) -> String {
    let key = (p.command, code.to_string());
    let cached = cache().lock().unwrap().get(&key).cloned();
    let result = match cached {
        Some(r) => r,
        None => {
            let Some(path) = locate(p.command) else {
                return missing(p, code, sourcepos);
            };
            let r = draw_with(p, &path, code).map(|svg| clean_svg(p, &svg));
            let mut c = cache().lock().unwrap();
            if c.len() >= CACHE_SIZE {
                c.clear();
            }
            c.insert(key, r.clone());
            r
        }
    };
    match result {
        Ok(svg) => figure(p.kind, sourcepos, &svg),
        Err(e) => invalid(p.name, p.lang, code, sourcepos, &e),
    }
}

/// The program on `PATH`, or where package managers put it (apps started
/// from the Finder or a desktop menu get a short `PATH`).
fn locate(command: &str) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("MARKDOWN_VIEWER_DIAGRAM_PATH") {
        // Tests point this at stand-in programs.
        return std::env::split_paths(&dir)
            .find_map(|d| crate::platform::find_program(&d.join(command).to_string_lossy()));
    }
    crate::platform::find_program(command).or_else(|| {
        ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"]
            .iter()
            .find_map(|d| crate::platform::find_program(&format!("{d}/{command}")))
    })
}

fn missing(p: &Program, code: &str, sourcepos: &str) -> String {
    format!(
        "<figure class=\"mdv-diagram mdv-diagram-missing\"{sourcepos}><figcaption>Install {} \
         (<code>{}</code>) to draw this diagram.</figcaption><pre lang=\"{}\"><code>{}</code></pre></figure>\n",
        p.name,
        p.command,
        p.lang,
        escape_text(code)
    )
}

fn draw_with(p: &Program, path: &std::path::Path, code: &str) -> Result<String, String> {
    // A scratch folder as the working directory: nothing next to the
    // document is read or written.
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut cmd = Command::new(path);
    cmd.args(p.args).current_dir(dir.path());
    let mut input = code.to_string();
    if p.command == "plantuml" {
        // Never read other files or the web while drawing.
        cmd.env("PLANTUML_SECURITY_PROFILE", "SANDBOX");
        if !input.trim_start().starts_with("@start") {
            input = format!("@startuml\n{input}\n@enduml\n");
        }
    }
    let out_file = match p.file_ext {
        Some(ext) => {
            let src = dir.path().join(format!("in.{ext}"));
            std::fs::write(&src, &input).map_err(|e| e.to_string())?;
            let out = dir.path().join("out.svg");
            cmd.arg(&src).arg(&out);
            input.clear();
            Some(out)
        }
        None => None,
    };
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("couldn't run {}: {e}", p.command))?;
    let mut stdin = child.stdin.take();
    let writer = std::thread::spawn(move || {
        if let Some(s) = stdin.as_mut() {
            let _ = s.write_all(input.as_bytes());
        }
    });
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = stdout.read_to_end(&mut v);
        v
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + p.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{} took longer than {} seconds",
                    p.command,
                    p.timeout.as_secs()
                ));
            }
        }
    };
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    let err = err_reader.join().unwrap_or_default();
    let svg = match &out_file {
        Some(f) => std::fs::read(f).unwrap_or_default(),
        None => out,
    };
    let svg = String::from_utf8_lossy(&svg).into_owned();
    if !status.success() || !svg.contains("<svg") {
        let why = err.trim();
        let why = if why.is_empty() {
            format!("{} failed ({status})", p.command)
        } else {
            why.lines().take(5).collect::<Vec<_>>().join(" ")
        };
        return Err(why);
    }
    Ok(svg)
}

/// A program's SVG made safe and fit to put inline in the page: from the
/// `<svg` on, without scripts, event handlers or embedded HTML (the
/// page's policy blocks scripts anyway), and with Graphviz's black
/// following the theme.
fn clean_svg(p: &Program, svg: &str) -> String {
    let start = svg.find("<svg").unwrap_or(0);
    let end = svg.rfind("</svg>").map_or(svg.len(), |e| e + 6);
    let mut s = strip_elements(&svg[start..end], "script");
    s = strip_elements(&s, "foreignObject");
    s = strip_comments(&s);
    s = strip_event_handlers(&s);
    if p.command == "dot" {
        for (from, to) in [
            ("stroke=\"black\"", "stroke=\"currentColor\""),
            ("fill=\"black\"", "fill=\"currentColor\""),
            ("stroke=\"#000000\"", "stroke=\"currentColor\""),
            ("fill=\"#000000\"", "fill=\"currentColor\""),
        ] {
            s = s.replace(from, to);
        }
    }
    s
}

fn strip_elements(s: &str, name: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    let open = format!("<{name}");
    let close = format!("</{name}>");
    while let Some(i) = find_ci(rest, &open) {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        rest = match find_ci(after, &close) {
            Some(j) => &after[j + close.len()..],
            None => match after.find("/>") {
                Some(j) => &after[j + 2..],
                None => "",
            },
        };
    }
    out.push_str(rest);
    out
}

fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    hay.to_ascii_lowercase().find(&needle.to_ascii_lowercase())
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("<!--") {
        out.push_str(&rest[..i]);
        rest = rest[i..].find("-->").map_or("", |j| &rest[i + j + 3..]);
    }
    out.push_str(rest);
    out
}

/// Removes ` on…="…"` attributes inside tags.
fn strip_event_handlers(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut quote: Option<char> = None;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match (in_tag, quote, c) {
            (_, Some(q), _) => {
                out.push(c);
                if c == q {
                    quote = None;
                }
            }
            (false, None, '<') => {
                in_tag = true;
                out.push(c);
            }
            (true, None, '>') => {
                in_tag = false;
                out.push(c);
            }
            (true, None, '"' | '\'') => {
                quote = Some(c);
                out.push(c);
            }
            (true, None, w) if w.is_whitespace() => {
                let rest = &s[i + 1..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
                    .collect();
                if name.len() > 2 && name[..2].eq_ignore_ascii_case("on") {
                    // Skip the name, `=` and the value.
                    let mut j = i + 1 + name.len();
                    let bytes = s.as_bytes();
                    while j < s.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < s.len() && bytes[j] == b'=' {
                        j += 1;
                        while j < s.len() && bytes[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        if j < s.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                            let q = bytes[j];
                            j += 1;
                            while j < s.len() && bytes[j] != q {
                                j += 1;
                            }
                            j += 1;
                        } else {
                            while j < s.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>'
                            {
                                j += 1;
                            }
                        }
                    }
                    while chars.peek().is_some_and(|(k, _)| *k < j) {
                        chars.next();
                    }
                    out.push(' ');
                } else {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bob_and_pikchr_follow_the_text_colour() {
        let html = bob("+--+\n|ab|-->\n+--+\n", " data-sourcepos=\"1:1-5:3\"");
        assert!(
            html.starts_with(
                "<figure class=\"mdv-diagram mdv-bob\" data-sourcepos=\"1:1-5:3\"><svg"
            )
        );
        assert!(html.contains("stroke: currentColor"), "{html}");
        assert!(!html.contains("class=\"backdrop\""));
        assert_eq!(
            html.trim_end().matches('\n').count() > 0,
            html.contains("<style>")
        );

        let html = pikchr("box \"Hi\"; arrow; circle \"x\"", "");
        assert!(html.contains("<svg") && html.contains("currentColor"));
        assert!(!html.contains("rgb(0,0,0)"));
        let bad = pikchr("box \"unterminated", "");
        assert!(bad.contains("Can't draw this Pikchr"), "{bad}");
    }

    #[test]
    fn qr_codes() {
        let html = qr("https://example.com\n", "");
        assert!(html.starts_with("<figure class=\"mdv-diagram mdv-qr\"><svg role=\"img\" aria-label=\"QR code: https://example.com\""), "{html}");
        assert!(!html.contains("<?xml"));
    }

    #[test]
    fn program_svg_is_cleaned() {
        let svg = "<?xml version=\"1.0\"?><!DOCTYPE svg><svg onload=\"alert(1)\" width=\"10\">\
                   <!-- c --><script>alert(2)</script><g ONCLICK='x' id=\"a\"><text fill=\"black\">a > b</text>\
                   <foreignObject><div>x</div></foreignObject></g></svg>";
        let out = clean_svg(&GRAPHVIZ, svg);
        assert_eq!(
            out,
            "<svg  width=\"10\"><g  id=\"a\"><text fill=\"currentColor\">a > b</text></g></svg>"
        );
    }
}
