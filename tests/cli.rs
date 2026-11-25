//! End-to-end tests that run the real binary.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_markdown-viewer");

/// This test's private folder for settings and state.
fn test_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir()
        .join(format!(
            "mdv-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
        .to_string_lossy()
        .replace(['(', ')'], "")
        .into();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The binary, isolated from the user's settings file and remembered
/// preferences (each test gets its own, empty, state).
fn mdv() -> Command {
    let mut cmd = Command::new(BIN);
    let dir = test_dir();
    cmd.env("MARKDOWN_VIEWER_CONFIG", dir.join("config.toml"))
        .env("MARKDOWN_VIEWER_STATE", dir.join("state.json"));
    cmd
}

/// Render the GFM fixture to stdout and snapshot the body (the page chrome
/// is mostly embedded CSS and a random nonce, so it's excluded).
#[test]
fn renders_fixture_body() {
    let out = mdv()
        .args(["tests/fixtures/gfm.md", "-o", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    let start = html.find("<article class=\"markdown-body\">").unwrap();
    let end = html.find("</article>").unwrap();
    insta::assert_snapshot!(&html[start..end]);
    assert!(!html.contains("front matter is hidden"));
    // The fixture has a diagram, so the standalone page inlines mermaid.
    assert!(html.contains("<pre class=\"mermaid\">graph LR"));
    assert!(html.contains("globalThis[\"mermaid\"]"));
    assert!(
        !html.contains("/__mdv/"),
        "standalone page must not need the server"
    );
}

#[test]
fn missing_file_is_an_error() {
    let out = mdv().arg("does-not-exist.md").output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot open"));
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http_get(addr: &str, path: &str, host: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    write!(
        s,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
fn watch_serves_and_pushes_updates() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# First\n\n![img](pic.png)\n").unwrap();
    std::fs::write(dir.path().join("pic.png"), b"\x89PNG").unwrap();

    let mut child = mdv()
        .args(["--watch", "--no-open"])
        .arg(&md)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    // Keep our end of the pipe open for the whole test, as a terminal would.
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let _guard = Running(child);

    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    let url = first.split(" at ").nth(1).unwrap().trim();
    let addr = url
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();

    // Page, live-reload script, static file, sandboxing, host check.
    let page = http_get(&addr, "/", &addr);
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("First"));
    assert!(page.contains("/__mdv/events"));
    assert!(http_get(&addr, "/pic.png", &addr).starts_with("HTTP/1.1 200"));
    assert!(http_get(&addr, "/../../etc/passwd", &addr).starts_with("HTTP/1.1 404"));
    assert!(http_get(&addr, "/", "evil.example:80").starts_with("HTTP/1.1 403"));
    let mermaid = http_get(&addr, "/__mdv/mermaid.js?v=1", &addr);
    assert!(mermaid.starts_with("HTTP/1.1 200"), "{}", &mermaid[..200]);
    assert!(
        mermaid
            .to_ascii_lowercase()
            .contains("content-encoding: gzip")
    );

    // Subscribe to events, then change the file like an editor would
    // (write a temp file and rename it over the original).
    let mut sse = TcpStream::connect(&addr).unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
    let mut reader = BufReader::new(sse);
    wait_for(&mut reader, "First");

    let tmp = dir.path().join(".doc.md.swp");
    std::fs::write(&tmp, "# Second\n").unwrap();
    std::fs::rename(&tmp, &md).unwrap();
    wait_for(&mut reader, "Second");
}

fn wait_for(reader: &mut impl BufRead, needle: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line.starts_with("data:") && line.contains(needle) {
            return;
        }
    }
    panic!("never saw {needle:?} in event stream");
}

/// examples/example.md is the feature tour; make sure every feature in it
/// still renders.
#[test]
fn example_renders_every_feature() {
    let out = mdv()
        .args(["examples/example.md", "-o", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    let body = &html[html.find("<article").unwrap()..html.find("</article>").unwrap()];
    let count = |needle: &str| body.matches(needle).count();

    assert!(!body.contains("YAML front matter"), "front matter hidden");
    assert_eq!(count("<pre class=\"mermaid\">"), 10, "mermaid diagrams");
    assert_eq!(
        count("markdown-alert-"),
        5 * 2,
        "five alerts (div + title class)"
    );
    assert_eq!(count("type=\"checkbox\""), 6, "task list items");
    assert_eq!(count("data-math-style=\"inline\""), 4, "inline math");
    assert_eq!(count("data-math-style=\"display\""), 3, "display math");
    assert!(body.contains("$5 or $10"), "dollar amounts stay text");
    assert_eq!(count("<table>"), 1);
    assert_eq!(count("class=\"footnote-ref\""), 2);
    for tag in ["<h1", "<h2", "<h3", "<h4", "<h5", "<h6"] {
        assert!(body.contains(tag), "heading {tag}");
    }
    for needle in [
        "<del>strikethrough</del>",
        "<em><strong>bold italic</strong></em>",
        "<br />",
        "<a href=\"https://github.github.com/gfm/\">",
        "<a href=\"other.md\">",
        "<img src=\"images/logo.svg\"",
        "images/logo.svg#gh-dark-mode-only",
        "octicon-light-bulb",
        "class=\"octicon octicon-link\"",
        "<p>A blockquote.</p>\n<blockquote>",
        "<ol start=\"3\">",
        "<details>",
        "<kbd>Ctrl</kbd>",
        "<sub>2</sub>",
        "🚀",
        "👍",
        "lang=\"rust\"",
        "hl-",
    ] {
        assert!(body.contains(needle), "missing {needle:?}");
    }
    // The page CSP allows only nonce'd scripts, so the example's <script> can't run.
    assert!(html.contains("script-src 'nonce-"));
}

/// Ctrl-C (SIGINT) tells connected pages to close before the server exits.
#[cfg(unix)]
#[test]
fn ctrl_c_sends_close_event_then_exits() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();

    let mut child = mdv()
        .args(["--watch", "--no-open"])
        .arg(&md)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    let addr = first
        .split(" at http://")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches('/')
        .to_string();

    // --no-open means we didn't open the window, so pages must not close it.
    let page = http_get(&addr, "/", &addr);
    assert!(page.contains("data-close-on-exit=\"false\""));

    let mut sse = TcpStream::connect(&addr).unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
    let mut reader = BufReader::new(sse);
    wait_for(&mut reader, "Hi");

    let status = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut line = String::new();
    let mut saw_close = false;
    while Instant::now() < deadline {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line.trim() == "event: close" {
            saw_close = true;
            break;
        }
    }
    assert!(saw_close, "no close event before shutdown");
    assert!(child.wait().unwrap().success(), "clean exit on Ctrl-C");
}

/// Closing stdout (e.g. piping into `head -1`) must not take the server down.
#[test]
fn server_survives_closed_stdout() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Still here\n").unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let mut child = mdv()
        .args(["--watch", "--no-open", "--port", &port.to_string()])
        .arg(&md)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // close the pipe before it prints anything
    let _guard = Running(child);

    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect(&addr).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "server never came up");
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(300)); // past both status lines
    let page = http_get(&addr, "/", &addr);
    assert!(
        page.contains("Still here"),
        "server died after stdout closed"
    );
}

/// --exit-when-closed keeps running while a page is connected, rides out a
/// quick reconnect, and exits once the page has been gone for a while.
#[test]
fn exit_when_closed_after_page_goes_away() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();

    let mut child = mdv()
        .args(["--watch", "--no-open", "--exit-when-closed"])
        .arg(&md)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    let addr = first
        .split(" at http://")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches('/')
        .to_string();

    let connect = || {
        let mut sse = TcpStream::connect(&addr).unwrap();
        sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
        let mut reader = BufReader::new(sse);
        wait_for(&mut reader, "Hi");
        reader
    };

    // A page connects, then "reloads": gone briefly, then back.
    let page = connect();
    drop(page);
    std::thread::sleep(Duration::from_millis(500));
    let page = connect();
    std::thread::sleep(Duration::from_secs(6));
    assert!(
        child.try_wait().unwrap().is_none(),
        "exited while a page was open"
    );

    // The page closes for good: the server should exit on its own.
    drop(page);
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "still running after the page closed"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(status.success());
    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    assert!(rest.contains("Page closed; exiting."), "{rest}");
}

/// A running `--watch` server and its address.
struct Served {
    addr: String,
    _stdout: BufReader<std::process::ChildStdout>,
    _guard: Running,
}

fn start(mut cmd: Command) -> Served {
    let mut child = cmd.arg("--no-open").stdout(Stdio::piped()).spawn().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    let addr = first
        .split(" at http://")
        .nth(1)
        .unwrap_or_else(|| panic!("unexpected output: {first}"))
        .trim()
        .trim_end_matches('/')
        .to_string();
    Served {
        addr,
        _stdout: stdout,
        _guard: Running(child),
    }
}

fn http(addr: &str, request: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).into_owned()
}

#[test]
fn folder_mode_serves_readmes_listings_and_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("README.md"), "# Project home\n").unwrap();
    std::fs::create_dir_all(root.join("docs/sub folder")).unwrap();
    std::fs::write(root.join("docs/guide.md"), "# Guide v1\n").unwrap();
    std::fs::write(root.join("docs/sub folder/deep.md"), "# Deep\n").unwrap();

    let server = start({
        let mut c = mdv();
        c.arg(root);
        c
    });
    let a = &server.addr;

    let home = http_get(a, "/", a);
    assert!(home.contains("Project home"), "README shown at /");
    assert!(home.contains("data-doc=\"README.md\""), "{home}");

    let redirect = http_get(a, "/docs", a);
    assert!(redirect.starts_with("HTTP/1.1 308"), "{redirect}");
    assert!(redirect.to_lowercase().contains("location: /docs/"));

    let listing = http_get(a, "/docs/", a);
    assert!(
        listing.contains("guide.md"),
        "folder without README lists files"
    );
    assert!(listing.contains("sub folder/"), "{listing}");

    let files = http_get(a, "/__mdv/files", a);
    let json = files.split("\r\n\r\n").nth(1).unwrap();
    let json: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        json["files"],
        serde_json::json!(["README.md", "docs/guide.md", "docs/sub folder/deep.md"])
    );

    // Any linked page is live, not just the first one.
    let mut sse = TcpStream::connect(a).unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(
        sse,
        "GET /__mdv/events?doc=docs%2Fguide.md HTTP/1.1\r\nHost: {a}\r\n\r\n"
    )
    .unwrap();
    let mut reader = BufReader::new(sse);
    wait_for(&mut reader, "Guide v1");
    std::fs::write(root.join("docs/guide.md"), "# Guide v2\n").unwrap();
    wait_for(&mut reader, "Guide v2");

    // Events for paths outside the folder are refused.
    let bad = http_get(a, "/__mdv/events?doc=..%2F..%2Fetc%2Fpasswd", a);
    assert!(bad.starts_with("HTTP/1.1 404"), "{bad}");
}

#[test]
fn theme_and_sidebar_choices_are_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let state = test_dir().join("state.json");
    let _ = std::fs::remove_file(&state);

    let server = start({
        let mut c = mdv();
        c.args(["--watch"]).arg(&md);
        c
    });
    let a = &server.addr;
    assert!(http_get(a, "/", a).contains("data-mdv-theme=\"auto\""));

    let body = r#"{"theme":"dark","sidebar":"contents"}"#;
    let post = |extra: &str| {
        http(
            a,
            &format!(
                "POST /__mdv/prefs HTTP/1.1\r\nHost: {a}\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
        )
    };
    assert!(
        post("").starts_with("HTTP/1.1 403"),
        "needs the custom header"
    );
    assert!(post("X-MDV: 1\r\n").starts_with("HTTP/1.1 204"));

    let page = http_get(a, "/", a);
    assert!(
        page.contains("data-mdv-theme=\"dark\""),
        "applied this session"
    );
    assert!(page.contains("data-mdv-sidebar=\"contents\""));
    let saved = std::fs::read_to_string(&state).unwrap();
    assert!(
        saved.contains("\"dark\"") && saved.contains("contents"),
        "{saved}"
    );
    drop(server);

    // A new run starts with the remembered theme, unless --theme says otherwise.
    let again = start({
        let mut c = mdv();
        c.args(["--watch"]).arg(&md);
        c
    });
    assert!(http_get(&again.addr, "/", &again.addr).contains("data-mdv-theme=\"dark\""));
    let explicit = start({
        let mut c = mdv();
        c.args(["--watch", "--theme", "light"]).arg(&md);
        c
    });
    assert!(http_get(&explicit.addr, "/", &explicit.addr).contains("data-mdv-theme=\"light\""));
}

#[test]
fn settings_file_style_and_css_apply() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let css = dir.path().join("mine.css");
    std::fs::write(&css, ".markdown-body { --my-marker: 1; }").unwrap();
    std::fs::write(
        test_dir().join("config.toml"),
        format!("theme = \"dark\"\nstyle = \"book\"\ncss = {:?}\n", css),
    )
    .unwrap();

    let out = mdv().arg(&md).args(["-o", "-"]).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(html.contains("data-mdv-theme=\"dark\""));
    assert!(html.contains("font-family: Charter"), "book style");
    assert!(html.contains("--my-marker: 1"), "custom CSS");

    // Command-line options win over the settings file.
    let out = mdv()
        .arg(&md)
        .args(["-o", "-", "--style", "wide", "--theme", "light"])
        .output()
        .unwrap();
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(html.contains("data-mdv-theme=\"light\""));
    assert!(html.contains("max-width: none") && !html.contains("font-family: Charter"));

    // Typos in the settings file are reported, not ignored.
    std::fs::write(test_dir().join("config.toml"), "them = \"dark\"\n").unwrap();
    let out = mdv().arg(&md).args(["-o", "-"]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("config.toml") && err.contains("them"), "{err}");
    std::fs::remove_file(test_dir().join("config.toml")).unwrap();
}

#[test]
fn viewer_ui_is_in_viewed_pages_but_not_exports() {
    let md = std::path::Path::new("tests/fixtures/gfm.md");
    let export = mdv().arg(md).args(["-o", "-"]).output().unwrap();
    let export = String::from_utf8(export.stdout).unwrap();
    assert!(!export.contains("mdv-toolbar"), "exports stay clean");

    let opened = mdv().arg(md).arg("--no-open").output().unwrap();
    assert!(opened.status.success());
    let path = String::from_utf8(opened.stdout).unwrap();
    let page = std::fs::read_to_string(path.trim()).unwrap();
    assert!(page.contains("mdv-toolbar"), "viewer UI included");
    assert!(page.contains("data-served=\"false\""));
}

#[test]
fn pdf_export() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out.pdf");
    let result = mdv()
        .args(["examples/example.md", "--pdf"])
        .arg(&out)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&result.stderr);
    if err.contains("none was found") {
        eprintln!("skipping: no Chromium-based browser installed");
        return;
    }
    assert!(result.status.success(), "{err}");
    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.starts_with(b"%PDF"), "not a PDF");
    assert!(
        pdf.len() > 50_000,
        "suspiciously small: {} bytes",
        pdf.len()
    );
}
