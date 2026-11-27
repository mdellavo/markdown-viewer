//! What this build is: for `--version` and the About section of the
//! viewer's help (press `?`). Build details come from build.rs.

use std::sync::OnceLock;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPOSITORY: &str = "https://github.com/mdellavo/markdown-viewer";
const COMMIT: &str = env!("MDV_GIT_COMMIT");
const COMMIT_DATE: &str = env!("MDV_GIT_DATE");
const TARGET: &str = env!("MDV_TARGET");
const PROFILE: &str = env!("MDV_PROFILE");
const RUSTC: &str = env!("MDV_RUSTC");
/// The crates the program is built with (generated from `cargo metadata`
/// by the `generated_files` test).
const CREDITS: &str = include_str!("../assets/credits.json");

/// `markdown-viewer --version` (`-V` stays short).
pub fn long_version() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let mut lines = vec![VERSION.to_string()];
        if !COMMIT.is_empty() {
            let date = if COMMIT_DATE.is_empty() {
                String::new()
            } else {
                format!(" ({COMMIT_DATE})")
            };
            lines.push(format!("commit {}{date}", short(COMMIT)));
        }
        lines.push(format!("built for {TARGET} ({PROFILE}) with {RUSTC}"));
        lines.push(format!(
            "Mermaid {}, KaTeX {}",
            crate::render::MERMAID_VERSION.trim(),
            crate::render::KATEX_VERSION.trim()
        ));
        lines.push(REPOSITORY.to_string());
        lines.join("\n")
    })
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(10)]
}

/// The build, as JSON for pages: nothing about this computer, so it's safe
/// in exported pages and static sites.
pub fn build_json() -> &'static str {
    static JSON: OnceLock<String> = OnceLock::new();
    JSON.get_or_init(|| {
        serde_json::json!({
            "version": VERSION,
            "commit": COMMIT,
            "commitDate": COMMIT_DATE,
            "target": TARGET,
            "profile": PROFILE,
            "rustc": RUSTC,
            "mermaid": crate::render::MERMAID_VERSION.trim(),
            "katex": crate::render::KATEX_VERSION.trim(),
            "repository": REPOSITORY,
            "license": env!("CARGO_PKG_LICENSE"),
            "bundled": bundled(),
            "credits": serde_json::from_str::<serde_json::Value>(CREDITS)
                .unwrap_or(serde_json::Value::Null),
        })
        .to_string()
    })
}

/// What pages include: (name, version, license, link). Their licenses are
/// in `assets/LICENSE-*`.
fn bundled() -> serde_json::Value {
    let mut list = serde_json::json!([
        {
            "name": "github-markdown-css",
            "note": "GitHub's Markdown styles",
            "license": "MIT",
            "url": "https://github.com/sindresorhus/github-markdown-css",
        },
        {
            "name": "Mermaid",
            "version": crate::render::MERMAID_VERSION.trim(),
            "note": "diagrams",
            "license": "MIT",
            "url": "https://mermaid.js.org/",
        },
        {
            "name": "KaTeX",
            "version": crate::render::KATEX_VERSION.trim(),
            "note": "math",
            "license": "MIT",
            "url": "https://katex.org/",
        },
        {
            "name": "Primer Octicons",
            "note": "icons",
            "license": "MIT",
            "url": "https://github.com/primer/octicons",
        },
    ]);
    // Vega-Lite, abcjs, SmilesDrawer, before the icons.
    if let Some(items) = list.as_array_mut() {
        for (i, lib) in crate::libraries::LIBRARIES.iter().enumerate() {
            items.insert(
                3 + i,
                serde_json::json!({
                    "name": lib.name,
                    "version": lib.version,
                    "note": lib.note,
                    "license": lib.license,
                    "url": lib.url,
                }),
            );
        }
    }
    list
}

/// About this viewer, for served pages only (`/__mdv/about`): where its
/// files are and how it's set up.
#[derive(serde::Serialize)]
pub struct Local {
    /// The settings file, and whether it exists.
    pub settings: Option<(String, bool)>,
    /// Remembered preferences, reading positions and recent files.
    pub state: Option<String>,
    /// The folder being served.
    pub root: String,
    /// The editor click-to-source would run (program name).
    pub editor: Option<String>,
    pub obsidian: bool,
    /// The repository GitHub references link to.
    pub github: Option<String>,
    pub custom_css: Option<String>,
    pub os: &'static str,
    pub arch: &'static str,
}

impl Local {
    pub fn gather(
        root: &std::path::Path,
        editor: Option<&str>,
        render: &crate::render::RenderOptions,
        custom_css: Option<&std::path::Path>,
    ) -> Self {
        let editor = crate::editor::command(std::path::Path::new("FILE"), 1, editor)
            .ok()
            .map(|(program, _)| program);
        Local {
            settings: crate::config::config_path().map(|p| {
                let exists = p.is_file();
                (p.display().to_string(), exists)
            }),
            state: crate::config::state_path().map(|p| p.display().to_string()),
            root: root.display().to_string(),
            editor,
            obsidian: render.obsidian,
            github: render.github.clone(),
            custom_css: custom_css.map(|p| p.display().to_string()),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_texts() {
        assert!(long_version().starts_with(VERSION));
        assert!(long_version().contains("built for "));
        let json: serde_json::Value = serde_json::from_str(build_json()).unwrap();
        assert_eq!(json["version"], VERSION);
        assert_eq!(json["license"], "MIT");
        assert!(
            json["credits"]["crates"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["name"] == "comrak")
        );
        assert_eq!(json["bundled"].as_array().unwrap().len(), 7);
        assert!(!build_json().contains(&std::env::temp_dir().display().to_string()));
    }
}
