//! Records the frames of the README demo: an editor window typing into a
//! file, and markdown-viewer updating beside it. Ignored by `cargo test`;
//! `docs/demo.sh` runs it and turns the frames into `docs/demo-*.gif`.
//!
//! The stage is `docs/demo/desktop.html`. The document starts as
//! `docs/demo/notes.md`, then `docs/demo/typed.md` is typed at its end,
//! saving at each `<!-- save -->` line. Frames are screenshots with a
//! duration each (an ffmpeg concat list), so the result doesn't depend on
//! how fast the machine is.

mod common;

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::*;
use headless_chrome::protocol::cdp::Page;
use headless_chrome::{Browser, LaunchOptions, Tab};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const SAVE: &str = "<!-- save -->\n";

struct Frames {
    dir: PathBuf,
    list: String,
    count: usize,
}

impl Frames {
    fn capture(&mut self, tab: &Tab, seconds: f64) {
        let clip = Page::Viewport {
            x: 0.0,
            y: 0.0,
            width: WIDTH.into(),
            height: HEIGHT.into(),
            scale: 1.0,
        };
        let png = tab
            .capture_screenshot(
                Page::CaptureScreenshotFormatOption::Png,
                None,
                Some(clip),
                true,
            )
            .unwrap();
        self.count += 1;
        let name = format!("{:05}.png", self.count);
        std::fs::write(self.dir.join(&name), png).unwrap();
        writeln!(self.list, "file '{name}'\nduration {seconds:.3}").unwrap();
    }

    fn finish(mut self) {
        // The concat demuxer ignores the last duration unless the file repeats.
        let last = format!("{:05}.png", self.count);
        writeln!(self.list, "file '{last}'").unwrap();
        std::fs::write(self.dir.join("frames.txt"), self.list).unwrap();
    }
}

fn js(tab: &Tab, code: &str) -> serde_json::Value {
    tab.evaluate(code, false)
        .unwrap_or_else(|e| panic!("JS failed: {e}\n{code}"))
        .value
        .unwrap_or(serde_json::Value::Null)
}

fn until(tab: &Tab, condition: &str, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while js(tab, &format!("!!({condition})")) != serde_json::json!(true) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn show(tab: &Tab, text: &str, pos: usize, dirty: bool) {
    let text_js = serde_json::to_string(text).unwrap();
    // `pos` counts UTF-16 units in JavaScript; the demo text is ASCII.
    js(tab, &format!("show({text_js}, {pos}, {dirty})"));
}

/// A moment for the viewer to scroll to the change. Diagrams are drawn
/// before live updates are swapped in, so there's nothing else to wait for.
fn settle() {
    std::thread::sleep(Duration::from_millis(700));
}

#[test]
#[ignore = "records the README demo: run docs/demo.sh"]
fn record_demo() {
    let out = PathBuf::from(std::env::var("MDV_DEMO_FRAMES").expect("set MDV_DEMO_FRAMES"));
    let theme = std::env::var("MDV_DEMO_THEME").unwrap_or_else(|_| "light".into());
    std::fs::create_dir_all(&out).unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/demo");
    let initial = std::fs::read_to_string(src.join("notes.md")).unwrap();
    let typed = std::fs::read_to_string(src.join("typed.md")).unwrap();

    let work = tempfile::tempdir().unwrap();
    let doc = work.path().join("notes.md");
    std::fs::write(&doc, &initial).unwrap();
    std::fs::copy(src.join("desktop.html"), work.path().join("desktop.html")).unwrap();
    // The viewer's remembered theme, with the sidebar closed.
    std::fs::write(
        test_dir().join("state.json"),
        format!(r#"{{"theme":"{theme}","sidebar":""}}"#),
    )
    .unwrap();
    let mut cmd = mdv();
    cmd.arg("--watch").arg(&doc);
    let served = start(cmd);

    let path = headless_chrome::browser::default_executable().expect("needs Chrome");
    let options = LaunchOptions::default_builder()
        .path(Some(path))
        .headless(true)
        .sandbox(false)
        .window_size(Some((WIDTH, HEIGHT + 100)))
        .idle_browser_timeout(Duration::from_secs(300))
        .args(vec![OsStr::new("--hide-scrollbars")])
        .build()
        .unwrap();
    let browser = Browser::new(options).unwrap();
    let tab = browser.new_tab().unwrap();
    tab.call_method(
        headless_chrome::protocol::cdp::Emulation::SetDeviceMetricsOverride {
            width: WIDTH,
            height: HEIGHT,
            device_scale_factor: 1.0,
            mobile: false,
            scale: None,
            screen_width: None,
            screen_height: None,
            position_x: None,
            position_y: None,
            dont_set_visible_size: None,
            screen_orientation: None,
            viewport: None,
            display_feature: None,
            device_posture: None,
        },
    )
    .unwrap();
    tab.navigate_to(&format!(
        "http://{}/desktop.html?file=notes.md&theme={theme}",
        served.addr
    ))
    .unwrap()
    .wait_until_navigated()
    .unwrap();
    until(
        &tab,
        "viewer.contentDocument && viewer.contentDocument.querySelector('.mdv-toolbar')",
        "the viewer",
    );
    settle();

    let mut frames = Frames {
        dir: out,
        list: String::new(),
        count: 0,
    };
    let mut text = initial;
    show(&tab, &text, text.len(), false);
    frames.capture(&tab, 1.5);

    for chunk in typed.split_inclusive(SAVE) {
        for c in chunk.trim_end_matches(SAVE).chars() {
            text.push(c);
            show(&tab, &text, text.len(), true);
            frames.capture(&tab, if c == '\n' { 0.18 } else { 0.05 });
        }
        if !chunk.ends_with(SAVE) {
            continue;
        }
        // Save: the title loses its dirty marker and the viewer catches up.
        let before = js(&tab, "updates").as_u64().unwrap();
        std::fs::write(&doc, &text).unwrap();
        show(&tab, &text, text.len(), false);
        until(&tab, &format!("updates > {before}"), "the live update");
        settle();
        frames.capture(&tab, 1.6);
    }
    frames.capture(&tab, 2.5);
    frames.finish();
}
