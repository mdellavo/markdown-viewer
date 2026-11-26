//! Ticking task list items in the viewer: changes the `[ ]` / `[x]` of one
//! list item in the Markdown file.
//!
//! The page sends the item's source position (from `data-sourcepos`: the
//! line and the column of its list marker) and the state it showed. The
//! file is only changed if that position still holds a task marker in that
//! state, so a click on a stale page can't edit the wrong line.

use std::path::Path;
use std::sync::Mutex;

/// Why an item couldn't be changed.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// No task item at that position (the file changed since the page was
    /// rendered).
    NotATask,
    /// The item is already in the requested state.
    AlreadySet,
}

/// Set the task item whose list marker is at `line`:`col` (1-based; `col`
/// in bytes) to `done`. Returns the new text.
pub fn set(text: &str, line: u32, col: u32, done: bool) -> Result<String, Error> {
    let (line, col) = (line as usize, col as usize);
    if line == 0 || col == 0 {
        return Err(Error::NotATask);
    }
    // Byte offset of the start of the line.
    let mut start = 0;
    for _ in 1..line {
        start += text[start..].find('\n').ok_or(Error::NotATask)? + 1;
    }
    let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
    let row = text[start..end].trim_end_matches('\r');
    let at = col - 1;
    if at >= row.len() || !row.is_char_boundary(at) {
        return Err(Error::NotATask);
    }
    let box_at = marker_box(&row[at..]).ok_or(Error::NotATask)?;
    let state = row.as_bytes()[at + box_at + 1];
    let is_done = matches!(state, b'x' | b'X');
    if is_done == done {
        return Err(Error::AlreadySet);
    }
    let pos = start + at + box_at + 1;
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..pos]);
    out.push(if done { 'x' } else { ' ' });
    out.push_str(&text[pos + 1..]);
    Ok(out)
}

/// For text starting at a list marker (`-`, `*`, `+`, `1.`, `1)`), the
/// offset of the `[` of its task box, if it has one.
fn marker_box(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'-' | b'*' | b'+')) {
        i = 1;
    } else {
        while i < b.len() && i < 9 && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 || !matches!(b.get(i), Some(b'.' | b')')) {
            return None;
        }
        i += 1;
    }
    let spaces = b[i..]
        .iter()
        .take_while(|c| matches!(c, b' ' | b'\t'))
        .count();
    if spaces == 0 {
        return None;
    }
    i += spaces;
    let after = b.get(i + 3);
    (b.get(i) == Some(&b'[')
        && matches!(b.get(i + 1), Some(b' ' | b'x' | b'X'))
        && b.get(i + 2) == Some(&b']')
        && matches!(after, None | Some(b' ' | b'\t')))
    .then_some(i)
}

/// Change the item in `file`. Writes in place (editors watching the file
/// reload it, as they would after any other program saved it).
pub fn set_in_file(file: &Path, line: u32, col: u32, done: bool) -> Result<(), String> {
    // One change at a time, so two quick clicks can't both read the old file.
    static LOCK: Mutex<()> = Mutex::new(());
    let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    match set(&text, line, col, done) {
        Ok(new) => std::fs::write(file, new).map_err(|e| e.to_string()),
        Err(Error::AlreadySet) => Ok(()),
        Err(Error::NotATask) => Err("the file has changed; try again".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_and_unticks_items() {
        let text = "# List\r\n\r\n- [ ] one\r\n- [x] two\r\n  1. [X] nested\r\n> * [ ] quoted\n";
        assert_eq!(
            set(text, 3, 1, true).unwrap(),
            text.replacen("- [ ] one", "- [x] one", 1)
        );
        assert_eq!(
            set(text, 4, 1, false).unwrap(),
            text.replacen("- [x] two", "- [ ] two", 1)
        );
        assert_eq!(
            set(text, 5, 3, false).unwrap(),
            text.replacen("1. [X]", "1. [ ]", 1)
        );
        assert_eq!(
            set(text, 6, 3, true).unwrap(),
            text.replacen("* [ ] quoted", "* [x] quoted", 1)
        );
    }

    #[test]
    fn refuses_anything_but_a_task_marker() {
        let text = "- [ ] one\n- not a task\n-[ ] no space\n- [ ]x\n12345678901. [ ] long\n";
        assert_eq!(set(text, 1, 1, false), Err(Error::AlreadySet));
        assert_eq!(set(text, 2, 1, true), Err(Error::NotATask));
        assert_eq!(set(text, 3, 1, true), Err(Error::NotATask));
        assert_eq!(set(text, 4, 1, true), Err(Error::NotATask));
        assert_eq!(set(text, 5, 1, true), Err(Error::NotATask));
        // Wrong column, or past the end of the file.
        assert_eq!(set(text, 1, 2, true), Err(Error::NotATask));
        assert_eq!(set(text, 1, 99, true), Err(Error::NotATask));
        assert_eq!(set(text, 9, 1, true), Err(Error::NotATask));
        assert_eq!(set(text, 0, 1, true), Err(Error::NotATask));
        // An empty item at the end of the file.
        assert_eq!(set("* [ ]", 1, 1, true).unwrap(), "* [x]");
        // Multi-byte text before the column is fine.
        assert_eq!(set("é\n- [ ] a", 2, 1, true).unwrap(), "é\n- [x] a");
    }
}
