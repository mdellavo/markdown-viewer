//! Helpers shared by the end-to-end tests.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const BIN: &str = env!("CARGO_BIN_EXE_markdown-viewer");

/// This test's private folder for settings and state.
pub fn test_dir() -> std::path::PathBuf {
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
pub fn mdv() -> Command {
    let mut cmd = Command::new(BIN);
    let dir = test_dir();
    cmd.env("MARKDOWN_VIEWER_CONFIG", dir.join("config.toml"))
        .env("MARKDOWN_VIEWER_STATE", dir.join("state.json"));
    cmd
}

pub struct Running(pub Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn http_get(addr: &str, path: &str, host: &str) -> String {
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

pub fn wait_for(reader: &mut impl BufRead, needle: &str) {
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

/// A running `--watch` server and its address.
pub struct Served {
    pub addr: String,
    _stdout: BufReader<std::process::ChildStdout>,
    _guard: Running,
}

pub fn start(mut cmd: Command) -> Served {
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

pub fn http(addr: &str, request: &str) -> String {
    let mut s = TcpStream::connect(addr).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).into_owned()
}

/// A fake editor that writes the arguments it gets to `log`: a `.cmd` file
/// on Windows, a shell script elsewhere. Returns an `--editor` template.
pub fn fake_editor(dir: &std::path::Path, log: &std::path::Path) -> String {
    let editor = if cfg!(windows) {
        let editor = dir.join("fake-editor.cmd");
        std::fs::write(&editor, format!("@echo %*> \"{}\"\r\n", log.display())).unwrap();
        editor
    } else {
        let editor = dir.join("fake-editor.sh");
        std::fs::write(&editor, format!("#!/bin/sh\necho \"$@\" > {log:?}\n")).unwrap();
        Command::new("chmod")
            .arg("+x")
            .arg(&editor)
            .status()
            .unwrap();
        editor
    };
    format!("\"{}\" {{file}}:{{line}}", editor.display())
}

/// `file:` URL for an absolute path (`file:///C:/…` on Windows).
pub fn file_url(path: &std::path::Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    format!("file://{}", path.replace(' ', "%20"))
}
