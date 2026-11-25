//! Watches the source Markdown file and publishes freshly rendered HTML.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use tokio::sync::watch;

use crate::render::Renderer;

const DEBOUNCE: Duration = Duration::from_millis(100);

/// Keeps the OS watcher alive; drop it to stop watching.
pub struct FileWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

/// Watch `source` and send its re-rendered body on `tx` whenever it changes.
///
/// The parent directory is watched rather than the file itself: many editors
/// save by writing a temp file and renaming it over the original, which would
/// silently end a watch placed on the file's inode.
pub fn watch_file(
    source: &Path,
    renderer: Arc<Renderer>,
    tx: watch::Sender<String>,
) -> Result<FileWatcher> {
    let source: PathBuf = source.to_path_buf();
    let dir = source
        .parent()
        .context("source file has no parent directory")?
        .to_path_buf();
    let file_name = source.file_name().map(|n| n.to_os_string());

    let target = source.clone();
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let touched = events.iter().any(|ev| {
            ev.event
                .paths
                .iter()
                .any(|p| p.file_name().map(|n| n.to_os_string()) == file_name)
        });
        if !touched {
            return;
        }
        // The file may briefly not exist mid-save; keep the last good render.
        if let Ok(markdown) = std::fs::read_to_string(&target) {
            let body = renderer.render_body(&markdown);
            tx.send_if_modified(|current| {
                if *current == body {
                    false
                } else {
                    *current = body;
                    true
                }
            });
        }
    })
    .context("failed to start file watcher")?;

    debouncer
        .watch(&dir, RecursiveMode::NonRecursive)
        .with_context(|| format!("failed to watch {}", dir.display()))?;

    Ok(FileWatcher {
        _debouncer: debouncer,
    })
}
