//! User settings (a TOML file of defaults for command-line options) and
//! remembered viewer preferences (theme and sidebar, saved by the page).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::browser::Mode;
use crate::theme::{Style, Theme};

/// `~/.config/markdown-viewer/config.toml`. Every key is optional; command
/// line options override them.
#[derive(Debug, Default, Deserialize, PartialEq)]
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
    /// "contents", "files", or empty for closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidebar: Option<String>,
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
            && matches!(sidebar.as_str(), "" | "contents" | "files")
        {
            self.sidebar = Some(sidebar);
        }
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = state_path() else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("cannot write {}", path.display()))
    }
}

/// `$MARKDOWN_VIEWER_CONFIG`, else `$XDG_CONFIG_HOME/markdown-viewer/config.toml`,
/// else `~/.config/markdown-viewer/config.toml` (on macOS too, as is usual
/// for command-line tools).
pub fn config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MARKDOWN_VIEWER_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home().map(|h| h.join(".config")))?;
    Some(base.join("markdown-viewer/config.toml"))
}

/// `$MARKDOWN_VIEWER_STATE`, else the platform's place for app state.
pub fn state_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MARKDOWN_VIEWER_STATE") {
        return Some(PathBuf::from(p));
    }
    let base = if cfg!(target_os = "macos") {
        home()?.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".local/state")))?
    };
    Some(base.join("markdown-viewer/state.json"))
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
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
    fn prefs_merge_ignores_invalid_sidebar() {
        let mut p = Prefs::default();
        p.merge(Prefs {
            theme: Some(Theme::Light),
            sidebar: Some("contents".into()),
        });
        p.merge(Prefs {
            theme: None,
            sidebar: Some("evil".into()),
        });
        assert_eq!(p.theme, Some(Theme::Light));
        assert_eq!(p.sidebar.as_deref(), Some("contents"));
    }
}
