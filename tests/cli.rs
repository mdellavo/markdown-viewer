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

/// examples/slides.md is the presentation example: seven `---` rules (eight
/// slides) after the front matter, with code, a diagram and math.
#[test]
fn slides_example_renders() {
    let out = mdv()
        .args(["examples/slides.md", "-o", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    let body = &html[html.find("<article").unwrap()..html.find("</article>").unwrap()];
    assert!(body.contains("class=\"mdv-front-matter\""), "front matter");
    assert_eq!(body.matches("<hr").count(), 7, "slide rules");
    assert!(body.contains("<pre class=\"mermaid\">"), "diagram");
    assert!(body.contains("data-math-style=\"display\""), "math");
}

/// examples/vault is the Obsidian example: Obsidian mode turns on by itself
/// there, and every wiki link, embed and callout in Home.md resolves.
#[test]
fn vault_example_renders() {
    let out = mdv()
        .args(["examples/vault/Home.md", "-o", "-"])
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
        body.contains("href=\"Projects/Garden.md\" data-wikilink"),
        "a note found by name"
    );
    assert!(body.contains("href=\"Ideas.md#someday\""), "a heading link");
    assert_eq!(count("mdv-wikilink-missing"), 1, "one missing note");
    assert!(
        body.contains("src=\"attachments/logo.svg\" alt=\"logo.svg\" width=\"120\""),
        "image embed"
    );
    assert_eq!(
        count("class=\"mdv-embed-note\""),
        2,
        "note and section embeds"
    );
    assert_eq!(count("class=\"callout\""), 4, "callouts");
    assert!(
        body.contains("data-callout-fold=\"collapsed\""),
        "folded callout"
    );
    assert!(body.contains("<mark>Highlighted text</mark>"), "highlight");
    assert!(!body.contains("You can't see this"), "comment hidden");
    assert!(
        body.contains("<a class=\"mdv-tag\" data-mdv-tag=\"projects/garden\">#projects/garden</a>"),
        "nested tag"
    );
    assert!(body.contains("<code>#tags</code>"), "not in code");
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
    assert!(
        body.contains(
            "<td class=\"mdv-tags\"><a class=\"mdv-tag\" data-mdv-tag=\"markdown\">#markdown</a>"
        ),
        "front matter tags as chips"
    );
    // #tags in the text are for Obsidian notes; here they stay text.
    assert!(!body.contains("data-mdv-tag=\"tags\""));
    assert_eq!(count("<pre class=\"mermaid\">"), 10, "mermaid diagrams");
    assert_eq!(
        count("markdown-alert-"),
        5 * 2,
        "five alerts (div + title class)"
    );
    assert_eq!(count("type=\"checkbox\""), 6, "task list items");
    assert_eq!(count("data-math-style=\"inline\""), 4, "inline math");
    assert_eq!(
        count("data-math-style=\"display\""),
        4,
        "display math (one AsciiMath)"
    );
    assert!(body.contains("$5 or $10"), "dollar amounts stay text");
    assert_eq!(
        count("<table>"),
        3,
        "the table, the front matter and the calendar"
    );
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
        "<sup>2</sup>",
        "H<sub>2</sub>O",
        "<dt>Markdown</dt>",
        "<abbr title=\"HyperText Markup Language\">HTML</abbr>",
        "<abbr title=\"GitHub Flavored Markdown\">GFM</abbr>",
        "<circle class=\"mdv-map-point\"",
        "<title>name: Boston</title>",
        "<figure class=\"mdv-stl\" data-mdv-mesh=",
        "<span class=\"mdv-color\" style=\"background-color: #0969da\"",
        "<figure class=\"mdv-diagram mdv-bob\"><svg",
        "<figure class=\"mdv-diagram mdv-pikchr\"><svg",
        "<figure class=\"mdv-diagram mdv-wavedrom\"><svg",
        "<figure class=\"mdv-diagram mdv-qr\"><svg",
        "<pre class=\"mdv-lib mdv-lib-vega\">",
        "<pre class=\"mdv-lib mdv-lib-abc\">",
        "<pre class=\"mdv-lib mdv-lib-smiles\">",
        "<figcaption>Scholar – Victim, 1-0, after 4. Qxf7#</figcaption>",
        "White to move",
        "<title>name: Providence to Newport</title>",
        "aria-label=\"3D model, 4 triangles\"",
        "aria-label=\"3D model, 12 triangles\"",
        "aria-label=\"3D model, 6 triangles\"",
        "aria-label=\"3D model, 24 triangles\"",
        "<video class=\"mdv-video\" src=\"images/clip.webm\" controls",
        "<audio class=\"mdv-audio\" src=\"images/tone.ogg\" controls",
        "<pre data-math-style=\"display\" lang=\"asciimath\"><code>\\begin{gathered}",
        "text-anchor=\"start\">OH</text>",
        "<figcaption>Benzoic acid</figcaption>",
        "methane, 5 atoms",
        "<figure class=\"mdv-calendar\"><figcaption>Release planning</figcaption>",
        "Repeats weekly on Mon, Wed, Fri",
    ] {
        assert!(body.contains(needle), "missing {needle:?}");
    }
    assert_eq!(
        count("<figure class=\"mdv-map\""),
        6,
        "GeoJSON, TopoJSON, GPX, KML and WKT maps, and a GPX file"
    );
    assert_eq!(count("<figure class=\"mdv-diagram mdv-chess\""), 2);
    assert_eq!(
        count("<figure class=\"mdv-stl\""),
        5,
        "STL, OBJ, binary STL, GLB and 3MF"
    );
    // Graphviz draws only where it's installed.
    assert_eq!(
        count("<figure class=\"mdv-diagram mdv-dot\"")
            + count("<figure class=\"mdv-diagram mdv-diagram-missing\""),
        1
    );
    // The libraries are inlined, once each.
    assert!(html.contains("window.vegaInterpreter"));
    assert!(html.contains("ABCJS"));
    assert!(html.contains("window.SmilesDrawer"));
    assert!(!body.contains("Can't draw"), "every drawing is valid");
    assert!(
        !body.contains("*[HTML]"),
        "abbreviation definitions are hidden"
    );
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

    // The <html> tag (the page's styles also name every theme).
    let root = |page: String| {
        page[page.find("<html").unwrap()..]
            .split('>')
            .next()
            .unwrap()
            .to_string()
    };

    let server = start({
        let mut c = mdv();
        c.args(["--watch"]).arg(&md);
        c
    });
    let a = &server.addr;
    assert!(root(http_get(a, "/", a)).contains("data-mdv-theme=\"auto\""));

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

    let page = root(http_get(a, "/", a));
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
    assert!(root(http_get(&again.addr, "/", &again.addr)).contains("data-mdv-theme=\"dark\""));
    // (A second launch for the same file would reuse that viewer.)
    drop(again);
    let explicit = start({
        let mut c = mdv();
        c.args(["--watch", "--theme", "light"]).arg(&md);
        c
    });
    assert!(
        root(http_get(&explicit.addr, "/", &explicit.addr)).contains("data-mdv-theme=\"light\"")
    );
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
    // Credits for the help: from Cargo's view of the dependencies.
    let credits = credits_json();
    if update {
        std::fs::write("assets/credits.json", &credits).unwrap();
    } else {
        assert!(
            std::fs::read_to_string("assets/credits.json").unwrap_or_default() == credits,
            "assets/credits.json is out of date; run: UPDATE_GENERATED=1 cargo test --test cli generated_files"
        );
    }
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

/// The crates markdown-viewer is built with: the direct dependencies
/// (name, version, license, link), and how many crates are compiled in
/// altogether. Dev and build dependencies aren't part of the program.
fn credits_json() -> String {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        // Not --offline: a build only downloads its own platform's crates,
        // and the credits cover every platform's.
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let packages: std::collections::HashMap<&str, &serde_json::Value> = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p))
        .collect();
    let nodes: std::collections::HashMap<&str, &serde_json::Value> = meta["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["id"].as_str().unwrap(), n))
        .collect();
    let root = meta["resolve"]["root"].as_str().unwrap();
    // Dependencies of `id` that end up in the program (normal ones).
    let normal = |id: &str| -> Vec<String> {
        nodes[id]["deps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| {
                d["dep_kinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|k| k["kind"].is_null())
            })
            .map(|d| d["pkg"].as_str().unwrap().to_string())
            .collect()
    };
    let mut direct: Vec<serde_json::Value> = normal(root)
        .iter()
        .map(|id| {
            let p = packages[id.as_str()];
            let url = p["repository"]
                .as_str()
                .or(p["homepage"].as_str())
                .map(|u| u.trim_end_matches(".git").trim_end_matches('/').to_string())
                .unwrap_or_else(|| {
                    format!("https://crates.io/crates/{}", p["name"].as_str().unwrap())
                });
            serde_json::json!({
                "name": p["name"],
                "version": p["version"],
                "license": p["license"],
                "url": url,
            })
        })
        .collect();
    direct.sort_by_key(|c| c["name"].as_str().unwrap().to_string());
    let mut seen = std::collections::HashSet::new();
    let mut todo = normal(root);
    while let Some(id) = todo.pop() {
        if seen.insert(id.clone()) {
            todo.extend(normal(&id));
        }
    }
    let credits = serde_json::json!({ "crates": direct, "total": seen.len() });
    serde_json::to_string_pretty(&credits).unwrap() + "\n"
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
fn folder_export_keeps_index_and_readme() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(src.join("docs")).unwrap();
    std::fs::write(src.join("README.md"), "# Top\n").unwrap();
    std::fs::write(src.join("docs/index.md"), "# The index\n").unwrap();
    std::fs::write(src.join("docs/README.md"), "# The readme\n").unwrap();
    std::fs::write(src.join("C# notes.md"), "# Sharp\n").unwrap();
    let out = dir.path().join("site");
    let run = mdv().arg(&src).arg("-o").arg(&out).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let index = std::fs::read_to_string(out.join("docs/index.html")).unwrap();
    assert!(index.contains("The index"), "index.md is the folder's page");
    let readme = std::fs::read_to_string(out.join("docs/README.html")).unwrap();
    assert!(readme.contains("The readme"));
    // A listing links to names with `#` correctly.
    std::fs::remove_file(src.join("README.md")).unwrap();
    let out2 = dir.path().join("site2");
    assert!(
        mdv()
            .arg(&src)
            .arg("-o")
            .arg(&out2)
            .status()
            .unwrap()
            .success()
    );
    let listing = std::fs::read_to_string(out2.join("index.html")).unwrap();
    assert!(listing.contains("href=\"C%23%20notes.html\""), "{listing}");
    assert!(out2.join("C# notes.html").is_file());
}

#[test]
fn folder_files_cannot_act_on_the_viewer() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("README.md"), "# Home\n").unwrap();
    std::fs::write(root.join("page.html"), "<script>alert(1)</script>").unwrap();
    std::fs::write(
        root.join("pic.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    )
    .unwrap();
    std::fs::write(root.join("doc.pdf"), "%PDF-1.4").unwrap();
    std::fs::create_dir_all(root.join("evil.example")).unwrap();
    std::fs::create_dir_all(root.join("list/a#b")).unwrap();
    std::fs::write(root.join("list/a#b/x.md"), "# X\n").unwrap();
    std::fs::write(root.join("list/100%25.md"), "# P\n").unwrap();

    let server = start({
        let mut c = mdv();
        c.arg(root);
        c
    });
    let a = &server.addr;

    // A <base> in a document can't move where the page loads scripts from.
    let page = http_get(a, "/", a);
    assert!(page.contains("base-uri 'none'"), "{page}");

    // The folder's own HTML and SVG files run sandboxed; PDFs can't be.
    for file in ["/page.html", "/pic.svg"] {
        let resp = http_get(a, file, a).to_lowercase();
        assert!(resp.contains("content-security-policy: sandbox"), "{resp}");
        assert!(resp.contains("x-content-type-options: nosniff"));
    }
    let pdf = http_get(a, "/doc.pdf", a).to_lowercase();
    assert!(
        pdf.starts_with("http/1.1 200") && !pdf.contains("sandbox"),
        "{pdf}"
    );

    // A folder redirect stays on this host.
    let redirect = http_get(a, "//evil.example", a).to_lowercase();
    assert!(
        redirect.contains("location: /evil.example/\r\n"),
        "{redirect}"
    );
    let redirect = http_get(a, "/list/a%23b", a).to_lowercase();
    assert!(
        redirect.contains("location: /list/a%23b/\r\n"),
        "{redirect}"
    );

    // Listing links work for names with `#` and `%`.
    let listing = http_get(a, "/list/", a);
    assert!(listing.contains("href=\"a%23b/\""), "{listing}");
    assert!(listing.contains("href=\"100%2525.md\""), "{listing}");
    assert!(http_get(a, "/list/100%2525.md", a).starts_with("HTTP/1.1 200"));
    assert!(http_get(a, "/list/a%23b/x.md", a).starts_with("HTTP/1.1 200"));
}

#[test]
fn concurrent_preference_saves_are_all_kept() {
    let dir = tempfile::tempdir().unwrap();
    let names: Vec<String> = (0..12).map(|i| format!("n{i}.md")).collect();
    for n in &names {
        std::fs::write(dir.path().join(n), "# N\n\ntext\n").unwrap();
    }
    let state = test_dir().join("state.json");
    let _ = std::fs::remove_file(&state);
    let server = start({
        let mut c = mdv();
        c.arg(dir.path());
        c
    });
    let a = server.addr.clone();
    std::thread::scope(|s| {
        for (i, n) in names.iter().enumerate() {
            let a = &a;
            s.spawn(move || {
                let body = format!(r#"{{"doc":"{n}","line":{}}}"#, i + 1);
                let resp = post(a, "/__mdv/position", &body);
                assert!(resp.starts_with("HTTP/1.1 204"), "{resp}");
            });
        }
    });
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&state).unwrap()).unwrap();
    let positions = saved["positions"].as_object().unwrap();
    assert_eq!(positions.len(), names.len(), "{saved}");
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
    assert!(
        !page.contains(" data-source data-about="),
        "the generated Recent page has no source view"
    );
    assert!(page.contains("href=\"/notes/todo%20list.md\""), "{page}");
    assert!(http_get(a, "/notes/todo%20list.md", a).contains("Todo"));

    // The list as JSON (the sidebar's Recent tab).
    let list = http_get(a, "/__mdv/recent", a);
    assert!(
        list.contains("\"href\":\"/notes/todo%20list.md\""),
        "{list}"
    );
    assert!(list.contains("\"enabled\":true"), "{list}");
    // Only remembered paths can be opened, and only by the viewer's pages.
    let other = home_path.join("other.md");
    std::fs::write(&other, "# Other\n").unwrap();
    let body = serde_json::json!({ "path": other }).to_string();
    assert!(post(a, "/__mdv/recent/open", &body).starts_with("HTTP/1.1 404"));
    let no_header = http(
        a,
        &format!(
            "POST /__mdv/recent/remove HTTP/1.1\r\nHost: {a}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    assert!(no_header.starts_with("HTTP/1.1 403"), "{no_header}");
    // Removing forgets it.
    let todo = serde_json::json!({ "path": notes.join("todo list.md") }).to_string();
    assert!(post(a, "/__mdv/recent/remove", &todo).starts_with("HTTP/1.1 204"));
    let state = std::fs::read_to_string(test_dir().join("state.json")).unwrap();
    assert!(!state.contains("todo list.md"), "{state}");
}

#[test]
fn about_shows_the_build_and_where_files_are() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Doc\n").unwrap();

    // --version has the build details; -V is just the version.
    let long = mdv().arg("--version").output().unwrap();
    let long = String::from_utf8_lossy(&long.stdout);
    assert!(
        long.contains("built for ") && long.contains("Mermaid "),
        "{long}"
    );
    let short = mdv().arg("-V").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&short.stdout).lines().count(), 1);

    // Static site pages (which have the viewer UI) carry the build, but
    // nothing about this computer.
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
    let page = std::fs::read_to_string(site.join("doc.html")).unwrap();
    assert!(
        page.contains(&format!(
            "&quot;version&quot;:&quot;{}&quot;",
            env!("CARGO_PKG_VERSION")
        )),
        "build info in the page"
    );
    let dir_name = dunce::canonicalize(dir.path()).unwrap();
    assert!(
        !page.contains(&*dir_name.to_string_lossy()),
        "no local paths"
    );
    assert!(!page.contains("state.json") && !page.contains("config.toml"));

    // Served pages can ask the viewer for the rest.
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&md).args(["--editor", "zed"]);
        c
    });
    let a = &server.addr;
    let about = http_get(a, "/__mdv/about", a);
    let json: serde_json::Value =
        serde_json::from_str(about.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(json["root"], dir_name.to_string_lossy().as_ref());
    assert_eq!(json["editor"], "zed");
    assert!(
        json["settings"][0]
            .as_str()
            .unwrap()
            .ends_with("config.toml")
    );
    assert_eq!(json["settings"][1], false, "no settings file yet");
}

#[test]
fn settings_are_changed_saved_and_applied() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Doc\n\n```rust\nfn main() {}\n```\n").unwrap();
    let config = test_dir().join("config.toml");
    std::fs::write(&config, "# mine\neditor = \"zed\"  # keep me\n").unwrap();
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&md).args(["--style", "wide"]);
        c
    });
    let a = &server.addr;
    let body = |response: &str| response.split("\r\n\r\n").nth(1).unwrap_or("").to_string();

    let info: serde_json::Value =
        serde_json::from_str(&body(&http_get(a, "/__mdv/settings", a))).unwrap();
    assert_eq!(info["values"]["editor"], "zed");
    assert_eq!(info["overridden"], serde_json::json!(["style"]));

    // Without the X-MDV header (which other websites can't send): refused.
    let json = r#"{"key":"line_numbers","value":true}"#;
    let forbidden = http(
        a,
        &format!(
            "POST /__mdv/settings HTTP/1.1\r\nHost: {a}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        ),
    );
    assert!(forbidden.starts_with("HTTP/1.1 403"), "{forbidden}");

    // Saved (keeping the file's comments) and applied to the page.
    let set = post(a, "/__mdv/settings", json);
    assert!(set.starts_with("HTTP/1.1 200"), "{set}");
    assert_eq!(body(&set), r#"{"reload":false}"#);
    assert_eq!(
        std::fs::read_to_string(&config).unwrap(),
        "# mine\neditor = \"zed\"  # keep me\nline_numbers = true\n"
    );
    assert!(
        http_get(a, "/", a).contains("mdv-numbered"),
        "code lines numbered"
    );

    // Page-level settings ask pages to reload.
    let set = post(
        a,
        "/__mdv/settings",
        r#"{"key":"number_headings","value":true}"#,
    );
    assert_eq!(body(&set), r#"{"reload":true}"#);

    // Invalid values are refused, and the file is left alone.
    let before = std::fs::read_to_string(&config).unwrap();
    for bad in [
        r#"{"key":"style","value":"fancy"}"#,
        r#"{"key":"nope","value":1}"#,
        r#"{"key":"css","value":"/no/such/file.css"}"#,
    ] {
        let refused = post(a, "/__mdv/settings", bad);
        assert!(refused.starts_with("HTTP/1.1 400"), "{bad}: {refused}");
    }
    assert_eq!(std::fs::read_to_string(&config).unwrap(), before);

    // null removes a setting.
    post(a, "/__mdv/settings", r#"{"key":"editor","value":null}"#);
    assert!(!std::fs::read_to_string(&config).unwrap().contains("editor"));
    std::fs::remove_file(&config).unwrap();
}

#[test]
fn word_count_setting() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Doc\n").unwrap();
    let config = test_dir().join("config.toml");
    std::fs::write(&config, "word_count = false\n").unwrap();
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&md);
        c
    });
    let a = &server.addr;
    // The page's <html> tag (the page's CSS and scripts mention the
    // attribute too).
    let html_tag = || {
        let page = http_get(a, "/", a);
        let start = page.find("<html").unwrap();
        page[start..start + page[start..].find('>').unwrap()].to_string()
    };
    assert!(
        html_tag().contains("data-mdv-stats=\"off\""),
        "hidden by the setting"
    );
    // The viewer's own choice (the w key) wins over the setting.
    post(a, "/__mdv/prefs", r#"{"word_count":true}"#);
    assert!(
        !html_tag().contains("data-mdv-stats=\"off\""),
        "shown by the viewer's choice"
    );
    std::fs::remove_file(&config).unwrap();
}

/// By default, a viewer that opened its own window stops once that window
/// closes; with --keep-running it doesn't. Linux only: a fake browser
/// stands in for opening a window (on macOS --browser must be an app).
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn watch_mode_exits_when_its_window_closes() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let browser = dir.path().join("fake-browser");
    std::fs::write(&browser, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o755)).unwrap();

    let launch = |extra: &[&str]| {
        let mut child = mdv()
            .args(["--watch", "--tab", "--browser"])
            .arg(&browser)
            .args(extra)
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
        (child, stdout, addr)
    };
    let connect = |addr: &str| {
        let mut sse = TcpStream::connect(addr).unwrap();
        sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
        let mut reader = BufReader::new(sse);
        wait_for(&mut reader, "Hi");
        reader
    };

    // Default: the page connects, closes, and the run ends by itself.
    let (mut child, _out, addr) = launch(&[]);
    let page = connect(&addr);
    drop(page);
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "didn't exit after the window closed"
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(status.success());

    // --keep-running: still serving after the page has gone.
    let (child, _out, addr) = launch(&["--keep-running"]);
    let mut child = Running(child);
    drop(connect(&addr));
    std::thread::sleep(Duration::from_secs(7));
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "exited despite --keep-running"
    );
}

/// Without --watch, opening a page starts a viewer in the background: the
/// command returns at once, the page is served (with settings and live
/// reload), and the viewer stops when the window closes. Linux only: a
/// fake browser stands in for opening the window.
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn opening_a_page_runs_a_background_viewer() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let browser = dir.path().join("fake-browser");
    std::fs::write(&browser, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o755)).unwrap();

    let started = Instant::now();
    let out = mdv()
        .args(["--tab", "--browser"])
        .arg(&browser)
        .arg(&md)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "returned at once"
    );
    let addr = text
        .split(" at http://")
        .nth(1)
        .unwrap_or_else(|| panic!("no address in {text:?}"))
        .trim()
        .trim_end_matches('/')
        .to_string();

    // A served page: the settings dialog is there.
    assert!(http_get(&addr, "/", &addr).contains("data-settings"));

    // The window opens and closes: the background viewer stops.
    let mut sse = TcpStream::connect(&addr).unwrap();
    write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    wait_for(&mut BufReader::new(sse.try_clone().unwrap()), "Hi");
    drop(sse);
    // Still ours? (Under load, another test's viewer may get the freed
    // port, so ask who's there.)
    let ours = || {
        let Ok(mut s) = TcpStream::connect(&addr) else {
            return false;
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = write!(
            s,
            "GET /__mdv/whoami HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
        );
        let mut reply = String::new();
        let _ = s.read_to_string(&mut reply);
        reply.contains("doc.md")
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while ours() {
        assert!(
            Instant::now() < deadline,
            "the background viewer kept running"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// If the background viewer can't start (here, its port is taken), the
/// command says why. It fails before opening any window.
#[test]
fn background_viewer_errors_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port().to_string();
    let out = mdv().arg(&md).args(["--port", &port]).output().unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains(&format!("could not listen on 127.0.0.1:{port}")),
        "{err}"
    );
}

/// Serve `body` (as `content_type`) for every request, from a thread.
fn serve_once_per_request(body: &'static str, content_type: &'static str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    addr
}

#[test]
fn renders_markdown_from_a_web_address() {
    let addr = serve_once_per_request(
        "# From the web\n\n[Next](next.md) ![logo](img/logo.png) [top](#from-the-web)\n",
        "text/markdown; charset=utf-8",
    );
    let out = mdv()
        .arg(format!("http://{addr}/docs/intro.md"))
        .args(["-o", "-"])
        .output()
        .unwrap();
    let html = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(html.contains("<title>intro.md</title>"), "{html}");
    assert!(html.contains("From the web</h1>") || html.contains(">From the web<"));
    // Relative links and images point back at the web, not at this computer.
    assert!(
        html.contains(&format!("href=\"http://{addr}/docs/next.md\"")),
        "{html}"
    );
    assert!(html.contains(&format!("src=\"http://{addr}/docs/img/logo.png\"")));
    assert!(html.contains("href=\"#from-the-web\""));

    // An HTML page isn't Markdown.
    let page = serve_once_per_request("<!DOCTYPE html><html></html>", "text/html");
    let out = mdv()
        .arg(format!("http://{page}/"))
        .args(["-o", "-"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("is a web page, not a Markdown file"));
}

#[test]
fn highlighted_source_of_served_files_only() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("notes");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("a.md"), "# A\n\n<script>x</script>\n").unwrap();
    std::fs::write(dir.path().join("secret.md"), "# Secret\n").unwrap();
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&folder);
        c
    });
    let a = &server.addr;
    let html = http_get(a, "/__mdv/source-html?doc=a.md", a);
    assert!(html.starts_with("HTTP/1.1 200"), "{html}");
    assert!(html.contains("data-line=\"3\"") && !html.contains("data-line=\"4\""));
    assert!(!html.contains("<script>"), "source is escaped");
    for doc in ["../secret.md", "missing.md"] {
        let refused = http_get(a, &format!("/__mdv/source-html?doc={doc}"), a);
        assert!(refused.starts_with("HTTP/1.1 404"), "{doc}: {refused}");
    }
}

#[test]
fn source_of_standard_input_and_web_addresses() {
    // Standard input, served (as `markdown-viewer - --watch`).
    let mut child = mdv()
        .args(["-", "--watch", "--no-open"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"# Piped\n\nFrom *stdin*.\n")
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let child = Running(child);
    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    let a = first
        .split(" at http://")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches('/')
        .to_string();
    assert!(
        http_get(&a, "/", &a).contains(" data-source data-about="),
        "offers the source view"
    );
    let html = http_get(&a, "/__mdv/source-html?doc=(stdin)", &a);
    assert!(
        html.starts_with("HTTP/1.1 200") && html.contains("data-line=\"3\""),
        "{html}"
    );
    let text = http_get(&a, "/__mdv/source?doc=(stdin)", &a);
    assert!(
        text.ends_with("# Piped\n\nFrom *stdin*.\n"),
        "copying gets the Markdown: {text}"
    );
    drop(child);

    // A web address.
    let web = serve_once_per_request("# Remote\n\nText.\n", "text/markdown");
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(format!("http://{web}/notes.md"));
        c
    });
    let a = &server.addr;
    assert!(http_get(a, "/", a).contains(" data-source data-about="));
    let html = http_get(a, "/__mdv/source-html?doc=(notes.md)", a);
    assert!(
        html.contains("Remote") && html.contains("data-line=\"3\""),
        "{html}"
    );
}

#[test]
fn source_highlighting_can_be_turned_off() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Title\n\n**bold**\n").unwrap();
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&md);
        c
    });
    let a = &server.addr;
    let highlighted = http_get(a, "/__mdv/source-html?doc=doc.md", a);
    assert!(highlighted.contains("x-mdv-highlighted: true") && highlighted.contains("hl-"));
    let plain = http_get(a, "/__mdv/source-html?doc=doc.md&plain=1", a);
    assert!(
        plain.contains("data-line=\"3\"") && !plain.contains("hl-"),
        "{plain}"
    );
    // The setting turns it off, at once.
    post(
        a,
        "/__mdv/settings",
        r#"{"key":"source_highlighting","value":false}"#,
    );
    let off = http_get(a, "/__mdv/source-html?doc=doc.md", a);
    assert!(
        off.contains("x-mdv-highlighted: false") && !off.contains("hl-"),
        "{off}"
    );
    let _ = std::fs::remove_file(test_dir().join("config.toml"));
}

#[test]
fn diff_marks_changes_since_a_revision() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap()
    };
    if !git(&["init", "-q"]).status.success() {
        return; // git not available
    }
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test"]);
    let doc = root.join("doc.md");
    std::fs::write(&doc, "# Notes\n\nThe quick brown fox.\n\nGoing away.\n").unwrap();
    git(&["add", "doc.md"]);
    git(&["commit", "-qm", "first"]);
    std::fs::write(&doc, "# Notes\n\nThe quick red fox.\n\n- new item\n").unwrap();

    let out = mdv()
        .arg(&doc)
        .args(["--diff", "HEAD", "-o", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(
        html.contains("<title>doc.md (since HEAD)</title>"),
        "{html}"
    );
    assert!(
        html.contains("<del class=\"mdv-del\">brown</del>"),
        "{html}"
    );
    assert!(html.contains("<ins class=\"mdv-ins\">red</ins>"), "{html}");
    assert!(html.contains("mdv-diff-del\"><p>Going away.</p>"), "{html}");
    assert!(html.contains("Changes since <code>HEAD</code>"));

    // Served, the comparison follows the file.
    let server = start({
        let mut c = mdv();
        c.arg("--watch").arg(&doc).args(["--diff", "HEAD"]);
        c
    });
    let a = &server.addr;
    assert!(http_get(a, "/", a).contains("<ins class=\"mdv-ins\">red</ins>"));
    std::fs::write(&doc, "# Notes\n\nThe quick green fox.\n").unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let page = http_get(a, "/", a);
        if page.contains("<ins class=\"mdv-ins\">green</ins>") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "no update: {page}");
        std::thread::sleep(Duration::from_millis(200));
    }

    // A revision git doesn't know, and a file outside git, are errors.
    let bad = mdv()
        .arg(&doc)
        .args(["--diff", "nope", "-o", "-"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("git couldn't find nope"));
    let outside = tempfile::tempdir().unwrap();
    let lone = outside.path().join("lone.md");
    std::fs::write(&lone, "# Lone\n").unwrap();
    let bad = mdv()
        .arg(&lone)
        .args(["--diff", "HEAD", "-o", "-"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&bad.stderr).contains("isn't in a git repository"));
}

/// Graphviz, PlantUML and D2 blocks run the program (here stand-ins) and
/// put its cleaned-up SVG in the page; failures show the source and why.
#[test]
fn diagram_programs_draw_their_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let svg = dir.path().join("out.svg");
    std::fs::write(
        &svg,
        "<?xml version=\"1.0\"?>\n<svg onload=\"alert(1)\" width=\"80pt\"><script>alert(2)</script>\
         <g><text fill=\"black\">drawn</text></g></svg>\n",
    )
    .unwrap();
    // `dot` prints the SVG; `plantuml` fails.
    if cfg!(windows) {
        std::fs::write(
            bin.join("dot.cmd"),
            format!("@type \"{}\"\r\n", svg.display()),
        )
        .unwrap();
        std::fs::write(
            bin.join("plantuml.cmd"),
            "@echo Syntax Error? (line 2) 1>&2\r\n@exit /b 1\r\n",
        )
        .unwrap();
    } else {
        for (name, body) in [
            ("dot", format!("#!/bin/sh\ncat > /dev/null\ncat {svg:?}\n")),
            (
                "plantuml",
                "#!/bin/sh\ncat > /dev/null\necho 'Syntax Error? (line 2)' >&2\nexit 1\n"
                    .to_string(),
            ),
        ] {
            let p = bin.join(name);
            std::fs::write(&p, body).unwrap();
            Command::new("chmod").arg("+x").arg(&p).status().unwrap();
        }
    }
    let md = dir.path().join("d.md");
    std::fs::write(
        &md,
        "```dot\ndigraph { a -> b }\n```\n\n```plantuml\nAlice -> Bob\n```\n\n```d2\na -> b\n```\n",
    )
    .unwrap();
    let out = mdv()
        .env("MARKDOWN_VIEWER_DIAGRAM_PATH", &bin)
        .arg(&md)
        .args(["-o", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(
        html.contains("<figure class=\"mdv-diagram mdv-dot\"><svg  width=\"80pt\"><g><text fill=\"currentColor\">drawn</text></g></svg></figure>"),
        "{html}"
    );
    assert!(!html.contains("alert("));
    assert!(html.contains("Can't draw this PlantUML: Syntax Error? (line 2)"));
    assert!(html.contains("Install D2 (<code>d2</code>) to draw this diagram."));
}

/// Pages that use a library get it: inlined in a standalone page, served
/// at /__mdv/lib/, and written once into a static site's _mdv folder.
#[test]
fn chart_libraries_are_bundled() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("docs");
    std::fs::create_dir(&docs).unwrap();
    std::fs::write(
        docs.join("chart.md"),
        "```vega-lite\n{\"mark\": \"bar\"}\n```\n",
    )
    .unwrap();
    std::fs::write(docs.join("plain.md"), "# Plain\n").unwrap();
    let site = dir.path().join("site");
    let out = mdv().arg(&docs).arg("-o").arg(&site).output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let page = std::fs::read_to_string(site.join("chart.html")).unwrap();
    assert!(
        page.contains("data-libs=\"{&quot;vega&quot;:&quot;_mdv/vega.js&quot;}\""),
        "{page}"
    );
    assert!(
        std::fs::read_to_string(site.join("_mdv/vega.js"))
            .unwrap()
            .contains("window.vegaInterpreter")
    );
    assert!(!site.join("_mdv/abcjs.js").exists());
    let plain = std::fs::read_to_string(site.join("plain.html")).unwrap();
    assert!(!plain.contains("data-libs"));

    let mut cmd = mdv();
    cmd.arg("--watch").arg(docs.join("chart.md"));
    let server = start(cmd);
    let page = http_get(&server.addr, "/", &server.addr);
    assert!(page.contains("/__mdv/lib/vega.js?v=6.5.0"), "{page}");
    let js = http_get(&server.addr, "/__mdv/lib/abcjs.js?v=1", &server.addr);
    assert!(js.starts_with("HTTP/1.1 200"));
    assert!(js.to_ascii_lowercase().contains("content-encoding: gzip"));
    let missing = http_get(&server.addr, "/__mdv/lib/nope.js", &server.addr);
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
}

/// Files in git show when they last changed, by whom, and an "Edit on
/// GitHub" link at the foot of the page; `last_updated = false` hides it.
#[test]
fn pages_say_when_the_file_last_changed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(dir.path()).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-03-04T05:06:07Z")
            .env("GIT_COMMITTER_DATE", "2026-03-04T05:06:07Z")
            .output()
            .unwrap()
    };
    if !git(&["init", "-q", "-b", "main"]).status.success() {
        return; // git not available
    }
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Grace Hopper"]);
    git(&["remote", "add", "origin", "https://github.com/o/r.git"]);
    let doc = root.join("doc.md");
    std::fs::write(&doc, "# Notes\n").unwrap();
    git(&["add", "doc.md"]);
    git(&["commit", "-qm", "first"]);
    let footer = "<footer class=\"mdv-page-meta\"><span>Last updated on <time datetime=\"2026-03-04\">4 March 2026</time> \
                  by Grace Hopper</span><a href=\"https://github.com/o/r/edit/main/doc.md\"";

    let out = mdv().arg(&doc).args(["-o", "-"]).output().unwrap();
    let html = String::from_utf8(out.stdout).unwrap();
    assert!(html.contains(footer), "{html}");

    let mut cmd = mdv();
    cmd.arg("--watch").arg(&doc);
    let server = start(cmd);
    assert!(http_get(&server.addr, "/", &server.addr).contains(footer));
    drop(server);

    // A file never committed has none.
    let new = root.join("new.md");
    std::fs::write(&new, "# New\n").unwrap();
    let out = mdv().arg(&new).args(["-o", "-"]).output().unwrap();
    assert!(
        !String::from_utf8(out.stdout)
            .unwrap()
            .contains("mdv-page-meta\">")
    );

    // Turned off in the settings file.
    std::fs::write(test_dir().join("config.toml"), "last_updated = false\n").unwrap();
    let out = mdv().arg(&doc).args(["-o", "-"]).output().unwrap();
    let _ = std::fs::remove_file(test_dir().join("config.toml"));
    assert!(
        !String::from_utf8(out.stdout)
            .unwrap()
            .contains("mdv-page-meta\">")
    );
}

/// Served pages stream: the start of the page, with a "Rendering…" note,
/// comes first; the note is hidden once the article arrives.
#[test]
fn served_pages_show_a_note_while_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n\nThere.\n").unwrap();
    let mut cmd = mdv();
    cmd.arg("--watch").arg(&md);
    let server = start(cmd);
    let page = http_get(&server.addr, "/", &server.addr);
    let note = page
        .find("<div class=\"mdv-loading\" role=\"status\">")
        .expect("the note");
    let hide = page
        .find(".mdv-loading { display: none !important; }")
        .expect("hidden after");
    let article = page.find("<article class=\"markdown-body\">").unwrap();
    assert!(note < hide && hide < article, "{page}");
    assert!(page.contains("Rendering doc.md…"));
    assert!(page.contains("<h1>") || page.contains("<h1 "));
    assert!(http_get(&server.addr, "/nope.md", &server.addr).starts_with("HTTP/1.1 404"));
}

/// Opening a file again just after its window closed: the viewer is still
/// in its grace period. The new launch claims it, so it stays up for the
/// page being opened instead of stopping before that page arrives.
#[test]
fn reopening_just_after_closing_keeps_the_viewer() {
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n").unwrap();
    let mut first = mdv()
        .args(["--watch", "--no-open", "--exit-when-closed"])
        .arg(&md)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(first.stdout.take().unwrap());
    let _guard = Running(first);
    let mut line = String::new();
    out.read_line(&mut line).unwrap();
    let addr = line
        .split(" at http://")
        .nth(1)
        .unwrap()
        .trim()
        .trim_end_matches('/')
        .to_string();
    // A page connects, then its window closes.
    let page = {
        let mut sse = TcpStream::connect(&addr).unwrap();
        sse.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(sse, "GET /__mdv/events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
        let mut reader = BufReader::new(sse);
        wait_for(&mut reader, "Hi");
        reader
    };
    drop(page);
    std::thread::sleep(Duration::from_millis(500));

    // Opened again during the 5 s grace period: reused…
    let again = mdv()
        .args(["--watch", "--no-open"])
        .arg(&md)
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&again.stdout);
    assert!(
        said.contains(&format!("Already open at http://{addr}/")),
        "{said}"
    );
    // …and still up after the grace period, for the page being opened.
    std::thread::sleep(Duration::from_secs(6));
    let page = http_get(&addr, "/", &addr);
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
}
