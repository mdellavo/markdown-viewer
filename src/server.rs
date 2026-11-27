//! Local HTTP server for `--watch`: rendered pages for every Markdown file
//! and folder under the root, a live-update stream per document, the
//! viewer's preferences and file list, and static files (for images).

use crate::platform::Canonical;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;

use crate::config::{Position, Prefs};
use crate::docs::{self, Docs};
use crate::render::{
    KATEX_CSS, KATEX_CSS_PATH, KATEX_JS, KATEX_JS_PATH, MERMAID_JS_GZ, MERMAID_PATH, PageOptions,
    RenderOptions, Renderer,
};

pub const EVENTS_PATH: &str = "/__mdv/events";
const PREFS_PATH: &str = "/__mdv/prefs";
const FILES_PATH: &str = "/__mdv/files";
const SEARCH_PATH: &str = "/__mdv/search";
const OPEN_PATH: &str = "/__mdv/open";
const POSITION_PATH: &str = "/__mdv/position";
const GOTO_PATH: &str = "/__mdv/goto";
const TASK_PATH: &str = "/__mdv/task";
const ABOUT_PATH: &str = "/__mdv/about";
const SETTINGS_PATH: &str = "/__mdv/settings";
const SOURCE_HTML_PATH: &str = "/__mdv/source-html";
const SOURCE_PATH: &str = "/__mdv/source";
const GITDIFF_PATH: &str = "/__mdv/gitdiff";
const CHECK_PATH: &str = "/__mdv/check";
const BACKLINKS_PATH: &str = "/__mdv/backlinks";
const RECENT_PATH: &str = "/__mdv/recent";
const RECENT_REMOVE_PATH: &str = "/__mdv/recent/remove";
const RECENT_OPEN_PATH: &str = "/__mdv/recent/open";

/// What to serve and how.
pub struct ServeOptions {
    pub port: u16,
    pub docs: Arc<Docs>,
    /// The file given on the command line, shown at `/`. `None` in folder
    /// mode, where `/` shows the folder's README or a listing.
    pub main: Option<PathBuf>,
    /// Template for every page (style etc.); per-page fields are filled in.
    pub page: PageOptions,
    /// `--css` file, re-read for each page so edits apply on reload.
    pub custom_css: Option<PathBuf>,
    /// The session's preferences (initial theme and sidebar).
    pub prefs: Prefs,
    /// Save preference changes from the page to the state file.
    pub remember: bool,
    /// Editor setting for click-to-source (see editor.rs).
    pub editor: Option<String>,
    /// What this server serves, reported at `/__mdv/whoami` so a second
    /// launch for the same target can find it.
    pub identity: String,
    /// Turns a settings file into what this viewer uses (command-line
    /// options still win), for the settings dialog. `None`: no dialog.
    pub reconfigure: Option<Reconfigure>,
    /// Settings that command-line options override in this run.
    pub overridden: Vec<String>,
    /// Highlight the source view.
    pub source_highlighting: bool,
}

/// What a viewer uses from the settings, recomputed when they change.
pub struct Reconfigured {
    pub render: RenderOptions,
    /// The page template (style, numbered headings).
    pub page: PageOptions,
    pub custom_css: Option<PathBuf>,
    pub editor: Option<String>,
    pub source_highlighting: bool,
}

pub type Reconfigure = Arc<dyn Fn(&crate::config::Config) -> Result<Reconfigured> + Send + Sync>;

/// Settings that can change while serving.
struct Live {
    /// Template for every page (style etc.); per-page fields are filled in.
    page: PageOptions,
    /// `--css` file, re-read for each page so edits apply on reload.
    custom_css: Option<PathBuf>,
    /// Editor setting for click-to-source (see editor.rs).
    editor: Option<String>,
    /// Highlight the source view.
    source_highlighting: bool,
}

#[derive(Clone)]
struct AppState {
    docs: Arc<Docs>,
    main: Option<PathBuf>,
    live: Arc<Mutex<Live>>,
    prefs: Arc<Mutex<Prefs>>,
    remember: bool,
    identity: Arc<String>,
    reconfigure: Option<Reconfigure>,
    overridden: Arc<Vec<String>>,
    /// "Go to this line" requests (`--goto`), as (root-relative doc, line).
    goto: tokio::sync::broadcast::Sender<(String, u32)>,
    port: u16,
    /// Flips to true when the server is shutting down.
    closing: watch::Receiver<bool>,
    /// Whether pages should close their window on shutdown (only windows
    /// we opened ourselves, never a tab the user was already using).
    close_on_exit: Arc<AtomicBool>,
    /// Number of pages currently connected to an event stream.
    clients: Arc<watch::Sender<usize>>,
}

/// Lets the caller adjust a running server and announce shutdown.
#[derive(Clone)]
pub struct ServerHandle {
    closing: watch::Sender<bool>,
    close_on_exit: Arc<AtomicBool>,
    clients: watch::Receiver<usize>,
}

impl ServerHandle {
    pub fn set_close_on_exit(&self, close: bool) {
        self.close_on_exit.store(close, Ordering::Relaxed);
    }

    /// Tell connected pages the server is stopping.
    pub fn announce_shutdown(&self) {
        let _ = self.closing.send(true);
    }

    /// Resolves once nobody is viewing any page: one connected and then
    /// every page has been gone for `grace` (long enough to ride out a
    /// reload or following a link). Also resolves if no page connects within
    /// `first_connect`, e.g. because the browser never opened.
    pub async fn abandoned(&self, grace: Duration, first_connect: Option<Duration>) {
        let mut clients = self.clients.clone();
        // Each wait yields only whether the condition was met, so no borrow
        // of the channel outlives its statement. With no first_connect limit,
        // wait for a page however long it takes.
        let connected = match first_connect {
            Some(limit) => tokio::time::timeout(limit, clients.wait_for(|n| *n > 0))
                .await
                .is_ok_and(|r| r.is_ok()),
            None => clients.wait_for(|n| *n > 0).await.is_ok(),
        };
        if !connected {
            return;
        }
        loop {
            if clients.wait_for(|n| *n == 0).await.is_err() {
                return;
            }
            let back = tokio::time::timeout(grace, clients.wait_for(|n| *n > 0))
                .await
                .is_ok_and(|r| r.is_ok());
            if !back {
                return;
            }
        }
    }
}

/// Counts a connected page for as long as its event stream is alive.
struct ClientGuard(Arc<watch::Sender<usize>>);

impl ClientGuard {
    fn new(clients: Arc<watch::Sender<usize>>) -> Self {
        clients.send_modify(|n| *n += 1);
        Self(clients)
    }
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.0.send_modify(|n| *n = n.saturating_sub(1));
    }
}

/// A bound (but not yet running) server.
pub struct Server {
    listener: TcpListener,
    router: Router,
    addr: SocketAddr,
    handle: ServerHandle,
}

impl Server {
    /// Bind to `127.0.0.1:port` (`port` 0 picks a free one).
    pub async fn bind(opts: ServeOptions) -> Result<Self> {
        let port = opts.port;
        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .with_context(|| format!("could not listen on 127.0.0.1:{port}"))?;
        let addr = listener.local_addr()?;
        let (closing_tx, closing) = watch::channel(false);
        let close_on_exit = Arc::new(AtomicBool::new(false));
        let (clients_tx, clients_rx) = watch::channel(0usize);
        let handle = ServerHandle {
            closing: closing_tx,
            close_on_exit: close_on_exit.clone(),
            clients: clients_rx,
        };
        let state = AppState {
            docs: opts.docs,
            main: opts.main,
            live: Arc::new(Mutex::new(Live {
                page: opts.page,
                custom_css: opts.custom_css,
                editor: opts.editor,
                source_highlighting: opts.source_highlighting,
            })),
            prefs: Arc::new(Mutex::new(opts.prefs)),
            remember: opts.remember,
            identity: Arc::new(opts.identity),
            reconfigure: opts.reconfigure,
            overridden: Arc::new(opts.overridden),
            goto: tokio::sync::broadcast::channel(16).0,
            port: addr.port(),
            closing,
            close_on_exit,
            clients: Arc::new(clients_tx),
        };
        let router = Router::new()
            .route("/", get(index))
            .route(EVENTS_PATH, get(events))
            .route(PREFS_PATH, post(save_prefs))
            .route(FILES_PATH, get(list_files))
            .route(SEARCH_PATH, get(search))
            .route(OPEN_PATH, post(open_in_editor))
            .route(POSITION_PATH, post(save_position))
            .route(crate::instances::WHOAMI_PATH, get(whoami))
            .route(GOTO_PATH, post(goto))
            .route(TASK_PATH, post(set_task))
            .route(ABOUT_PATH, get(about))
            .route(SETTINGS_PATH, get(get_settings).post(set_setting))
            .route(SOURCE_HTML_PATH, get(source_html))
            .route(SOURCE_PATH, get(source))
            .route(GITDIFF_PATH, get(gitdiff))
            .route(CHECK_PATH, get(check_links))
            .route(BACKLINKS_PATH, get(backlinks))
            .route(RECENT_PATH, get(recent_list))
            .route(RECENT_REMOVE_PATH, post(recent_remove))
            .route(RECENT_OPEN_PATH, post(recent_open))
            .route(MERMAID_PATH, get(mermaid_js))
            .route(KATEX_JS_PATH, get(katex_js))
            .route(KATEX_CSS_PATH, get(katex_css))
            .fallback(get(path_handler))
            .layer(middleware::from_fn_with_state(state.clone(), check_host))
            .with_state(state);
        Ok(Self {
            listener,
            router,
            addr,
            handle,
        })
    }

    pub fn handle(&self) -> ServerHandle {
        self.handle.clone()
    }

    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    pub async fn run(self) -> Result<()> {
        axum::serve(self.listener, self.router)
            .await
            .context("server error")
    }
}

/// Reject requests whose Host header isn't our loopback address. This blocks
/// DNS-rebinding attacks from web pages trying to read files we serve.
async fn check_host(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let ok = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|host| {
            let p = state.port;
            host == format!("127.0.0.1:{p}") || host == format!("localhost:{p}")
        });
    if ok {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "forbidden host").into_response()
    }
}

async fn index(State(state): State<AppState>) -> Response {
    match state.main.clone() {
        Some(main) => doc_page(&state, &main),
        None => {
            let root = state.docs.root().to_path_buf();
            dir_page(&state, &root)
        }
    }
}

/// Folders show their README, or a listing if there isn't one.
fn dir_page(state: &AppState, dir: &Path) -> Response {
    match docs::readme(dir) {
        Some(readme) => doc_page(state, &readme),
        None => doc_page(state, dir),
    }
}

/// A live page for a document (a Markdown file, or a folder listing).
fn doc_page(state: &AppState, doc: &Path) -> Response {
    let rx = match state.docs.subscribe(doc) {
        Ok(rx) => rx,
        Err(e) => return (StatusCode::NOT_FOUND, format!("{e:#}")).into_response(),
    };
    let body = rx.borrow().clone();
    let prefs = state.prefs.lock().unwrap().clone();
    let (template, css_path) = {
        let live = state.live.lock().unwrap();
        (live.page.clone(), live.custom_css.clone())
    };
    let custom_css = css_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    let resume_line = prefs
        .positions
        .get(doc.to_string_lossy().as_ref())
        .map(|p| p.line);
    let page = PageOptions {
        title: file_title(doc),
        theme: prefs.theme.unwrap_or(template.theme),
        sidebar: prefs.sidebar,
        resume_line,
        font_scale: prefs.font_scale,
        git_changes: prefs.git.unwrap_or(false),
        // The viewer's own choice (w), else the setting.
        hide_word_count: prefs
            .word_count
            .map_or(template.hide_word_count, |show| !show),
        doc: Some(rel_path(state.docs.root(), doc)),
        close_on_exit: state.close_on_exit.load(Ordering::Relaxed),
        custom_css: custom_css.or_else(|| template.custom_css.clone()),
        settings: state.reconfigure.is_some(),
        // The source view: Markdown files, and documents with no file that
        // the viewer keeps the text of.
        source: docs::is_markdown(doc)
            || state.main.as_deref() == Some(doc) && state.docs.source(doc).is_some(),
        ..template
    };
    no_store(Html(state.docs.renderer().wrap_page(&body, &page)).into_response())
}

#[derive(Deserialize)]
struct EventsQuery {
    #[serde(default)]
    doc: String,
}

async fn events(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Response {
    let doc = if q.doc.is_empty() || is_main(&state, &q.doc) {
        match &state.main {
            Some(main) => Some(main.clone()),
            None => {
                let root = state.docs.root();
                Some(docs::readme(root).unwrap_or_else(|| root.to_path_buf()))
            }
        }
    } else {
        resolve(state.docs.root(), &q.doc, true).filter(|p| p.is_dir() || docs::is_viewable(p))
    };
    let Some(doc) = doc else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let rx = match state.docs.subscribe(&doc) {
        Ok(rx) => rx,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    // WatchStream yields the current value first, so a reconnecting page
    // (e.g. after the laptop slept) immediately catches up.
    let updates = WatchStream::new(rx)
        .map(|body| Ok::<_, Infallible>(Event::default().event("update").data(sse_safe(&body))));
    let shutdown = WatchStream::from_changes(state.closing.clone())
        .filter(|closing| *closing)
        .map(|_| Ok(Event::default().event("close").data("bye")));
    // The guard lives as long as the stream, i.e. until the page goes away.
    let guard = ClientGuard::new(state.clients.clone());
    // `--goto` requests, for every page (a folder-mode page showing another
    // document navigates to it).
    let gotos = tokio_stream::wrappers::BroadcastStream::new(state.goto.subscribe())
        .filter_map(|r| r.ok())
        .map(|(doc, line)| {
            Ok(Event::default()
                .event("goto")
                .data(serde_json::json!({ "doc": doc, "line": line }).to_string()))
        });
    let stream = updates.merge(shutdown).merge(gotos).map(move |event| {
        let _alive = &guard;
        event
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response()
}

/// The page saves its theme and sidebar here. Requiring a custom header
/// means other websites can't call it (that would need a CORS preflight,
/// which we never approve).
async fn save_prefs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<Prefs>,
) -> StatusCode {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN;
    }
    // Positions have their own endpoint, which checks the file.
    let update = Prefs {
        positions: Default::default(),
        ..update
    };
    remember(&state, update).await;
    StatusCode::NO_CONTENT
}

async fn remember(state: &AppState, update: Prefs) {
    state.prefs.lock().unwrap().merge(update.clone());
    if state.remember {
        let _ =
            tokio::task::spawn_blocking(move || Prefs::update(|saved| saved.merge(update))).await;
    }
}

/// Recently opened files, for the sidebar's Recent tab.
async fn recent_list(State(state): State<AppState>) -> Response {
    let enabled = state.live.lock().unwrap().page.recent;
    let root = state.docs.root().to_path_buf();
    let items = if enabled {
        tokio::task::spawn_blocking(move || crate::recent::shown(&Prefs::load().recent, &root))
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    no_store(Json(serde_json::json!({ "enabled": enabled, "items": items })).into_response())
}

#[derive(Deserialize)]
struct RecentPath {
    path: String,
}

/// Forget one recent item.
async fn recent_remove(headers: HeaderMap, Json(req): Json<RecentPath>) -> StatusCode {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN;
    }
    match tokio::task::spawn_blocking(move || crate::recent::forget(&req.path)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Open a recent item that this viewer doesn't serve, by running
/// markdown-viewer for it (which reuses a viewer already showing it).
/// Only paths in the remembered list are opened.
async fn recent_open(headers: HeaderMap, Json(req): Json<RecentPath>) -> StatusCode {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN;
    }
    let known = Prefs::load().recent.iter().any(|r| r.path == req.path);
    if !known || !Path::new(&req.path).exists() {
        return StatusCode::NOT_FOUND;
    }
    let Ok(exe) = std::env::current_exe() else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--")
        .arg(&req.path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::platform::detach(&mut cmd);
    match cmd.spawn() {
        Ok(mut child) => {
            // Reap it when it exits (it hands off to a background viewer).
            std::thread::spawn(move || child.wait());
            StatusCode::NO_CONTENT
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// A document reference from the page: its path relative to the root
/// (empty for the main document) and a 1-based source line.
#[derive(Deserialize)]
struct DocLine {
    #[serde(default)]
    doc: String,
    line: u32,
}

/// The file a page's `doc` refers to, if it's a real Markdown file.
fn doc_file(state: &AppState, doc: &str) -> Option<PathBuf> {
    let path = if doc.is_empty() || is_main(state, doc) {
        state.main.clone()?
    } else {
        resolve(state.docs.root(), doc, false)?
    };
    (path.is_file() && docs::is_markdown(&path)).then_some(path)
}

fn is_main(state: &AppState, doc: &str) -> bool {
    state
        .main
        .as_ref()
        .is_some_and(|m| rel_path(state.docs.root(), m) == doc)
}

/// Click-to-source: open the document in the editor at a line.
async fn open_in_editor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DocLine>,
) -> Response {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(file) = doc_file(&state, &req.doc) else {
        return (StatusCode::NOT_FOUND, "no file to open").into_response();
    };
    let editor = state.live.lock().unwrap().editor.clone();
    match crate::editor::open(&file, req.line, editor.as_deref()) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")).into_response(),
    }
}

/// Remember where the reader is in a document.
async fn save_position(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DocLine>,
) -> StatusCode {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN;
    }
    let Some(file) = doc_file(&state, &req.doc) else {
        return StatusCode::NOT_FOUND;
    };
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let mut update = Prefs::default();
    update.positions.insert(
        file.to_string_lossy().into_owned(),
        Position { line: req.line, at },
    );
    remember(&state, update).await;
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
}

async fn search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> Response {
    let docs = state.docs.clone();
    match tokio::task::spawn_blocking(move || docs.search(&q.q)).await {
        Ok(hits) => no_store(Json(serde_json::json!({ "hits": hits })).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct GotoRequest {
    /// Absolute path of the file, as the editor knows it.
    file: PathBuf,
    line: u32,
}

/// `markdown-viewer --goto file:line`: scroll pages to a line.
async fn goto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GotoRequest>,
) -> StatusCode {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN;
    }
    let root = state.docs.root();
    let Ok(file) = req.file.canonical() else {
        return StatusCode::NOT_FOUND;
    };
    if !file.starts_with(root) || !docs::is_markdown(&file) {
        return StatusCode::NOT_FOUND;
    }
    // Pages compare this with their own document (data-doc).
    let _ = state.goto.send((rel_path(root, &file), req.line.max(1)));
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct TaskRequest {
    doc: String,
    line: u32,
    col: u32,
    done: bool,
}

/// Tick or untick a task list item: changes its `[ ]` in the file, and the
/// page updates like after any other edit.
async fn set_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<TaskRequest>,
) -> Response {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(file) = doc_file(&state, &req.doc) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let done = tokio::task::spawn_blocking(move || {
        crate::tasks::set_in_file(&file, req.line, req.col, req.done)
    })
    .await;
    match done {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(msg)) => (StatusCode::CONFLICT, msg).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The settings file, for the settings dialog: its values, where it is,
/// which settings command-line options override, and what the viewer
/// found by itself (for "default" hints).
async fn get_settings(State(state): State<AppState>) -> Response {
    if state.reconfigure.is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let root = state.docs.root().to_path_buf();
    let overridden = state.overridden.clone();
    let info = tokio::task::spawn_blocking(move || {
        let path = crate::config::config_path();
        let text = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();
        let (values, error) = match toml::from_str::<toml::Table>(&text) {
            Ok(table) => (serde_json::to_value(table).unwrap_or_default(), None),
            Err(e) => (serde_json::json!({}), Some(e.to_string())),
        };
        let editor = crate::editor::command(Path::new("FILE"), 1, None)
            .ok()
            .map(|(program, _)| program);
        serde_json::json!({
            "path": path.as_ref().map(|p| p.display().to_string()),
            "exists": path.as_ref().is_some_and(|p| p.is_file()),
            "values": values,
            "error": error,
            "overridden": *overridden,
            "lint_rules": crate::lint::RULES,
            "detected": {
                "github": crate::references::detect(&root),
                "editor": editor,
                "obsidian": crate::obsidian::in_vault(&root),
            },
        })
    })
    .await;
    match info {
        Ok(info) => no_store(Json(info).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct SettingUpdate {
    key: String,
    /// `null` removes the setting (back to the default).
    value: serde_json::Value,
}

/// Change one setting: write it to the settings file (if the result is
/// valid) and apply it to this viewer. Answers `{"reload": true}` when
/// pages must reload to show it (style, custom CSS, numbered headings);
/// rendering changes reach pages as live updates.
async fn set_setting(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<SettingUpdate>,
) -> Response {
    if headers.get("x-mdv").is_none() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(reconfigure) = state.reconfigure.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let value = match toml_value(&update.value) {
        Ok(v) => v,
        Err(msg) => return (StatusCode::BAD_REQUEST, msg).into_response(),
    };
    let applied = tokio::task::spawn_blocking(move || -> Result<bool> {
        let next = crate::config::update_file(&update.key, value, |config| reconfigure(config))?;
        let reload = {
            let mut live = state.live.lock().unwrap();
            let reload = live.page.style != next.page.style
                || live.page.number_headings != next.page.number_headings
                || live.custom_css != next.custom_css;
            live.page = next.page;
            live.custom_css = next.custom_css;
            live.editor = next.editor;
            live.source_highlighting = next.source_highlighting;
            reload
        };
        if state.docs.renderer().options() != &next.render {
            state
                .docs
                .set_renderer(Arc::new(Renderer::with_options(next.render)));
        }
        Ok(reload)
    })
    .await;
    match applied {
        Ok(Ok(reload)) => no_store(Json(serde_json::json!({ "reload": reload })).into_response()),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, format!("{e:#}")).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// A setting's value from the page, as TOML: true/false, a whole number,
/// text, a list of text, or null (remove it).
fn toml_value(value: &serde_json::Value) -> Result<Option<toml_edit::Value>, String> {
    Ok(Some(match value {
        serde_json::Value::Null => return Ok(None),
        serde_json::Value::Bool(b) => (*b).into(),
        serde_json::Value::Number(n) => n.as_i64().ok_or("expected a whole number")?.into(),
        serde_json::Value::String(s) if s.trim().is_empty() => return Ok(None),
        serde_json::Value::String(s) => s.trim().into(),
        serde_json::Value::Array(items) => {
            let mut list = toml_edit::Array::new();
            for item in items {
                list.push(item.as_str().ok_or("expected a list of text")?);
            }
            list.into()
        }
        serde_json::Value::Object(_) => return Err("unexpected value".into()),
    }))
}

/// About this viewer, for the help's About section: where its files are
/// and how it's set up. Only pages it serves can read it.
async fn about(State(state): State<AppState>) -> Response {
    let local = tokio::task::spawn_blocking(move || {
        let (editor, css) = {
            let live = state.live.lock().unwrap();
            (live.editor.clone(), live.custom_css.clone())
        };
        crate::about::Local::gather(
            state.docs.root(),
            editor.as_deref(),
            state.docs.renderer().options(),
            css.as_deref(),
        )
    })
    .await;
    match local {
        Ok(local) => no_store(Json(local).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The document's Markdown, syntax highlighted (the source view).
/// `?plain=1` asks for it unhighlighted (instant, shown while the
/// highlighted version is made); so does the `source_highlighting` setting.
async fn source_html(State(state): State<AppState>, Query(q): Query<SourceQuery>) -> Response {
    let Some(text) = doc_source(&state, &q.doc).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let plain = q.plain.as_deref().is_some_and(|v| v != "0" && v != "false");
    let highlighted = !plain && state.live.lock().unwrap().source_highlighting;
    match tokio::task::spawn_blocking(move || crate::source::render(&text, highlighted)).await {
        Ok(html) => no_store(
            (
                [(
                    header::HeaderName::from_static("x-mdv-highlighted"),
                    if highlighted { "true" } else { "false" },
                )],
                Html(html.as_str().to_string()),
            )
                .into_response(),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
struct SourceQuery {
    #[serde(default)]
    doc: String,
    /// `plain=1`: without highlighting.
    #[serde(default)]
    plain: Option<String>,
}

/// A document's Markdown: its file, or for the page shown at `/` with no
/// file behind it (standard input, the clipboard, a web address), the text
/// the viewer keeps for it.
async fn doc_source(state: &AppState, doc: &str) -> Option<String> {
    if let Some(file) = doc_file(state, doc) {
        return tokio::fs::read_to_string(&file).await.ok();
    }
    virtual_source(state, doc)
}

fn virtual_source(state: &AppState, doc: &str) -> Option<String> {
    let main = state.main.as_ref()?;
    (doc.is_empty() || is_main(state, doc))
        .then(|| state.docs.source(main))
        .flatten()
}

/// The document's Markdown (for copying the page).
async fn source(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Response {
    let text = doc_source(&state, &q.doc).await;
    match text {
        Some(text) => {
            no_store(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response())
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Lines changed since the last git commit.
async fn gitdiff(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Response {
    let Some(file) = doc_file(&state, &q.doc) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::task::spawn_blocking(move || crate::gitdiff::diff(&file)).await {
        Ok(diff) => no_store(Json(diff).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Broken links in a document (offline checks only).
async fn check_links(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Response {
    let Some(file) = doc_file(&state, &q.doc) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let renderer = state.docs.renderer();
    let root = state.docs.root().to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let checker = crate::links::Checker::new(renderer, root);
        let (broken, external, _) = checker.check(&file);
        (broken, external.len())
    })
    .await;
    match result {
        Ok((broken, external)) => no_store(
            Json(serde_json::json!({ "broken": broken, "external": external })).into_response(),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Lines in other files that link to this document.
async fn backlinks(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Response {
    let Some(file) = doc_file(&state, &q.doc) else {
        return no_store(Json(serde_json::json!({ "hits": [] })).into_response());
    };
    let docs = state.docs.clone();
    match tokio::task::spawn_blocking(move || docs.backlinks(&file)).await {
        Ok(hits) => no_store(Json(serde_json::json!({ "hits": hits })).into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn whoami(State(state): State<AppState>) -> Response {
    Json(serde_json::json!({
        "target": state.identity.as_str(),
        "pid": std::process::id(),
    }))
    .into_response()
}

async fn list_files(State(state): State<AppState>) -> Response {
    let docs = state.docs.clone();
    match tokio::task::spawn_blocking(move || docs.list_documents()).await {
        Ok((files, truncated)) => no_store(
            Json(serde_json::json!({ "files": files, "truncated": truncated })).into_response(),
        ),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// mermaid.min.js, sent still gzip-compressed (every browser accepts that).
async fn mermaid_js() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CONTENT_ENCODING, "gzip"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        MERMAID_JS_GZ,
    )
        .into_response()
}

async fn katex_js() -> Response {
    immutable("text/javascript; charset=utf-8", KATEX_JS)
}

async fn katex_css() -> Response {
    immutable("text/css; charset=utf-8", KATEX_CSS)
}

/// A versioned asset (the URL carries the version), cacheable for good.
fn immutable(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        body,
    )
        .into_response()
}

/// SSE treats `\r` as a line break, and ignores events with empty data.
fn sse_safe(body: &str) -> String {
    let s = body.replace("\r\n", "\n").replace('\r', "\n");
    if s.is_empty() { " ".into() } else { s }
}

/// Any other path: a folder, a Markdown page, or a static file.
async fn path_handler(State(state): State<AppState>, req: Request) -> Response {
    let url_path = req.uri().path();
    let Some(path) = resolve(state.docs.root(), url_path, true) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if path.is_dir() {
        // Relative links on a folder's page need the trailing slash.
        if !url_path.ends_with('/') {
            // Built from the resolved path: the request's own could start
            // with `//`, which a browser reads as another host.
            let rel = rel_path(state.docs.root(), &path);
            let encoded: Vec<String> = rel
                .split('/')
                .map(|s| {
                    percent_encoding::utf8_percent_encode(s, crate::obsidian::SEGMENT).to_string()
                })
                .collect();
            return Redirect::permanent(&format!("/{}/", encoded.join("/"))).into_response();
        }
        return dir_page(&state, &path);
    }
    if docs::is_viewable(&path) {
        return doc_page(&state, &path);
    }
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut resp = Response::new(Body::from(bytes));
            let headers = resp.headers_mut();
            if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
                headers.insert(header::CONTENT_TYPE, v);
            }
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            // Files from the folder (an HTML page, an SVG) must not run
            // scripts on the viewer's origin, where they could call its
            // endpoints. Chrome's PDF viewer doesn't work sandboxed, and a
            // PDF runs no page scripts.
            if mime != mime_guess::mime::APPLICATION_PDF {
                headers.insert(
                    header::CONTENT_SECURITY_POLICY,
                    HeaderValue::from_static("sandbox"),
                );
            }
            no_store(resp)
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Map a URL path to a file (or, with `allow_dirs`, a folder) inside
/// `root`, refusing anything that escapes it (`..`, absolute paths, or
/// symlinks pointing outside).
fn resolve(root: &Path, url_path: &str, allow_dirs: bool) -> Option<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(url_path)
        .decode_utf8()
        .ok()?;
    let rel = decoded.trim_start_matches('/');
    let candidate = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel).canonical().ok()?
    };
    let ok_kind = candidate.is_file() || (allow_dirs && candidate.is_dir());
    (candidate.starts_with(root) && ok_kind).then_some(candidate)
}

/// `/`-separated path of `path` relative to `root` ("" for the root).
fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn no_store(mut resp: Response) -> Response {
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

pub fn file_title(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "markdown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_stays_inside_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonical().unwrap();
        std::fs::write(root.join("a.png"), b"x").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/b c.png"), b"x").unwrap();

        assert_eq!(resolve(&root, "/a.png", false), Some(root.join("a.png")));
        assert_eq!(
            resolve(&root, "/sub/b%20c.png", false),
            Some(root.join("sub/b c.png"))
        );
        assert_eq!(resolve(&root, "/../etc/passwd", false), None);
        assert_eq!(resolve(&root, "/sub/../../etc/passwd", false), None);
        assert_eq!(resolve(&root, "/%2e%2e/etc/passwd", false), None);
        assert_eq!(resolve(&root, "/sub", false), None);
        assert_eq!(resolve(&root, "/sub", true), Some(root.join("sub")));
        assert_eq!(resolve(&root, "/", true), Some(root.clone()));
        assert_eq!(resolve(&root, "/..", true), None);
        assert_eq!(resolve(&root, "/", false), None);
    }

    #[test]
    fn rel_paths_use_slashes() {
        let root = Path::new("/r");
        assert_eq!(rel_path(root, Path::new("/r/a/b.md")), "a/b.md");
        assert_eq!(rel_path(root, Path::new("/r")), "");
    }

    #[test]
    fn sse_safe_normalises() {
        assert_eq!(sse_safe("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(sse_safe(""), " ");
    }
}
