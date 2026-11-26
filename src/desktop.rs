//! Linux (freedesktop) integration: `--install-desktop` /
//! `--uninstall-desktop`.
//!
//! Installs, for the current user only, a "Markdown Viewer" application
//! entry (`~/.local/share/applications/markdown-viewer.desktop`) and its
//! icon, registered for Markdown, CSV and TSV files. File managers then
//! offer it under Open With, and it can be made the default. Each file is
//! opened with `markdown-viewer --watch --exit-when-closed`.
//!
//! The .deb, .rpm and AUR packages install the same entry system-wide
//! (`packaging/linux/markdown-viewer.desktop`, kept equal to [`entry`] by a
//! test).

use crate::platform::Canonical;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

pub const ICON: &str = include_str!("../assets/macos/AppIcon.svg");
const FILE_NAME: &str = "markdown-viewer.desktop";
const ICON_NAME: &str = "markdown-viewer";
const MIME_TYPES: &[&str] = &[
    "text/markdown",
    "text/x-markdown",
    "text/csv",
    "text/tab-separated-values",
];

/// The desktop entry, running `exe`.
pub fn entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Markdown Viewer\n\
         GenericName=Markdown Viewer\n\
         Comment=Render Markdown with GitHub styling and live-reload it in your browser\n\
         Exec={} --watch --exit-when-closed %f\n\
         Icon={ICON_NAME}\n\
         Terminal=false\n\
         NoDisplay=false\n\
         StartupNotify=false\n\
         Categories=Utility;TextTools;\n\
         Keywords=markdown;md;readme;preview;csv;\n\
         MimeType={};\n",
        quote_exec(exe),
        MIME_TYPES.join(";")
    )
}

/// Quote a program path for `Exec=` (the desktop entry spec's rules).
fn quote_exec(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-+".contains(c));
    if plain {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    // `%` is special in Exec, even inside quotes.
    out.replace('%', "%%")
}

struct Paths {
    entry: PathBuf,
    icon: PathBuf,
}

fn paths(data_home: &Path) -> Paths {
    Paths {
        entry: data_home.join("applications").join(FILE_NAME),
        icon: data_home
            .join("icons/hicolor/scalable/apps")
            .join(format!("{ICON_NAME}.svg")),
    }
}

/// `$XDG_DATA_HOME`, else `~/.local/share`.
fn data_home() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/share"))
}

/// This program's path, preferring the one on `PATH` (e.g. Homebrew's
/// stable `bin/` link rather than a versioned Cellar path that an upgrade
/// removes).
fn program() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot find this program's path")?;
    let real = exe.canonical().unwrap_or_else(|_| exe.clone());
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("markdown-viewer"))
            .find(|p| p.canonical().is_ok_and(|c| c == real))
    });
    Ok(on_path.unwrap_or(exe))
}

pub fn install() -> Result<()> {
    if cfg!(target_os = "macos") {
        bail!("on macOS, use --install-finder");
    }
    if cfg!(windows) {
        bail!("desktop integration is for Linux; Windows Explorer integration isn't available yet");
    }
    let exe = program()?;
    let data = data_home()?;
    install_into(&data, &exe)?;
    let p = paths(&data);
    refresh(&data);
    println!("Installed {}", p.entry.display());
    println!("Installed {}", p.icon.display());
    println!();
    println!("Markdown files now offer Markdown Viewer under Open With.");
    println!("To open them with it by default:");
    println!("  xdg-mime default {FILE_NAME} text/markdown");
    println!();
    println!("It runs {}.", exe.display());
    println!("If that moves, run `markdown-viewer --install-desktop` again.");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    if cfg!(target_os = "macos") {
        bail!("on macOS, use --uninstall-finder");
    }
    if cfg!(windows) {
        bail!("desktop integration is for Linux");
    }
    let data = data_home()?;
    let removed = uninstall_from(&data)?;
    refresh(&data);
    if removed.is_empty() {
        println!("Nothing to remove.");
    }
    for p in removed {
        println!("Removed {}", p.display());
    }
    Ok(())
}

fn install_into(data: &Path, exe: &Path) -> Result<()> {
    let p = paths(data);
    for file in [&p.entry, &p.icon] {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
    }
    std::fs::write(&p.entry, entry(&exe.to_string_lossy()))
        .with_context(|| format!("cannot write {}", p.entry.display()))?;
    std::fs::write(&p.icon, ICON).with_context(|| format!("cannot write {}", p.icon.display()))
}

fn uninstall_from(data: &Path) -> Result<Vec<PathBuf>> {
    let p = paths(data);
    let mut removed = Vec::new();
    for file in [p.entry, p.icon] {
        if file.exists() {
            std::fs::remove_file(&file)
                .with_context(|| format!("cannot remove {}", file.display()))?;
            removed.push(file);
        }
    }
    Ok(removed)
}

/// Tell the desktop about the change, where the tools exist (it notices
/// on its own eventually otherwise).
fn refresh(data: &Path) {
    let quiet = |cmd: &mut Command| {
        let _ = cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    };
    quiet(Command::new("update-desktop-database").arg(data.join("applications")));
    quiet(
        Command::new("gtk-update-icon-cache")
            .args(["-f", "-t"])
            .arg(data.join("icons/hicolor")),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_quotes_the_program() {
        assert!(
            entry("/usr/bin/markdown-viewer")
                .contains("\nExec=/usr/bin/markdown-viewer --watch --exit-when-closed %f\n")
        );
        assert_eq!(quote_exec("/opt/my apps/mdv"), "\"/opt/my apps/mdv\"");
        assert_eq!(quote_exec("/a/$x\"y%"), "\"/a/\\$x\\\"y%%\"");
    }

    #[test]
    fn packaged_entry_matches() {
        let packaged = include_str!("../packaging/linux/markdown-viewer.desktop");
        assert_eq!(
            packaged,
            entry("markdown-viewer"),
            "packaging/linux/markdown-viewer.desktop is out of date"
        );
    }

    #[test]
    fn installs_and_uninstalls() {
        let dir = tempfile::tempdir().unwrap();
        install_into(dir.path(), Path::new("/bin/mdv")).unwrap();
        let p = paths(dir.path());
        assert!(
            std::fs::read_to_string(&p.entry)
                .unwrap()
                .contains("Exec=/bin/mdv ")
        );
        assert!(
            std::fs::read_to_string(&p.icon)
                .unwrap()
                .starts_with("<svg")
        );
        assert_eq!(uninstall_from(dir.path()).unwrap().len(), 2);
        assert!(uninstall_from(dir.path()).unwrap().is_empty());
    }
}
