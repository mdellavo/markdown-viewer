//! Page CSS: GitHub's stylesheet and syntax colours for every theme at
//! once, switched by `<html data-mdv-theme="auto|light|dark">`, so the
//! viewer can change theme instantly without re-rendering. Also the
//! built-in styles (`--style`).

use std::sync::OnceLock;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use syntect::highlighting::ThemeSet;
use syntect::html::{ClassStyle, css_for_theme_with_class_style};

/// GitHub's stylesheet (sindresorhus/github-markdown-css, auto variant):
/// shared rules plus one block of colour variables per scheme.
const GITHUB_CSS: &str = include_str!("../assets/github-markdown.css");

pub const HL_PREFIX: &str = "hl-";
const HL_LIGHT_THEME: &str = "InspiredGitHub";
const HL_DARK_THEME: &str = "base16-ocean.dark";

/// Page colour scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the operating system's light/dark setting.
    #[default]
    Auto,
    Light,
    Dark,
}

impl Theme {
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Auto => "auto",
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
}

/// Built-in page styles, layered on top of GitHub's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// GitHub's README look.
    #[default]
    Github,
    /// GitHub's look, using the full window width.
    Wide,
    /// Serif type and a narrower column, for long-form reading.
    Book,
    /// Smaller type and tighter spacing, for dense documents.
    Compact,
}

impl Style {
    pub fn css(self) -> &'static str {
        match self {
            Style::Github => "",
            Style::Wide => {
                ".markdown-body { max-width: none; padding-left: 60px; padding-right: 60px; }\n"
            }
            Style::Book => {
                ".markdown-body { max-width: 760px; font-size: 18px; line-height: 1.7; \
                 font-family: Charter, \"Bitstream Charter\", \"Iowan Old Style\", \
                 \"Palatino Linotype\", Palatino, Georgia, serif; }\n\
                 .markdown-body h1, .markdown-body h2 { border-bottom: none; padding-bottom: 0; }\n\
                 .markdown-body h1, .markdown-body h2, .markdown-body h3 { font-weight: 600; }\n\
                 .markdown-body p { hyphens: auto; }\n"
            }
            Style::Compact => {
                ".markdown-body { max-width: 1100px; font-size: 14px; line-height: 1.45; \
                 padding: 24px 32px; }\n\
                 .markdown-body p, .markdown-body ul, .markdown-body ol, .markdown-body pre, \
                 .markdown-body table, .markdown-body blockquote { margin-bottom: 10px; }\n\
                 .markdown-body h1, .markdown-body h2, .markdown-body h3 { margin-top: 18px; \
                 margin-bottom: 10px; }\n"
            }
        }
    }
}

/// The page's base stylesheet for all themes. Built once.
pub fn base_css() -> &'static str {
    static CSS: OnceLock<String> = OnceLock::new();
    CSS.get_or_init(build_base_css)
}

fn build_base_css() -> String {
    let (shared, light_vars, dark_vars) = split_github_css(GITHUB_CSS);
    let (hl_light, hl_dark) = highlight_css();

    let mut css = String::with_capacity(GITHUB_CSS.len() + hl_light.len() * 3);
    css.push_str(&shared);
    // The colour variables go on the root too, so the viewer's own UI
    // (toolbar, sidebar) can use them, and on .markdown-body to beat the
    // defaults GitHub's shared rules set there.
    let light = format!("html, .markdown-body {{\n{light_vars}\n}}\n{hl_light}{LIGHT_EXTRAS}");
    let dark = format!("html, .markdown-body {{\n{dark_vars}\n}}\n{hl_dark}{DARK_EXTRAS}");
    css.push_str(&themed(&light, &dark));
    css.push_str(LAYOUT);
    css.push_str(CALLOUTS);
    css.push_str(CODE_AND_TOC);
    css.push_str(&numbering_css());
    // Print (and PDF) always in light colours, whatever the screen theme.
    css.push_str("@media print {\n");
    css.push_str(&scope_rules(&light, ":root[data-mdv-theme]"));
    css.push_str(PRINT);
    css.push_str("}\n");
    css
}

/// Rules that differ per scheme besides colour variables: the page
/// background, and GitHub's `#gh-dark-mode-only` / `#gh-light-mode-only`
/// image fragments.
const LIGHT_EXTRAS: &str = "html { background: #ffffff; }\n\
    .markdown-body img[src$=\"#gh-dark-mode-only\"] { display: none; }\n";
const DARK_EXTRAS: &str = "html { background: #0d1117; }\n\
    .markdown-body img[src$=\"#gh-light-mode-only\"] { display: none; }\n";

const LAYOUT: &str = "\nbody { margin: 0; }\n\
    .markdown-body { box-sizing: border-box; min-width: 200px; max-width: 980px; \
    margin: 0 auto; padding: 45px; }\n\
    @media (max-width: 767px) { .markdown-body { padding: 15px; } }\n\
    .markdown-body div.mermaid { margin-bottom: 16px; text-align: center; overflow-x: auto; }\n\
    .markdown-body pre.mermaid-error { border-left: 4px solid #d1242f; }\n\
    .markdown-body .math-display { overflow-x: auto; overflow-y: hidden; margin-bottom: 16px; }\n\
    .markdown-body .markdown-alert-title .octicon { margin-right: 8px; }\n";

/// Obsidian callouts, styled like GitHub alerts. Folded ones start closed.
const CALLOUTS: &str = "\
.markdown-body .callout { --mdv-callout: var(--borderColor-accent-emphasis); \
  padding: 8px 16px; margin-bottom: 16px; border-left: .25em solid var(--mdv-callout); }\n\
.markdown-body .callout-title { color: var(--mdv-callout); font-weight: 500; margin-bottom: 4px; }\n\
.markdown-body .callout[data-callout-fold=\"collapsed\"] .callout-content { display: none; }\n\
.markdown-body .callout:not([data-callout-fold=\"none\"]) .callout-title { cursor: pointer; }\n\
.markdown-body .callout:not([data-callout-fold=\"none\"]) .callout-title::after { content: \" ▾\"; }\n\
.markdown-body .callout[data-callout-fold=\"collapsed\"] .callout-title::after { content: \" ▸\"; }\n\
.markdown-body .callout-content > :last-child { margin-bottom: 0; }\n\
.markdown-body .callout:is([data-callout=\"tip\"], [data-callout=\"hint\"], [data-callout=\"success\"], \
  [data-callout=\"check\"], [data-callout=\"done\"]) { --mdv-callout: var(--borderColor-success-emphasis); }\n\
.markdown-body .callout:is([data-callout=\"important\"], [data-callout=\"example\"], \
  [data-callout=\"abstract\"], [data-callout=\"summary\"], [data-callout=\"tldr\"]) \
  { --mdv-callout: var(--borderColor-done-emphasis); }\n\
.markdown-body .callout:is([data-callout=\"warning\"], [data-callout=\"caution\"], \
  [data-callout=\"attention\"], [data-callout=\"question\"], [data-callout=\"help\"], \
  [data-callout=\"faq\"]) { --mdv-callout: var(--borderColor-attention-emphasis); }\n\
.markdown-body .callout:is([data-callout=\"failure\"], [data-callout=\"fail\"], [data-callout=\"missing\"], \
  [data-callout=\"danger\"], [data-callout=\"error\"], [data-callout=\"bug\"]) \
  { --mdv-callout: var(--borderColor-danger-emphasis); }\n\
.markdown-body .callout:is([data-callout=\"quote\"], [data-callout=\"cite\"]) \
  { --mdv-callout: var(--borderColor-default); }\n\
.markdown-body .mdv-wikilink-missing { opacity: .65; text-decoration: underline dashed; }\n\
.markdown-body .mdv-front-matter { margin-bottom: 16px; overflow-x: auto; }\n\
.markdown-body .mdv-front-matter table { margin-bottom: 0; }\n\
.markdown-body .mdv-front-matter-error pre { border-left: 4px solid #d1242f; }\n";

/// Code block extras (codeblocks.rs) and the `[TOC]` contents (toc.rs).
const CODE_AND_TOC: &str = "\
.markdown-body .mdv-code-figure { margin: 0 0 16px; }\n\
.markdown-body .mdv-code-title { font: 12px var(--fontStack-monospace, monospace); \
  color: var(--fgColor-muted); padding: 6px 16px; background: var(--bgColor-muted); \
  border: 1px solid var(--borderColor-muted); border-bottom: none; border-radius: 6px 6px 0 0; }\n\
.markdown-body .mdv-code-figure pre { border-top-left-radius: 0; border-top-right-radius: 0; margin-bottom: 0; }\n\
.markdown-body pre.mdv-lines .mdv-line { display: block; min-height: 1.45em; }\n\
.markdown-body pre.mdv-lines { counter-reset: mdv-line; }\n\
.markdown-body pre.mdv-numbered .mdv-line::before { counter-increment: mdv-line; content: counter(mdv-line); \
  display: inline-block; width: 2.5em; margin-right: 1em; text-align: right; color: var(--fgColor-muted); \
  opacity: .7; user-select: none; }\n\
.markdown-body pre.mdv-lines .mdv-hl { background: var(--bgColor-attention-muted); \
  margin: 0 -16px; padding: 0 16px; }\n\
.markdown-body pre.mdv-wrapped, .markdown-body pre.mdv-wrapped code { white-space: pre-wrap; word-break: break-word; }\n\
.markdown-body .mdv-toc { margin-bottom: 16px; padding: 8px 16px; border-left: 3px solid var(--borderColor-default); }\n\
.markdown-body .mdv-toc ul { margin: 0; padding-left: 1.4em; }\n\
.markdown-body .mdv-toc > ul { padding-left: 0; list-style: none; }\n\
.markdown-body .mdv-data-note { color: var(--fgColor-muted); font-size: 12px; }\n\
.markdown-body table.mdv-data td { white-space: pre-wrap; }\n";

/// CSS counters for `data-mdv-number="h1"` (number from h1) or `"h2"`
/// (a single h1 is the title). Four levels deep.
fn numbering_css() -> String {
    let mut css = String::new();
    for top in [1usize, 2] {
        let root = format!(":root[data-mdv-number=\"h{top}\"] .markdown-body");
        css.push_str(&format!("{root} {{ counter-reset: mdv-h{top}; }}\n"));
        let levels: Vec<usize> = (top..top + 4).filter(|l| *l <= 6).collect();
        for (i, level) in levels.iter().enumerate() {
            let next = levels.get(i + 1);
            let reset = next
                .map(|n| format!(" counter-reset: mdv-h{n};"))
                .unwrap_or_default();
            let content: Vec<String> = levels[..=i]
                .iter()
                .map(|l| format!("counter(mdv-h{l})"))
                .collect();
            css.push_str(&format!(
                "{root} h{level} {{{reset} }}\n\
                 {root} h{level}::before {{ counter-increment: mdv-h{level}; \
                 content: {} \". \"; }}\n",
                content.join(" \".\" ")
            ));
        }
    }
    css
}

/// Print layout: full width, no UI, and no page breaks inside blocks.
const PRINT: &str = "@page { margin: 16mm 14mm; }\n\
    .mdv-ui { display: none !important; }\n\
    body { padding: 0 !important; }\n\
    .markdown-body { max-width: none; padding: 0; font-size: 12pt; }\n\
    .markdown-body pre { white-space: pre-wrap; word-break: break-word; }\n\
    .markdown-body pre, .markdown-body table, .markdown-body blockquote, \
    .markdown-body .markdown-alert, .markdown-body div.mermaid, .markdown-body .math-display, \
    .markdown-body img, .markdown-body tr { break-inside: avoid; }\n\
    .markdown-body h1, .markdown-body h2, .markdown-body h3, .markdown-body h4 { break-after: avoid; }\n\
    .markdown-body .anchor { display: none; }\n";

/// Scope two rule sets to the page's theme attribute, with `auto`
/// following the OS.
fn themed(light: &str, dark: &str) -> String {
    let light_root = ":root[data-mdv-theme=\"light\"]";
    let dark_root = ":root[data-mdv-theme=\"dark\"]";
    let auto_root = ":root[data-mdv-theme=\"auto\"]";
    format!(
        "{}{}@media (prefers-color-scheme: light) {{\n{}}}\n@media (prefers-color-scheme: dark) {{\n{}}}\n",
        scope_rules(light, light_root),
        scope_rules(dark, dark_root),
        scope_rules(light, auto_root),
        scope_rules(dark, auto_root),
    )
}

/// Prefix every selector of every rule with `scope`. Handles the flat,
/// one-rule-per-block CSS we generate here (no nested at-rules). A selector
/// starting with `html` becomes `scope` itself, since `:root` is `html`.
fn scope_rules(css: &str, scope: &str) -> String {
    let mut out = String::with_capacity(css.len() * 2);
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let close = rest[open..]
            .find('}')
            .map(|i| open + i)
            .unwrap_or(rest.len() - 1);
        let (selectors, body) = (&rest[..open], &rest[open..=close]);
        let (comments, selectors) = split_leading_comments(selectors);
        out.push_str(comments);
        let scoped: Vec<String> = selectors
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| match s.strip_prefix("html") {
                Some(tail) => format!("{scope}{tail}"),
                None => format!("{scope} {s}"),
            })
            .collect();
        out.push_str(&scoped.join(",\n"));
        out.push(' ');
        out.push_str(body);
        out.push('\n');
        rest = &rest[close + 1..];
    }
    out
}

/// Separate `/* … */` comments preceding a selector from the selector.
fn split_leading_comments(s: &str) -> (&str, &str) {
    let mut end = 0;
    let mut rest = s;
    loop {
        let trimmed = rest.trim_start();
        if let Some(after) = trimmed.strip_prefix("/*") {
            match after.find("*/") {
                Some(i) => {
                    let consumed = rest.len() - trimmed.len() + 2 + i + 2;
                    end += consumed;
                    rest = &rest[consumed..];
                }
                None => break,
            }
        } else {
            break;
        }
    }
    (&s[..end], &s[end..])
}

/// Split GitHub's auto stylesheet into (shared rules, light variables,
/// dark variables). The variables live in two
/// `@media (prefers-color-scheme: …) { .markdown-body, [data-theme="…"] { … } }`
/// blocks; everything else is shared.
fn split_github_css(css: &str) -> (String, String, String) {
    let mut shared = css.to_string();
    let mut take = |scheme: &str| -> String {
        let start = shared
            .find(&format!("@media (prefers-color-scheme: {scheme}) {{"))
            .unwrap_or_else(|| panic!("github-markdown.css: no {scheme} block"));
        let inner_open = start + shared[start..].find('{').unwrap() + 1;
        let decl_open = inner_open + shared[inner_open..].find('{').unwrap() + 1;
        let decl_close = decl_open + shared[decl_open..].find('}').unwrap();
        let media_close = decl_close + 1 + shared[decl_close + 1..].find('}').unwrap();
        let vars = shared[decl_open..decl_close].trim().to_string();
        shared.replace_range(start..=media_close, "");
        vars
    };
    let dark = take("dark");
    let light = take("light");
    (shared, light, dark)
}

/// Syntax colours for (light, dark), without the theme's background so code
/// blocks keep GitHub's.
fn highlight_css() -> (String, String) {
    let themes = ThemeSet::load_defaults();
    let style = ClassStyle::SpacedPrefixed { prefix: HL_PREFIX };
    let css = |name: &str| {
        let raw = css_for_theme_with_class_style(&themes.themes[name], style).unwrap_or_default();
        strip_code_background(&raw)
    };
    (css(HL_LIGHT_THEME), css(HL_DARK_THEME))
}

/// Syntect themes paint a background on the outer `.code` scope; drop it.
fn strip_code_background(css: &str) -> String {
    let code_sel = format!(".{HL_PREFIX}code {{");
    let mut out = String::with_capacity(css.len());
    let mut in_code_rule = false;
    for line in css.lines() {
        if line.trim_start().starts_with(&code_sel) {
            in_code_rule = true;
        }
        if in_code_rule && line.trim_start().starts_with("background-color") {
            continue;
        }
        if line.contains('}') {
            in_code_rule = false;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_github_css_into_shared_and_variables() {
        let (shared, light, dark) = split_github_css(GITHUB_CSS);
        assert!(
            !shared.contains("prefers-color-scheme"),
            "both media blocks removed"
        );
        assert!(light.contains("--bgColor-default: #ffffff"), "{light}");
        assert!(dark.contains("--bgColor-default: #0d1117"), "{dark}");
        assert!(shared.contains(".markdown-body h1"), "shared rules kept");
    }

    #[test]
    fn every_theme_is_switchable_at_runtime() {
        let css = base_css();
        for root in [
            ":root[data-mdv-theme=\"light\"],\n:root[data-mdv-theme=\"light\"] .markdown-body",
            ":root[data-mdv-theme=\"dark\"] .markdown-body",
            ":root[data-mdv-theme=\"auto\"] .markdown-body",
            ":root[data-mdv-theme=\"dark\"] .hl-",
        ] {
            assert!(css.contains(root), "missing {root}");
        }
        assert!(css.contains(":root[data-mdv-theme=\"dark\"] { background: #0d1117; }"));
        assert!(css.contains("img[src$=\"#gh-dark-mode-only\"] { display: none; }"));
    }

    #[test]
    fn numbering_counts_four_levels() {
        let css = numbering_css();
        assert!(css.contains(":root[data-mdv-number=\"h2\"] .markdown-body h2::before"));
        assert!(
            css.contains("content: counter(mdv-h2) \".\" counter(mdv-h3) \". \""),
            "{css}"
        );
        assert!(
            !css.contains("h2\"] .markdown-body h6::before"),
            "only four levels"
        );
    }

    #[test]
    fn scope_rules_handles_lists_comments_and_html() {
        let out = scope_rules("/* c */\n.a, .b { x: 1; }\nhtml { y: 2; }\n", ":root[t]");
        assert!(out.contains("/* c */"), "{out}");
        assert!(out.contains(":root[t] .a,\n:root[t] .b { x: 1; }"), "{out}");
        assert!(out.contains(":root[t] { y: 2; }"), "{out}");
    }

    #[test]
    fn strips_only_code_background() {
        let css = ".hl-code {\n color: #000;\n background-color: #fff;\n}\n.hl-x {\n background-color: #f00;\n}\n";
        let out = strip_code_background(css);
        assert!(!out.contains("#fff"));
        assert!(out.contains("#f00"));
    }
}
