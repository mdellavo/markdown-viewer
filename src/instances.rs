//! A registry of running viewers, so opening a file (or folder) that's
//! already being served reuses that viewer and its window instead of
//! starting another.
//!
//! Each running server writes `instances/<hash>.json` next to the state
//! file, holding its port and what it serves, and removes it on exit. A
//! leftover entry from a crashed run is ignored: before reusing an entry we
//! ask the server who it is (`/__mdv/whoami`).

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const WHOAMI_PATH: &str = "/__mdv/whoami";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub port: u16,
    pub pid: u32,
    /// What it serves (canonical path).
    pub target: String,
}

/// Removes the registry entry when dropped (i.e. when the server stops).
pub struct Registration(PathBuf);

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn dir() -> Option<PathBuf> {
    crate::config::state_path().and_then(|p| p.parent().map(|d| d.join("instances")))
}

fn entry_path(target: &Path) -> Option<PathBuf> {
    Some(dir()?.join(format!(
        "{:016x}.json",
        fnv1a(target.to_string_lossy().as_bytes())
    )))
}

/// Stable across runs and Rust versions (unlike `DefaultHasher`).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Record that this process serves `target` on `port`.
pub fn register(target: &Path, port: u16) -> Option<Registration> {
    let path = entry_path(target)?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let entry = Entry {
        port,
        pid: std::process::id(),
        target: target.to_string_lossy().into_owned(),
    };
    std::fs::write(&path, serde_json::to_string(&entry).ok()?).ok()?;
    Some(Registration(path))
}

/// The URL of a live viewer already serving `target`, if there is one.
pub fn find(target: &Path) -> Option<String> {
    let path = entry_path(target)?;
    let entry: Entry = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
    if entry.target != target.to_string_lossy() || entry.pid == std::process::id() {
        return None;
    }
    if whoami(entry.port).as_deref() == Some(entry.target.as_str()) {
        Some(format!("http://127.0.0.1:{}/", entry.port))
    } else {
        // Stale (the server crashed or the port was reused): forget it.
        let _ = std::fs::remove_file(&path);
        None
    }
}

/// Ask the server on `port` what it serves.
fn whoami(port: u16) -> Option<String> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let timeout = Duration::from_millis(800);
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?;
    write!(
        s,
        "GET {WHOAMI_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    s.read_to_string(&mut response).ok()?;
    let body = response.split_once("\r\n\r\n")?.1;
    let value: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    value["target"].as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn dead_entries_are_ignored() {
        // Nothing listens on port 1.
        assert_eq!(whoami(1), None);
    }
}
