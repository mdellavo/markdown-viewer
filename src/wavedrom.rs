//! ```` ```wavedrom ```` timing diagrams, drawn as SVG here (no script).
//! Covers WaveDrom's signal lanes: `name`, `wave` (`0 1 h l H L p n P N
//! x z = 2-9 . | u d`), `data` labels, `phase`, `period`, groups (as
//! labelled lanes), spacer rows, `head`/`foot` text and `config.hscale`.
//! Edges and arrows (`node`, `edge`) are not drawn. The text may be JSON
//! or the relaxed form WaveDrom examples use (unquoted keys, single
//! quotes, trailing commas).

use std::fmt::Write;

use serde_json::Value;

use crate::diagrams::{figure, invalid};
use crate::render::escape_text;

/// One period, before `hscale`.
const PERIOD: f64 = 40.0;
const ROW: f64 = 30.0;
const AMP: f64 = 18.0;
/// Most periods drawn per lane.
const MAX_PERIODS: usize = 1000;

pub fn render(code: &str, sourcepos: &str) -> String {
    match parse(code).and_then(|v| draw(&v)) {
        Ok(svg) => figure("wavedrom", sourcepos, &svg),
        Err(e) => invalid("WaveDrom", "wavedrom", code, sourcepos, &e),
    }
}

fn parse(code: &str) -> Result<Value, String> {
    serde_json::from_str(code)
        .or_else(|_| serde_json::from_str(&relaxed_to_json(code)))
        .map_err(|e| format!("not valid JSON: {e}"))
}

/// WaveDrom's relaxed JSON (JavaScript object syntax) to JSON: quote bare
/// keys, turn single-quoted strings into double-quoted ones, drop trailing
/// commas and `//` comments.
fn relaxed_to_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 32);
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' | '\'' => {
                out.push('"');
                i += 1;
                while i < chars.len() && chars[i] != c {
                    match chars[i] {
                        '\\' if i + 1 < chars.len() => {
                            if chars[i + 1] == '\'' {
                                out.push('\'');
                            } else {
                                out.push('\\');
                                out.push(chars[i + 1]);
                            }
                            i += 2;
                            continue;
                        }
                        '"' => out.push_str("\\\""),
                        ch => out.push(ch),
                    }
                    i += 1;
                }
                out.push('"');
                i += 1;
            }
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            ',' => {
                let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
                if !matches!(next, Some(']') | Some('}')) {
                    out.push(',');
                }
                i += 1;
            }
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '$')
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let next = chars[i..].iter().find(|c| !c.is_whitespace());
                if next == Some(&':') {
                    let _ = write!(out, "\"{word}\"");
                } else {
                    out.push_str(&word);
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// A lane to draw: a signal, a spacer or a group label.
enum Lane<'a> {
    Signal(&'a Value, usize),
    Spacer,
    Group(String, usize),
}

fn lanes<'a>(items: &'a [Value], depth: usize, out: &mut Vec<Lane<'a>>) {
    for item in items {
        match item {
            Value::Array(a) => {
                let (label, rest) = match a.first() {
                    Some(Value::String(s)) => (s.clone(), &a[1..]),
                    _ => (String::new(), &a[..]),
                };
                if !label.is_empty() {
                    out.push(Lane::Group(label, depth));
                }
                lanes(rest, depth + 1, out);
            }
            Value::Object(o) if o.contains_key("wave") || o.contains_key("name") => {
                out.push(Lane::Signal(item, depth))
            }
            Value::Object(_) => out.push(Lane::Spacer),
            _ => {}
        }
    }
}

/// What the line does in one period.
#[derive(Clone, Copy, PartialEq, Debug)]
enum State {
    Low,
    High,
    /// Clock: rising (p) or falling (n) at the start of each period.
    Clock(bool),
    /// Bus value: its fill (0 for none), and whether a value starts here.
    Data(u8),
    Unknown,
    HighZ,
}

fn text_width(s: &str) -> f64 {
    s.chars().count() as f64 * 7.2
}

fn draw(v: &Value) -> Result<String, String> {
    let signal = v
        .get("signal")
        .and_then(Value::as_array)
        .ok_or("a WaveDrom diagram needs \"signal\": [ … ]")?;
    let hscale = v
        .pointer("/config/hscale")
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
        .clamp(0.25, 10.0);
    let period_w = PERIOD * hscale;
    let mut all = Vec::new();
    lanes(signal, 0, &mut all);
    if all.is_empty() {
        return Err("no signals to draw".into());
    }
    let label_w = all
        .iter()
        .map(|l| match l {
            Lane::Signal(s, d) => {
                text_width(s.get("name").and_then(Value::as_str).unwrap_or("")) + *d as f64 * 12.0
            }
            Lane::Group(g, d) => text_width(g) + *d as f64 * 12.0,
            Lane::Spacer => 0.0,
        })
        .fold(0.0, f64::max)
        + 16.0;
    let head = v.pointer("/head/text").and_then(Value::as_str);
    let foot = v.pointer("/foot/text").and_then(Value::as_str);
    let top = if head.is_some() { 28.0 } else { 6.0 };
    let mut body = String::new();
    let mut max_periods = 0usize;
    for (row, lane) in all.iter().enumerate() {
        let y = top + row as f64 * ROW;
        match lane {
            Lane::Spacer => {}
            Lane::Group(label, depth) => {
                let _ = write!(
                    body,
                    "<text class=\"mdv-wave-group\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
                    4.0 + *depth as f64 * 12.0,
                    y + ROW / 2.0 + 4.0,
                    escape_text(label)
                );
            }
            Lane::Signal(s, depth) => {
                let name = s.get("name").and_then(Value::as_str).unwrap_or("");
                let _ = write!(
                    body,
                    "<text class=\"mdv-wave-name\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
                    4.0 + *depth as f64 * 12.0,
                    y + ROW / 2.0 + 4.0,
                    escape_text(name)
                );
                let wave = s.get("wave").and_then(Value::as_str).unwrap_or("");
                let period = s
                    .get("period")
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0)
                    .clamp(0.1, 16.0);
                let phase = s.get("phase").and_then(Value::as_f64).unwrap_or(0.0);
                let data: Vec<String> = match s.get("data") {
                    Some(Value::Array(a)) => a
                        .iter()
                        .map(|d| match d {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect(),
                    Some(Value::String(s)) => s.split_whitespace().map(String::from).collect(),
                    _ => Vec::new(),
                };
                let n = lane_svg(
                    &mut body,
                    wave,
                    &data,
                    label_w - phase * period_w * period,
                    label_w,
                    y + (ROW - AMP) / 2.0,
                    period_w * period,
                );
                max_periods = max_periods.max((n as f64 * period - phase).ceil().max(0.0) as usize);
            }
        }
    }
    let width = label_w + max_periods as f64 * period_w + 8.0;
    let mut height = top + all.len() as f64 * ROW + 4.0;
    // Period gridlines, faint, behind everything.
    let mut grid = String::new();
    for i in 0..=max_periods {
        let x = label_w + i as f64 * period_w;
        let _ = write!(
            grid,
            "<line class=\"mdv-wave-grid\" x1=\"{x:.1}\" y1=\"{top:.1}\" x2=\"{x:.1}\" y2=\"{:.1}\"/>",
            top + all.len() as f64 * ROW
        );
    }
    let mut svg = String::new();
    if let Some(h) = head {
        let _ = write!(
            svg,
            "<text class=\"mdv-wave-title\" x=\"{:.1}\" y=\"18\" text-anchor=\"middle\">{}</text>",
            width / 2.0,
            escape_text(h)
        );
    }
    if let Some(f) = foot {
        height += 22.0;
        let _ = write!(
            svg,
            "<text class=\"mdv-wave-title\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
            width / 2.0,
            height - 8.0,
            escape_text(f)
        );
    }
    Ok(format!(
        "<svg viewBox=\"0 0 {width:.1} {height:.1}\" width=\"{width:.0}\" role=\"img\" \
         aria-label=\"Timing diagram\" xmlns=\"http://www.w3.org/2000/svg\">{grid}{body}{svg}</svg>"
    ))
}

/// Draw one lane's wave from `x0` (clipped to `clip`, the end of the name
/// column) with its top at `y`. Returns the number of periods.
fn lane_svg(
    out: &mut String,
    wave: &str,
    data: &[String],
    x0: f64,
    clip: f64,
    y: f64,
    pw: f64,
) -> usize {
    // Expand into one state per period, with gaps.
    let mut states: Vec<(State, bool)> = Vec::new();
    let mut gaps = Vec::new();
    let mut prev: Option<State> = None;
    for c in wave.chars().take(MAX_PERIODS) {
        let state = match c {
            '0' | 'l' | 'L' | 'd' => State::Low,
            '1' | 'h' | 'H' | 'u' => State::High,
            'p' | 'P' => State::Clock(true),
            'n' | 'N' => State::Clock(false),
            '=' | '2' => State::Data(2),
            '3'..='9' => State::Data(c as u8 - b'0'),
            'x' => State::Unknown,
            'z' => State::HighZ,
            '.' | '|' => {
                if c == '|' {
                    gaps.push(states.len());
                }
                match prev {
                    Some(p) => {
                        states.push((p, false));
                        continue;
                    }
                    None => State::Unknown,
                }
            }
            _ => State::Unknown,
        };
        states.push((state, true));
        prev = Some(state);
    }
    let hi = y;
    let lo = y + AMP;
    let mid = y + AMP / 2.0;
    let slope = (pw * 0.15).min(3.0);
    let mut line = String::new();
    let mut shapes = String::new();
    let mut labels = String::new();
    let mut data_i = 0;
    let mut i = 0;
    let mut last_level: Option<f64> = None;
    while i < states.len() {
        let (state, _) = states[i];
        // A run: this state and the periods that continue it.
        let mut j = i + 1;
        while j < states.len() && states[j] == (state, false) {
            j += 1;
        }
        let xa = x0 + i as f64 * pw;
        let xb = x0 + j as f64 * pw;
        match state {
            State::Low | State::High => {
                let level = if state == State::High { hi } else { lo };
                let from = last_level.unwrap_or(level);
                let _ = write!(
                    line,
                    "M{xa:.1},{from:.1}L{:.1},{level:.1}L{xb:.1},{level:.1}",
                    if from == level { xa } else { xa + slope }
                );
                last_level = Some(level);
            }
            State::Clock(rising) => {
                let (first, second) = if rising { (hi, lo) } else { (lo, hi) };
                let mut from = last_level.unwrap_or(second);
                for k in i..j {
                    let a = x0 + k as f64 * pw;
                    let m = a + pw / 2.0;
                    let b = a + pw;
                    let _ = write!(
                        line,
                        "M{a:.1},{from:.1}L{a:.1},{first:.1}L{m:.1},{first:.1}L{m:.1},{second:.1}L{b:.1},{second:.1}"
                    );
                    from = second;
                }
                last_level = Some(second);
            }
            State::HighZ => {
                if let Some(l) = last_level {
                    let _ = write!(line, "M{xa:.1},{l:.1}L{:.1},{mid:.1}", xa + slope);
                }
                let _ = write!(line, "M{xa:.1},{mid:.1}L{xb:.1},{mid:.1}");
                last_level = Some(mid);
            }
            State::Data(_) | State::Unknown => {
                let class = match state {
                    State::Data(n) => format!("mdv-wave-bus mdv-wave-{n}"),
                    _ => "mdv-wave-bus mdv-wave-x".into(),
                };
                let s = slope.min((xb - xa) / 2.0);
                let _ = write!(
                    shapes,
                    "<path class=\"{class}\" d=\"M{xa:.1},{mid:.1}L{:.1},{hi:.1}L{:.1},{hi:.1}L{xb:.1},{mid:.1}L{:.1},{lo:.1}L{:.1},{lo:.1}Z\"/>",
                    xa + s,
                    xb - s,
                    xb - s,
                    xa + s
                );
                if let State::Data(n) = state {
                    if let Some(label) = data.get(data_i) {
                        // On a coloured fill, dark text whatever the theme.
                        let on_fill = if n >= 3 { " mdv-wave-on-fill" } else { "" };
                        let _ = write!(
                            labels,
                            "<text class=\"mdv-wave-data{on_fill}\" x=\"{:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>",
                            (xa + xb) / 2.0,
                            mid + 4.0,
                            escape_text(label)
                        );
                    }
                    data_i += 1;
                }
                last_level = None;
            }
        }
        i = j;
    }
    let mut gap_marks = String::new();
    for g in gaps {
        let x = x0 + g as f64 * pw + pw / 2.0;
        let _ = write!(
            gap_marks,
            "<path class=\"mdv-wave-gap\" d=\"M{:.1},{:.1}C{x:.1},{:.1} {x:.1},{:.1} {:.1},{:.1}\"/>",
            x - 3.0,
            lo + 2.0,
            mid,
            mid,
            x + 3.0,
            hi - 2.0
        );
    }
    let clip_w = (x0 + states.len() as f64 * pw - clip).max(0.0);
    let _ = write!(
        out,
        "<svg x=\"{clip:.1}\" y=\"{:.1}\" width=\"{clip_w:.1}\" height=\"{:.1}\" viewBox=\"{clip:.1} {:.1} {clip_w:.1} {:.1}\" overflow=\"hidden\">\
         {shapes}<path class=\"mdv-wave-line\" d=\"{line}\"/>{labels}{gap_marks}</svg>",
        y - 2.0,
        AMP + 4.0,
        y - 2.0,
        AMP + 4.0
    );
    states.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relaxed_json_is_accepted() {
        let v = parse("{ signal: [\n  { name: 'clk', wave: 'p...' }, // the clock\n  { name: \"it's\", wave: 'x=.x', data: ['a',], },\n]}").unwrap();
        assert_eq!(v["signal"][1]["name"], "it's");
        assert_eq!(v["signal"][1]["data"][0], "a");
    }

    #[test]
    fn lanes_are_drawn() {
        let html = render(
            "{ signal: [{ name: 'clk', wave: 'p....' }, {}, ['Bus', { name: 'dat', wave: 'x.34.x', data: 'head body' }], { name: 'z', wave: '01z|0' }], head: { text: 'Read <cycle>' } }",
            " data-sourcepos=\"1:1-3:3\"",
        );
        assert!(
            html.starts_with(
                "<figure class=\"mdv-diagram mdv-wavedrom\" data-sourcepos=\"1:1-3:3\"><svg"
            ),
            "{html}"
        );
        assert!(html.contains(">clk</text>"));
        assert!(html.contains(">Bus</text>"));
        assert!(html.contains("mdv-wave-3") && html.contains("mdv-wave-4"));
        assert!(html.contains(">head</text>") && html.contains(">body</text>"));
        assert!(html.contains("Read &lt;cycle&gt;"));
        assert!(html.contains("mdv-wave-gap"));
        assert!(render("{ nope: 1 }", "").contains("needs \"signal\""));
        assert!(render("not json", "").contains("Can't draw this WaveDrom"));
    }
}
