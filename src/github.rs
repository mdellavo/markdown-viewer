//! Small post-processing steps that make comrak's HTML match what GitHub
//! renders: alert icons, the link icon shown when hovering a heading, and
//! colour chips after colours in inline code (`` `#0969da` ``).

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

/// `(class marking an alert of this kind, its icon's SVG)` for each type.
fn alert_icons() -> &'static [(String, String)] {
    static R: OnceLock<Vec<(String, String)>> = OnceLock::new();
    R.get_or_init(|| {
        ALERT_ICONS
            .iter()
            .map(|(kind, icon, svg)| {
                let path = svg_path(svg);
                (
                    format!("\"markdown-alert markdown-alert-{kind}\""),
                    format!(
                        "<svg class=\"octicon octicon-{icon}\" viewBox=\"0 0 16 16\" width=\"16\" \
                         height=\"16\" aria-hidden=\"true\"><path d=\"{path}\"></path></svg>"
                    ),
                )
            })
            .collect()
    })
}

/// Put each alert's icon at the start of its title. The alert's tags may
/// carry extra attributes (e.g. source positions), so find the title
/// paragraph after each alert rather than matching exact text.
fn add_alert_icons(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 4096);
    let mut rest = html;
    loop {
        let next = alert_icons()
            .iter()
            .filter_map(|(marker, svg)| rest.find(marker.as_str()).map(|at| (at, svg)))
            .min_by_key(|(at, _)| *at);
        let Some((at, svg)) = next else { break };
        let Some(title) = rest[at..].find("class=\"markdown-alert-title\"") else {
            break;
        };
        let Some(close) = rest[at + title..].find('>') else {
            break;
        };
        let insert_at = at + title + close + 1;
        out.push_str(&rest[..insert_at]);
        out.push_str(svg);
        rest = &rest[insert_at..];
    }
    out.push_str(rest);
    out
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
        html = add_alert_icons(&html);
    }
    // GitHub's CSS draws the hover link icon on an `.octicon-link` inside
    // the heading's anchor; comrak's anchor is empty.
    if html.contains("class=\"anchor\"></a>") {
        html = html.replace(
            "class=\"anchor\"></a>",
            "class=\"anchor\"><span class=\"octicon octicon-link\" aria-hidden=\"true\"></span></a>",
        );
    }
    if html.contains("<code") {
        html = color_chips(&html);
    }
    html
}

/// GitHub shows a swatch after a colour written alone in inline code:
/// `#rgb`, `#rrggbb` (and with alpha), `rgb(…)`/`rgba(…)` and
/// `hsl(…)`/`hsla(…)`.
fn color_chips(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    let mut in_pre = false;
    while let Some(i) = rest.find("<code") {
        let after = &rest[i..];
        // Inline code: `<code>` or `<code data-sourcepos="…">` (code blocks
        // have a language class or sit in <pre>, and are left alone).
        let Some(open_end) = after.find('>') else {
            break;
        };
        let attrs = &after[5..open_end];
        let inline = attrs.is_empty()
            || (attrs.starts_with(" data-sourcepos=\"") && !attrs[17..].contains(' '));
        let body_start = open_end + 1;
        // Inside a <pre> since the last step?
        let before = &rest[..i];
        if let Some(p) = before.rfind("<pre") {
            in_pre = before[p..].find("</pre>").is_none();
        } else if before.contains("</pre>") {
            in_pre = false;
        }
        out.push_str(&rest[..i + body_start]);
        rest = &after[body_start..];
        if !inline || in_pre {
            continue;
        }
        if let Some(end) = rest.find("</code>")
            && is_color(&rest[..end])
        {
            let color = &rest[..end];
            out.push_str(color);
            out.push_str(&format!(
                "<span class=\"mdv-color\" style=\"background-color: {color}\" aria-hidden=\"true\"></span>"
            ));
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

fn is_color(s: &str) -> bool {
    if let Some(hex) = s.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    let Some((name, args)) = s.split_once('(') else {
        return false;
    };
    let Some(args) = args.strip_suffix(')') else {
        return false;
    };
    let parts: Vec<&str> = args
        .split([',', ' ', '/'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let number = |p: &str, units: &[&str]| {
        let n = units.iter().find_map(|u| p.strip_suffix(u)).unwrap_or(p);
        !n.is_empty() && n.parse::<f64>().is_ok_and(f64::is_finite)
    };
    matches!(name, "rgb" | "rgba" | "hsl" | "hsla")
        && matches!(parts.len(), 3 | 4)
        && parts.iter().all(|p| number(p, &["%", "deg"]))
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
    fn alert_icons_survive_extra_attributes() {
        let html = "<div class=\"markdown-alert markdown-alert-tip\" data-sourcepos=\"1:1-2:3\">\n\
                    <p class=\"markdown-alert-title\">Tip</p>\n</div>\n\
                    <div class=\"markdown-alert markdown-alert-note\">\n<p class=\"markdown-alert-title\">Note</p>\n</div>";
        let out = finish(html.into());
        assert!(
            out.contains("markdown-alert-title\"><svg class=\"octicon octicon-light-bulb\""),
            "{out}"
        );
        assert!(
            out.contains("markdown-alert-title\"><svg class=\"octicon octicon-info\""),
            "{out}"
        );
        assert_eq!(out.matches("<svg").count(), 2);
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

    #[test]
    fn colours_in_inline_code_get_a_chip() {
        let html = "<p><code>#0969da</code> <code data-sourcepos=\"1:1-1:10\">rgb(9, 105, 218)</code> \
                    <code>hsl(212 92% 45% / 0.5)</code> <code>#xyz</code> <code>red</code> \
                    <code>rgb(1,2,3);x</code></p><pre lang=\"css\"><code>#fff</code></pre>";
        let out = finish(html.into());
        assert_eq!(out.matches("class=\"mdv-color\"").count(), 3, "{out}");
        assert!(out.contains(
            "<code>#0969da<span class=\"mdv-color\" style=\"background-color: #0969da\""
        ));
        assert!(out.contains("<code>#fff</code></pre>"));
    }
}
