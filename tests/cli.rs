//! End-to-end tests that run the real binary.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_markdown-viewer");

/// Render the GFM fixture to stdout and snapshot the body (the page chrome
/// is mostly embedded CSS and a random nonce, so it's excluded).
#[test]
fn renders_fixture_body() {
    let out = Command::new(BIN)
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
    let out = Command::new(BIN).arg("does-not-exist.md").output().unwrap();
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

    let mut child = Command::new(BIN)
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
    let out = Command::new(BIN)
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

    let mut child = Command::new(BIN)
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

    let mut child = Command::new(BIN)
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
