//! What differs between macOS, Linux and Windows: canonical paths, `file:`
//! URLs, the home folder, finding programs and browsers, and starting
//! programs detached from our console.

use std::path::{Path, PathBuf};
use std::process::Command;

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

/// `Path::canonicalize`, without Windows' `\\?\` prefix (which breaks
/// comparisons with ordinary paths, `file:` URLs, git and messages).
pub trait Canonical {
    fn canonical(&self) -> std::io::Result<PathBuf>;
}

impl Canonical for Path {
    fn canonical(&self) -> std::io::Result<PathBuf> {
        dunce::canonicalize(self)
    }
}

/// The user's home folder: `$HOME`, else (Windows) `%USERPROFILE%`.
pub fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}

const PATH_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

/// `file:` URL for an absolute path: `file:///home/a/b.html`, or on
/// Windows `file:///C:/Users/a/b.html`.
pub fn file_url(path: &Path) -> String {
    file_url_for(&path.to_string_lossy(), cfg!(windows))
}

fn file_url_for(path: &str, windows: bool) -> String {
    let path = if windows {
        let slashed = path.replace('\\', "/");
        // `C:/x` → `/C:/x`
        if slashed.as_bytes().get(1) == Some(&b':') {
            format!("/{slashed}")
        } else {
            slashed
        }
    } else {
        path.to_string()
    };
    format!("file://{}", utf8_percent_encode(&path, PATH_ENCODE))
}

/// Start `cmd` so that it outlives us and isn't stopped by Ctrl-C in our
/// terminal: its own process group on Unix; on Windows, its own process
/// group and no console window.
pub fn detach(cmd: &mut Command) {
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(cmd, 0);
    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(
            cmd,
            CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW,
        );
    }
    #[cfg(not(any(unix, windows)))]
    let _ = cmd;
}

/// A program on `PATH`, by name or path. On Windows, also with the
/// extensions Windows runs (`code` is `code.cmd`). Rust runs `.cmd` and
/// `.bat` files through `cmd.exe` with safe quoting (and refuses arguments
/// it can't quote safely), so the result can be passed to `Command::new`.
pub fn find_program(name: &str) -> Option<PathBuf> {
    let extensions: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat", ".com"]
    } else {
        &[""]
    };
    let with_ext = |base: &Path| {
        extensions.iter().find_map(|ext| {
            let mut p = base.as_os_str().to_owned();
            p.push(ext);
            let p = PathBuf::from(p);
            p.is_file().then_some(p)
        })
    };
    let given = Path::new(name);
    if given.components().count() > 1 || given.is_absolute() {
        return with_ext(given);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| with_ext(&dir.join(name)))
}

/// Chromium-family and Firefox installs on Windows: (names it goes by,
/// path under Program Files or Local AppData).
const WINDOWS_BROWSERS: &[(&[&str], &[&str])] = &[
    (
        &["chrome", "google chrome", "google-chrome"],
        &["Google", "Chrome", "Application", "chrome.exe"],
    ),
    (
        &["msedge", "edge", "microsoft edge", "microsoft-edge"],
        &["Microsoft", "Edge", "Application", "msedge.exe"],
    ),
    (
        &["brave", "brave browser", "brave-browser"],
        &["BraveSoftware", "Brave-Browser", "Application", "brave.exe"],
    ),
    (&["vivaldi"], &["Vivaldi", "Application", "vivaldi.exe"]),
    (&["chromium"], &["Chromium", "Application", "chrome.exe"]),
    (
        &["firefox", "mozilla firefox"],
        &["Mozilla Firefox", "firefox.exe"],
    ),
];

/// Where Windows programs are installed: Program Files (64- and 32-bit)
/// and the user's Local AppData (per-user installs).
fn windows_install_roots() -> Vec<PathBuf> {
    [
        "ProgramFiles",
        "ProgramW6432",
        "ProgramFiles(x86)",
        "LOCALAPPDATA",
    ]
    .iter()
    .filter_map(std::env::var_os)
    .map(PathBuf::from)
    .collect()
}

/// A browser by name (`chrome`, `edge`, `firefox`…) or path, on Windows,
/// where browsers usually aren't on `PATH`.
pub fn windows_browser(name: &str) -> Option<PathBuf> {
    find_windows_browser(name, &windows_install_roots())
}

fn find_windows_browser(name: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    if let Some(p) = find_program(name) {
        return Some(p);
    }
    let wanted = name.trim().to_ascii_lowercase();
    let wanted = wanted.trim_end_matches(".exe");
    let (_, parts) = WINDOWS_BROWSERS
        .iter()
        .find(|(names, _)| names.contains(&wanted))?;
    roots.iter().find_map(|root| {
        let p = parts.iter().fold(root.clone(), |p, part| p.join(part));
        p.is_file().then_some(p)
    })
}

/// Installed Chromium-family browsers on Windows, in order of preference
/// (Edge is always there).
pub fn windows_chromium_browsers() -> Vec<PathBuf> {
    let roots = windows_install_roots();
    ["chrome", "msedge", "brave", "vivaldi", "chromium"]
        .iter()
        .filter_map(|name| find_windows_browser(name, &roots))
        .collect()
}

/// The program in a Windows `shell\open\command` value:
/// `"C:\…\chrome.exe" --single-argument %1` → `C:\…\chrome.exe`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn exe_from_command(command: &str) -> Option<PathBuf> {
    let command = command.trim();
    let exe = if let Some(rest) = command.strip_prefix('"') {
        &rest[..rest.find('"')?]
    } else {
        let lower = command.to_ascii_lowercase();
        &command[..lower.find(".exe")? + 4]
    };
    (!exe.is_empty()).then(|| PathBuf::from(exe))
}

/// The default browser's program on Windows, from the registry: the
/// https handler's ProgId, then that ProgId's open command.
#[cfg(windows)]
pub fn default_browser() -> Option<PathBuf> {
    use winreg::RegKey;
    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER};
    let choice = RegKey::predef(HKEY_CURRENT_USER).open_subkey(
        r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\https\UserChoice",
    );
    let prog_id: String = choice.ok()?.get_value("ProgId").ok()?;
    let command: String = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey(format!(r"{prog_id}\shell\open\command"))
        .ok()?
        .get_value("")
        .ok()?;
    exe_from_command(&command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls() {
        assert_eq!(
            file_url_for("/tmp/my docs#1/a.html", false),
            "file:///tmp/my%20docs%231/a.html"
        );
        assert_eq!(
            file_url_for(r"C:\Users\Ann Lee\a.html", true),
            "file:///C:/Users/Ann%20Lee/a.html"
        );
        assert_eq!(
            file_url_for(r"\\server\share\a.html", true),
            "file:////server/share/a.html"
        );
    }

    #[test]
    fn programs_from_open_commands() {
        assert_eq!(
            exe_from_command(
                r#""C:\Program Files\Google\Chrome\Application\chrome.exe" --single-argument %1"#
            ),
            Some(PathBuf::from(
                r"C:\Program Files\Google\Chrome\Application\chrome.exe"
            ))
        );
        assert_eq!(
            exe_from_command(r"C:\Firefox\firefox.EXE -osint -url %1"),
            Some(PathBuf::from(r"C:\Firefox\firefox.EXE"))
        );
        assert_eq!(exe_from_command("\"unterminated"), None);
        assert_eq!(exe_from_command("no program"), None);
    }

    #[test]
    fn finds_browsers_in_install_folders() {
        let root = tempfile::tempdir().unwrap();
        let edge = ["Microsoft", "Edge", "Application", "msedge.exe"]
            .iter()
            .fold(root.path().to_path_buf(), |p, part| p.join(part));
        std::fs::create_dir_all(edge.parent().unwrap()).unwrap();
        std::fs::write(&edge, "").unwrap();
        let roots = [PathBuf::from("/nowhere"), root.path().to_path_buf()];
        assert_eq!(find_windows_browser("Edge", &roots), Some(edge.clone()));
        assert_eq!(find_windows_browser("msedge.exe", &roots), Some(edge));
        assert_eq!(find_windows_browser("chrome", &roots), None);
        assert_eq!(find_windows_browser("unknown", &roots), None);
    }
}
