use crate::platform::Canonical;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser};

mod abbr;
mod about;
mod asciimath;
mod blame;
mod browser;
mod calendar;
mod chess;
mod codeblocks;
mod config;
mod desktop;
mod diagrams;
mod diffview;
mod docs;
mod editor;
mod embeds;
#[cfg(target_os = "macos")]
mod finder;
mod frontmatter;
mod geo;
mod geoformats;
mod gitdiff;
mod github;
mod instances;
mod libraries;
mod links;
mod lint;
mod modelformats;
mod molecules;
mod obsidian;
mod pagemeta;
mod pdf;
mod platform;
mod recent;
mod references;
mod remote;
mod render;
mod server;
mod site;
mod source;
mod stl;
mod tables;
mod tags;
mod tasks;
mod theme;
mod toc;
mod wavedrom;

use config::{Config, Prefs};
use render::{FrontMatter, PageOptions, RenderOptions, Renderer, Style, Theme};

/// Render Markdown with GitHub styling and view it in your browser.
#[derive(Debug, Clone, Parser)]
#[command(name = "markdown-viewer", version, long_version = about::long_version(), about)]
struct Cli {
    /// Markdown file to render, a folder to browse (default: the current
    /// folder), a web address (https://…, including GitHub and GitLab file
    /// and repository pages), or "-" to read standard input. Folders are always served
    /// with live reload.
    file: Option<PathBuf>,

    /// Serve the page and live-reload it whenever the file changes.
    #[arg(short, long)]
    watch: bool,

    /// Write the rendered HTML to this file ("-" for stdout) instead of
    /// opening it. For a folder, write a static site into this folder.
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

    /// Scroll a running viewer to FILE:LINE, starting one if needed. For
    /// editor commands (see editors/ in the repository).
    #[arg(long, value_name = "FILE:LINE", conflicts_with_all = ["file", "clipboard", "output", "pdf"])]
    goto: Option<String>,

    /// With --goto: do nothing if no viewer is showing the file (for
    /// editors following the cursor, so a closed preview stays closed).
    #[arg(long, requires = "goto")]
    if_running: bool,

    /// Check links and images in FILE (or every Markdown file in a folder)
    /// and report broken ones; exits with status 1 if any are.
    #[arg(long, conflicts_with_all = ["watch", "output", "pdf", "clipboard", "goto"])]
    check_links: bool,

    /// Show FILE with what changed since a git revision marked: added text
    /// in green, removed text struck through in red (e.g. --diff HEAD for
    /// uncommitted changes, --diff HEAD~3, --diff main).
    #[arg(long, value_name = "REV", conflicts_with_all = ["clipboard", "goto", "check_links", "lint", "recent"])]
    diff: Option<String>,

    /// Show the files and folders you opened recently (also what you get
    /// with no FILE in your home folder).
    #[arg(long, conflicts_with_all = ["file", "clipboard", "goto", "output", "pdf", "check_links", "lint"])]
    recent: bool,

    /// With --check-links: also check web links (needs curl).
    #[arg(long, requires = "check_links")]
    online: bool,

    /// Report writing problems in FILE (or every Markdown file in a
    /// folder): skipped heading levels, images without alt text, empty
    /// links and more; exits with status 1 if there are any. Combines with
    /// --check-links.
    #[arg(long, conflicts_with_all = ["watch", "output", "pdf", "clipboard", "goto"])]
    lint: bool,

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

    /// With --watch: exit once the page has been closed, even with
    /// --no-open (used by the Finder integration). Without --no-open, this
    /// is the default.
    #[arg(long, conflicts_with = "keep_running")]
    exit_when_closed: bool,

    /// With --watch: keep serving after the window is closed (until
    /// Ctrl-C).
    #[arg(long)]
    keep_running: bool,

    /// Internal: this is the background viewer started for a page opened
    /// without --watch (see `launch_in_background`).
    #[arg(long, hide = true)]
    background: bool,

    /// macOS: add "Markdown Viewer" to Finder (Open With, double-click and a
    /// right-click Quick Action) for the current user.
    #[arg(long, exclusive = true, hide = !cfg!(target_os = "macos"))]
    install_finder: bool,

    /// macOS: remove the Finder integration.
    #[arg(long, exclusive = true, hide = !cfg!(target_os = "macos"))]
    uninstall_finder: bool,

    /// Linux: add "Markdown Viewer" to your desktop's applications, for
    /// Open With on Markdown, CSV and TSV files (current user only).
    #[arg(long, exclusive = true, hide = cfg!(any(target_os = "macos", windows)))]
    install_desktop: bool,

    /// Linux: remove the desktop integration.
    #[arg(long, exclusive = true, hide = cfg!(any(target_os = "macos", windows)))]
    uninstall_desktop: bool,

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

    /// Number the lines of every code block (or per block, with
    /// `showLineNumbers` in its info string).
    #[arg(long)]
    line_numbers: bool,

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
    /// Show the word count in the toolbar, unless the viewer remembers
    /// otherwise.
    word_count: bool,
    /// Stop serving when the window we opened closes.
    exit_when_closed: bool,
    /// Highlight the source view.
    source_highlighting: bool,
    last_updated: bool,
    line_numbers: bool,
    front_matter: FrontMatter,
    /// `None`: decide by whether the file is in an Obsidian vault.
    obsidian: Option<bool>,
    editor: Option<String>,
    github_references: bool,
    /// The repository for references, from the settings file.
    github_repo: Option<String>,
    lint_ignore: Vec<String>,
    /// Remember opened files for the Recent page.
    recent: bool,
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
            word_count: config.word_count.unwrap_or(true),
            exit_when_closed: config.exit_when_closed.unwrap_or(true),
            source_highlighting: config.source_highlighting.unwrap_or(true),
            last_updated: config.last_updated.unwrap_or(true),
            line_numbers: cli.line_numbers || config.line_numbers.unwrap_or(false),
            front_matter: cli.front_matter.or(config.front_matter).unwrap_or_default(),
            obsidian,
            editor: cli.editor.clone().or(config.editor),
            github_references: config.github_references.unwrap_or(true),
            github_repo: config
                .github_repo
                .as_deref()
                .and_then(references::from_setting),
            lint_ignore: config.lint_ignore.unwrap_or_default(),
            recent: config.recent.unwrap_or(true),
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
            line_numbers: self.line_numbers,
            github: if self.github_references {
                self.github_repo.clone().or_else(|| references::detect(at))
            } else {
                None
            },
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
            hide_word_count: !self.word_count,
            recent: self.recent,
            last_updated: self.last_updated,
            github_repo: self.github_repo.clone(),
            ..Default::default()
        }
    }
}

/// What to show.
enum Source {
    File(PathBuf),
    Folder(PathBuf),
    /// Text with no file behind it: standard input, the clipboard or the
    /// Recent page, served from `root` (relative links resolve there).
    Text {
        name: &'static str,
        text: String,
        root: PathBuf,
    },
    /// A file compared with an earlier git revision (`--diff`).
    Diff {
        file: PathBuf,
        rev: String,
    },
    /// Markdown from the web, served from an empty folder of its own.
    Url {
        remote: Box<remote::Remote>,
        text: String,
        root: PathBuf,
    },
}

/// An empty folder to serve web documents from, so nothing on disk is
/// reachable from their pages.
fn web_root() -> Result<PathBuf> {
    let dir = std::env::temp_dir().join("markdown-viewer-web");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.canonical()?)
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
    if cli.install_desktop {
        return desktop::install();
    }
    if cli.uninstall_desktop {
        return desktop::uninstall();
    }
    let settings = Settings::new(&cli, Config::load()?)?;
    let cwd = std::env::current_dir()?.canonical()?;

    // --goto: scroll a viewer that's already showing the file, else start
    // one (below) opened at that line.
    let goto = cli.goto.as_deref().map(parse_goto).transpose()?;
    if let Some((file, line)) = &goto
        && let Some(url) = instances::find_serving(file)
    {
        if !instances::send_goto(&url, file, *line) {
            bail!("couldn't reach the viewer at {url}");
        }
        say(&format!("Showing {}:{line} at {url}", file.display()));
        return Ok(());
    }
    if goto.is_some() && cli.if_running {
        return Ok(());
    }
    // The Recent page: asked for, or no FILE in the home folder (browsing
    // all of it is rarely what you want). Its links go through the server,
    // so it's always served.
    let home = platform::home().and_then(|h| h.canonical().ok());
    let show_recent = cli.recent
        || (cli.file.is_none()
            && !cli.clipboard
            && goto.is_none()
            && settings.recent
            && home.as_ref() == Some(&cwd));
    let watch = cli.watch || goto.is_some() || show_recent;

    if cli.check_links || cli.lint {
        let target = cli.file.clone().unwrap_or_else(|| cwd.clone());
        return check(&settings, &target, cli.check_links, cli.online, cli.lint);
    }

    let source = if let Some((file, _)) = &goto {
        Source::File(file.clone())
    } else if cli.clipboard {
        Source::Text {
            name: "Clipboard",
            text: read_clipboard()?,
            root: cwd.clone(),
        }
    } else if show_recent {
        let home = home.context("cannot find your home folder")?;
        Source::Text {
            name: "Recent",
            text: recent::page(&Prefs::load().recent, &home),
            root: home,
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
                    root: cwd.clone(),
                }
            }
            Some(p) if remote::is_url(&p.to_string_lossy()) => {
                let remote = Box::new(remote::Remote::new(&p.to_string_lossy())?);
                let text = remote.fetch()?;
                Source::Url {
                    remote,
                    text,
                    root: web_root()?,
                }
            }
            Some(p) => {
                let path = p
                    .canonical()
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

    let source = match (source, &cli.diff) {
        (Source::File(file), Some(rev)) => Source::Diff {
            file,
            rev: rev.clone(),
        },
        (_, Some(_)) => bail!("--diff needs a Markdown file in a git repository"),
        (source, None) => source,
    };

    // Remember what was opened (not exported) for the Recent page.
    if settings.recent
        && cli.output.is_none()
        && cli.pdf.is_none()
        && let Source::File(path) | Source::Folder(path) = &source
    {
        recent::record(path);
    }

    let (markdown, dir, title) = match &source {
        Source::Folder(path) => {
            if let Some(out) = &cli.output {
                if out.as_os_str() == "-" {
                    bail!("{} is a folder; -o - needs a Markdown file", path.display());
                }
                return export_site(&settings, path, out);
            }
            if cli.pdf.is_some() {
                bail!(
                    "{} is a folder; --pdf needs a Markdown file",
                    path.display()
                );
            }
            return serve(&cli, &settings, &source, None);
        }
        Source::File(path) => (
            read(path)?,
            path.parent()
                .context("file has no parent folder")?
                .to_path_buf(),
            server::file_title(path),
        ),
        Source::Text { name, text, root } => (text.clone(), root.clone(), name.to_string()),
        Source::Diff { file, rev } => (
            read(file)?,
            file.parent()
                .context("file has no parent folder")?
                .to_path_buf(),
            format!("{} (since {rev})", server::file_title(file)),
        ),
        Source::Url { remote, text, root } => (text.clone(), root.clone(), remote.title()),
    };
    // Without --watch, a page is served in the background (which always
    // stops when its window closes), or with --no-open written to a file.
    if !watch && (cli.exit_when_closed || cli.keep_running) {
        bail!("--exit-when-closed and --keep-running need --watch");
    }
    if !watch && cli.no_open && cli.port.is_some() {
        bail!("--port needs --watch, or a page to open");
    }

    // Served and background viewers render the page themselves: start
    // them before rendering anything here, so the window opens sooner.
    if cli.output.is_none() && cli.pdf.is_none() {
        if watch {
            return serve(&cli, &settings, &source, goto.map(|(_, line)| line));
        }
        // Opening a page: a viewer serves it in the background (so it has
        // everything a served page has: settings, live reload…) and stops
        // when the window is closed. We return once it's serving.
        if !cli.no_open && !cli.background {
            let stdin_text = match &source {
                Source::Text {
                    name: "stdin",
                    text,
                    ..
                } => Some(text.as_str()),
                _ => None,
            };
            return launch_in_background(stdin_text);
        }
    }

    let (renderer, mut body) = render_standalone(&settings, &markdown, &dir);
    if let Source::File(path) = &source
        && let Some(delim) = tables::delimiter(path)
    {
        body = tables::to_html(&markdown, delim);
    }
    if let Source::Url { remote, .. } = &source {
        body = remote::absolutize(&body, remote);
    }
    if let Source::Diff { file, rev } = &source {
        let old = diffview::old_text(file, rev)?;
        body = diffview::render(renderer.options(), file, &old, &markdown, rev);
    }

    if let Some(output) = &cli.output {
        let page = with_meta(settings.page(title, settings.export_theme()), &source);
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
            ..with_meta(settings.page(title, Theme::Light), &source)
        };
        pdf::print(&renderer.wrap_page(&body, &page), out)?;
        say(&format!("Wrote {}", out.display()));
        return Ok(());
    }

    // --no-open: write a self-contained page to the temp dir.
    let prefs = Prefs::load();
    let page = PageOptions {
        base_href: Some(dir_url(&dir)),
        ui: true,
        sidebar: prefs.sidebar.clone(),
        hide_word_count: !prefs.word_count.unwrap_or(settings.word_count),
        ..with_meta(
            settings.page(title.clone(), settings.viewing_theme(&prefs)),
            &source,
        )
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
        browser::open(
            &platform::file_url(&out),
            settings.browser.as_deref(),
            settings.open,
        )?;
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
        // The man page shouldn't change with every commit.
        .long_version(None::<&str>)
        .mut_arg("install_desktop", |a| a.hide(false))
        .mut_arg("uninstall_desktop", |a| a.hide(false))
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
            depth: 0,
        };
        renderer.render_doc(markdown, Some(&ctx))
    } else {
        renderer.render_in(markdown, Some(dir), None)
    };
    (renderer, body)
}

/// `-o DIR` for a folder: a static site.
fn export_site(settings: &Settings, root: &Path, out: &Path) -> Result<()> {
    let options = settings.render_options(false, root);
    let renderer = Renderer::with_options(options.clone());
    let page = |title: String| settings.page(title, settings.export_theme());
    let summary = site::export(&renderer, options.obsidian, root, out, &page)?;
    if summary.truncated {
        say("Only the first 2,000 Markdown files were exported.");
    }
    let files = match summary.files {
        0 => String::new(),
        1 => " and 1 file".into(),
        n => format!(" and {n} files"),
    };
    say(&format!(
        "Wrote {} pages{files} to {} (start at {})",
        summary.pages,
        out.display(),
        out.join("index.html").display()
    ));
    Ok(())
}

/// `path/to/file.md:42` → (canonical file, 42). Without a line, line 1.
fn parse_goto(spec: &str) -> Result<(PathBuf, u32)> {
    let (path, line) = match spec.rsplit_once(':') {
        Some((p, l)) if !p.is_empty() && l.chars().all(|c| c.is_ascii_digit()) && !l.is_empty() => {
            (p, l.parse().unwrap_or(1))
        }
        _ => (spec, 1),
    };
    let file = Path::new(path)
        .canonical()
        .with_context(|| format!("cannot open {path}"))?;
    if !file.is_file() {
        bail!("{path} is not a file");
    }
    Ok((file, line.max(1)))
}

/// --check-links: report broken links; fail if there are any.
/// `--check-links` and/or `--lint`: report problems as `file:line: what
/// (why)`, and exit with status 1 if there are any.
fn check(settings: &Settings, target: &Path, links: bool, online: bool, lint: bool) -> Result<()> {
    let target = target
        .canonical()
        .with_context(|| format!("cannot open {}", target.display()))?;
    let (root, files) = if target.is_dir() {
        let (files, _) = docs::markdown_files(&target);
        let files = files.iter().map(|f| target.join(f)).collect();
        (target.clone(), files)
    } else {
        let parent = target.parent().context("file has no parent folder")?;
        let root = obsidian::vault_root(parent).unwrap_or_else(|| parent.to_path_buf());
        (root, vec![target.clone()])
    };
    let renderer = Arc::new(Renderer::with_options(settings.render_options(true, &root)));
    let checker = links::Checker::new(renderer, root.clone());
    // Lint without Obsidian embeds (other files' problems) or GitHub links.
    let lint_renderer = Renderer::with_options(RenderOptions {
        obsidian: false,
        github: None,
        ..settings.render_options(true, &root)
    });
    let shown = |f: &Path| -> String {
        let base = if target.is_dir() { &target } else { &root };
        f.strip_prefix(base)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| f.display().to_string())
    };

    let mut problems: Vec<(String, u32, String, String)> = Vec::new();
    let mut web: Vec<(String, links::Link)> = Vec::new();
    let mut total = 0;
    let mut lint_count = 0;
    for file in &files {
        if lint {
            let text = read(file)?;
            for p in lint::check(
                &text,
                &lint_renderer.render_body(&text),
                &settings.lint_ignore,
            ) {
                lint_count += 1;
                problems.push((shown(file), p.line, p.message, p.rule.to_string()));
            }
        }
        if !links {
            continue;
        }
        let (broken, external, count) = checker.check(file);
        total += count;
        for b in broken {
            problems.push((shown(file), b.line, b.target, b.reason));
        }
        web.extend(external.into_iter().map(|l| (shown(file), l)));
    }
    if online && !web.is_empty() {
        let urls = web.iter().map(|(_, l)| l.target.clone()).collect();
        let failures = links::check_online(&urls).map_err(anyhow::Error::msg)?;
        for (file, link) in &web {
            if let Some(reason) = failures.get(&link.target) {
                problems.push((file.clone(), link.line, link.target.clone(), reason.clone()));
            }
        }
    }
    problems.sort();
    for (file, line, target, reason) in &problems {
        say(&format!("{file}:{line}: {target} ({reason})"));
    }
    let not_checked = if online || web.is_empty() {
        String::new()
    } else {
        format!(" {} web links not checked (add --online).", web.len())
    };
    let files_word = if files.len() == 1 { "file" } else { "files" };
    if links {
        say(&format!(
            "Checked {total} links in {} {files_word}: {} broken.{not_checked}",
            files.len(),
            problems.len() - lint_count
        ));
    }
    if lint {
        let word = if lint_count == 1 {
            "problem"
        } else {
            "problems"
        };
        say(&format!(
            "Linted {} {files_word}: {lint_count} {word}.",
            files.len()
        ));
    }
    if !problems.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// Serve with live reload. `start_line` opens the page at that line.
fn serve(cli: &Cli, settings: &Settings, source: &Source, start_line: Option<u32>) -> Result<()> {
    // What we serve from, what `/` shows, and who we are (for reuse).
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
        Source::Text { name, root, .. } => {
            (root.clone(), Some(root.join(format!("({name})"))), None)
        }
        // Served from the file's folder (so images load); a second launch
        // for the same file and revision reuses this viewer.
        Source::Diff { file, rev } => {
            let parent = file.parent().context("file has no parent folder")?;
            let name = server::file_title(file);
            (
                parent.to_path_buf(),
                Some(parent.join(format!("({name} since {rev})"))),
                Some(PathBuf::from(format!("{}#diff={rev}", file.display()))),
            )
        }
        // A second launch for the same address reuses this viewer.
        Source::Url { remote, root, .. } => (
            root.clone(),
            Some(root.join(format!("({})", remote.title()))),
            Some(PathBuf::from(&remote.url)),
        ),
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
        let docs = docs::Docs::new(root.clone(), renderer)?;
        // Render the first page while the browser starts, so it's ready when
        // asked for (pages are otherwise rendered on their first request).
        if matches!(source, Source::File(_) | Source::Folder(_)) {
            let first = main
                .clone()
                .unwrap_or_else(|| docs::readme(&root).unwrap_or_else(|| root.clone()));
            let docs = docs.clone();
            tokio::task::spawn_blocking(move || {
                let _ = docs.subscribe(&first);
            });
        }
        if let (Source::Text { name, text, .. }, Some(key)) = (source, &main) {
            // The Recent page is generated; it has no source worth showing.
            if *name != "Recent" {
                docs.set_source(key, text);
            }
            docs.add_virtual(key, docs.render_text(text, &root));
            if cli.clipboard && cli.watch && !cli.background {
                follow_clipboard(docs.clone(), key.clone(), root.clone(), text.clone());
            }
            if *name == "Recent" {
                follow_recent(docs.clone(), key.clone(), root.clone(), text.clone());
            }
        }
        if let (Source::Diff { file, rev }, Some(key)) = (source, &main) {
            let old = diffview::old_text(file, rev)?;
            let new = read(file)?;
            docs.add_virtual(
                key,
                diffview::render(docs.renderer().options(), file, &old, &new, rev),
            );
            follow_diff(docs.clone(), key.clone(), file.clone(), rev.clone());
        }
        if let (Source::Url { remote, text, .. }, Some(key)) = (source, &main) {
            docs.set_source(key, text);
            docs.add_virtual(
                key,
                remote::absolutize(&docs.render_text(text, &root), remote),
            );
            follow_url(
                docs.clone(),
                key.clone(),
                root.clone(),
                (**remote).clone(),
                text.clone(),
            );
        }
        let saved = Prefs::load();
        let prefs = Prefs {
            theme: Some(settings.viewing_theme(&saved)),
            ..saved
        };
        // Load the source view's syntax definitions in the background.
        if settings.source_highlighting {
            std::thread::spawn(source::warm_up);
        }
        let page = served_page(settings);
        // The settings dialog: a changed settings file, merged with this
        // run's command-line options exactly as at startup.
        // Obsidian mode stays as it started: it chose the served root (the
        // vault) and the watches, which can't change while running.
        let obsidian_now = settings.render_options(true, &root).obsidian;
        let reconfigure: server::Reconfigure = {
            let cli = cli.clone();
            let root = root.clone();
            Arc::new(move |config: &Config| {
                let settings = Settings::new(&cli, config.clone())?;
                Ok(server::Reconfigured {
                    render: RenderOptions {
                        obsidian: obsidian_now,
                        ..settings.render_options(true, &root)
                    },
                    page: served_page(&settings),
                    custom_css: settings.css_path.clone(),
                    editor: settings.editor.clone(),
                    source_highlighting: settings.source_highlighting,
                })
            })
        };
        let server = server::Server::bind(server::ServeOptions {
            port: settings.port,
            docs,
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
            reconfigure: Some(reconfigure),
            overridden: overridden_settings(cli),
            source_highlighting: settings.source_highlighting,
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
            Source::Url { remote, .. } => remote.url.clone(),
            Source::Diff { file, rev } => format!("{} (changes since {rev})", file.display()),
            _ => identity
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        };
        say(&format!("Serving {shown} at {url}"));
        let exit = exit_policy(cli, settings);
        say(match exit {
            Exit::Never => "Watching for changes. Press Ctrl-C to stop.",
            _ if cli.no_open => {
                "Watching for changes. Stops when the page is closed, or press Ctrl-C."
            }
            _ => "Watching for changes. Stops when the window is closed, or press Ctrl-C.",
        });
        let mut server = tokio::spawn(server.run());
        if !cli.no_open {
            // Set before opening: the browser may load the page before
            // `open` returns. Corrected if we could only open a tab.
            handle.set_close_on_exit(settings.open != browser::Mode::Tab);
            let page_url = match start_line {
                Some(line) => format!("{url}#mdv-line-{line}"),
                None => url.clone(),
            };
            let opened = browser::open(&page_url, settings.browser.as_deref(), settings.open)?;
            handle.set_close_on_exit(opened == browser::Opened::Window);
        }
        // Stop once the page has been closed for a few seconds (long enough
        // to survive a reload, or moving to another file in a folder).
        let abandoned = async {
            match exit {
                Exit::Never => std::future::pending::<()>().await,
                Exit::WhenClosed { first_connect } => {
                    handle
                        .abandoned(Duration::from_secs(5), first_connect)
                        .await
                }
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

/// Run this command again as a detached background viewer
/// (`--watch --exit-when-closed`), relay the first line it prints (where
/// it's serving), and return. Standard input, if it was the source, is
/// passed on. The viewer's later output goes to `viewer.log` next to the
/// state file, where its errors are read from if it fails to start.
fn launch_in_background(stdin_text: Option<&str>) -> Result<()> {
    use std::io::{BufRead as _, Seek as _};
    let exe = std::env::current_exe().context("cannot find this program's path")?;
    let mut args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    args.extend(["--watch", "--exit-when-closed", "--background"].map(Into::into));

    let log_path = config::state_path()
        .and_then(|p| p.parent().map(|d| d.join("viewer.log")))
        .unwrap_or_else(|| std::env::temp_dir().join("markdown-viewer.log"));
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("cannot write {}", log_path.display()))?;
    let start = log.seek(std::io::SeekFrom::End(0))?;

    let mut cmd = std::process::Command::new(exe);
    cmd.args(&args)
        .stdin(if stdin_text.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(log);
    platform::detach(&mut cmd);
    let mut child = cmd.spawn().context("cannot start the viewer")?;
    if let (Some(text), Some(mut stdin)) = (stdin_text, child.stdin.take()) {
        stdin.write_all(text.as_bytes())?;
    }
    let mut first = String::new();
    if let Some(out) = child.stdout.take() {
        std::io::BufReader::new(out).read_line(&mut first)?;
    }
    if !first.trim().is_empty() {
        say(first.trim_end());
        return Ok(());
    }
    // It stopped before serving: show why.
    let status = child.wait()?;
    let mut text = String::new();
    if let Ok(mut f) = std::fs::File::open(&log_path) {
        let _ = f.seek(std::io::SeekFrom::Start(start));
        let _ = std::io::Read::read_to_string(&mut f, &mut text);
    }
    let why = text
        .trim()
        .trim_start_matches("markdown-viewer: ")
        .to_string();
    if why.is_empty() {
        bail!("the viewer stopped ({status})");
    }
    bail!("{why}")
}

/// When watch mode stops by itself.
#[derive(Debug, PartialEq)]
enum Exit {
    /// Only on Ctrl-C.
    Never,
    /// Once every page has been closed. `first_connect`: also give up if
    /// no page connects in that time.
    WhenClosed { first_connect: Option<Duration> },
}

/// By default, a viewer that opened its own window stops when it closes
/// (but waits however long the browser takes to connect, so a page opened
/// by hand later still works). `--exit-when-closed` also applies with
/// `--no-open`, and gives up after a minute with no page. `--keep-running`
/// or `exit_when_closed = false` keeps serving until Ctrl-C.
fn exit_policy(cli: &Cli, settings: &Settings) -> Exit {
    if cli.exit_when_closed {
        Exit::WhenClosed {
            first_connect: Some(Duration::from_secs(60)),
        }
    } else if cli.no_open || cli.keep_running || !settings.exit_when_closed {
        Exit::Never
    } else {
        Exit::WhenClosed {
            first_connect: None,
        }
    }
}

/// The template for served pages.
fn served_page(settings: &Settings) -> PageOptions {
    PageOptions {
        served: true,
        live: true,
        ui: true,
        custom_css: None,
        ..settings.page(String::new(), Theme::Auto)
    }
}

/// A page of a file gets the footer saying when it last changed in git.
fn with_meta(page: PageOptions, source: &Source) -> PageOptions {
    match source {
        Source::File(path) => page.with_meta_for(path, false),
        _ => page,
    }
}

/// Settings-file keys that this run's command-line options override.
fn overridden_settings(cli: &Cli) -> Vec<String> {
    [
        ("theme", cli.theme.is_some()),
        ("style", cli.style.is_some()),
        ("css", cli.css.is_some()),
        ("open", cli.app || cli.window || cli.tab),
        ("browser", cli.browser.is_some()),
        ("port", cli.port.is_some()),
        ("editor", cli.editor.is_some()),
        ("front_matter", cli.front_matter.is_some()),
        ("number_headings", cli.number_headings),
        ("line_numbers", cli.line_numbers),
        ("obsidian", cli.obsidian || cli.no_obsidian),
        ("exit_when_closed", cli.exit_when_closed || cli.keep_running),
    ]
    .into_iter()
    .filter(|(_, set)| *set)
    .map(|(key, _)| key.to_string())
    .collect()
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
                docs.set_source(&key, &text);
                docs.set_virtual(&key, html);
                last = text;
            }
        }
    });
}

/// Keep a `--diff` page current: compare again when the file changes (and
/// now and then, as the revision can move, e.g. HEAD after a commit).
fn follow_diff(docs: Arc<docs::Docs>, key: PathBuf, file: PathBuf, rev: String) {
    tokio::spawn(async move {
        let mut last: Option<(String, String)> = None;
        let mut ticks = 0u32;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            ticks += 1;
            let Ok(new) = tokio::fs::read_to_string(&file).await else {
                continue;
            };
            let changed = last.as_ref().is_none_or(|(_, text)| *text != new);
            if !changed && !ticks.is_multiple_of(10) {
                continue;
            }
            let (f, r) = (file.clone(), rev.clone());
            let Ok(Ok(old)) = tokio::task::spawn_blocking(move || diffview::old_text(&f, &r)).await
            else {
                continue;
            };
            if last.as_ref() == Some(&(old.clone(), new.clone())) {
                continue;
            }
            let html = diffview::render(docs.renderer().options(), &file, &old, &new, &rev);
            docs.set_virtual(&key, html);
            last = Some((old, new));
        }
    });
}

/// Keep the Recent page current: items removed, opened from other
/// viewers, or simply older.
fn follow_recent(docs: Arc<docs::Docs>, key: PathBuf, root: PathBuf, mut last: String) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let at = root.clone();
            let Ok(text) =
                tokio::task::spawn_blocking(move || recent::page(&Prefs::load().recent, &at)).await
            else {
                continue;
            };
            if text != last {
                docs.set_virtual(&key, docs.render_text(&text, &root));
                last = text;
            }
        }
    });
}

/// Fetch a web document again every few seconds, and update the page
/// when it changes (live reload for addresses).
fn follow_url(
    docs: Arc<docs::Docs>,
    key: PathBuf,
    root: PathBuf,
    remote: remote::Remote,
    mut last: String,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            let fetch = remote.clone();
            let Ok(Ok(text)) = tokio::task::spawn_blocking(move || fetch.fetch()).await else {
                continue;
            };
            if text != last {
                let html = remote::absolutize(&docs.render_text(&text, &root), &remote);
                docs.set_source(&key, &text);
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
    } else if cfg!(windows) {
        &[(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-Clipboard -Raw",
            ],
        )]
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
    bail!("couldn't read the clipboard (needs pbpaste, PowerShell, wl-paste, xclip or xsel)")
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

/// `file://` URL for a directory, with a trailing slash so it works as `<base>`.
fn dir_url(dir: &Path) -> String {
    let mut url = platform::file_url(dir);
    if !url.ends_with('/') {
        url.push('/');
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_mode_stops_when_its_window_closes() {
        let policy = |args: &[&str], config: &str| {
            let cli = Cli::parse_from([&["markdown-viewer", "--watch"], args].concat());
            let settings = Settings::new(&cli, Config::parse(config).unwrap()).unwrap();
            exit_policy(&cli, &settings)
        };
        let closes = Exit::WhenClosed {
            first_connect: None,
        };
        // Default: we opened the window, so its closing ends the run.
        assert_eq!(policy(&[], ""), closes);
        // Nothing opened (or asked to stay): Ctrl-C only.
        assert_eq!(policy(&["--no-open"], ""), Exit::Never);
        assert_eq!(policy(&["--keep-running"], ""), Exit::Never);
        assert_eq!(policy(&[], "exit_when_closed = false"), Exit::Never);
        // --exit-when-closed: even without opening, and gives up after a minute.
        let strict = Exit::WhenClosed {
            first_connect: Some(Duration::from_secs(60)),
        };
        assert_eq!(policy(&["--no-open", "--exit-when-closed"], ""), strict);
        assert_eq!(
            policy(&["--exit-when-closed"], "exit_when_closed = false"),
            strict
        );
        assert!(
            Cli::try_parse_from([
                "markdown-viewer",
                "--watch",
                "--keep-running",
                "--exit-when-closed"
            ])
            .is_err()
        );
    }

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
