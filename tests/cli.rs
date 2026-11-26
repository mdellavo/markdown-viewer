//! End-to-end tests that run the real binary.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

mod common;
use common::*;

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
    assert!(html.contains("<th>title</th>"), "front matter table");
    assert!(
        !html[start..end].contains("data-sourcepos"),
        "exports carry no source positions"
    );
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

    assert!(
        body.contains("class=\"mdv-front-matter\""),
        "front matter table"
    );
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
    assert_eq!(count("<table>"), 2, "the table, plus the front matter's");
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
        "<nav class=\"mdv-toc\"",
        "href=\"cities.csv\"",
        "<figcaption class=\"mdv-code-title\">geometry.py</figcaption>",
        "mdv-line mdv-hl",
        "mdv-numbered",
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

/// The shell completions and man page in the repo (installed by Homebrew
/// and the Linux packages) must match the current options. To update them:
/// `UPDATE_GENERATED=1 cargo test --test cli generated_files`.
#[test]
fn generated_files_are_up_to_date() {
    let files: [(&[&str], &str); 5] = [
        (
            &["--generate-completions", "bash"],
            "completions/markdown-viewer.bash",
        ),
        (
            &["--generate-completions", "zsh"],
            "completions/_markdown-viewer",
        ),
        (
            &["--generate-completions", "fish"],
            "completions/markdown-viewer.fish",
        ),
        (
            &["--generate-completions", "powershell"],
            "completions/_markdown-viewer.ps1",
        ),
        (&["--generate-man"], "man/markdown-viewer.1"),
    ];
    let update = std::env::var_os("UPDATE_GENERATED").is_some();
    for (args, path) in files {
        let out = mdv().args(args).output().unwrap();
        assert!(out.status.success());
        if update {
            std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
            std::fs::write(path, &out.stdout).unwrap();
        } else {
            let committed = std::fs::read(path).unwrap_or_default();
            assert!(
                committed == out.stdout,
                "{path} is out of date; run: UPDATE_GENERATED=1 cargo test --test cli generated_files"
            );
        }
    }
}

fn post(addr: &str, path: &str, json: &str) -> String {
    http(
        addr,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nX-MDV: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        ),
    )
}

#[test]
fn second_launch_reuses_the_running_viewer() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let first = start({
        let mut c = mdv();
        c.arg("--watch").arg(&md);
        c
    });
    let again = mdv()
        .args(["--watch", "--no-open"])
        .arg(&md)
        .output()
        .unwrap();
    assert!(again.status.success());
    let out = String::from_utf8_lossy(&again.stdout);
    assert!(
        out.contains(&format!("Already open at http://{}/", first.addr)),
        "{out}"
    );
}

#[test]
fn reads_standard_input() {
    let mut child = mdv()
        .args(["-", "-o", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"# From stdin\n\n$x^2$\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(html.contains("<h1 id=\"from-stdin\"") && html.contains("data-math-style"));
    assert!(html.contains("<title>stdin</title>"));
}

#[test]
fn numbered_headings_and_front_matter_options() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "---\ntitle: T\n---\n# Title\n\n## A\n\n## B\n").unwrap();
    // Just the <html> tag and the document body (the stylesheet also
    // mentions data-mdv-number).
    let html = |args: &[&str]| {
        let out = mdv()
            .arg(&md)
            .args(["-o", "-"])
            .args(args)
            .output()
            .unwrap();
        let page = String::from_utf8(out.stdout).unwrap();
        let tag = page[page.find("<html").unwrap()..]
            .split('>')
            .next()
            .unwrap()
            .to_string();
        tag + &page[page.find("<article").unwrap()..]
    };
    let plain = html(&[]);
    assert!(!plain.contains("data-mdv-number"));
    assert!(plain.contains("mdv-front-matter"), "table by default");
    let numbered = html(&["--number-headings", "--front-matter", "hide"]);
    assert!(
        numbered.contains("data-mdv-number=\"h2\""),
        "one h1: number from h2"
    );
    assert!(!numbered.contains("class=\"mdv-front-matter\""));
}

#[test]
fn obsidian_vault_links_resolve_and_open() {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path();
    std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
    std::fs::create_dir_all(vault.join("notes/deep")).unwrap();
    std::fs::write(
        vault.join("notes/Home.md"),
        "# Home\n\nSee [[Other Note]] and [[Missing]].\n\n> [!tip] Hint\n> ==marked== %%secret%%\n",
    )
    .unwrap();
    std::fs::write(vault.join("notes/deep/Other Note.md"), "# Other\n").unwrap();

    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(vault.join("notes/Home.md"));
        c
    });
    let a = &server.addr;
    let page = http_get(a, "/", a);
    assert!(
        page.contains("href=\"/notes/deep/Other%20Note.md\""),
        "vault-wide link"
    );
    assert!(page.contains("mdv-wikilink-missing"));
    assert!(page.contains("class=\"callout\"") && page.contains("<mark"));
    assert!(!page.contains("secret"), "comments hidden");
    // The whole vault is served, so the link works.
    assert!(http_get(a, "/notes/deep/Other%20Note.md", a).contains("<h1 id=\"other\""));

    // Outside a vault (or with --no-obsidian) it's plain Markdown.
    let out = mdv()
        .arg(vault.join("notes/Home.md"))
        .args(["-o", "-", "--no-obsidian"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&out.stdout).contains("data-wikilink"));
}

#[test]
fn search_positions_and_click_to_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("README.md"), "# Readme\n\nNothing here.\n").unwrap();
    std::fs::write(
        root.join("guide.md"),
        "# Guide\n\nIntro.\n\n## Setup\n\nInstall the Widget first.\n",
    )
    .unwrap();
    // A fake editor that records what it was asked to open.
    let log = root.join("editor.log");
    let editor = fake_editor(root, &log);

    let server = start({
        let mut c = mdv();
        c.arg(root).arg("--editor").arg(&editor);
        c
    });
    let a = &server.addr;

    // Served pages carry source positions.
    assert!(http_get(a, "/guide.md", a).contains("data-sourcepos=\"7:1-7:25\""));

    let hits = http_get(a, "/__mdv/search?q=widget", a);
    let json: serde_json::Value =
        serde_json::from_str(hits.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(json["hits"][0]["file"], "guide.md");
    assert_eq!(json["hits"][0]["line"], 7);

    // Click-to-source runs the editor at the right file and line.
    assert!(post(a, "/__mdv/open", r#"{"doc":"guide.md","line":7}"#).starts_with("HTTP/1.1 204"));
    let deadline = Instant::now() + Duration::from_secs(5);
    // The script creates the file before writing to it: wait for content.
    while std::fs::read_to_string(&log).map_or(true, |t| t.trim().is_empty())
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    let opened = std::fs::read_to_string(&log).unwrap();
    assert!(opened.trim().ends_with("guide.md:7"), "{opened}");
    assert!(
        post(a, "/__mdv/open", r#"{"doc":"../../etc/passwd","line":1}"#)
            .starts_with("HTTP/1.1 404")
    );

    // A saved reading position comes back when the page is opened again.
    assert!(
        post(a, "/__mdv/position", r#"{"doc":"guide.md","line":5}"#).starts_with("HTTP/1.1 204")
    );
    assert!(http_get(a, "/guide.md", a).contains("data-mdv-resume=\"5\""));
}

#[test]
fn check_links_reports_broken_ones() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("a.md"),
        "# A\n\n## Part\n\n[ok](b.md) [self](#part)\n",
    )
    .unwrap();
    std::fs::write(
        root.join("b.md"),
        "# B\n\n[gone](missing.md)\n\n[bad anchor](a.md#nope) [web](https://example.com)\n",
    )
    .unwrap();

    let out = mdv().arg("--check-links").arg(root).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("b.md:3: missing.md (file not found)"),
        "{text}"
    );
    assert!(
        text.contains("b.md:5: a.md#nope (no heading or anchor #nope)"),
        "{text}"
    );
    assert!(
        text.contains("Checked 5 links in 2 files: 2 broken. 1 web links not checked"),
        "{text}"
    );

    let ok = mdv()
        .arg("--check-links")
        .arg(root.join("a.md"))
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stdout)
    );
}

#[test]
fn goto_scrolls_a_running_viewer() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n\nOne.\n\nTwo.\n").unwrap();
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&md);
        c
    });
    let a = &server.addr;
    let mut sse = TcpStream::connect(a).unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {a}\r\n\r\n").unwrap();
    let mut reader = BufReader::new(sse);
    wait_for(&mut reader, "Hi");

    let out = mdv()
        .arg("--goto")
        .arg(format!("{}:5", md.display()))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains(&format!("at http://{a}/")));
    wait_for(&mut reader, "\"line\":5");
}

#[test]
fn toc_and_code_block_extras_render() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(
        &md,
        "# Doc\n\n[TOC]\n\n## First\n\n```rust title=\"main.rs\" {2}\nfn a() {}\nfn b() {}\n```\n\n## Second\n\n```\nplain\n```\n",
    )
    .unwrap();
    let render = |args: &[&str]| {
        let out = mdv()
            .arg(&md)
            .args(["-o", "-"])
            .args(args)
            .output()
            .unwrap();
        let page = String::from_utf8(out.stdout).unwrap();
        page[page.find("<article").unwrap()..].to_string()
    };
    let html = render(&[]);
    assert!(html.contains("<nav class=\"mdv-toc\""), "{html}");
    assert!(
        html.contains("<a href=\"#first\">First</a>")
            && html.contains("<a href=\"#second\">Second</a>")
    );
    assert!(html.contains("<figcaption class=\"mdv-code-title\">main.rs</figcaption>"));
    assert!(html.contains("mdv-line mdv-hl"), "line 2 highlighted");
    assert!(!html.contains("mdv-numbered"), "numbers are opt-in");
    let numbered = render(&["--line-numbers"]);
    assert_eq!(numbered.matches("mdv-numbered").count(), 2, "every block");
}

#[test]
fn vault_embeds_backlinks_git_source_and_text_size() {
    let dir = tempfile::tempdir().unwrap();
    let vault = dir.path();
    std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
    std::fs::write(
        vault.join("Home.md"),
        "# Home\n\n![[Other]]\n\nSee [[Other#Two|part two]].\n",
    )
    .unwrap();
    std::fs::write(
        vault.join("Other.md"),
        "---\ntags: [x]\n---\n# Other\n\nEmbedded text.\n\n## Two\n\nSecond part.\n",
    )
    .unwrap();
    let git_ok = Command::new("git")
        .arg("-C")
        .arg(vault)
        .args(["init", "-q"])
        .status()
        .is_ok_and(|s| s.success());
    if git_ok {
        for args in [
            &["config", "user.email", "t@example.com"][..],
            &["config", "user.name", "T"],
            &["add", "."],
            &["commit", "-q", "-m", "x"],
        ] {
            Command::new("git")
                .arg("-C")
                .arg(vault)
                .args(args)
                .status()
                .unwrap();
        }
        std::fs::write(
            vault.join("Home.md"),
            "# Home\n\n![[Other]]\n\nSee [[Other#Two|part two]].\n\nNew line.\n",
        )
        .unwrap();
    }

    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(vault.join("Home.md"));
        c
    });
    let a = &server.addr;
    let page = http_get(a, "/", a);
    assert!(
        page.contains("<div class=\"mdv-embed-note\">"),
        "note embedded"
    );
    assert!(page.contains("Embedded text."));
    assert!(
        !page.contains("<th>tags</th>"),
        "embedded front matter left out"
    );

    let body = |resp: String| resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    let links: serde_json::Value =
        serde_json::from_str(&body(http_get(a, "/__mdv/backlinks?doc=Other.md", a))).unwrap();
    assert_eq!(links["hits"][0]["file"], "Home.md", "{links}");

    let source = body(http_get(a, "/__mdv/source?doc=Home.md", a));
    assert!(source.starts_with("# Home\n"), "{source}");

    if git_ok {
        let diff: serde_json::Value =
            serde_json::from_str(&body(http_get(a, "/__mdv/gitdiff?doc=Home.md", a))).unwrap();
        assert_eq!(diff["status"], "ok", "{diff}");
        assert_eq!(diff["changes"][0]["kind"], "added", "{diff}");
        assert_eq!(diff["changes"][0]["start"], 6, "{diff}");
    }

    assert!(post(a, "/__mdv/prefs", r#"{"font_scale":1.25}"#).starts_with("HTTP/1.1 204"));
    assert!(http_get(a, "/", a).contains("style=\"--mdv-scale: 1.25\""));
}

#[test]
fn task_endpoint_checks_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("notes");
    std::fs::create_dir(&folder).unwrap();
    let md = folder.join("todo.md");
    let outside = dir.path().join("outside.md");
    std::fs::write(&md, "- [ ] one\n- [ ] two\n").unwrap();
    std::fs::write(&outside, "- [ ] secret\n").unwrap();
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&folder);
        c
    });
    let a = &server.addr;
    let set = |doc: &str, line: u32, done: bool| {
        post(
            a,
            "/__mdv/task",
            &format!(r#"{{"doc":"{doc}","line":{line},"col":1,"done":{done}}}"#),
        )
    };
    // Without the X-MDV header (a cross-site form post can't add one).
    let body = r#"{"doc":"todo.md","line":1,"col":1,"done":true}"#;
    let forbidden = http(
        a,
        &format!(
            "POST /__mdv/task HTTP/1.1\r\nHost: {a}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(forbidden.starts_with("HTTP/1.1 403"), "{forbidden}");

    assert!(set("todo.md", 2, true).starts_with("HTTP/1.1 204"));
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        "- [ ] one\n- [x] two\n"
    );
    // Already in that state: nothing to do.
    assert!(set("todo.md", 2, true).starts_with("HTTP/1.1 204"));
    // Not a task item any more (the page is stale): refused, file unchanged.
    std::fs::write(&md, "# Changed\n- [ ] one\n").unwrap();
    let stale = set("todo.md", 1, true);
    assert!(stale.starts_with("HTTP/1.1 409"), "{stale}");
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        "# Changed\n- [ ] one\n"
    );
    // Nothing outside the served folder.
    assert!(set("../outside.md", 1, true).starts_with("HTTP/1.1 404"));
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "- [ ] secret\n");
}

#[test]
fn folder_export_writes_a_site() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("README.md"), "# Home\n\n[A](a.md)\n").unwrap();
    std::fs::write(dir.path().join("a.md"), "# A\n").unwrap();
    let out = dir.path().join("site");
    let run = mdv().arg(dir.path()).arg("-o").arg(&out).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let home = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(home.contains("href=\"a.html\""), "{home}");
    assert!(out.join("a.html").is_file() && out.join("_mdv/files.js").is_file());
    // Not to stdout, and not into a folder with other things in it.
    assert!(
        !mdv()
            .arg(dir.path())
            .args(["-o", "-"])
            .status()
            .unwrap()
            .success()
    );
    let busy = dir.path().join("busy");
    std::fs::create_dir(&busy).unwrap();
    std::fs::write(busy.join("keep.txt"), "mine").unwrap();
    let refused = mdv().arg(dir.path()).arg("-o").arg(&busy).output().unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("isn't empty"));
}

#[test]
fn csv_and_tsv_files_are_tables() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.csv"), "name,n\n\"a, b\",2\nc,10\n").unwrap();
    std::fs::write(dir.path().join("data.tsv"), "x\ty\n1\t2\n").unwrap();
    std::fs::write(dir.path().join("README.md"), "# Data\n\n[CSV](data.csv)\n").unwrap();
    let out = mdv()
        .arg(dir.path().join("data.csv"))
        .args(["-o", "-"])
        .output()
        .unwrap();
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(html.contains("<td>a, b</td>"), "{html}");
    assert!(html.contains("<th align=\"right\">n</th>"));

    // Served: a page of its own, and in the listing and file list.
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(dir.path());
        c
    });
    let a = &server.addr;
    let page = http_get(a, "/data.tsv", a);
    assert!(page.contains("data-doc=\"data.tsv\""), "{page}");
    let events = http_get(a, "/__mdv/files", a);
    assert!(
        events.contains("\"data.csv\"") && events.contains("\"data.tsv\""),
        "{events}"
    );

    // And in a static site.
    let site = dir.path().join("site");
    assert!(
        mdv()
            .arg(dir.path())
            .arg("-o")
            .arg(&site)
            .status()
            .unwrap()
            .success()
    );
    let exported = std::fs::read_to_string(site.join("data.csv.html")).unwrap();
    assert!(exported.contains("<td>a, b</td>"));
    assert!(site.join("data.tsv.html").is_file());
    let index = std::fs::read_to_string(site.join("index.html")).unwrap();
    assert!(index.contains("href=\"data.csv.html\""), "{index}");
}

#[test]
fn github_references_follow_the_remote() {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    };
    git(&["init", "-q"]);
    git(&["remote", "add", "origin", "git@github.com:octo/proj.git"]);
    let md = dir.path().join("notes.md");
    std::fs::write(&md, "Fixes #7, thanks @octo-cat. `#8` isn't.\n").unwrap();
    let render = || {
        let out = mdv().arg(&md).args(["-o", "-"]).output().unwrap();
        String::from_utf8(out.stdout).unwrap()
    };
    let html = render();
    assert!(
        html.contains("<a class=\"mdv-ref\" href=\"https://github.com/octo/proj/issues/7\">#7</a>"),
        "{html}"
    );
    assert!(html.contains("href=\"https://github.com/octo-cat\">@octo-cat</a>"));
    assert!(html.contains("<code>#8</code>"));

    // The settings file can name another repository, or turn them off.
    let config = test_dir().join("config.toml");
    std::fs::write(&config, "github_repo = \"other/repo\"\n").unwrap();
    assert!(render().contains("https://github.com/other/repo/issues/7"));
    std::fs::write(&config, "github_references = false\n").unwrap();
    assert!(!render().contains("mdv-ref"));
    std::fs::write(&config, "github_repo = \"nope\"\n").unwrap();
    let bad = mdv().arg(&md).args(["-o", "-"]).output().unwrap();
    assert!(!bad.status.success());
    std::fs::remove_file(&config).unwrap();
}

#[test]
fn lint_reports_problems() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# A\n\n### Skip\n\n![](x.png)\n").unwrap();
    std::fs::write(dir.path().join("ok.md"), "# OK\n\n## Fine\n").unwrap();
    let out = mdv().arg("--lint").arg(dir.path()).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(
        text.contains("a.md:3: heading level jumps from 1 to 3 (heading-increment)"),
        "{text}"
    );
    assert!(
        text.contains("a.md:5: image without alt text: x.png (image-alt)"),
        "{text}"
    );
    assert!(text.contains("Linted 2 files: 2 problems."), "{text}");

    // Rules can be turned off; unknown ones are an error.
    let config = test_dir().join("config.toml");
    std::fs::write(
        &config,
        "lint_ignore = [\"heading-increment\", \"image-alt\"]\n",
    )
    .unwrap();
    let out = mdv().arg("--lint").arg(dir.path()).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    std::fs::write(&config, "lint_ignore = [\"nope\"]\n").unwrap();
    let out = mdv().arg("--lint").arg(dir.path()).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown rule \"nope\""));
    std::fs::remove_file(&config).unwrap();
}

#[test]
fn desktop_integration() {
    let dir = tempfile::tempdir().unwrap();
    let install = mdv()
        .arg("--install-desktop")
        .env("XDG_DATA_HOME", dir.path())
        .output()
        .unwrap();
    let entry = dir.path().join("applications/markdown-viewer.desktop");
    if cfg!(target_os = "macos") {
        assert!(!install.status.success());
        assert!(String::from_utf8_lossy(&install.stderr).contains("--install-finder"));
        return;
    }
    if cfg!(windows) {
        assert!(!install.status.success());
        assert!(String::from_utf8_lossy(&install.stderr).contains("for Linux"));
        return;
    }
    assert!(
        install.status.success(),
        "{}",
        String::from_utf8_lossy(&install.stderr)
    );
    let text = std::fs::read_to_string(&entry).unwrap();
    assert!(
        text.contains("--watch --exit-when-closed %f") && text.contains("MimeType=text/markdown;")
    );
    assert!(
        dir.path()
            .join("icons/hicolor/scalable/apps/markdown-viewer.svg")
            .is_file()
    );
    let uninstall = mdv()
        .arg("--uninstall-desktop")
        .env("XDG_DATA_HOME", dir.path())
        .output()
        .unwrap();
    assert!(uninstall.status.success());
    assert!(!entry.exists());
}

#[test]
fn recent_files_are_remembered_and_listed() {
    let home = tempfile::tempdir().unwrap();
    let home_path = dunce::canonicalize(home.path()).unwrap();
    let notes = home_path.join("notes");
    std::fs::create_dir(&notes).unwrap();
    std::fs::write(notes.join("todo list.md"), "# Todo\n").unwrap();
    // Opening a file (here served, without a browser) remembers it;
    // exporting doesn't.
    drop(start({
        let mut c = mdv();
        c.arg("--watch")
            .arg(notes.join("todo list.md"))
            .env("HOME", &home_path);
        c
    }));
    let export = mdv()
        .arg(notes.join("todo list.md"))
        .args(["-o", "-"])
        .env("HOME", &home_path)
        .output()
        .unwrap();
    assert!(export.status.success());
    let state = std::fs::read_to_string(test_dir().join("state.json")).unwrap();
    assert_eq!(state.matches("todo list.md").count(), 1, "{state}");

    // No FILE in the home folder: the Recent page, linking to the file.
    let server = start({
        let mut c = mdv();
        c.current_dir(&home_path).env("HOME", &home_path);
        c
    });
    let a = &server.addr;
    let page = http_get(a, "/", a);
    assert!(page.contains("<title>(Recent)</title>"), "{page}");
    assert!(page.contains("href=\"/notes/todo%20list.md\""), "{page}");
    assert!(http_get(a, "/notes/todo%20list.md", a).contains("Todo"));
}
