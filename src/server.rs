//! Local HTTP server: the rendered page, a live-reload event stream, and
//! static files from the Markdown file's directory (for relative images).

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;

use crate::render::{
    KATEX_CSS, KATEX_CSS_PATH, KATEX_JS, KATEX_JS_PATH, MERMAID_JS_GZ, MERMAID_PATH, PageOptions,
    Renderer,
};

pub const EVENTS_PATH: &str = "/__mdv/events";

#[derive(Clone)]
struct AppState {
    renderer: Arc<Renderer>,
    page: PageOptions,
    /// Canonical directory that static files are served from.
    root: PathBuf,
    body: watch::Receiver<String>,
    port: u16,
    /// Flips to true when the server is shutting down.
    closing: watch::Receiver<bool>,
    /// Whether pages should close their window on shutdown (only windows
    /// we opened ourselves, never a tab the user was already using).
    close_on_exit: Arc<AtomicBool>,
    /// Number of pages currently connected to the event stream.
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

    /// Resolves once nobody is viewing the page any more: a page connected
    /// and then every page has been gone for `grace` (long enough to ride
    /// out a reload). Also resolves if no page connects within
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
            // Gone; give a reloading page time to reconnect.
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
    pub async fn bind(
        port: u16,
        root: &Path,
        renderer: Arc<Renderer>,
        page: PageOptions,
        body: watch::Receiver<String>,
    ) -> Result<Self> {
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
            renderer,
            page,
            root: root.canonicalize()?,
            body,
            port: addr.port(),
            closing,
            close_on_exit,
            clients: Arc::new(clients_tx),
        };
        let router = Router::new()
            .route("/", get(index))
            .route(EVENTS_PATH, get(events))
            .route(MERMAID_PATH, get(mermaid_js))
            .route(KATEX_JS_PATH, get(katex_js))
            .route(KATEX_CSS_PATH, get(katex_css))
            .fallback(get(static_file))
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
    let body = state.body.borrow().clone();
    let page = PageOptions {
        close_on_exit: state.close_on_exit.load(Ordering::Relaxed),
        ..state.page.clone()
    };
    no_store(Html(state.renderer.wrap_page(&body, &page)).into_response())
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    // WatchStream yields the current value first, so a reconnecting page
    // (e.g. after the laptop slept) immediately catches up.
    let updates = WatchStream::new(state.body.clone())
        .map(|body| Ok(Event::default().event("update").data(sse_safe(&body))));
    let shutdown = WatchStream::from_changes(state.closing.clone())
        .filter(|closing| *closing)
        .map(|_| Ok(Event::default().event("close").data("bye")));
    // The guard lives as long as the stream, i.e. until the page goes away.
    let guard = ClientGuard::new(state.clients.clone());
    let stream = updates.merge(shutdown).map(move |event| {
        let _alive = &guard;
        event
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

/// mermaid.min.js, sent still gzip-compressed (every browser accepts that).
/// The URL carries the version, so it can be cached for good.
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

async fn static_file(State(state): State<AppState>, req: Request) -> Response {
    let Some(path) = resolve(&state.root, req.uri().path()) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let is_markdown = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"));
    if is_markdown {
        // Linked Markdown files render as pages too (without live reload).
        let Ok(text) = tokio::fs::read_to_string(&path).await else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let page = PageOptions {
            title: file_title(&path),
            live: false,
            ..state.page.clone()
        };
        return no_store(Html(state.renderer.render_page(&text, &page)).into_response());
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

/// Map a URL path to a file inside `root`, refusing anything that escapes it
/// (`..`, absolute paths, or symlinks pointing outside).
fn resolve(root: &Path, url_path: &str) -> Option<PathBuf> {
    let decoded = percent_encoding::percent_decode_str(url_path)
        .decode_utf8()
        .ok()?;
    let rel = decoded.trim_start_matches('/');
    if rel.is_empty() {
        return None;
    }
    let candidate = root.join(rel).canonicalize().ok()?;
    (candidate.starts_with(root) && candidate.is_file()).then_some(candidate)
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

        assert_eq!(resolve(&root, "/a.png"), Some(root.join("a.png")));
        assert_eq!(
            resolve(&root, "/sub/b%20c.png"),
            Some(root.join("sub/b c.png"))
        );
        assert_eq!(resolve(&root, "/../etc/passwd"), None);
        assert_eq!(resolve(&root, "/sub/../../etc/passwd"), None);
        assert_eq!(resolve(&root, "/%2e%2e/etc/passwd"), None);
        assert_eq!(resolve(&root, "/sub"), None);
        assert_eq!(resolve(&root, "/"), None);
    }

    #[test]
    fn sse_safe_normalises() {
        assert_eq!(sse_safe("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(sse_safe(""), " ");
    }
}
