//! Small post-processing steps that make comrak's HTML match what GitHub
//! renders: alert icons, and the link icon shown when hovering a heading.

use std::sync::OnceLock;

/// GitHub's alert icons (Primer Octicons, MIT; see assets/LICENSE-octicons).
const ALERT_ICONS: [(&str, &str, &str); 5] = [
    (
        "note",
        "info",
        include_str!("../assets/octicons/info-16.svg"),
    ),
    (
        "tip",
        "light-bulb",
        include_str!("../assets/octicons/light-bulb-16.svg"),
    ),
    (
        "important",
        "report",
        include_str!("../assets/octicons/report-16.svg"),
    ),
    (
        "warning",
        "alert",
        include_str!("../assets/octicons/alert-16.svg"),
    ),
    (
        "caution",
        "stop",
        include_str!("../assets/octicons/stop-16.svg"),
    ),
];

/// `(opening tags to find, replacement with the icon)` for each alert type.
fn alert_replacements() -> &'static [(String, String)] {
    static R: OnceLock<Vec<(String, String)>> = OnceLock::new();
    R.get_or_init(|| {
        ALERT_ICONS
            .iter()
            .map(|(kind, icon, svg)| {
                let path = svg_path(svg);
                let find = format!(
                    "<div class=\"markdown-alert markdown-alert-{kind}\">\n<p class=\"markdown-alert-title\">"
                );
                let replace = format!(
                    "<div class=\"markdown-alert markdown-alert-{kind}\">\n<p class=\"markdown-alert-title\">\
                     <svg class=\"octicon octicon-{icon}\" viewBox=\"0 0 16 16\" width=\"16\" height=\"16\" \
                     aria-hidden=\"true\"><path d=\"{path}\"></path></svg>"
                );
                (find, replace)
            })
            .collect()
    })
}

/// The `d` attribute of a single-path octicon SVG.
fn svg_path(svg: &str) -> &str {
    let start = svg.find(" d=\"").expect("octicon has a path") + 4;
    let end = start + svg[start..].find('"').expect("closed d attribute");
    &svg[start..end]
}

/// Apply GitHub's finishing touches to rendered HTML.
pub fn finish(html: String) -> String {
    let mut html = html;
    if html.contains("markdown-alert-title") {
        for (find, replace) in alert_replacements() {
            if html.contains(find.as_str()) {
                html = html.replace(find.as_str(), replace);
            }
        }
    }
    // GitHub's CSS draws the hover link icon on an `.octicon-link` inside
    // the heading's anchor; comrak's anchor is empty.
    if html.contains("class=\"anchor\"></a>") {
        html = html.replace(
            "class=\"anchor\"></a>",
            "class=\"anchor\"><span class=\"octicon octicon-link\" aria-hidden=\"true\"></span></a>",
        );
    }
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_alert_gets_its_icon() {
        for (kind, icon, _) in ALERT_ICONS {
            let html = format!(
                "<div class=\"markdown-alert markdown-alert-{kind}\">\n<p class=\"markdown-alert-title\">X</p>\n</div>\n"
            );
            let out = finish(html);
            assert!(out.contains(&format!("octicon-{icon}")), "{kind}: {out}");
            assert!(out.contains("<path d=\"M"), "{kind}: {out}");
        }
    }

    #[test]
    fn heading_anchor_gets_link_icon() {
        let out = finish("<h2><a href=\"#x\" class=\"anchor\"></a>X</h2>".into());
        assert!(out.contains("<span class=\"octicon octicon-link\""));
    }

    #[test]
    fn icons_have_no_markup_beyond_a_path() {
        for (_, _, svg) in ALERT_ICONS {
            assert!(!svg_path(svg).contains(['<', '>', '"']));
        }
    }
}
