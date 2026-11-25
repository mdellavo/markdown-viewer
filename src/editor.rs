//! Opening a file at a line in the user's editor (click-to-source).
//!
//! The editor is, in order: the `editor` setting, `$MARKDOWN_VIEWER_EDITOR`,
//! `$VISUAL` / `$EDITOR` if it's a known GUI editor, then the first known
//! editor found on `PATH`. On macOS the last resort is the default text
//! editor (without jumping to the line). Terminal editors such as vim can't
//! be used: there's no terminal to run them in.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// Known editors and how to open `{file}` at `{line}`.
const KNOWN: &[(&str, &[&str])] = &[
    ("zed", &["{file}:{line}"]),
    ("code", &["-g", "{file}:{line}"]),
    ("code-insiders", &["-g", "{file}:{line}"]),
    ("cursor", &["-g", "{file}:{line}"]),
    ("windsurf", &["-g", "{file}:{line}"]),
    ("codium", &["-g", "{file}:{line}"]),
    ("subl", &["{file}:{line}"]),
    ("mate", &["-l", "{line}", "{file}"]),
    ("idea", &["--line", "{line}", "{file}"]),
];

/// Open `file` at `line` (1-based).
pub fn open(file: &Path, line: u32, configured: Option<&str>) -> Result<()> {
    let (program, args) = command(file, line, configured)?;
    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let mut child = cmd
        .spawn()
        .with_context(|| format!("cannot run editor {program}"))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// The command to run, as (program, args).
pub fn command(file: &Path, line: u32, configured: Option<&str>) -> Result<(String, Vec<String>)> {
    let file = file.to_string_lossy();
    let line = line.max(1).to_string();
    let fill = |a: &str| a.replace("{file}", &file).replace("{line}", &line);

    let template = configured
        .map(str::to_string)
        .or_else(|| std::env::var("MARKDOWN_VIEWER_EDITOR").ok())
        .filter(|t| !t.trim().is_empty());
    if let Some(template) = template {
        let mut words = split_words(&template).into_iter();
        let program = words.next().context("the editor setting is empty")?;
        let mut args: Vec<String> = words.map(|w| fill(&w)).collect();
        if !template.contains("{file}") {
            // A bare command: use its known form, else just pass the file.
            match known_args(&program) {
                Some(known) => args.extend(known.iter().map(|a| fill(a))),
                None => args.push(file.to_string()),
            }
        }
        return Ok((program, args));
    }

    for var in ["VISUAL", "EDITOR"] {
        if let Ok(value) = std::env::var(var) {
            let program = split_words(&value).into_iter().next().unwrap_or_default();
            if let Some(known) = known_args(&program) {
                return Ok((program, known.iter().map(|a| fill(a)).collect()));
            }
        }
    }
    for (name, args) in KNOWN {
        if on_path(name) {
            return Ok((name.to_string(), args.iter().map(|a| fill(a)).collect()));
        }
    }
    if cfg!(target_os = "macos") {
        return Ok(("open".into(), vec!["-t".into(), file.into_owned()]));
    }
    bail!(
        "no editor found; set `editor` in the settings file (e.g. editor = \"zed {{file}}:{{line}}\")"
    )
}

/// Arguments for a known editor, matched by its command's file name.
fn known_args(program: &str) -> Option<&'static [&'static str]> {
    let name = Path::new(program).file_name()?.to_str()?;
    KNOWN.iter().find(|(n, _)| *n == name).map(|(_, a)| *a)
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// Split a command line on spaces, honouring double quotes.
fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            ' ' | '\t' if !quoted => {
                if any {
                    words.push(std::mem::take(&mut current));
                    any = false;
                }
            }
            c => {
                current.push(c);
                any = true;
            }
        }
    }
    if any {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_fill_file_and_line() {
        let (p, a) = command(
            Path::new("/a b/c.md"),
            7,
            Some("myedit --goto \"{file}\":{line}"),
        )
        .unwrap();
        assert_eq!(p, "myedit");
        assert_eq!(a, ["--goto", "/a b/c.md:7"]);
    }

    #[test]
    fn bare_known_editor_uses_its_line_syntax() {
        let (p, a) = command(Path::new("/x.md"), 3, Some("code")).unwrap();
        assert_eq!(
            (p.as_str(), a),
            ("code", vec!["-g".to_string(), "/x.md:3".into()])
        );
        let (p, a) = command(Path::new("/x.md"), 3, Some("/usr/local/bin/mate")).unwrap();
        assert_eq!(
            (p.as_str(), a),
            (
                "/usr/local/bin/mate",
                vec!["-l".to_string(), "3".into(), "/x.md".into()]
            )
        );
    }

    #[test]
    fn unknown_bare_editor_gets_just_the_file() {
        let (_, a) = command(Path::new("/x.md"), 3, Some("gedit")).unwrap();
        assert_eq!(a, ["/x.md"]);
    }

    #[test]
    fn splits_quoted_words() {
        assert_eq!(split_words("a \"b c\" d"), ["a", "b c", "d"]);
    }
}
