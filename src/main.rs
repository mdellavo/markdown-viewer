use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

mod browser;
mod config;
mod docs;
#[cfg(target_os = "macos")]
mod finder;
mod github;
mod pdf;
mod render;
mod server;
mod theme;

use config::{Config, Prefs};
use render::{PageOptions, Renderer, Style, Theme};

/// Render Markdown with GitHub styling and view it in your browser.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Markdown file to render, or a folder to browse (default: the current
    /// folder). Folders are always served with live reload.
    file: Option<PathBuf>,

    /// Serve the page and live-reload it whenever the file changes.
    #[arg(short, long)]
    watch: bool,

    /// Write the rendered HTML to this file ("-" for stdout) instead of opening it.
    #[arg(short, long, value_name = "PATH", conflicts_with_all = ["watch", "pdf"])]
    output: Option<PathBuf>,

    /// Write a PDF to this file instead of opening it (needs Chrome,
    /// Chromium, Edge, Brave or Vivaldi).
    #[arg(long, value_name = "PATH", conflicts_with = "watch")]
    pdf: Option<PathBuf>,

    /// Port for --watch (default: any free port).
    #[arg(short, long)]
    port: Option<u16>,

    /// Don't open a browser.
    #[arg(long)]
    no_open: bool,

    /// Browser to open instead of the default (e.g. "Google Chrome", "firefox").
    #[arg(long, value_name = "NAME")]
    browser: Option<String>,

    /// Open as a minimal app window with no tabs or address bar (default).
    /// Chrome, Edge, Brave, Chromium and Vivaldi only; other browsers get
    /// a new window.
    #[arg(long, group = "open_as")]
    app: bool,

    /// Open in a normal new browser window.
    #[arg(long, group = "open_as")]
    window: bool,

    /// Open in a tab of an existing browser window.
    #[arg(long, group = "open_as")]
    tab: bool,

    /// With --watch: exit once the page has been closed (used by the Finder
    /// integration, where there's no terminal to press Ctrl-C in).
    #[arg(long)]
    exit_when_closed: bool,

    /// macOS: add "Markdown Viewer" to Finder (Open With, double-click and a
    /// right-click Quick Action) for the current user.
    #[arg(long, exclusive = true, hide = !cfg!(target_os = "macos"))]
    install_finder: bool,

    /// macOS: remove the Finder integration.
    #[arg(long, exclusive = true, hide = !cfg!(target_os = "macos"))]
    uninstall_finder: bool,

    /// Colour theme (default: the one you last chose in the viewer, else
    /// auto). Press t in the viewer to switch.
    #[arg(long, value_enum)]
    theme: Option<Theme>,

    /// Page style.
    #[arg(long, value_enum)]
    style: Option<Style>,

    /// Extra CSS file, applied after the built-in styles.
    #[arg(long, value_name = "PATH")]
    css: Option<PathBuf>,
}

fn main() {
    if let Err(err) = run(Cli::parse()) {
        eprintln!("markdown-viewer: {err:#}");
        std::process::exit(1);
    }
}

/// Command-line options merged with the settings file.
struct Settings {
    style: Style,
    css_path: Option<PathBuf>,
    css: Option<String>,
    open: browser::Mode,
    browser: Option<String>,
    port: u16,
    /// `--theme` if given; it wins over remembered and configured themes.
    theme_flag: Option<Theme>,
    config_theme: Option<Theme>,
}

impl Settings {
    fn new(cli: &Cli, config: Config) -> Result<Self> {
        let css_path = cli.css.clone().or(config.css.clone());
        let css = match &css_path {
            Some(p) => Some(
                std::fs::read_to_string(p)
                    .with_context(|| format!("cannot read CSS file {}", p.display()))?,
            ),
            None => None,
        };
        let open = if cli.tab {
            browser::Mode::Tab
        } else if cli.window {
            browser::Mode::Window
        } else if cli.app {
            browser::Mode::App
        } else {
            config.open_mode().unwrap_or_default()
        };
        Ok(Self {
            style: cli.style.or(config.style).unwrap_or_default(),
            css_path,
            css,
            open,
            browser: cli.browser.clone().or(config.browser),
            port: cli.port.or(config.port).unwrap_or(0),
            theme_flag: cli.theme,
            config_theme: config.theme,
        })
    }

    /// Theme for something you'll read now: the flag, else the last theme
    /// picked in the viewer, else the settings file, else auto.
    fn viewing_theme(&self, prefs: &Prefs) -> Theme {
        self.theme_flag
            .or(prefs.theme)
            .or(self.config_theme)
            .unwrap_or_default()
    }

    /// Theme for an exported file: the flag or settings file only.
    fn export_theme(&self) -> Theme {
        self.theme_flag.or(self.config_theme).unwrap_or_default()
    }

    fn page(&self, title: String, theme: Theme) -> PageOptions {
        PageOptions {
            title,
            theme,
            style: self.style,
            custom_css: self.css.clone(),
            fixed_theme: self.theme_flag.is_some(),
            ..Default::default()
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    if cli.install_finder || cli.uninstall_finder {
        return finder_integration(cli.install_finder);
    }
    let settings = Settings::new(&cli, Config::load()?)?;
    let target = cli.file.clone().unwrap_or_else(|| PathBuf::from("."));
    let target = target
        .canonicalize()
        .with_context(|| format!("cannot open {}", target.display()))?;

    if target.is_dir() {
        if cli.output.is_some() || cli.pdf.is_some() {
            bail!(
                "{} is a folder; -o and --pdf need a Markdown file",
                target.display()
            );
        }
        return serve(&cli, &settings, target, None);
    }
    if !target.is_file() {
        bail!("{} is not a file", target.display());
    }
    if !cli.watch && (cli.exit_when_closed || cli.port.is_some()) {
        bail!("--port and --exit-when-closed need --watch");
    }
    let markdown = read(&target)?;
    let renderer = Renderer::new();
    let title = server::file_title(&target);
    let dir = target.parent().context("source has no parent directory")?;

    if let Some(output) = &cli.output {
        let page = settings.page(title, settings.export_theme());
        let html = renderer.render_page(&markdown, &page);
        if output.as_os_str() == "-" {
            std::io::stdout().write_all(html.as_bytes())?;
        } else {
            std::fs::write(output, html)
                .with_context(|| format!("cannot write {}", output.display()))?;
        }
        return Ok(());
    }

    if let Some(out) = &cli.pdf {
        // PDFs are always light: they're for printing and sharing.
        let page = PageOptions {
            base_href: Some(dir_url(dir)),
            ..settings.page(title, Theme::Light)
        };
        pdf::print(&renderer.render_page(&markdown, &page), out)?;
        say(&format!("Wrote {}", out.display()));
        return Ok(());
    }

    if cli.watch {
        return serve(&cli, &settings, dir.to_path_buf(), Some(target.clone()));
    }

    // One-shot: write a self-contained page to the temp dir and open it.
    let prefs = Prefs::load();
    let page = PageOptions {
        base_href: Some(dir_url(dir)),
        ui: true,
        sidebar: prefs.sidebar.clone(),
        ..settings.page(title, settings.viewing_theme(&prefs))
    };
    let out = temp_page_path(&target)?;
    std::fs::write(&out, renderer.render_page(&markdown, &page))
        .with_context(|| format!("cannot write {}", out.display()))?;
    say(&out.display().to_string());
    if !cli.no_open {
        browser::open(&file_url(&out), settings.browser.as_deref(), settings.open)?;
    }
    Ok(())
}

/// Serve `root` with live reload. `main` is the file to show at `/`;
/// `None` means folder mode.
fn serve(cli: &Cli, settings: &Settings, root: PathBuf, main: Option<PathBuf>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let renderer = Arc::new(Renderer::new());
        let docs = docs::Docs::new(root.clone(), renderer.clone())?;
        let saved = Prefs::load();
        let prefs = Prefs {
            theme: Some(settings.viewing_theme(&saved)),
            sidebar: saved.sidebar,
        };
        let page = PageOptions {
            served: true,
            live: true,
            ui: true,
            custom_css: None,
            ..settings.page(String::new(), Theme::Auto)
        };
        let server = server::Server::bind(server::ServeOptions {
            port: settings.port,
            docs,
            renderer,
            main: main.clone(),
            page,
            custom_css: settings.css_path.clone(),
            prefs,
            remember: true,
        })
        .await?;
        let url = server.url();
        let handle = server.handle();
        let shown = main.as_ref().unwrap_or(&root);
        say(&format!("Serving {} at {url}", shown.display()));
        say("Watching for changes. Press Ctrl-C to stop.");
        let mut server = tokio::spawn(server.run());
        if !cli.no_open {
            // Set before opening: the browser may load the page before
            // `open` returns. Corrected if we could only open a tab.
            handle.set_close_on_exit(settings.open != browser::Mode::Tab);
            let opened = browser::open(&url, settings.browser.as_deref(), settings.open)?;
            handle.set_close_on_exit(opened == browser::Opened::Window);
        }
        // With --exit-when-closed, stop once the page has been closed for a
        // few seconds (long enough to survive a reload).
        let abandoned = async {
            if cli.exit_when_closed {
                handle
                    .abandoned(Duration::from_secs(5), Duration::from_secs(60))
                    .await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::select! {
            res = &mut server => res?,
            _ = abandoned => {
                say("Page closed; exiting.");
                Ok(())
            }
            _ = tokio::signal::ctrl_c() => {
                // Let pages close their window (or show "stopped") before we go.
                handle.announce_shutdown();
                tokio::time::sleep(Duration::from_millis(300)).await;
                Ok(())
            }
        }
    })
}

#[cfg(target_os = "macos")]
fn finder_integration(install: bool) -> Result<()> {
    if install {
        finder::install()
    } else {
        finder::uninstall()
    }
}

#[cfg(not(target_os = "macos"))]
fn finder_integration(_install: bool) -> Result<()> {
    bail!("Finder integration is only available on macOS")
}

/// Print a status line. Unlike `println!`, this doesn't panic if stdout has
/// been closed (e.g. `markdown-viewer --watch f.md | head -1`), which would
/// otherwise take the server down with it.
fn say(msg: &str) {
    let _ = writeln!(std::io::stdout(), "{msg}");
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

/// Stable per-source path so repeated runs reuse (and overwrite) one file.
fn temp_page_path(source: &Path) -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("markdown-viewer");
    std::fs::create_dir_all(&dir)?;
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "page".into());
    Ok(dir.join(format!("{stem}-{:08x}.html", h.finish() as u32)))
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

/// `file://` URL for a file.
fn file_url(path: &Path) -> String {
    let path = path.to_string_lossy();
    format!("file://{}", utf8_percent_encode(&path, PATH_ENCODE))
}

/// `file://` URL for a directory, with a trailing slash so it works as `<base>`.
fn dir_url(dir: &Path) -> String {
    let mut url = file_url(dir);
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_encodes_and_ends_with_slash() {
        assert_eq!(
            dir_url(Path::new("/tmp/my docs#1")),
            "file:///tmp/my%20docs%231/"
        );
    }

    #[test]
    fn cli_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
