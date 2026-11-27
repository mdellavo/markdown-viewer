//! `--pdf`: print the rendered page to PDF with a Chromium-based browser in
//! headless mode, after its scripts (diagrams, math) have run.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Browsers tried in order. `MARKDOWN_VIEWER_CHROME` overrides the search.
#[cfg(target_os = "macos")]
const CANDIDATES: &[&str] = &[
    "Google Chrome.app/Contents/MacOS/Google Chrome",
    "Chromium.app/Contents/MacOS/Chromium",
    "Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "Brave Browser.app/Contents/MacOS/Brave Browser",
    "Vivaldi.app/Contents/MacOS/Vivaldi",
];
#[cfg(not(target_os = "macos"))]
const CANDIDATES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "microsoft-edge-stable",
    "brave-browser",
    "vivaldi-stable",
];

/// Find a Chromium-based browser to print with.
pub fn find_browser() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MARKDOWN_VIEWER_CHROME") {
        return Some(PathBuf::from(p));
    }
    if cfg!(windows) {
        return crate::platform::windows_chromium_browsers()
            .into_iter()
            .next();
    }
    if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let dirs = [
            Some(PathBuf::from("/Applications")),
            home.map(|h| h.join("Applications")),
        ];
        for dir in dirs.iter().flatten() {
            for app in CANDIDATES {
                let p = dir.join(app);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    } else {
        let path = std::env::var_os("PATH")?;
        CANDIDATES.iter().find_map(|name| {
            std::env::split_paths(&path)
                .map(|d| d.join(name))
                .find(|p| p.is_file())
        })
    }
}

/// Print `html` (a self-contained page; relative links resolve via its
/// `<base>`) to `out`.
pub fn print(html: &str, out: &Path) -> Result<()> {
    let browser = find_browser().context(
        "--pdf needs Chrome, Chromium, Edge, Brave or Vivaldi; none was found \
         (set MARKDOWN_VIEWER_CHROME to its path)",
    )?;
    let work = tempfile::tempdir()?;
    let page = work.path().join("page.html");
    std::fs::write(&page, html)?;
    let out_abs = if out.is_absolute() {
        out.to_path_buf()
    } else {
        std::env::current_dir()?.join(out)
    };
    // Printed to the work folder first, so a failure leaves any existing
    // PDF alone.
    let printed = work.path().join("out.pdf");

    let output = Command::new(&browser)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        // A throwaway profile, so this never touches (or waits on) the
        // user's running browser.
        .arg(format!(
            "--user-data-dir={}",
            work.path().join("profile").display()
        ))
        // Let diagrams and math render before printing.
        .arg("--virtual-time-budget=15000")
        .arg("--run-all-compositor-stages-before-draw")
        .arg("--no-pdf-header-footer")
        .arg("--print-to-pdf-no-header")
        .arg(format!("--print-to-pdf={}", printed.display()))
        .arg(crate::platform::file_url(&page))
        .output()
        .with_context(|| format!("cannot run {}", browser.display()))?;

    let ok = std::fs::metadata(&printed).is_ok_and(|m| m.len() > 0);
    if !ok {
        bail!(
            "{} didn't produce a PDF: {}",
            browser.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    // Renaming fails across file systems (the temp folder may be on
    // another one); copy then.
    if std::fs::rename(&printed, &out_abs).is_err() {
        std::fs::copy(&printed, &out_abs)
            .with_context(|| format!("cannot write {}", out_abs.display()))?;
    }
    Ok(())
}
