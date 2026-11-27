//! Code blocks drawn in the page by a bundled JavaScript library, like
//! Mermaid: ```` ```vega-lite ```` (and ```` ```vega ````) charts, ```` ```abc ````
//! sheet music (abcjs) and ```` ```smiles ```` chemical structures
//! (SmilesDrawer). render.rs writes `<pre class="mdv-lib mdv-lib-ID">` with
//! the source; `assets/libraries-init.js` loads the library on pages that
//! have one (inlined in standalone pages, from `/__mdv/lib/ID.js` when
//! served, from `_mdv/ID.js` in static sites) and draws it.
//!
//! The libraries are gzip-compressed in `assets/libs/` (Vega's bundle is
//! vega, vega-lite and vega-interpreter, which evaluates expressions
//! without `eval`, so it works under the page's CSP).

use std::io::Read as _;
use std::sync::OnceLock;

pub struct Library {
    /// File and key name (`vega` → `/__mdv/lib/vega.js`).
    pub id: &'static str,
    pub name: &'static str,
    /// Code block languages.
    pub langs: &'static [&'static str],
    /// Class on the block, also how pages are checked for it.
    pub class: &'static str,
    pub gz: &'static [u8],
    pub version: &'static str,
    pub license: &'static str,
    pub url: &'static str,
    pub note: &'static str,
}

pub const LIBRARIES: &[Library] = &[
    Library {
        id: "vega",
        name: "Vega-Lite",
        langs: &["vega-lite", "vegalite", "vega"],
        class: "mdv-lib-vega",
        gz: include_bytes!("../assets/libs/vega.js.gz"),
        version: "6.5.0 (Vega 6.4.0)",
        license: "BSD-3-Clause",
        url: "https://vega.github.io/vega-lite/",
        note: "charts",
    },
    Library {
        id: "abcjs",
        name: "abcjs",
        langs: &["abc"],
        class: "mdv-lib-abc",
        gz: include_bytes!("../assets/libs/abcjs.js.gz"),
        version: "6.7.3",
        license: "MIT",
        url: "https://www.abcjs.net/",
        note: "sheet music",
    },
    Library {
        id: "smiles-drawer",
        name: "SmilesDrawer",
        langs: &["smiles"],
        class: "mdv-lib-smiles",
        gz: include_bytes!("../assets/libs/smiles-drawer.js.gz"),
        version: "2.4.2",
        license: "MIT",
        url: "https://github.com/reymond-group/smilesDrawer",
        note: "chemical structures",
    },
];

/// Where the server exposes the libraries (`/__mdv/lib/{id}.js`).
pub const PATH_PREFIX: &str = "/__mdv/lib/";

pub fn by_id(id: &str) -> Option<&'static Library> {
    LIBRARIES.iter().find(|l| l.id == id)
}

/// A library's script, decompressed (for inlining and static sites).
pub fn js(lib: &Library) -> &'static str {
    static JS: OnceLock<Vec<String>> = OnceLock::new();
    let all = JS.get_or_init(|| {
        LIBRARIES
            .iter()
            .map(|l| {
                let mut js = String::new();
                flate2::read::GzDecoder::new(l.gz)
                    .read_to_string(&mut js)
                    .expect("bundled library is valid gzip");
                js
            })
            .collect()
    });
    let i = LIBRARIES
        .iter()
        .position(|l| l.id == lib.id)
        .expect("a bundled library");
    &all[i]
}

/// The libraries a rendered body uses.
pub fn used(body: &str) -> Vec<&'static Library> {
    if !body.contains("mdv-lib ") {
        return Vec::new();
    }
    LIBRARIES
        .iter()
        .filter(|l| body.contains(&format!("class=\"mdv-lib {}\"", l.class)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inlined in a `<script>`, a library must not contain `</script` or
    /// `<!--` (which would end or confuse the element early).
    #[test]
    fn bundles_are_safe_to_inline() {
        for lib in LIBRARIES {
            let js = js(lib).to_ascii_lowercase();
            assert!(js.len() > 10_000, "{}", lib.id);
            assert!(!js.contains("</script"), "{}", lib.id);
            assert!(!js.contains("<!--"), "{}", lib.id);
        }
        assert!(js(by_id("vega").unwrap()).contains("window.vegaInterpreter"));
    }
}
