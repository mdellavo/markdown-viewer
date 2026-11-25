//! Local HTTP server for `--watch`: rendered pages for every Markdown file
//! and folder under the root, a live-update stream per document, the
//! viewer's preferences and file list, and static files (for images).

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
    Renderer,
};

pub const EVENTS_PATH: &str = "/__mdv/events";
const PREFS_PATH: &str = "/__mdv/prefs";
const FILES_PATH: &str = "/__mdv/files";
const SEARCH_PATH: &str = "/__mdv/search";
const OPEN_PATH: &str = "/__mdv/open";
const POSITION_PATH: &str = "/__mdv/position";
const GOTO_PATH: &str = "/__mdv/goto";
const SOURCE_PATH: &str = "/__mdv/source";
const GITDIFF_PATH: &str = "/__mdv/gitdiff";
const CHECK_PATH: &str = "/__mdv/check";
const BACKLINKS_PATH: &str = "/__mdv/backlinks";

/// What to serve and how.
pub struct ServeOptions {
    pub port: u16,
    pub docs: Arc<Docs>,
    pub renderer: Arc<Renderer>,
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
}

#[derive(Clone)]
struct AppState {
    docs: Arc<Docs>,
    renderer: Arc<Renderer>,
    main: Option<PathBuf>,
    page: PageOptions,
    custom_css: Option<PathBuf>,
    prefs: Arc<Mutex<Prefs>>,
    remember: bool,
    editor: Option<String>,
    identity: Arc<String>,
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
    pub async fn abandoned(&self, grace: Duration, first_connect: Duration) {
        let mut clients = self.clients.clone();
        // Each wait yields only whether the condition was met, so no borrow
        // of the channel outlives its statement.
        let connected = tokio::time::timeout(first_connect, clients.wait_for(|n| *n > 0))
            .await
            .is_ok_and(|r| r.is_ok());
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
            renderer: opts.renderer,
            main: opts.main,
            page: opts.page,
            custom_css: opts.custom_css,
            prefs: Arc::new(Mutex::new(opts.prefs)),
            remember: opts.remember,
            editor: opts.editor,
            identity: Arc::new(opts.identity),
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
            .route(SOURCE_PATH, get(source))
            .route(GITDIFF_PATH, get(gitdiff))
            .route(CHECK_PATH, get(check_links))
            .route(BACKLINKS_PATH, get(backlinks))
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
    let custom_css = state
        .custom_css
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok());
    let resume_line = prefs
        .positions
        .get(doc.to_string_lossy().as_ref())
        .map(|p| p.line);
    let page = PageOptions {
        title: file_title(doc),
        theme: prefs.theme.unwrap_or(state.page.theme),
        sidebar: prefs.sidebar,
        resume_line,
        font_scale: prefs.font_scale,
        git_changes: prefs.git.unwrap_or(false),
        doc: Some(rel_path(state.docs.root(), doc)),
        close_on_exit: state.close_on_exit.load(Ordering::Relaxed),
        custom_css: custom_css.or_else(|| state.page.custom_css.clone()),
        ..state.page.clone()
    };
    no_store(Html(state.renderer.wrap_page(&body, &page)).into_response())
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
        resolve(state.docs.root(), &q.doc, true).filter(|p| p.is_dir() || docs::is_markdown(p))
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
        let _ = tokio::task::spawn_blocking(move || {
            let mut saved = Prefs::load();
            saved.merge(update);
            saved.save()
        })
        .await;
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
    match crate::editor::open(&file, req.line, state.editor.as_deref()) {
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
    let Ok(file) = req.file.canonicalize() else {
        return StatusCode::NOT_FOUND;
    };
    if !file.starts_with(root) || !docs::is_markdown(&file) {
        return StatusCode::NOT_FOUND;
    }
    // Pages compare this with their own document (data-doc).
    let _ = state.goto.send((rel_path(root, &file), req.line.max(1)));
    StatusCode::NO_CONTENT
}

/// The document's Markdown (for copying the page).
async fn source(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Response {
    let text = match doc_file(&state, &q.doc) {
        Some(file) => tokio::fs::read_to_string(&file).await.ok(),
        // Standard input and the clipboard have no file.
        None => None,
    };
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
    let renderer = state.renderer.clone();
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
    match tokio::task::spawn_blocking(move || docs.list_files()).await {
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
            return Redirect::permanent(&format!("{url_path}/")).into_response();
        }
        return dir_page(&state, &path);
    }
    if docs::is_markdown(&path) {
        return doc_page(&state, &path);
    }
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            let mut resp = Response::new(Body::from(bytes));
            if let Ok(v) = HeaderValue::from_str(mime.as_ref()) {
                resp.headers_mut().insert(header::CONTENT_TYPE, v);
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
        root.join(rel).canonicalize().ok()?
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
        let root = dir.path().canonicalize().unwrap();
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
