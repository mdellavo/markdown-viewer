//! CSV and TSV files, shown as tables (as GitHub shows them). The viewer
//! makes the columns sortable like any other table.

use std::fmt::Write;
use std::path::Path;

use crate::render::escape_text;

/// Rows shown at most; bigger files say how many were left out.
pub const MAX_ROWS: usize = 10_000;

/// The field separator for a CSV or TSV file, by extension.
pub fn delimiter(path: &Path) -> Option<char> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "csv" => Some(','),
        "tsv" | "tab" => Some('\t'),
        _ => None,
    }
}

/// Parse delimited text: quoted fields (`"a, b"`, with `""` for a quote)
/// may hold separators and line breaks; `\r\n` and `\n` both end a row.
pub fn parse(text: &str, delim: char) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                c => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            c if c == delim => row.push(std::mem::take(&mut field)),
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    // Blank lines aren't rows.
    rows.retain(|r| !(r.len() == 1 && r[0].is_empty()));
    rows
}

/// The table's HTML: the first row is the header. Columns of numbers are
/// right-aligned.
pub fn to_html(text: &str, delim: char) -> String {
    let rows = parse(text, delim);
    let Some((header, body)) = rows.split_first() else {
        return "<p>This file is empty.</p>\n".into();
    };
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let shown = &body[..body.len().min(MAX_ROWS)];
    let numeric: Vec<bool> = (0..width)
        .map(|col| {
            let mut cells = shown
                .iter()
                .filter_map(|r| r.get(col))
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .peekable();
            cells.peek().is_some() && cells.all(is_number)
        })
        .collect();
    let align = |col: usize| {
        if numeric[col] { " align=\"right\"" } else { "" }
    };

    let mut out = String::with_capacity(text.len() * 2);
    out.push_str("<table class=\"mdv-data\">\n<thead>\n<tr>\n");
    for col in 0..width {
        let name = header.get(col).map(String::as_str).unwrap_or("");
        let _ = writeln!(out, "<th{}>{}</th>", align(col), escape_text(name));
    }
    out.push_str("</tr>\n</thead>\n<tbody>\n");
    for row in shown {
        out.push_str("<tr>\n");
        for col in 0..width {
            let cell = row.get(col).map(String::as_str).unwrap_or("");
            let _ = writeln!(out, "<td{}>{}</td>", align(col), escape_text(cell));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table>\n");
    let rows_word = |n: usize| if n == 1 { "row" } else { "rows" };
    if body.len() > shown.len() {
        let _ = writeln!(
            out,
            "<p class=\"mdv-data-note\">Showing the first {} of {} {}.</p>",
            shown.len(),
            body.len(),
            rows_word(body.len())
        );
    } else {
        let _ = writeln!(
            out,
            "<p class=\"mdv-data-note\">{} {}, {} {}.</p>",
            body.len(),
            rows_word(body.len()),
            width,
            if width == 1 { "column" } else { "columns" }
        );
    }
    out
}

/// `12`, `-3.5`, `1,234`, `45%`, `$9.99`, `1e6`…
fn is_number(cell: &str) -> bool {
    let t = cell
        .trim_start_matches(['$', '€', '£', '¥'])
        .trim_end_matches('%')
        .replace(',', "");
    !t.is_empty() && t.parse::<f64>().is_ok_and(f64::is_finite)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quotes_separators_and_line_breaks() {
        let text = "\u{feff}name,note\r\n\"Smith, J\",\"said \"\"hi\"\"\nthen left\"\r\n\nb,\n";
        assert_eq!(
            parse(text, ','),
            vec![
                vec!["name", "note"],
                vec!["Smith, J", "said \"hi\"\nthen left"],
                vec!["b", ""],
            ]
        );
        assert_eq!(
            parse("a\tb\n1\t2", '\t'),
            vec![vec!["a", "b"], vec!["1", "2"]]
        );
        assert!(parse("", ',').is_empty());
    }

    #[test]
    fn renders_a_table() {
        let html = to_html("City,Population\nOslo,\"709,037\"\n<b>,\n", ',');
        assert!(
            html.contains("<th>City</th>\n<th align=\"right\">Population</th>"),
            "{html}"
        );
        assert!(html.contains("<td>&lt;b&gt;</td>"));
        assert!(html.contains("<td align=\"right\">709,037</td>"));
        assert!(html.contains("2 rows, 2 columns."));
        // Ragged rows are padded to the widest.
        assert_eq!(to_html("a\n1,2\n", ',').matches("</th>").count(), 2);
        assert_eq!(to_html("", ','), "<p>This file is empty.</p>\n");
        let big: String = (0..MAX_ROWS + 5).fold("n\n".into(), |s, i| s + &format!("{i}\n"));
        assert!(to_html(&big, ',').contains("Showing the first 10000 of 10005 rows."));
        assert_eq!(delimiter(Path::new("a/B.TSV")), Some('\t'));
        assert_eq!(delimiter(Path::new("a.md")), None);
    }
}
