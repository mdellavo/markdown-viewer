//! User settings (a TOML file of defaults for command-line options) and
//! remembered viewer preferences (theme and sidebar, saved by the page).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::browser::Mode;
use crate::render::FrontMatter;
use crate::theme::{Style, Theme};

/// Text size limits.
pub const MIN_SCALE: f32 = 0.5;
pub const MAX_SCALE: f32 = 3.0;

/// How many files' reading positions to remember.
const MAX_POSITIONS: usize = 300;

/// `~/.config/markdown-viewer/config.toml`. Every key is optional; command
/// line options override them.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub theme: Option<Theme>,
    pub style: Option<Style>,
    /// Extra stylesheet, applied after the built-in styles. `~/` is expanded.
    pub css: Option<PathBuf>,
    /// How to open the page: "app", "window" or "tab".
    pub open: Option<String>,
    pub browser: Option<String>,
    pub port: Option<u16>,
    /// Editor for click-to-source, e.g. "zed" or "code -g {file}:{line}".
    pub editor: Option<String>,
    /// Front matter: "table" (like GitHub) or "hide".
    pub front_matter: Option<FrontMatter>,
    /// Number headings.
    pub number_headings: Option<bool>,
    /// Number the lines of every code block.
    pub line_numbers: Option<bool>,
    /// Obsidian notes; by default, on inside a vault (a `.obsidian` folder).
    pub obsidian: Option<bool>,
    /// Link `#123`, `@user` and commit hashes to GitHub (default: on, in a
    /// git checkout with a GitHub remote).
    pub github_references: Option<bool>,
    /// The repository for GitHub references, `owner/repo` or its web
    /// address, if it isn't found from the git remote.
    pub github_repo: Option<String>,
    /// `--lint` rules to skip.
    pub lint_ignore: Option<Vec<String>>,
    /// Remember the files and folders you open, for the Recent page
    /// (default: on).
    pub recent: Option<bool>,
    /// Show the word count and reading time in the toolbar (default: on;
    /// `w` in the viewer toggles it for you).
    pub word_count: Option<bool>,
    /// Stop serving once the window markdown-viewer opened has been closed
    /// (default: on).
    pub exit_when_closed: Option<bool>,
    /// Syntax highlight the Markdown in the source view (default: on).
    pub source_highlighting: Option<bool>,
    /// Show when a file last changed in git, and by whom, with an "Edit on
    /// GitHub" link, at the foot of the page (default: on).
    pub last_updated: Option<bool>,
}

impl Config {
    /// Load the settings file if there is one.
    pub fn load() -> Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).with_context(|| format!("in {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut config: Config = toml::from_str(text)?;
        config.css = config.css.map(|p| expand_home(&p));
        if let Some(open) = &config.open {
            open_mode(open)?;
        }
        if let Some(repo) = &config.github_repo
            && crate::references::from_setting(repo).is_none()
        {
            anyhow::bail!("github_repo: expected owner/repo or a GitHub address, got {repo:?}");
        }
        for rule in config.lint_ignore.iter().flatten() {
            if !crate::lint::RULES.iter().any(|(name, _)| name == rule) {
                let names: Vec<&str> = crate::lint::RULES.iter().map(|(n, _)| *n).collect();
                anyhow::bail!(
                    "lint_ignore: unknown rule {rule:?} (rules: {})",
                    names.join(", ")
                );
            }
        }
        Ok(config)
    }

    pub fn open_mode(&self) -> Option<Mode> {
        self.open.as_deref().and_then(|s| open_mode(s).ok())
    }
}

fn open_mode(s: &str) -> Result<Mode> {
    Ok(match s {
        "app" => Mode::App,
        "window" => Mode::Window,
        "tab" => Mode::Tab,
        other => bail!("open = {other:?}: expected \"app\", \"window\" or \"tab\""),
    })
}

/// What the viewer remembers between runs: the last theme and sidebar
/// chosen in the page.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Prefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    /// "contents", "files", "links", or empty for closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<String>,
    /// Text size, as a multiple of normal (the + and - keys).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_scale: Option<f32>,
    /// Show what changed since the last git commit (the D key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git: Option<bool>,
    /// Show the word count and reading time in the toolbar (the w key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_count: Option<bool>,
    /// Where you were reading, per file (absolute path → position).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub positions: BTreeMap<String, Position>,
    /// Recently opened files and folders, newest first (see recent.rs).
    /// Never changed by pages: `merge` ignores it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent: Vec<RecentItem>,
}

/// A recently opened file or folder (absolute path) and when.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentItem {
    pub path: String,
    pub at: u64,
}

/// A reading position: the source line of the block at the top of the
/// window (robust to edits elsewhere), and when it was saved.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub line: u32,
    pub at: u64,
}

impl Prefs {
    /// Load remembered preferences; anything unreadable is ignored.
    pub fn load() -> Self {
        state_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Apply a change sent by the page, keeping only valid values.
    pub fn merge(&mut self, update: Prefs) {
        if update.theme.is_some() {
            self.theme = update.theme;
        }
        if let Some(sidebar) = update.sidebar
            && matches!(
                sidebar.as_str(),
                "" | "contents" | "files" | "links" | "recent" | "tags"
            )
        {
            self.sidebar = Some(sidebar);
        }
        if let Some(scale) = update.font_scale
            && scale.is_finite()
        {
            self.font_scale = Some(scale.clamp(MIN_SCALE, MAX_SCALE));
        }
        if update.git.is_some() {
            self.git = update.git;
        }
        if update.word_count.is_some() {
            self.word_count = update.word_count;
        }
        for (file, pos) in update.positions {
            self.positions.insert(file, pos);
        }
        // Forget the oldest positions beyond the limit.
        while self.positions.len() > MAX_POSITIONS {
            let oldest = self
                .positions
                .iter()
                .min_by_key(|(_, p)| p.at)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => self.positions.remove(&k),
                None => break,
            };
        }
    }

    /// Change the saved preferences: load, apply `change`, save. Several
    /// viewers and requests share the file, so this holds a lock (across
    /// threads and processes) for the whole read-modify-write, and the
    /// file is replaced in one step, so a reader never sees half of it.
    pub fn update(change: impl FnOnce(&mut Prefs)) -> Result<()> {
        static IN_PROCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let Some(path) = state_path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let _guard = IN_PROCESS.lock().unwrap_or_else(|e| e.into_inner());
        let lock_path = path.with_extension("lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("cannot open {}", lock_path.display()))?;
        lock.lock()
            .with_context(|| format!("cannot lock {}", lock_path.display()))?;
        let mut prefs = Prefs::load();
        change(&mut prefs);
        write_atomically(&path, serde_json::to_string_pretty(&prefs)?.as_bytes())
        // The lock is released when `lock` is dropped.
    }
}

/// Write `path` by writing a temporary file next to it and renaming it into
/// place, so it is never seen half-written.
pub fn write_atomically(path: &Path, contents: &[u8]) -> Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, contents).with_context(|| format!("cannot write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        anyhow::Error::new(e).context(format!("cannot write {}", path.display()))
    })
}

/// Change one setting in the settings file (`None` removes it, going back
/// to the default), keeping everything else in the file as it was,
/// comments included. The result must be a valid settings file that
/// `check` accepts, or nothing is written. Returns what `check` returned.
pub fn update_file<T>(
    key: &str,
    value: Option<toml_edit::Value>,
    check: impl FnOnce(&Config) -> Result<T>,
) -> Result<T> {
    let path = config_path().context("no place for a settings file (HOME isn't set)")?;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
    };
    let new = set_key(&text, key, value)
        .with_context(|| format!("{} isn't a valid settings file", path.display()))?;
    let checked = check(&Config::parse(&new)?)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_atomically(&path, new.as_bytes())?;
    Ok(checked)
}

/// `text` with `key` set to `value` (or removed).
fn set_key(text: &str, key: &str, value: Option<toml_edit::Value>) -> Result<String> {
    let mut doc: toml_edit::DocumentMut = text.parse()?;
    match value {
        Some(mut value) => {
            // Keep the old value's spacing and trailing comment.
            if let Some(old) = doc.get(key).and_then(|item| item.as_value()) {
                *value.decor_mut() = old.decor().clone();
            }
            doc[key] = toml_edit::Item::Value(value);
        }
        None => {
            doc.remove(key);
        }
    }
    Ok(doc.to_string())
}

/// `$MARKDOWN_VIEWER_CONFIG`, else `$XDG_CONFIG_HOME/markdown-viewer/config.toml`,
/// else `~/.config/markdown-viewer/config.toml` (on macOS too, as is usual
/// for command-line tools), or on Windows `%APPDATA%\markdown-viewer\config.toml`.
pub fn config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MARKDOWN_VIEWER_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None if cfg!(windows) => PathBuf::from(std::env::var_os("APPDATA")?),
        None => home()?.join(".config"),
    };
    Some(base.join("markdown-viewer").join("config.toml"))
}

/// `$MARKDOWN_VIEWER_STATE`, else the platform's place for app state.
pub fn state_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MARKDOWN_VIEWER_STATE") {
        return Some(PathBuf::from(p));
    }
    let base = if cfg!(target_os = "macos") {
        home()?.join("Library/Application Support")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".local/state")))?
    };
    Some(base.join("markdown-viewer").join("state.json"))
}

pub fn home() -> Option<PathBuf> {
    crate::platform::home()
}

fn expand_home(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), home()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_a_key_keeps_the_rest_of_the_file() {
        let text = "# My settings\ntheme = \"dark\"   # at night\n\n# Editor\neditor = \"zed\"\n";
        let out = set_key(text, "style", Some("book".into())).unwrap();
        assert_eq!(
            out,
            "# My settings\ntheme = \"dark\"   # at night\n\n# Editor\neditor = \"zed\"\nstyle = \"book\"\n"
        );
        let out = set_key(&out, "theme", Some("light".into())).unwrap();
        assert!(
            out.starts_with("# My settings\ntheme = \"light\"   # at night\n"),
            "{out}"
        );
        let out = set_key(&out, "editor", None).unwrap();
        assert!(!out.contains("editor ="), "{out}");
        let mut list = toml_edit::Array::new();
        list.push("multiple-h1");
        let out = set_key("", "lint_ignore", Some(list.into())).unwrap();
        assert_eq!(
            Config::parse(&out).unwrap().lint_ignore,
            Some(vec!["multiple-h1".into()])
        );
        // Values the settings file doesn't accept are caught by parsing.
        let bad = set_key("", "style", Some("fancy".into())).unwrap();
        assert!(Config::parse(&bad).is_err());
        assert!(set_key("not = [valid", "style", None).is_err());
    }

    #[test]
    fn parses_settings() {
        let c = Config::parse(
            "theme = \"dark\"\nstyle = \"book\"\ncss = \"/x.css\"\nopen = \"window\"\nbrowser = \"firefox\"\nport = 7000\n",
        )
        .unwrap();
        assert_eq!(c.theme, Some(Theme::Dark));
        assert_eq!(c.style, Some(Style::Book));
        assert_eq!(c.css.as_deref(), Some(Path::new("/x.css")));
        assert_eq!(c.open_mode(), Some(Mode::Window));
        assert_eq!(c.browser.as_deref(), Some("firefox"));
        assert_eq!(c.port, Some(7000));
    }

    #[test]
    fn rejects_typos_and_bad_values() {
        assert!(Config::parse("them = \"dark\"").is_err(), "unknown key");
        assert!(Config::parse("theme = \"blue\"").is_err());
        assert!(Config::parse("open = \"popup\"").is_err());
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn positions_are_capped_dropping_the_oldest() {
        let mut p = Prefs::default();
        for i in 0..(MAX_POSITIONS as u64 + 5) {
            let mut update = Prefs::default();
            update
                .positions
                .insert(format!("/f{i}.md"), Position { line: 1, at: i });
            p.merge(update);
        }
        assert_eq!(p.positions.len(), MAX_POSITIONS);
        assert!(!p.positions.contains_key("/f0.md"), "oldest forgotten");
        assert!(
            p.positions
                .contains_key(&format!("/f{}.md", MAX_POSITIONS + 4))
        );
    }

    #[test]
    fn prefs_merge_ignores_invalid_sidebar() {
        let mut p = Prefs::default();
        p.merge(Prefs {
            theme: Some(Theme::Light),
            sidebar: Some("contents".into()),
            ..Default::default()
        });
        p.merge(Prefs {
            sidebar: Some("evil".into()),
            ..Default::default()
        });
        assert_eq!(p.theme, Some(Theme::Light));
        assert_eq!(p.sidebar.as_deref(), Some("contents"));
    }
}
