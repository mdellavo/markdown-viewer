//! Browser tests: the viewer UI driven in headless Chrome against a real
//! server. Skipped when no Chrome-family browser is installed, unless
//! MARKDOWN_VIEWER_BROWSER_TESTS=required (as in CI).

mod common;

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::*;
use headless_chrome::{Browser, LaunchOptions, Tab};

fn browser() -> Option<Browser> {
    let required = std::env::var("MARKDOWN_VIEWER_BROWSER_TESTS").as_deref() == Ok("required");
    let path = match headless_chrome::browser::default_executable() {
        Ok(p) => p,
        Err(e) if !required => {
            eprintln!("skipping browser test: {e}");
            return None;
        }
        Err(e) => panic!("browser tests are required but no browser was found: {e}"),
    };
    let options = LaunchOptions::default_builder()
        .path(Some(path))
        .headless(true)
        // CI runners can't use Chrome's sandbox; this only loads our pages.
        .sandbox(false)
        .window_size(Some((1300, 900)))
        .idle_browser_timeout(Duration::from_secs(120))
        .args(vec![OsStr::new("--hide-scrollbars")])
        .build()
        .unwrap();
    Some(Browser::new(options).expect("launch browser"))
}

/// Evaluate JavaScript (an expression, or statements ending in `return`)
/// and return its JSON value. Async code is awaited.
fn js(tab: &Tab, code: &str) -> serde_json::Value {
    let wrapped = format!("(async () => {{ {code} }})()");
    tab.evaluate(&wrapped, true)
        .unwrap_or_else(|e| panic!("JS failed: {e}\n{code}"))
        .value
        .unwrap_or(serde_json::Value::Null)
}

/// Wait until a JS condition is true.
fn until(tab: &Tab, condition: &str, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if js(tab, &format!("return !!({condition});")) == serde_json::json!(true) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn open(browser: &Browser, url: &str) -> Arc<Tab> {
    let tab = browser.new_tab().unwrap();
    tab.navigate_to(url)
        .unwrap()
        .wait_until_navigated()
        .unwrap();
    until(
        &tab,
        "document.querySelector('.mdv-toolbar')",
        "the viewer UI",
    );
    tab
}

fn serve(path: &Path, extra: &[&str]) -> Served {
    let mut cmd = mdv();
    cmd.arg("--watch").arg(path).args(extra);
    start(cmd)
}

fn press(tab: &Tab, key: &str) {
    tab.press_key(key).unwrap();
}

#[test]
fn theme_contents_help_and_copy_buttons() {
    let Some(browser) = browser() else { return };
    let server = serve(Path::new("tests/fixtures/gfm.md"), &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    until(
        &tab,
        "document.querySelector('div.mermaid svg')",
        "the diagram",
    );

    // auto -> light, then light -> dark: the second switch must redraw the
    // diagram whatever the machine's own light/dark setting is (auto may
    // already look like light).
    press(&tab, "t");
    until(
        &tab,
        "document.documentElement.dataset.mdvTheme === 'light'",
        "light theme",
    );
    std::thread::sleep(Duration::from_millis(500));
    let light = js(
        &tab,
        "return document.querySelector('div.mermaid svg').outerHTML.length;",
    );
    press(&tab, "t");
    until(
        &tab,
        "document.documentElement.dataset.mdvTheme === 'dark'",
        "dark theme",
    );
    until(
        &tab,
        &format!("document.querySelector('div.mermaid svg').outerHTML.length !== {light}"),
        "the diagram to redraw",
    );

    press(&tab, "o");
    until(
        &tab,
        "document.documentElement.classList.contains('mdv-sidebar-open')",
        "the contents",
    );
    assert!(
        js(
            &tab,
            "return document.querySelectorAll('.mdv-panel a').length;"
        )
        .as_u64()
            > Some(3)
    );

    press(&tab, "?");
    until(
        &tab,
        "!document.querySelector('.mdv-overlay').hidden",
        "help",
    );
    press(&tab, "Escape");

    let copies = js(
        &tab,
        "return document.querySelectorAll('.mdv-copy').length;",
    );
    assert_eq!(copies, 1, "one code block, one copy button");
    assert_eq!(
        js(
            &tab,
            "return document.querySelectorAll('.markdown-alert-title svg').length;"
        ),
        2,
        "alert icons"
    );
}

#[test]
fn file_finder_and_text_search() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("README.md"), "# Home\n").unwrap();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(
        root.join("docs/guide.md"),
        "# Guide\n\nSome text.\n\nThe frobnicator lives here.\n",
    )
    .unwrap();
    let mut cmd = mdv();
    cmd.arg(root);
    let server = start(cmd);
    let tab = open(&browser, &format!("http://{}/", server.addr));

    // `f` opens the finder with the cursor in the filter (a regression
    // test: it once toggled the list shut and typing went to the page).
    press(&tab, "f");
    until(
        &tab,
        "document.activeElement.classList.contains('mdv-filter')",
        "the filter",
    );
    // Pressing it again (from the page) must keep the list open.
    js(&tab, "document.activeElement.blur(); return 0;");
    press(&tab, "f");
    until(
        &tab,
        "document.activeElement.classList.contains('mdv-filter')",
        "the filter, still",
    );
    // A tree: the docs folder starts closed (the open file isn't in it)
    // and opens with a click.
    until(
        &tab,
        "document.querySelector('.mdv-tree-dir > button[title=\"docs/\"][aria-expanded=\"false\"]')",
        "the closed docs folder",
    );
    js(
        &tab,
        "document.querySelector('.mdv-tree-dir > button[title=\"docs/\"]').click(); return 0;",
    );
    until(
        &tab,
        "document.querySelector('.mdv-tree-kids:not([hidden]) a[title=\"docs/guide.md\"]')",
        "guide.md inside the opened folder",
    );
    js(
        &tab,
        "document.querySelector('.mdv-tree-dir > button[title=\"docs/\"]').click(); return 0;",
    );
    js(
        &tab,
        "document.querySelector('.mdv-filter').focus(); return 0;",
    );
    tab.type_str("guid").unwrap();
    // Filtering shows matches with their folders open.
    until(
        &tab,
        "[...document.querySelectorAll('.mdv-panel:not([hidden]) a')].some(a => a.title === 'docs/guide.md' && a.offsetParent)",
        "the file in subfolder docs/",
    );

    // Text search finds the line and links straight to it.
    js(
        &tab,
        "document.querySelector('.mdv-filter').value = ''; return 0;",
    );
    tab.type_str("frobnicator").unwrap();
    // Wait for this query's results, not the previous query's.
    until(
        &tab,
        "[...document.querySelectorAll('a.mdv-hit')].some(a => a.textContent.includes('frobnicator'))",
        "a text match",
    );
    let href = js(
        &tab,
        "return document.querySelector('a.mdv-hit').getAttribute('href');",
    );
    assert_eq!(href, "/docs/guide.md#mdv-line-5");
    press(&tab, "Enter");
    until(&tab, "location.pathname === '/docs/guide.md'", "navigation");
    until(
        &tab,
        "document.querySelector('.mdv-flash')",
        "the matched line highlighted",
    );
}

#[test]
fn live_edit_jumps_to_the_change() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    let mut text = String::from("# Top\n\n");
    for i in 0..60 {
        text.push_str(&format!("Paragraph {i} with some words.\n\n"));
    }
    std::fs::write(&md, &text).unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    assert_eq!(js(&tab, "return window.scrollY;"), 0);

    std::fs::write(
        &md,
        text.replace("Paragraph 50 with", "Paragraph 50 EDITED with"),
    )
    .unwrap();
    until(
        &tab,
        "[...document.querySelectorAll('.markdown-body p')].some(p => p.textContent.includes('EDITED'))",
        "the update",
    );
    until(
        &tab,
        "window.scrollY > 500",
        "the page to scroll to the edit",
    );
    let flashed = js(
        &tab,
        "return document.querySelector('.mdv-flash')?.textContent || '';",
    );
    assert!(flashed.as_str().unwrap().contains("EDITED"), "{flashed}");
}

#[test]
fn double_click_opens_the_source_in_the_editor() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Title\n\nFirst paragraph.\n\nSecond paragraph.\n").unwrap();
    let log = dir.path().join("editor.log");
    let template = fake_editor(dir.path(), &log);
    let server = serve(&md, &["--editor", &template]);
    let tab = open(&browser, &format!("http://{}/", server.addr));

    js(
        &tab,
        "const p = [...document.querySelectorAll('.markdown-body p')][1];
         p.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
         return 0;",
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    // The script creates the file before writing to it: wait for content.
    while std::fs::read_to_string(&log).map_or(true, |t| t.trim().is_empty())
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    let opened = std::fs::read_to_string(&log).expect("editor was run");
    assert!(opened.trim().ends_with("doc.md:5"), "{opened}");
}

#[test]
fn reading_position_is_restored() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    let mut text = String::new();
    for i in 0..40 {
        text.push_str(&format!("## Section {i}\n\nText for section {i}.\n\n"));
    }
    std::fs::write(&md, &text).unwrap();
    let server = serve(&md, &[]);
    let url = format!("http://{}/", server.addr);
    let tab = open(&browser, &url);
    js(
        &tab,
        "document.getElementById('section-30').scrollIntoView(); return 0;",
    );
    // The position is saved a second after scrolling stops.
    std::thread::sleep(Duration::from_millis(1600));

    let again = open(&browser, &url);
    until(
        &again,
        "Math.abs(document.getElementById('section-30').getBoundingClientRect().top) < 120",
        "the page to reopen at Section 30",
    );
}

#[test]
fn presentation_and_zoom() {
    let Some(browser) = browser() else { return };
    let server = serve(Path::new("examples/example.md"), &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    // Regression: with many diagrams still drawing, the first live update
    // used to look like an edit and scroll the page away from the top.
    until(
        &tab,
        "document.querySelectorAll('div.mermaid svg').length === 10",
        "all diagrams",
    );
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        js(&tab, "return window.scrollY;"),
        0,
        "the page stayed at the top"
    );

    press(&tab, "s");
    until(
        &tab,
        "document.querySelector('.mdv-present')",
        "presentation mode",
    );
    let first = js(
        &tab,
        "return document.querySelector('.mdv-present-bar').textContent;",
    );
    assert!(first.as_str().unwrap().starts_with("1 / "), "{first}");
    press(&tab, "ArrowRight");
    until(
        &tab,
        "document.querySelector('.mdv-present-bar').textContent.startsWith('2 / ')",
        "the next slide",
    );
    press(&tab, "Escape");
    until(
        &tab,
        "!document.querySelector('.mdv-present')",
        "presentation to end",
    );

    js(
        &tab,
        "document.querySelector('.markdown-body img[src=\"images/logo.svg\"]').click(); return 0;",
    );
    until(
        &tab,
        "document.querySelector('.mdv-zoom img')",
        "the zoomed image",
    );
    press(&tab, "Escape");
    until(
        &tab,
        "!document.querySelector('.mdv-zoom')",
        "zoom to close",
    );
}

#[test]
fn folded_callouts_open_for_search_hits_and_stay_open() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".obsidian")).unwrap();
    let md = dir.path().join("note.md");
    let text = "# Note\n\nIntro.\n\n> [!warning]- Risks\n> Scope creep.\n\nEnd.\n";
    std::fs::write(&md, text).unwrap();
    let server = serve(&md, &[]);
    // Line 6 is inside the folded callout, as a search hit would link.
    let tab = open(&browser, &format!("http://{}/#mdv-line-6", server.addr));
    let open_fold =
        "document.querySelector('.callout').getAttribute('data-callout-fold') === 'expanded'";
    until(&tab, open_fold, "the callout to unfold");
    std::thread::sleep(Duration::from_millis(800));
    assert_eq!(
        js(&tab, &format!("return {open_fold};")),
        true,
        "still open after the first sync"
    );

    // An edit elsewhere keeps the reader's folding.
    std::fs::write(&md, text.replace("End.", "End, edited.")).unwrap();
    until(
        &tab,
        "document.body.textContent.includes('End, edited.')",
        "the edit",
    );
    assert_eq!(
        js(&tab, &format!("return {open_fold};")),
        true,
        "still open after an edit"
    );
}

#[test]
fn text_size_is_remembered() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Hi\n\nText.\n").unwrap();
    let server = serve(&md, &[]);
    let url = format!("http://{}/", server.addr);
    let tab = open(&browser, &url);
    press(&tab, "+");
    press(&tab, "+");
    until(
        &tab,
        "document.documentElement.style.getPropertyValue('--mdv-scale') === '1.25'",
        "larger text",
    );
    std::thread::sleep(Duration::from_millis(300));
    let again = open(&browser, &url);
    let kept = js(
        &again,
        "return document.documentElement.style.getPropertyValue('--mdv-scale');",
    );
    assert_eq!(kept, "1.25", "remembered after reopening");
    press(&again, "0");
    until(
        &again,
        "!document.documentElement.style.getPropertyValue('--mdv-scale')",
        "reset",
    );
}

#[test]
fn goto_from_the_command_line_scrolls_the_page() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    let mut text = String::from("# Top\n\n");
    for i in 0..80 {
        text.push_str(&format!("Paragraph {i}.\n\n"));
    }
    std::fs::write(&md, &text).unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    std::thread::sleep(Duration::from_millis(500));
    // "Paragraph 70." is on line 143.
    let out = mdv()
        .arg("--goto")
        .arg(format!("{}:143", md.display()))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    until(
        &tab,
        "(document.querySelector('.mdv-flash')?.textContent || '').includes('Paragraph 70.')",
        "line 143 highlighted",
    );
    assert!(js(&tab, "return window.scrollY;").as_f64().unwrap() > 1000.0);
}

#[test]
fn large_files_update_only_what_changed() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("big.md");
    let mut text = String::from("# Big\n\n");
    for i in 0..3000 {
        text.push_str(&format!("Paragraph {i} with $x_{i}$ and some text.\n\n"));
    }
    std::fs::write(&md, &text).unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    until(
        &tab,
        "document.querySelectorAll('.katex').length === 3000",
        "the math",
    );
    std::thread::sleep(Duration::from_millis(800));
    // Remember some blocks, above and below the edit.
    js(
        &tab,
        "const b = document.querySelector('.markdown-body').children;
         window.__above = b[10]; window.__below = b[2900]; window.__t = performance.now(); return 0;",
    );
    // Insert a line above paragraph 1500: blocks below shift by two lines.
    std::fs::write(
        &md,
        text.replace("Paragraph 1500 with", "Inserted.\n\nParagraph 1500 with"),
    )
    .unwrap();
    until(
        &tab,
        "document.body.textContent.includes('Inserted.')",
        "the update",
    );
    let kept = js(
        &tab,
        "const b = document.querySelector('.markdown-body').children;
         return JSON.stringify([b[10] === window.__above, b[2901] === window.__below,
                 b[2901].getAttribute('data-sourcepos'), document.querySelectorAll('.katex').length]);",
    );
    let kept: serde_json::Value = serde_json::from_str(kept.as_str().unwrap()).unwrap();
    assert_eq!(kept[0], true, "blocks above the edit weren't replaced");
    assert_eq!(kept[1], true, "blocks below the edit weren't replaced");
    // Paragraph 2899 was on line 2*2899+3 = 5801; two lines later now.
    assert!(
        kept[2].as_str().unwrap().starts_with("5803:"),
        "positions below the edit were updated: {}",
        kept[2]
    );
    assert_eq!(kept[3], 3000);
}

#[test]
fn git_changes_backlinks_links_and_wrapping() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("README.md"),
        "# Readme\n\nSee [the guide](guide.md).\n",
    )
    .unwrap();
    std::fs::write(
        root.join("guide.md"),
        "# Guide\n\nFirst.\n\n[broken](nowhere.md)\n\n```\na very long line\n```\n",
    )
    .unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .is_ok_and(|s| s.success())
    };
    let has_git = git(&["init", "-q"]);
    if has_git {
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "T"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "x"]);
        std::fs::write(
            root.join("guide.md"),
            "# Guide\n\nFirst, edited.\n\n[broken](nowhere.md)\n\n```\na very long line\n```\n",
        )
        .unwrap();
    }
    let mut cmd = mdv();
    cmd.arg(root);
    let server = start(cmd);
    let tab = open(&browser, &format!("http://{}/guide.md", server.addr));

    if has_git {
        press(&tab, "D");
        until(
            &tab,
            "document.querySelector('.mdv-git-modified')?.textContent === 'First, edited.'",
            "the changed paragraph marked",
        );
    }
    press(&tab, "b");
    until(
        &tab,
        "document.querySelector('.mdv-panel:not([hidden]) a.mdv-hit')",
        "a backlink",
    );
    assert_eq!(
        js(
            &tab,
            "return document.querySelector('.mdv-panel:not([hidden]) a.mdv-hit').title;"
        ),
        "README.md:3"
    );
    press(&tab, "l");
    until(
        &tab,
        "document.querySelector('.mdv-broken-item')",
        "the broken-link list",
    );
    assert!(
        js(
            &tab,
            "return document.querySelector('.mdv-broken').textContent;"
        )
        .as_str()
        .unwrap()
        .contains("nowhere.md")
    );
    press(&tab, "Escape");
    js(
        &tab,
        "document.querySelector('.mdv-wrap-btn').click(); return 0;",
    );
    until(
        &tab,
        "document.querySelector('pre.mdv-wrapped')",
        "wrapping",
    );
}

#[test]
fn task_boxes_update_the_file() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("todo.md");
    std::fs::write(
        &md,
        "---\ntitle: Todo\n---\n\n# Todo\n\n- [ ] write\n- [x] test\n  1. [ ] nested\n",
    )
    .unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    let boxes = "[...document.querySelectorAll('.task-list-item-checkbox')]";
    until(
        &tab,
        &format!("{boxes}.every(b => !b.disabled)"),
        "enabled boxes",
    );

    js(&tab, &format!("{boxes}[0].click(); return null;"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&md)
        .unwrap()
        .contains("- [x] write")
    {
        assert!(Instant::now() < deadline, "the file wasn't updated");
        std::thread::sleep(Duration::from_millis(100));
    }
    js(&tab, &format!("{boxes}[2].click(); return null;"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&md)
        .unwrap()
        .contains("1. [x] nested")
    {
        assert!(Instant::now() < deadline, "the nested item wasn't updated");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        std::fs::read_to_string(&md).unwrap(),
        "---\ntitle: Todo\n---\n\n# Todo\n\n- [x] write\n- [x] test\n  1. [x] nested\n"
    );
    // After the live update, the new boxes are ticked and still clickable.
    until(
        &tab,
        &format!("{boxes}.every(b => b.checked && !b.disabled)"),
        "the updated page",
    );
}

#[test]
fn static_site_works_from_disk() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("docs");
    std::fs::create_dir_all(src.join("guide")).unwrap();
    std::fs::write(
        src.join("README.md"),
        "# Home\n\nSee [the guide](guide/math.md).\n",
    )
    .unwrap();
    std::fs::write(
        src.join("guide/math.md"),
        "# Math\n\n$$e^{i\\pi} + 1 = 0$$\n\n```mermaid\ngraph LR\n  A --> B\n```\n\n- [ ] read-only here\n",
    )
    .unwrap();
    let out = dir.path().join("site");
    let status = mdv().arg(&src).arg("-o").arg(&out).status().unwrap();
    assert!(status.success());

    let url = file_url(&dunce::canonicalize(out.join("index.html")).unwrap());
    let tab = open(&browser, &url);
    // The Files sidebar lists the site's pages as a tree (folders first);
    // following a link works.
    press(&tab, "f");
    until(
        &tab,
        "document.querySelector('.mdv-tree-dir > button[title=\"guide/\"]')",
        "the file tree",
    );
    js(
        &tab,
        "document.querySelector('.mdv-tree-dir > button[title=\"guide/\"]').click(); return 0;",
    );
    until(
        &tab,
        "document.querySelectorAll('.mdv-panel:not([hidden]) a[title]').length === 2",
        "the file list",
    );
    let hrefs = js(
        &tab,
        "return JSON.stringify([...document.querySelectorAll('.mdv-panel:not([hidden]) a[title]')].map(a => a.getAttribute('href')));",
    );
    assert_eq!(
        hrefs,
        serde_json::json!("[\"guide/math.html\",\"README.html\"]")
    );
    js(
        &tab,
        "document.querySelector('.markdown-body a[href=\"guide/math.html\"]').click(); return null;",
    );
    until(
        &tab,
        "location.pathname.endsWith('/guide/math.html')",
        "the guide page",
    );
    until(
        &tab,
        "document.querySelector('.mdv-toolbar')",
        "the viewer UI",
    );
    // Math and diagrams load from the shared _mdv/ copies.
    until(&tab, "document.querySelector('.katex-html')", "math");
    until(
        &tab,
        "document.querySelector('.mermaid svg')",
        "the diagram",
    );
    let font = js(
        &tab,
        "return getComputedStyle(document.querySelector('.katex')).fontFamily;",
    );
    assert!(
        font.as_str().unwrap().contains("KaTeX"),
        "KaTeX CSS not applied: {font}"
    );
    assert_eq!(
        js(
            &tab,
            "return document.querySelector('.task-list-item-checkbox').disabled;"
        ),
        serde_json::json!(true)
    );
}

#[test]
fn tables_sort_and_stay_sorted_across_edits() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("t.md");
    let table = "| Name | Size |\n| --- | --- |\n| b | 10 |\n| a | 9 |\n| c | 100 |\n";
    std::fs::write(&md, format!("# T\n\n{table}")).unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    let names = "return [...document.querySelectorAll('.markdown-body tbody tr')].map(r => r.cells[0].textContent).join('');";
    let click = |col: usize| {
        js(
            &tab,
            &format!(
                "document.querySelectorAll('.markdown-body thead th')[{col}].click(); return null;"
            ),
        );
    };
    until(
        &tab,
        "document.querySelector('table.mdv-sortable')",
        "a sortable table",
    );
    click(1);
    assert_eq!(
        js(&tab, names),
        "abc",
        "numbers sort as numbers: 9 < 10 < 100"
    );
    click(0);
    assert_eq!(js(&tab, names), "abc");
    click(0);
    assert_eq!(js(&tab, names), "cba");

    // An edit above the table: it stays sorted, and each row keeps its own
    // source line (for click-to-source).
    std::fs::write(&md, format!("# T\n\nNew paragraph.\n\n{table}")).unwrap();
    until(
        &tab,
        "document.querySelector('.markdown-body p')",
        "the update",
    );
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(js(&tab, names), "cba");
    let lines = js(
        &tab,
        "return [...document.querySelectorAll('.markdown-body tbody tr')].map(r => r.cells[0].textContent + r.getAttribute('data-sourcepos').split(':')[0]).join(' ');",
    );
    assert_eq!(lines, "c9 b7 a8");
    click(0);
    assert_eq!(
        js(&tab, names),
        "bac",
        "a third click restores the file's order"
    );
}

#[test]
fn custom_css_overrides_theme_colours() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Title\n\nA [link](x.md).\n").unwrap();
    let css = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/custom.css");
    let server = serve(&md, &["--css", css.to_str().unwrap(), "--theme", "light"]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    let colours = "return [getComputedStyle(document.querySelector('.markdown-body p a')).color, \
                   getComputedStyle(document.documentElement).backgroundColor].join(' ');";
    // Plain `.markdown-body { --var }` rules beat the built-in theme.
    assert_eq!(js(&tab, colours), "rgb(154, 74, 18) rgb(248, 241, 227)");
    js(
        &tab,
        "document.documentElement.dataset.mdvTheme = 'dark'; return null;",
    );
    assert_eq!(js(&tab, colours), "rgb(224, 149, 92) rgb(31, 26, 20)");
}

#[test]
fn toc_folds_and_stays_folded_across_edits() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    let body = "# Doc\n\n[TOC]\n\n## Alpha\n\n### Alpha one\n\n## Beta\n\n### Beta one\n\nText.\n";
    std::fs::write(&md, body).unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    let open_state = "return [...document.querySelectorAll('.mdv-toc details')].map(d => d.open ? 1 : 0).join('');";
    until(
        &tab,
        "document.querySelector('.mdv-toc details')",
        "the contents",
    );
    assert_eq!(js(&tab, open_state), "111", "whole list, Alpha, Beta");

    // A section's text jumps to it without folding; its arrow folds it.
    js(
        &tab,
        "document.querySelector('.mdv-toc summary a[href=\"#beta\"]').click(); return null;",
    );
    until(&tab, "location.hash === '#beta'", "the jump");
    assert_eq!(js(&tab, open_state), "111");
    js(
        &tab,
        "document.querySelector('.mdv-toc summary a[href=\"#alpha\"]').parentElement.click(); return null;",
    );
    assert_eq!(js(&tab, open_state), "101");

    // An edit (adding a section) keeps Alpha folded.
    std::fs::write(&md, body.replace("Text.", "## Gamma\n\nText.")).unwrap();
    until(
        &tab,
        "document.querySelector('.mdv-toc a[href=\"#gamma\"]')",
        "the update",
    );
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(js(&tab, open_state), "101");

    // Folding the whole list; printing opens everything, then refolds.
    js(
        &tab,
        "document.querySelector('.mdv-toc > details > summary').click(); return null;",
    );
    assert_eq!(js(&tab, open_state), "001");
    js(
        &tab,
        "window.dispatchEvent(new Event('beforeprint')); return null;",
    );
    assert_eq!(js(&tab, open_state), "111");
    js(
        &tab,
        "window.dispatchEvent(new Event('afterprint')); return null;",
    );
    assert_eq!(js(&tab, open_state), "001");
}

#[test]
fn help_shows_about() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Doc\n").unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    press(&tab, "?");
    until(
        &tab,
        "[...document.querySelectorAll('.mdv-about td')].some(td => td.textContent === 'Serving')",
        "the viewer's details",
    );
    let rows = js(
        &tab,
        "return [...document.querySelectorAll('.mdv-about tr')].map(r => r.cells[0].textContent).join(',');",
    );
    let rows = rows.as_str().unwrap();
    for label in [
        "Version",
        "Built for",
        "Includes",
        "Page",
        "Serving",
        "Settings",
        "State",
    ] {
        assert!(rows.contains(label), "missing {label}: {rows}");
    }
    let credits = js(
        &tab,
        "return [...document.querySelectorAll('.mdv-credits a')].map(a => a.textContent).join(',');",
    );
    let credits = credits.as_str().unwrap();
    for name in ["Mermaid", "KaTeX", "comrak", "syntect", "axum"] {
        assert!(credits.contains(name), "missing credit {name}: {credits}");
    }
    let version = js(
        &tab,
        "return document.querySelector('.mdv-about a').getAttribute('href');",
    );
    assert!(
        version
            .as_str()
            .unwrap()
            .ends_with(&format!("/releases/tag/v{}", env!("CARGO_PKG_VERSION"))),
        "{version}"
    );
}

#[test]
fn settings_dialog_changes_the_page() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Doc\n\n```rust\nfn main() {}\n```\n").unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    press(&tab, ",");
    until(
        &tab,
        "document.getElementById('mdv-setting-line_numbers')",
        "the settings dialog",
    );
    js(
        &tab,
        "const box = document.getElementById('mdv-setting-line_numbers');
         box.checked = true; box.dispatchEvent(new Event('change')); return null;",
    );
    // The page updates without reloading.
    js(&tab, "window.mdvNotReloaded = true; return null;");
    until(
        &tab,
        "document.querySelector('.markdown-body pre.mdv-numbered')",
        "numbered code",
    );
    assert_eq!(js(&tab, "return window.mdvNotReloaded === true;"), true);
    // The word count setting hides it at once.
    js(
        &tab,
        "const box = document.getElementById('mdv-setting-word_count');
         box.checked = false; box.dispatchEvent(new Event('change')); return null;",
    );
    until(
        &tab,
        "getComputedStyle(document.querySelector('.mdv-stats')).display === 'none'",
        "the word count to hide",
    );
    let saved = std::fs::read_to_string(test_dir().join("config.toml")).unwrap_or_default();
    assert!(saved.contains("line_numbers = true"), "{saved}");
    assert!(saved.contains("word_count = false"), "{saved}");
    let _ = std::fs::remove_file(test_dir().join("config.toml"));
}

#[test]
fn word_count_can_be_hidden_and_stays_hidden() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(&md, "# Doc\n\nSome words to count.\n").unwrap();
    let server = serve(&md, &[]);
    let url = format!("http://{}/", server.addr);
    let tab = open(&browser, &url);
    let shown = "return getComputedStyle(document.querySelector('.mdv-stats')).display !== 'none';";
    until(
        &tab,
        "document.querySelector('.mdv-stats').textContent.includes('words')",
        "the count",
    );
    assert_eq!(js(&tab, shown), true);

    press(&tab, "w");
    assert_eq!(js(&tab, shown), false);
    // Remembered: a reload starts with it hidden.
    std::thread::sleep(Duration::from_millis(300));
    let tab = open(&browser, &url);
    assert_eq!(
        js(
            &tab,
            "return document.documentElement.getAttribute('data-mdv-stats');"
        ),
        "off"
    );
    assert_eq!(js(&tab, shown), false);
    press(&tab, "w");
    assert_eq!(js(&tab, shown), true);
    // Clicking it hides it too.
    js(
        &tab,
        "document.querySelector('.mdv-stats').click(); return null;",
    );
    assert_eq!(js(&tab, shown), false);
}

#[test]
fn diagrams_show_in_presentation_slides() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("deck.md");
    std::fs::write(
        &md,
        "# Deck\n\n---\n\n## Flow\n\n```mermaid\nflowchart LR\n  A --> B --> C\n```\n",
    )
    .unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    until(
        &tab,
        "document.querySelector('div.mermaid svg')",
        "the diagram",
    );
    press(&tab, "s");
    press(&tab, "ArrowRight");
    until(
        &tab,
        "document.querySelector('.mdv-slide div.mermaid svg')",
        "the diagram slide",
    );
    // Drawn at a real size, not shrunk to an icon (a toolbar icon rule
    // once caught every SVG in the presentation).
    let size = js(
        &tab,
        "const r = document.querySelector('.mdv-slide div.mermaid svg').getBoundingClientRect();
         return JSON.stringify([Math.round(r.width), Math.round(r.height)]);",
    );
    let size: Vec<i64> = serde_json::from_str(size.as_str().unwrap()).unwrap();
    assert!(size[0] > 200 && size[1] > 30, "{size:?}");
    let icon = js(
        &tab,
        "return Math.round(document.querySelector('.mdv-toolbar button svg').getBoundingClientRect().width);",
    );
    assert_eq!(icon, 16);
}

#[test]
fn source_view_keeps_your_place() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    let mut text = String::from("# Top\n\n");
    for i in 1..=40 {
        text.push_str(&format!(
            "## Section {i}\n\nParagraph {i} with **bold** text.\n\n"
        ));
    }
    std::fs::write(&md, &text).unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    let top_source_line = "const l = [...document.querySelectorAll('.mdv-source-view .mdv-line')]
        .find(l => l.getBoundingClientRect().bottom > 60);
        return l ? +l.dataset.line : 0;";

    // From section 20 (line 79) into the source: it opens there.
    js(
        &tab,
        "document.getElementById('section-20').scrollIntoView(); return null;",
    );
    press(&tab, "v");
    until(
        &tab,
        "document.documentElement.dataset.mdvView === 'source' && document.querySelector('.mdv-source-view .mdv-line')",
        "the source",
    );
    let line = js(&tab, top_source_line).as_i64().unwrap();
    assert!((78..=81).contains(&line), "source opened at line {line}");
    // The plain source shows first; the highlighted one follows, in place.
    until(
        &tab,
        "document.querySelector('.mdv-source-view [class*=\"hl-\"]')",
        "the highlighted source",
    );
    until(
        &tab,
        "!document.querySelector('.mdv-toolbar button.mdv-busy')",
        "the busy indicator to clear",
    );
    let line = js(&tab, top_source_line).as_i64().unwrap();
    assert!(
        (78..=81).contains(&line),
        "still at line {line} after highlighting"
    );

    // Scroll the source to section 30 (line 119), go back: the page is there.
    js(
        &tab,
        "const l = document.querySelector('.mdv-source-view .mdv-line[data-line=\"119\"]');
         window.scrollTo(0, l.getBoundingClientRect().top + window.scrollY - 10); return null;",
    );
    // An edit while the source is shown updates it.
    std::fs::write(
        &md,
        text.replace("Paragraph 35 with", "Paragraph 35 EDITED with"),
    )
    .unwrap();
    until(
        &tab,
        "document.querySelector('.mdv-source-view').textContent.includes('EDITED')",
        "the source to update",
    );
    press(&tab, "v");
    until(
        &tab,
        "!document.documentElement.dataset.mdvView",
        "the page",
    );
    let heading = js(
        &tab,
        "return [...document.querySelectorAll('.markdown-body h2')]
           .find(h => h.getBoundingClientRect().bottom > 0).textContent;",
    );
    assert!(
        heading.as_str().unwrap().contains("Section 30"),
        "{heading}"
    );
}

#[test]
fn a_base_in_the_document_cannot_move_the_libraries() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("doc.md");
    std::fs::write(
        &md,
        "<base href=\"https://evil.invalid/\">\n\n# Doc\n\n$$x^2$$\n\n```mermaid\nflowchart LR\n  A --> B\n```\n",
    )
    .unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    // The libraries still come from the viewer, so both draw.
    until(
        &tab,
        "document.querySelector('div.mermaid svg')",
        "the diagram",
    );
    until(&tab, "document.querySelector('.katex')", "the math");
    assert_eq!(
        js(&tab, "return document.baseURI === location.href;"),
        serde_json::Value::Bool(true)
    );
}

#[test]
fn outside_links_open_new_windows_and_the_toolbar_navigates() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(
        root.join("README.md"),
        "# Home\n\n[web](https://example.com/) [guide](docs/guide.md) \
         [data](data.csv) [folder](docs) [pic](pic.png) [top](#home) \
         [mail](mailto:a@example.com)\n",
    )
    .unwrap();
    std::fs::write(root.join("docs/guide.md"), "# Guide\n").unwrap();
    std::fs::write(root.join("data.csv"), "a,b\n1,2\n").unwrap();
    std::fs::write(root.join("pic.png"), "").unwrap();
    let mut cmd = mdv();
    cmd.arg(root);
    let server = start(cmd);
    let tab = open(&browser, &format!("http://{}/", server.addr));

    // Which links get a new window (followed with navigation stopped).
    let targets = js(
        &tab,
        "addEventListener('click', e => e.preventDefault());
         return JSON.stringify([...document.querySelectorAll('.markdown-body p a')].map(a => {
           a.click(); return a.textContent + ':' + (a.target || 'here');
         }));",
    );
    assert_eq!(
        targets,
        serde_json::json!(
            "[\"web:_blank\",\"guide:here\",\"data:here\",\"folder:here\",\"pic:_blank\",\"top:here\",\"mail:here\"]"
        )
    );

    // Back, forward and reload.
    tab.navigate_to(&format!("http://{}/docs/guide.md", server.addr))
        .unwrap()
        .wait_until_navigated()
        .unwrap();
    until(&tab, "document.querySelector('.mdv-toolbar')", "the guide");
    js(
        &tab,
        "document.querySelector('button[aria-label=Back]').click(); return 0;",
    );
    until(&tab, "location.pathname === '/'", "back to the home page");
    until(
        &tab,
        "document.querySelector('.mdv-toolbar')",
        "the home page UI",
    );
    js(
        &tab,
        "document.querySelector('button[aria-label=Forward]').click(); return 0;",
    );
    until(
        &tab,
        "location.pathname === '/docs/guide.md'",
        "forward to the guide",
    );
    until(
        &tab,
        "document.querySelector('.mdv-toolbar')",
        "the guide UI",
    );
    js(
        &tab,
        "window.mdvBeforeReload = 1; document.querySelector('button[aria-label=Reload]').click(); return 0;",
    );
    until(
        &tab,
        "window.mdvBeforeReload === undefined && document.querySelector('.mdv-toolbar')",
        "the reloaded page",
    );
}

#[test]
fn recent_items_can_be_removed_from_the_page_and_sidebar() {
    let Some(browser) = browser() else { return };
    let home = tempfile::tempdir().unwrap();
    let home = dunce::canonicalize(home.path()).unwrap();
    std::fs::create_dir_all(home.join("notes")).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut recent = Vec::new();
    for (i, name) in ["a.md", "b.md", "c.md"].iter().enumerate() {
        let path = home.join("notes").join(name);
        std::fs::write(&path, format!("# {name}\n")).unwrap();
        recent.push(serde_json::json!({ "path": path, "at": now - i as u64 * 60 }));
    }
    let state = test_dir().join("state.json");
    std::fs::write(&state, serde_json::json!({ "recent": recent }).to_string()).unwrap();
    let mut cmd = mdv();
    cmd.arg("--recent")
        .env("HOME", &home)
        .env("USERPROFILE", &home);
    let server = start(cmd);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    let count = "document.querySelectorAll('.markdown-body .mdv-recent-item').length";
    until(&tab, &format!("{count} === 3"), "three rows on the page");

    // Remove from the page: the row goes, and the state file forgets it.
    js(
        &tab,
        "document.querySelector('.mdv-recent-item[data-path$=\"b.md\"] .mdv-recent-remove').click(); return 0;",
    );
    until(&tab, &format!("{count} === 2"), "the row removed");
    let saved = || std::fs::read_to_string(&state).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while saved().contains("b.md") && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!saved().contains("b.md"), "{}", saved());

    // The sidebar tab lists the rest; removing there works too, and the
    // page catches up.
    press(&tab, "h");
    until(
        &tab,
        "document.querySelectorAll('.mdv-recent-row').length === 2",
        "two rows in the sidebar",
    );
    js(
        &tab,
        "document.querySelector('.mdv-recent-row a[title$=\"c.md\"]').parentElement.querySelector('.mdv-recent-remove').click(); return 0;",
    );
    until(
        &tab,
        "document.querySelectorAll('.mdv-recent-row').length === 1",
        "one row left in the sidebar",
    );
    until(
        &tab,
        &format!("{count} === 1"),
        "the page updated by the server",
    );

    // Rows link to their pages.
    let href = js(
        &tab,
        "return document.querySelector('.mdv-recent-row a').getAttribute('href');",
    );
    assert_eq!(href, "/notes/a.md");
}

#[test]
fn stl_models_turn_when_dragged() {
    let Some(browser) = browser() else { return };
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("model.md");
    std::fs::write(
        &md,
        "# Model\n\n```stl\nsolid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\n\
         facet normal 0 1 0\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 0 1\nendloop\nendfacet\nendsolid t\n```\n",
    )
    .unwrap();
    let server = serve(&md, &[]);
    let tab = open(&browser, &format!("http://{}/", server.addr));
    until(
        &tab,
        "document.querySelector('.mdv-stl polygon')",
        "the model",
    );
    let points = "return [...document.querySelectorAll('.mdv-stl polygon')].map(p => p.getAttribute('points')).join('|');";
    let before = js(&tab, points);
    // The viewer redraws exactly what the page drew, before any drag.
    js(
        &tab,
        "const svg = document.querySelector('.mdv-stl svg');
         svg.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
         return 0;",
    );
    assert_eq!(js(&tab, points), before, "the same maths as the server");
    js(
        &tab,
        "const svg = document.querySelector('.mdv-stl svg');
         const r = svg.getBoundingClientRect();
         const at = (type, dx) => svg.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0,
           clientX: r.left + 100 + dx, clientY: r.top + 100 }));
         at('pointerdown', 0); at('pointermove', 80); at('pointerup', 80);
         return 0;",
    );
    until(
        &tab,
        &format!(
            "[...document.querySelectorAll('.mdv-stl polygon')].map(p => p.getAttribute('points')).join('|') !== {before}"
        ),
        "the model turned",
    );
    // Double-click goes back to the first view.
    js(
        &tab,
        "document.querySelector('.mdv-stl svg').dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); return 0;",
    );
    assert_eq!(js(&tab, points), before);
}
