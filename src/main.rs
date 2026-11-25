use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};

mod browser;
mod config;
mod docs;
mod editor;
#[cfg(target_os = "macos")]
mod finder;
mod frontmatter;
mod github;
mod instances;
mod obsidian;
mod pdf;
mod render;
mod server;
mod theme;

use config::{Config, Prefs};
use render::{FrontMatter, PageOptions, RenderOptions, Renderer, Style, Theme};

/// Render Markdown with GitHub styling and view it in your browser.
#[derive(Debug, Parser)]
#[command(name = "markdown-viewer", version, about)]
struct Cli {
    /// Markdown file to render, a folder to browse (default: the current
    /// folder), or "-" to read standard input. Folders are always served
    /// with live reload.
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

    /// Preview the clipboard instead of a file. With --watch, the page
    /// follows the clipboard as it changes.
    #[arg(long, conflicts_with = "file")]
    clipboard: bool,

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

    /// Number headings (1, 1.1, 1.2, …).
    #[arg(long)]
    number_headings: bool,

    /// YAML front matter: shown as a table, like GitHub (default), or hidden.
    #[arg(long, value_enum, value_name = "HOW")]
    front_matter: Option<FrontMatter>,

    /// Obsidian notes: [[wiki links]], ![[embeds]], callouts, ==highlights==
    /// and %%comments%%. On by default inside a vault (a .obsidian folder).
    #[arg(long, overrides_with = "no_obsidian")]
    obsidian: bool,

    /// Turn off Obsidian notes, even inside a vault.
    #[arg(long, overrides_with = "obsidian")]
    no_obsidian: bool,

    /// Editor for double-click-to-source, e.g. "zed" or
    /// "code -g {file}:{line}" (default: $VISUAL/$EDITOR if it's a GUI
    /// editor, else the first one found).
    #[arg(long, value_name = "COMMAND")]
    editor: Option<String>,

    /// Print a shell completion script (bash, zsh, fish, elvish, powershell).
    #[arg(long, value_enum, value_name = "SHELL", exclusive = true, hide = true)]
    generate_completions: Option<clap_complete::Shell>,

    /// Print the man page.
    #[arg(long, exclusive = true, hide = true)]
    generate_man: bool,
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
    number_headings: bool,
    front_matter: FrontMatter,
    /// `None`: decide by whether the file is in an Obsidian vault.
    obsidian: Option<bool>,
    editor: Option<String>,
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
        let obsidian = if cli.obsidian {
            Some(true)
        } else if cli.no_obsidian {
            Some(false)
        } else {
            config.obsidian
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
            number_headings: cli.number_headings || config.number_headings.unwrap_or(false),
            front_matter: cli.front_matter.or(config.front_matter).unwrap_or_default(),
            obsidian,
            editor: cli.editor.clone().or(config.editor),
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

    fn render_options(&self, sourcepos: bool, at: &Path) -> RenderOptions {
        RenderOptions {
            sourcepos,
            obsidian: self.obsidian.unwrap_or_else(|| obsidian::in_vault(at)),
            front_matter: self.front_matter,
        }
    }

    fn page(&self, title: String, theme: Theme) -> PageOptions {
        PageOptions {
            title,
            theme,
            style: self.style,
            custom_css: self.css.clone(),
            fixed_theme: self.theme_flag.is_some(),
            number_headings: self.number_headings,
            ..Default::default()
        }
    }
}

/// What to show.
enum Source {
    File(PathBuf),
    Folder(PathBuf),
    /// Text with no file behind it: standard input or the clipboard.
    Text {
        name: &'static str,
        text: String,
    },
}

fn run(cli: Cli) -> Result<()> {
    if let Some(shell) = cli.generate_completions {
        clap_complete::generate(
            shell,
            &mut docs_command(),
            "markdown-viewer",
            &mut std::io::stdout(),
        );
        return Ok(());
    }
    if cli.generate_man {
        clap_mangen::Man::new(docs_command()).render(&mut std::io::stdout())?;
        return Ok(());
    }
    if cli.install_finder || cli.uninstall_finder {
        return finder_integration(cli.install_finder);
    }
    let settings = Settings::new(&cli, Config::load()?)?;
    let cwd = std::env::current_dir()?.canonicalize()?;

    let source = if cli.clipboard {
        Source::Text {
            name: "Clipboard",
            text: read_clipboard()?,
        }
    } else {
        match &cli.file {
            Some(p) if p.as_os_str() == "-" => {
                let mut text = String::new();
                std::io::stdin()
                    .read_to_string(&mut text)
                    .context("cannot read standard input")?;
                Source::Text {
                    name: "stdin",
                    text,
                }
            }
            Some(p) => {
                let path = p
                    .canonicalize()
                    .with_context(|| format!("cannot open {}", p.display()))?;
                if path.is_dir() {
                    Source::Folder(path)
                } else if path.is_file() {
                    Source::File(path)
                } else {
                    bail!("{} is not a file", p.display());
                }
            }
            None => Source::Folder(cwd.clone()),
        }
    };

    let (markdown, dir, title) = match &source {
        Source::Folder(path) => {
            if cli.output.is_some() || cli.pdf.is_some() {
                bail!(
                    "{} is a folder; -o and --pdf need a Markdown file",
                    path.display()
                );
            }
            return serve(&cli, &settings, &source);
        }
        Source::File(path) => (
            read(path)?,
            path.parent()
                .context("file has no parent folder")?
                .to_path_buf(),
            server::file_title(path),
        ),
        Source::Text { name, text } => (text.clone(), cwd.clone(), name.to_string()),
    };
    if !cli.watch && (cli.exit_when_closed || cli.port.is_some()) {
        bail!("--port and --exit-when-closed need --watch");
    }

    let (renderer, body) = render_standalone(&settings, &markdown, &dir);

    if let Some(output) = &cli.output {
        let page = settings.page(title, settings.export_theme());
        let html = renderer.wrap_page(&body, &page);
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
            base_href: Some(dir_url(&dir)),
            ..settings.page(title, Theme::Light)
        };
        pdf::print(&renderer.wrap_page(&body, &page), out)?;
        say(&format!("Wrote {}", out.display()));
        return Ok(());
    }

    if cli.watch {
        return serve(&cli, &settings, &source);
    }

    // One-shot: write a self-contained page to the temp dir and open it.
    let prefs = Prefs::load();
    let page = PageOptions {
        base_href: Some(dir_url(&dir)),
        ui: true,
        sidebar: prefs.sidebar.clone(),
        ..settings.page(title.clone(), settings.viewing_theme(&prefs))
    };
    let key = match &source {
        Source::File(path) => path.clone(),
        _ => dir.join(&title),
    };
    let out = temp_page_path(&key)?;
    std::fs::write(&out, renderer.wrap_page(&body, &page))
        .with_context(|| format!("cannot write {}", out.display()))?;
    say(&out.display().to_string());
    if !cli.no_open {
        browser::open(&file_url(&out), settings.browser.as_deref(), settings.open)?;
    }
    Ok(())
}

/// The command as documented in the completions and man page: the same on
/// every platform (the macOS-only flags are hidden from `--help` elsewhere,
/// but the generated files ship everywhere and say "macOS:").
fn docs_command() -> clap::Command {
    Cli::command()
        .mut_arg("install_finder", |a| a.hide(false))
        .mut_arg("uninstall_finder", |a| a.hide(false))
}

/// Render for a page that isn't served: Obsidian links (if on) resolve
/// relative to the document's folder.
fn render_standalone(settings: &Settings, markdown: &str, dir: &Path) -> (Renderer, String) {
    let options = settings.render_options(false, dir);
    let renderer = Renderer::with_options(options.clone());
    let body = if options.obsidian {
        let root = obsidian::vault_root(dir).unwrap_or_else(|| dir.to_path_buf());
        let (files, _) = docs::markdown_files(&root);
        let ctx = obsidian::LinkContext {
            root: &root,
            dir,
            files: &files,
            served: false,
        };
        renderer.render_doc(markdown, Some(&ctx))
    } else {
        renderer.render_body(markdown)
    };
    (renderer, body)
}

/// Serve with live reload.
fn serve(cli: &Cli, settings: &Settings, source: &Source) -> Result<()> {
    // What we serve from, what `/` shows, and who we are (for reuse).
    let cwd = std::env::current_dir()?.canonicalize()?;
    let (root, main, identity) = match source {
        Source::Folder(dir) => (dir.clone(), None, Some(dir.clone())),
        Source::File(file) => {
            let parent = file.parent().context("file has no parent folder")?;
            let options = settings.render_options(true, parent);
            // In a vault, serve the whole vault so linked notes open.
            let root = options
                .obsidian
                .then(|| obsidian::vault_root(parent))
                .flatten()
                .unwrap_or_else(|| parent.to_path_buf());
            (root, Some(file.clone()), Some(file.clone()))
        }
        Source::Text { name, .. } => (cwd.clone(), Some(cwd.join(format!("({name})"))), None),
    };

    // Already being served? Reuse that viewer (and bring its window
    // forward) instead of starting another.
    if let Some(target) = &identity
        && let Some(url) = instances::find(target)
    {
        if !cli.no_open && !browser::focus(&url, settings.browser.as_deref()) {
            browser::open(&url, settings.browser.as_deref(), settings.open)?;
        }
        say(&format!("Already open at {url}"));
        return Ok(());
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let renderer = Arc::new(Renderer::with_options(settings.render_options(true, &root)));
        let docs = docs::Docs::new(root.clone(), renderer.clone())?;
        if let (Source::Text { text, .. }, Some(key)) = (source, &main) {
            docs.add_virtual(key, docs.render_text(text, &root));
            if cli.clipboard && cli.watch {
                follow_clipboard(docs.clone(), key.clone(), root.clone(), text.clone());
            }
        }
        let saved = Prefs::load();
        let prefs = Prefs {
            theme: Some(settings.viewing_theme(&saved)),
            sidebar: saved.sidebar,
            positions: saved.positions,
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
            editor: settings.editor.clone(),
            identity: identity
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
        })
        .await?;
        let url = server.url();
        let port = url
            .trim_end_matches('/')
            .rsplit(':')
            .next()
            .and_then(|p| p.parse().ok())
            .unwrap_or(0);
        let _registration = identity.as_ref().and_then(|t| instances::register(t, port));
        let handle = server.handle();
        let shown = match source {
            Source::Text { name, .. } => name.to_string(),
            _ => identity
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        };
        say(&format!("Serving {shown} at {url}"));
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

/// Poll the clipboard and update the page when its text changes.
fn follow_clipboard(docs: Arc<docs::Docs>, key: PathBuf, root: PathBuf, mut last: String) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let Ok(Ok(text)) = tokio::task::spawn_blocking(read_clipboard).await else {
                continue;
            };
            if text != last && !text.trim().is_empty() {
                let html = docs.render_text(&text, &root);
                docs.set_virtual(&key, html);
                last = text;
            }
        }
    });
}

/// The clipboard's text: pbpaste on macOS; wl-paste, xclip or xsel on Linux.
fn read_clipboard() -> Result<String> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbpaste", &[])]
    } else {
        &[
            ("wl-paste", &["--no-newline"]),
            ("xclip", &["-selection", "clipboard", "-o"]),
            ("xsel", &["--clipboard", "--output"]),
        ]
    };
    for (program, args) in candidates {
        if let Ok(out) = std::process::Command::new(program).args(*args).output()
            && out.status.success()
        {
            return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
        }
    }
    bail!("couldn't read the clipboard (needs pbpaste, wl-paste, xclip or xsel)")
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
    fn generates_completions_and_man_page() {
        for shell in [
            clap_complete::Shell::Bash,
            clap_complete::Shell::Zsh,
            clap_complete::Shell::Fish,
        ] {
            let mut out = Vec::new();
            clap_complete::generate(shell, &mut docs_command(), "markdown-viewer", &mut out);
            let text = String::from_utf8(out).unwrap();
            assert!(text.contains("number-headings"), "{shell:?}");
        }
        let mut man = Vec::new();
        clap_mangen::Man::new(docs_command())
            .render(&mut man)
            .unwrap();
        assert!(
            String::from_utf8(man)
                .unwrap()
                .contains(".TH markdown-viewer")
        );
    }

    #[test]
    fn cli_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
