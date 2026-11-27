//! iCalendar (```` ```ics ```` blocks and `.ics` files shown as images): the
//! events as an agenda, grouped by month, with their times, places,
//! descriptions and how they repeat. Times are shown as written in the
//! file, with their time zone.

use std::fmt::Write;

use crate::diagrams::invalid;
use crate::render::escape_text;

const MAX_EVENTS: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct When {
    date: (i32, u32, u32),
    /// Hours and minutes; `None` for all-day.
    time: Option<(u32, u32)>,
}

#[derive(Debug, Default)]
struct Event {
    summary: String,
    start: Option<When>,
    end: Option<When>,
    zone: Option<String>,
    location: String,
    description: String,
    repeat: Option<String>,
    cancelled: bool,
}

pub fn render(code: &str, sourcepos: &str) -> String {
    match draw(code, sourcepos) {
        Ok(html) => html,
        Err(e) => invalid("calendar", "ics", code, sourcepos, &e),
    }
}

pub fn file(bytes: &[u8], sourcepos: &str) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "not a text file".to_string())?;
    draw(text, sourcepos)
}

/// Content lines, unfolded (a line starting with a space or tab continues
/// the one before).
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        match (
            line.strip_prefix(' ').or_else(|| line.strip_prefix('\t')),
            out.last_mut(),
        ) {
            (Some(rest), Some(last)) => last.push_str(rest),
            _ => out.push(line.to_string()),
        }
    }
    out
}

/// `NAME;PARAM=V:value` → (NAME, params, value).
/// A content line: name, parameters, value.
type Property = (String, Vec<(String, String)>, String);

fn property(line: &str) -> Option<Property> {
    let mut in_quotes = false;
    let colon = line.char_indices().find(|&(_, c)| {
        if c == '"' {
            in_quotes = !in_quotes;
        }
        c == ':' && !in_quotes
    })?;
    let (head, value) = (&line[..colon.0], &line[colon.0 + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| {
            (
                k.trim().to_ascii_uppercase(),
                v.trim_matches('"').to_string(),
            )
        })
        .collect();
    Some((name, params, value.to_string()))
}

/// Text values: `\n`, `\,`, `\;` and `\\` escapes.
fn text_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

/// `20261127` or `20261127T093000(Z)`.
fn when(v: &str) -> Option<When> {
    let v = v.trim();
    let num = |a: usize, b: usize| v.get(a..b)?.parse::<u32>().ok();
    let date = (num(0, 4)? as i32, num(4, 6)?, num(6, 8)?);
    if !(1..=12).contains(&date.1) || !(1..=31).contains(&date.2) {
        return None;
    }
    let time = match v.get(8..9) {
        Some("T") => Some((num(9, 11)?, num(11, 13)?)),
        _ => None,
    };
    Some(When { date, time })
}

fn draw(text: &str, sourcepos: &str) -> Result<String, String> {
    let lines = unfold(text);
    if !lines
        .iter()
        .any(|l| l.trim().eq_ignore_ascii_case("BEGIN:VCALENDAR"))
    {
        return Err("a calendar starts with BEGIN:VCALENDAR".into());
    }
    let mut name = String::new();
    let mut events: Vec<Event> = Vec::new();
    let mut current: Option<Event> = None;
    // Inside an alarm or other part of an event, whose lines aren't its own.
    let mut nested = 0;
    for line in &lines {
        let Some((prop, params, value)) = property(line) else {
            continue;
        };
        let param = |k: &str| params.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        match (prop.as_str(), current.as_mut()) {
            ("BEGIN", None) if value.eq_ignore_ascii_case("VEVENT") => {
                current = Some(Event::default());
                nested = 0;
            }
            ("BEGIN", Some(_)) => nested += 1,
            ("END", Some(_)) if nested > 0 => nested -= 1,
            ("END", Some(_)) if value.eq_ignore_ascii_case("VEVENT") => {
                if events.len() < MAX_EVENTS {
                    events.push(current.take().unwrap());
                }
                current = None;
            }
            ("X-WR-CALNAME", None) => name = text_value(&value),
            (_, Some(_)) if nested > 0 => {}
            ("SUMMARY", Some(e)) => e.summary = text_value(&value),
            ("LOCATION", Some(e)) => e.location = text_value(&value),
            ("DESCRIPTION", Some(e)) => e.description = text_value(&value),
            ("STATUS", Some(e)) => e.cancelled = value.trim().eq_ignore_ascii_case("CANCELLED"),
            ("RRULE", Some(e)) => e.repeat = Some(repeat(&value)),
            ("DTSTART", Some(e)) => {
                e.start = when(&value);
                e.zone = if value.trim().ends_with('Z') {
                    Some("UTC".into())
                } else {
                    param("TZID")
                };
            }
            ("DTEND", Some(e)) => e.end = when(&value),
            _ => {}
        }
    }
    events.retain(|e| e.start.is_some());
    if events.is_empty() {
        return Err("no events with a start date".into());
    }
    events.sort_by_key(|e| e.start);

    let mut html = format!("<figure class=\"mdv-calendar\"{sourcepos}>");
    if !name.is_empty() {
        let _ = write!(html, "<figcaption>{}</figcaption>", escape_text(&name));
    }
    html.push_str("<table>");
    let mut month = None;
    for e in &events {
        let start = e.start.unwrap();
        let (y, m, d) = start.date;
        if month != Some((y, m)) {
            month = Some((y, m));
            let _ = write!(
                html,
                "<tr class=\"mdv-cal-month\"><td colspan=\"3\">{} {y}</td></tr>",
                MONTHS[m as usize - 1]
            );
        }
        let mut day = format!("{} {d}", WEEKDAYS[weekday(y, m, d)]);
        // All-day events end the day after their last day.
        if let Some(end) = e.end
            && start.time.is_none()
            && end.time.is_none()
        {
            let last = add_days(end.date, -1);
            if last > start.date {
                let (_, lm, ld) = last;
                day.push_str(&if lm == m {
                    format!("–{ld}")
                } else {
                    format!(" – {ld} {}", &MONTHS[lm as usize - 1][..3])
                });
            }
        }
        let time = match (start.time, e.end.and_then(|e| e.time)) {
            (None, _) => "All day".to_string(),
            (Some((h, mi)), Some((eh, emi))) => format!("{h:02}:{mi:02}–{eh:02}:{emi:02}"),
            (Some((h, mi)), None) => format!("{h:02}:{mi:02}"),
        };
        let zone = match (&e.zone, start.time) {
            (Some(z), Some(_)) => {
                format!(" <span class=\"mdv-cal-zone\">{}</span>", escape_text(z))
            }
            _ => String::new(),
        };
        let mut what = format!(
            "<strong>{}</strong>",
            escape_text(if e.summary.is_empty() {
                "(no title)"
            } else {
                &e.summary
            })
        );
        if e.cancelled {
            what = format!("<del>{what}</del> <span class=\"mdv-cal-note\">cancelled</span>");
        }
        if !e.location.is_empty() {
            let _ = write!(
                what,
                "<br><span class=\"mdv-cal-note\">{}</span>",
                escape_text(&e.location)
            );
        }
        if let Some(r) = &e.repeat {
            let _ = write!(
                what,
                "<br><span class=\"mdv-cal-note\">{}</span>",
                escape_text(r)
            );
        }
        if !e.description.is_empty() {
            let mut d: String = e.description.chars().take(300).collect();
            if d.len() < e.description.len() {
                d.push('…');
            }
            let _ = write!(
                what,
                "<br><span class=\"mdv-cal-desc\">{}</span>",
                escape_text(&d).replace('\n', "<br>")
            );
        }
        let _ = write!(
            html,
            "<tr><td class=\"mdv-cal-day\">{day}</td><td class=\"mdv-cal-time\">{time}{zone}</td><td>{what}</td></tr>"
        );
    }
    html.push_str("</table></figure>\n");
    Ok(html)
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// Days since 1970-01-01 (proleptic Gregorian).
fn days(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

fn add_days((y, m, d): (i32, u32, u32), n: i64) -> (i32, u32, u32) {
    from_days(days(y, m, d) + n)
}

/// 0 = Monday.
fn weekday(y: i32, m: u32, d: u32) -> usize {
    // 1970-01-01 was a Thursday.
    (days(y, m, d) + 3).rem_euclid(7) as usize
}

/// An RRULE in words: "Repeats weekly on Mon, Wed until 2026-12-31".
fn repeat(rule: &str) -> String {
    let get = |k: &str| {
        rule.split(';')
            .filter_map(|p| p.split_once('='))
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.to_string())
    };
    let interval: u32 = get("INTERVAL").and_then(|i| i.parse().ok()).unwrap_or(1);
    let freq = get("FREQ").unwrap_or_default().to_ascii_uppercase();
    let unit = match freq.as_str() {
        "DAILY" => "day",
        "WEEKLY" => "week",
        "MONTHLY" => "month",
        "YEARLY" => "year",
        _ => return "Repeats".into(),
    };
    let mut s = if interval > 1 {
        format!("Repeats every {interval} {unit}s")
    } else {
        format!(
            "Repeats {}",
            if unit == "day" {
                "daily".into()
            } else {
                format!("{unit}ly")
            }
        )
    };
    if let Some(days) = get("BYDAY") {
        let names: Vec<&str> = days
            .split(',')
            .filter_map(|d| {
                let code =
                    d.trim_start_matches(|c: char| c == '-' || c == '+' || c.is_ascii_digit());
                ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
                    .iter()
                    .position(|x| code.eq_ignore_ascii_case(x))
                    .map(|i| WEEKDAYS[i])
            })
            .collect();
        if !names.is_empty() {
            let _ = write!(s, " on {}", names.join(", "));
        }
    }
    if let Some(n) = get("COUNT") {
        let _ = write!(s, ", {n} times");
    }
    if let Some(u) = get("UNTIL").and_then(|u| when(&u)) {
        let (y, m, d) = u.date;
        let _ = write!(s, " until {y}-{m:02}-{d:02}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Team\r\n\
BEGIN:VEVENT\r\nSUMMARY:Planning\\, Q1\r\nDTSTART;TZID=Europe/London:20261201T093000\r\n\
DTEND;TZID=Europe/London:20261201T103000\r\nLOCATION:Room <2>\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE;UNTIL=20261231T000000Z\r\n\
DESCRIPTION:Bring the\\nnumbers and a long line that is\r\n  folded\r\n\
BEGIN:VALARM\r\nDESCRIPTION:Reminder\r\nEND:VALARM\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nSUMMARY:Offsite\r\nDTSTART;VALUE=DATE:20261127\r\nDTEND;VALUE=DATE:20261130\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nSUMMARY:Call\r\nDTSTART:20261127T150000Z\r\nSTATUS:CANCELLED\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n";

    #[test]
    fn events_are_listed_by_month() {
        let html = render(ICS, "");
        assert!(
            html.starts_with("<figure class=\"mdv-calendar\"><figcaption>Team</figcaption><table>"),
            "{html}"
        );
        let nov = html.find("November 2026").unwrap();
        let dec = html.find("December 2026").unwrap();
        assert!(nov < dec);
        assert!(
            html.contains(
                "<td class=\"mdv-cal-day\">Fri 27–29</td><td class=\"mdv-cal-time\">All day</td>"
            ),
            "{html}"
        );
        assert!(html.contains("<del><strong>Call</strong></del>"));
        assert!(html.contains("15:00 <span class=\"mdv-cal-zone\">UTC</span>"));
        assert!(html.contains("<strong>Planning, Q1</strong>"));
        assert!(html.contains("Room &lt;2&gt;"));
        assert!(html.contains("Repeats weekly on Mon, Wed until 2026-12-31"));
        assert!(
            html.contains("Bring the<br>numbers and a long line that is folded"),
            "{html}"
        );
        assert!(
            !html.contains("Reminder"),
            "the alarm's description isn't the event's"
        );
        assert!(html.contains("Tue 1</td><td class=\"mdv-cal-time\">09:30–10:30 <span class=\"mdv-cal-zone\">Europe/London</span>"));
        assert!(render("BEGIN:VEVENT\nEND:VEVENT", "").contains("BEGIN:VCALENDAR"));
    }

    #[test]
    fn dates() {
        assert_eq!(weekday(2026, 11, 27), 4); // a Friday
        assert_eq!(add_days((2026, 12, 31), 1), (2027, 1, 1));
        assert_eq!(add_days((2024, 3, 1), -1), (2024, 2, 29));
    }
}
