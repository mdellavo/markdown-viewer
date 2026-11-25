//! Opening the page in a browser: as a minimal app window, a new window, or
//! a tab.
//!
//! There's no cross-browser API for this, so we work out which browser will
//! be used (the one given with `--browser`, or the system default) and use
//! that browser's own mechanism:
//!
//! - App window (no tabs or address bar): `--app=<url>`, Chromium-family only.
//! - New window: `--new-window <url>` for Chromium and Firefox; AppleScript
//!   `make new document` for Safari.
//! - Each mode falls back to the next (app → window → tab) when the browser
//!   can't do it.

use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// How to show the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// A minimal window without tabs, address bar or toolbar.
    #[default]
    App,
    /// A normal new browser window.
    Window,
    /// A tab in an existing window.
    Tab,
}

/// What actually got opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// A window of its own (app window or new window).
    Window,
    /// A tab, possibly in a window the user was already using.
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Chrome, Chromium, Brave, Edge, Vivaldi, Opera.
    Chromium,
    /// Firefox and LibreWolf.
    Firefox,
    Safari,
    Other,
}

/// Classify a browser by app name, bundle id, command, or `.desktop` id.
pub fn classify(name: &str) -> Kind {
    let n = name.to_ascii_lowercase();
    let has = |s: &str| n.contains(s);
    if has("chrome")
        || has("chromium")
        || has("brave")
        || has("edge")
        || has("vivaldi")
        || has("opera")
    {
        Kind::Chromium
    } else if has("firefox") || has("librewolf") {
        Kind::Firefox
    } else if has("safari") {
        Kind::Safari
    } else {
        Kind::Other
    }
}

/// The browser to use: what the user named, or the system default.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    /// `--browser NAME` (macOS app name, or a command on Linux).
    Named(String),
    /// macOS default browser by bundle id.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Bundle(String),
    /// Linux default browser as a command name.
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Command(String),
    /// Couldn't tell; use the OS default opener.
    Unknown,
}

/// Chromium browsers with a working `--app` mode (Opera dropped it).
fn supports_app_mode(name: &str) -> bool {
    classify(name) == Kind::Chromium && !name.to_ascii_lowercase().contains("opera")
}

/// Open `url` (an `http://` or `file://` URL).
pub fn open(url: &str, browser: Option<&str>, mode: Mode) -> Result<Opened> {
    let target = match browser {
        Some(name) => Target::Named(name.to_string()),
        None => default_browser(),
    };
    if mode != Mode::Tab {
        match window_command(url, &target, mode) {
            Some(cmd) => match spawn(cmd) {
                Ok(()) => return Ok(Opened::Window),
                Err(err) => eprintln!(
                    "markdown-viewer: couldn't open a separate window ({err:#}); opening in a tab"
                ),
            },
            None => eprintln!(
                "markdown-viewer: don't know how to open a separate window in {}; opening in a tab",
                describe(&target)
            ),
        }
    }
    match tab_command(url, &target) {
        Some(cmd) => spawn(cmd)?,
        None => opener::open_browser(url).with_context(|| format!("cannot open {url}"))?,
    }
    Ok(Opened::Tab)
}

/// A program and its arguments.
type Cmd = (String, Vec<String>);

fn cmd(program: &str, args: &[&str]) -> Cmd {
    (
        program.to_string(),
        args.iter().map(|s| s.to_string()).collect(),
    )
}

/// Command for an app window or new window. Asking for an app window from a
/// browser without app mode gets a new window instead.
fn window_command(url: &str, target: &Target, mode: Mode) -> Option<Cmd> {
    let name = match target {
        Target::Named(n) | Target::Bundle(n) | Target::Command(n) => n,
        Target::Unknown => return None,
    };
    let app_flag = format!("--app={url}");
    let browser_args: Vec<&str> = match classify(name) {
        Kind::Chromium if mode == Mode::App && supports_app_mode(name) => vec![&app_flag],
        Kind::Chromium | Kind::Firefox => vec!["--new-window", url],
        Kind::Safari => return cfg!(target_os = "macos").then(|| safari_new_window(url)),
        Kind::Other => return None,
    };
    Some(match target {
        // `open -n` hands the arguments to the browser, which forwards them
        // to its already-running instance if there is one.
        Target::Named(app) if cfg!(target_os = "macos") => cmd(
            "open",
            &[&["-na", app, "--args"], &browser_args[..]].concat(),
        ),
        Target::Bundle(id) => cmd(
            "open",
            &[&["-nb", id, "--args"], &browser_args[..]].concat(),
        ),
        Target::Named(c) | Target::Command(c) => cmd(c, &browser_args),
        Target::Unknown => unreachable!(),
    })
}

fn tab_command(url: &str, target: &Target) -> Option<Cmd> {
    match target {
        Target::Named(app) if cfg!(target_os = "macos") => Some(cmd("open", &["-a", app, url])),
        Target::Named(c) => Some(cmd(c, &[url])),
        _ => None,
    }
}

/// The URL is passed as an argument, never spliced into the script.
fn safari_new_window(url: &str) -> Cmd {
    cmd(
        "osascript",
        &[
            "-e",
            "on run argv",
            "-e",
            "tell application \"Safari\"",
            "-e",
            "make new document with properties {URL:item 1 of argv}",
            "-e",
            "activate",
            "-e",
            "end tell",
            "-e",
            "end run",
            url,
        ],
    )
}

fn describe(target: &Target) -> String {
    match target {
        Target::Named(n) | Target::Bundle(n) | Target::Command(n) => n.clone(),
        Target::Unknown => "the default browser".into(),
    }
}

/// Run a launcher command. It's put in its own process group so Ctrl-C in
/// the terminal doesn't also kill a browser we started, and reaped on a
/// background thread so it never blocks us.
fn spawn((program, args): Cmd) -> Result<()> {
    let mut command = Command::new(&program);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut child = command
        .spawn()
        .with_context(|| format!("cannot run {program}"))?;
    // `open` and `osascript` finish quickly and report failure via exit code.
    if program == "open" || program == "osascript" {
        let status = child.wait()?;
        if !status.success() {
            bail!("{program} exited with {status}");
        }
    } else {
        std::thread::spawn(move || child.wait());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn default_browser() -> Target {
    // LaunchServices records the user's choice; with no entry, it's Safari.
    let plist = std::env::var_os("HOME").map(|h| {
        std::path::Path::new(&h).join(
            "Library/Preferences/com.apple.LaunchServices/com.apple.launchservices.secure.plist",
        )
    });
    let Some(plist) = plist else {
        return Target::Unknown;
    };
    let output = Command::new("plutil")
        .args(["-extract", "LSHandlers", "json", "-o", "-"])
        .arg(&plist)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            match handler_from_launchservices(&String::from_utf8_lossy(&out.stdout)) {
                Some(id) => Target::Bundle(id),
                None => Target::Bundle("com.apple.safari".into()),
            }
        }
        // No preferences file yet means the user never changed the default.
        _ if !plist.exists() => Target::Bundle("com.apple.safari".into()),
        _ => Target::Unknown,
    }
}

#[cfg(not(target_os = "macos"))]
fn default_browser() -> Target {
    let out = Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .output();
    match out {
        Ok(out) if out.status.success() => {
            desktop_to_command(String::from_utf8_lossy(&out.stdout).trim())
                .map(Target::Command)
                .unwrap_or(Target::Unknown)
        }
        _ => Target::Unknown,
    }
}

/// Pick the https (else http) handler's bundle id from LaunchServices JSON.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn handler_from_launchservices(json: &str) -> Option<String> {
    let handlers: Vec<serde_json::Value> = serde_json::from_str(json).ok()?;
    ["https", "http"].iter().find_map(|scheme| {
        handlers
            .iter()
            .find(|h| h["LSHandlerURLScheme"].as_str() == Some(scheme))
            .and_then(|h| h["LSHandlerRoleAll"].as_str())
            .map(str::to_string)
    })
}

/// Map a `.desktop` id (e.g. `google-chrome.desktop`, `firefox_firefox.desktop`
/// for snaps) to a command, for browsers we know how to drive.
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn desktop_to_command(desktop: &str) -> Option<String> {
    let id = desktop.strip_suffix(".desktop").unwrap_or(desktop);
    let id = id.split('_').next_back().unwrap_or(id);
    const KNOWN: &[&str] = &[
        "google-chrome",
        "google-chrome-stable",
        "google-chrome-beta",
        "chromium",
        "chromium-browser",
        "brave-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
        "vivaldi-stable",
        "opera",
        "firefox",
        "firefox-esr",
        "librewolf",
    ];
    KNOWN.contains(&id).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_browsers() {
        assert_eq!(classify("com.google.chrome"), Kind::Chromium);
        assert_eq!(classify("Google Chrome"), Kind::Chromium);
        assert_eq!(classify("com.microsoft.edgemac"), Kind::Chromium);
        assert_eq!(classify("brave-browser"), Kind::Chromium);
        assert_eq!(classify("org.mozilla.firefox"), Kind::Firefox);
        assert_eq!(classify("com.apple.Safari"), Kind::Safari);
        assert_eq!(classify("company.thebrowser.Browser"), Kind::Other);
    }

    #[test]
    fn app_mode_for_chromium() {
        let chrome = Target::Bundle("com.google.chrome".into());
        assert_eq!(
            window_command("http://x/", &chrome, Mode::App),
            Some(cmd(
                "open",
                &["-nb", "com.google.chrome", "--args", "--app=http://x/"]
            ))
        );
        let brave = Target::Command("brave-browser".into());
        assert_eq!(
            window_command("http://x/", &brave, Mode::App),
            Some(cmd("brave-browser", &["--app=http://x/"]))
        );
    }

    #[test]
    fn app_mode_falls_back_to_new_window() {
        let firefox = Target::Command("firefox".into());
        assert_eq!(
            window_command("http://x/", &firefox, Mode::App),
            Some(cmd("firefox", &["--new-window", "http://x/"]))
        );
        let opera = Target::Command("opera".into());
        assert_eq!(
            window_command("http://x/", &opera, Mode::App),
            Some(cmd("opera", &["--new-window", "http://x/"]))
        );
    }

    #[test]
    fn new_window_for_default_chrome_uses_bundle_id() {
        let c = window_command(
            "http://x/",
            &Target::Bundle("com.google.chrome".into()),
            Mode::Window,
        );
        assert_eq!(
            c,
            Some(cmd(
                "open",
                &[
                    "-nb",
                    "com.google.chrome",
                    "--args",
                    "--new-window",
                    "http://x/"
                ]
            ))
        );
    }

    #[test]
    fn new_window_for_linux_command() {
        let c = window_command(
            "http://x/",
            &Target::Command("firefox".into()),
            Mode::Window,
        );
        assert_eq!(c, Some(cmd("firefox", &["--new-window", "http://x/"])));
    }

    #[test]
    fn no_new_window_for_unknown_browsers() {
        for mode in [Mode::App, Mode::Window] {
            assert_eq!(window_command("u", &Target::Unknown, mode), None);
            assert_eq!(
                window_command("u", &Target::Command("lynx".into()), mode),
                None
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn safari_url_is_an_argument_not_script() {
        let (prog, args) = window_command(
            "http://x/\"evil",
            &Target::Bundle("com.apple.safari".into()),
            Mode::App,
        )
        .unwrap();
        assert_eq!(prog, "osascript");
        assert_eq!(args.last().unwrap(), "http://x/\"evil");
        assert!(args[..args.len() - 1].iter().all(|a| !a.contains("evil")));
    }

    #[test]
    fn reads_launchservices_handler() {
        let json = r#"[{"LSHandlerContentType":"public.html","LSHandlerRoleAll":"x"},
                       {"LSHandlerURLScheme":"http","LSHandlerRoleAll":"org.mozilla.firefox"},
                       {"LSHandlerURLScheme":"https","LSHandlerRoleAll":"com.google.chrome"}]"#;
        assert_eq!(
            handler_from_launchservices(json).as_deref(),
            Some("com.google.chrome")
        );
        assert_eq!(handler_from_launchservices("[]"), None);
    }

    #[test]
    fn maps_desktop_ids() {
        assert_eq!(
            desktop_to_command("google-chrome.desktop").as_deref(),
            Some("google-chrome")
        );
        assert_eq!(
            desktop_to_command("firefox_firefox.desktop").as_deref(),
            Some("firefox")
        );
        assert_eq!(desktop_to_command("org.gnome.Epiphany.desktop"), None);
    }
}
