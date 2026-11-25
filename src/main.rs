use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::Parser;
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use tokio::sync::watch;

mod browser;
mod render;
mod server;
mod watcher;

use render::{PageOptions, Renderer, Theme};

/// Render a Markdown file with GitHub styling and view it in your browser.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Markdown file to render.
    file: PathBuf,

    /// Serve the page and live-reload it whenever the file changes.
    #[arg(short, long)]
    watch: bool,

    /// Write the rendered HTML to this file ("-" for stdout) instead of opening it.
    #[arg(short, long, value_name = "PATH", conflicts_with = "watch")]
    output: Option<PathBuf>,

    /// Port for --watch (default: any free port).
    #[arg(short, long, default_value_t = 0, requires = "watch")]
    port: u16,

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

    /// Colour theme.
    #[arg(long, value_enum, default_value_t = Theme::Auto)]
    theme: Theme,
}

fn main() {
    if let Err(err) = run(Cli::parse()) {
        eprintln!("markdown-viewer: {err:#}");
        std::process::exit(1);
    }
}

impl Cli {
    fn open_mode(&self) -> browser::Mode {
        if self.tab {
            browser::Mode::Tab
        } else if self.window {
            browser::Mode::Window
        } else {
            browser::Mode::App
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let source = cli
        .file
        .canonicalize()
        .with_context(|| format!("cannot open {}", cli.file.display()))?;
    if !source.is_file() {
        bail!("{} is not a file", cli.file.display());
    }
    let markdown = read(&source)?;
    let renderer = Arc::new(Renderer::new());
    let page = PageOptions {
        title: server::file_title(&source),
        theme: cli.theme,
        ..Default::default()
    };

    if let Some(output) = &cli.output {
        let html = renderer.render_page(&markdown, &page);
        if output.as_os_str() == "-" {
            std::io::stdout().write_all(html.as_bytes())?;
        } else {
            std::fs::write(output, html)
                .with_context(|| format!("cannot write {}", output.display()))?;
        }
        return Ok(());
    }

    if cli.watch {
        return serve(cli, source, &markdown, renderer, page);
    }

    // One-shot: write a self-contained page to the temp dir and open it.
    let dir = source.parent().context("source has no parent directory")?;
    let page = PageOptions {
        base_href: Some(dir_url(dir)),
        ..page
    };
    let out = temp_page_path(&source)?;
    std::fs::write(&out, renderer.render_page(&markdown, &page))
        .with_context(|| format!("cannot write {}", out.display()))?;
    say(&out.display().to_string());
    if !cli.no_open {
        browser::open(&file_url(&out), cli.browser.as_deref(), cli.open_mode())?;
    }
    Ok(())
}

fn serve(
    cli: Cli,
    source: PathBuf,
    markdown: &str,
    renderer: Arc<Renderer>,
    page: PageOptions,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let (tx, rx) = watch::channel(renderer.render_body(markdown));
        let _watcher = watcher::watch_file(&source, renderer.clone(), tx)?;
        let root = source.parent().context("source has no parent directory")?;
        let page = PageOptions {
            served: true,
            live: true,
            ..page
        };
        let server = server::Server::bind(cli.port, root, renderer, page, rx).await?;
        let url = server.url();
        let handle = server.handle();
        say(&format!("Serving {} at {url}", source.display()));
        say("Watching for changes. Press Ctrl-C to stop.");
        let mut server = tokio::spawn(server.run());
        if !cli.no_open {
            // Set before opening: the browser may load the page before
            // `open` returns. Corrected if we could only open a tab.
            let mode = cli.open_mode();
            handle.set_close_on_exit(mode != browser::Mode::Tab);
            let opened = browser::open(&url, cli.browser.as_deref(), mode)?;
            handle.set_close_on_exit(opened == browser::Opened::Window);
        }
        tokio::select! {
            res = &mut server => res?,
            _ = tokio::signal::ctrl_c() => {
                // Let pages close their window (or show "stopped") before we go.
                handle.announce_shutdown();
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                Ok(())
            }
        }
    })
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
