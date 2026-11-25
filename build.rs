//! Build script: embeds KaTeX's fonts into its stylesheet as `data:` URIs,
//! so one self-contained CSS file works both inlined into standalone pages
//! and served in watch mode. Only the woff2 fonts are shipped; the woff and
//! ttf fallbacks are dropped (every current browser supports woff2).

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/katex");

    let css = fs::read_to_string("assets/katex/katex.min.css").expect("read katex.min.css");
    let out = inline_fonts(&css, Path::new("assets/katex"));
    let dest = Path::new(&env::var("OUT_DIR").unwrap()).join("katex.css");
    fs::write(dest, out).expect("write katex.css");
}

/// Replace each `src:url(fonts/X.woff2) format("woff2"),url(...)...` font
/// source list with a single woff2 data URI.
fn inline_fonts(css: &str, dir: &Path) -> String {
    let mut out = String::with_capacity(css.len() * 2);
    let mut rest = css;
    while let Some(start) = rest.find("src:url(") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        // The font source list ends at the end of the declaration.
        let end = after.find(['}', ';']).expect("unterminated @font-face src");
        let list = &after[..end];
        let woff2 = list
            .split("url(")
            .skip(1)
            .filter_map(|s| s.split(')').next())
            .find(|u| u.ends_with(".woff2"))
            .unwrap_or_else(|| panic!("no woff2 font in {list}"));
        let bytes = fs::read(dir.join(woff2)).unwrap_or_else(|e| panic!("read {woff2}: {e}"));
        out.push_str("src:url(data:font/woff2;base64,");
        out.push_str(&base64(&bytes));
        out.push_str(") format(\"woff2\")");
        rest = &after[end..];
    }
    out.push_str(rest);
    assert!(
        !out.contains("url(fonts/"),
        "a KaTeX font reference was not inlined"
    );
    out
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
