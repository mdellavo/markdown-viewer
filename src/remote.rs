//! Markdown from the web: `markdown-viewer https://…/README.md`.
//!
//! The document is fetched with `curl` (as `--check-links --online` does).
//! GitHub and GitLab page addresses are turned into their raw files, and a
//! repository's address shows its README. Relative links in the document
//! are made absolute: links against the page (so they open on GitHub),
//! images against the raw file (so they load).

use std::process::Command;

use anyhow::{Context, Result, bail};
use url::Url;

/// Largest document fetched.
const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// Is this argument a web address rather than a file?
pub fn is_url(arg: &str) -> bool {
    let lower = arg.get(..8).unwrap_or(arg).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// A document on the web.
#[derive(Debug, Clone, PartialEq)]
pub struct Remote {
    /// The address as given (for titles and messages).
    pub url: String,
    /// What to download: the raw Markdown.
    pub fetch: Url,
    /// Relative links resolve against this.
    pub links: Url,
    /// Relative images resolve against this (the raw file).
    pub images: Url,
}

impl Remote {
    pub fn new(address: &str) -> Result<Self> {
        let given = Url::parse(address).with_context(|| format!("not a web address: {address}"))?;
        let host = given.host_str().unwrap_or("").to_ascii_lowercase();
        let parts: Vec<&str> = given
            .path_segments()
            .map(|s| s.filter(|p| !p.is_empty()).collect())
            .unwrap_or_default();
        let (fetch, links) = match (host.as_str(), parts.as_slice()) {
            // github.com/owner/repo/blob/ref/path → raw.githubusercontent.com/owner/repo/ref/path
            ("github.com", [owner, repo, "blob", rest @ ..]) if !rest.is_empty() => {
                (github_raw(owner, repo, rest)?, given.clone())
            }
            // github.com/owner/repo[/tree/ref/dir] → that folder's README
            ("github.com", [owner, repo, "tree", rest @ ..]) if !rest.is_empty() => {
                let mut path: Vec<&str> = rest.to_vec();
                path.push("README.md");
                let links = Url::parse(&format!(
                    "https://github.com/{owner}/{repo}/blob/{}",
                    path.join("/")
                ))?;
                (github_raw(owner, repo, &path)?, links)
            }
            ("github.com", [owner, repo]) => {
                let repo = repo.trim_end_matches(".git");
                (
                    github_raw(owner, repo, &["HEAD", "README.md"])?,
                    Url::parse(&format!(
                        "https://github.com/{owner}/{repo}/blob/HEAD/README.md"
                    ))?,
                )
            }
            // gitlab.com/group/project/-/blob/ref/path → …/-/raw/ref/path
            (h, _) if h.contains("gitlab") && given.path().contains("/-/blob/") => {
                let mut raw = given.clone();
                raw.set_path(&given.path().replacen("/-/blob/", "/-/raw/", 1));
                (raw, given.clone())
            }
            _ => (given.clone(), given.clone()),
        };
        Ok(Remote {
            url: address.to_string(),
            images: fetch.clone(),
            fetch,
            links,
        })
    }

    /// A short title: the file name, or the host for a bare address.
    pub fn title(&self) -> String {
        self.links
            .path_segments()
            .and_then(|mut s| s.rfind(|p| !p.is_empty()).map(str::to_string))
            .map(|name| {
                percent_encoding::percent_decode_str(&name)
                    .decode_utf8_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| self.links.host_str().unwrap_or("web page").to_string())
    }

    /// Download the Markdown.
    pub fn fetch(&self) -> Result<String> {
        let out = Command::new("curl")
            .args(["--silent", "--show-error", "--location", "--fail"])
            .args(["--max-time", "30", "--max-filesize", &MAX_BYTES.to_string()])
            .args([
                "--user-agent",
                concat!("markdown-viewer/", env!("CARGO_PKG_VERSION")),
            ])
            .args(["--write-out", "\n%{content_type}"])
            .arg(self.fetch.as_str())
            .output();
        let out = match out {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("opening web addresses needs curl, which wasn't found")
            }
            Err(e) => return Err(e).context("cannot run curl"),
        };
        if !out.status.success() {
            bail!(
                "cannot fetch {}: {}",
                self.fetch,
                String::from_utf8_lossy(&out.stderr)
                    .trim()
                    .trim_start_matches("curl: ")
            );
        }
        let body = String::from_utf8_lossy(&out.stdout);
        let (text, content_type) = body.rsplit_once('\n').unwrap_or((&body, ""));
        if content_type.to_ascii_lowercase().starts_with("text/html")
            && text.trim_start().starts_with('<')
        {
            bail!(
                "{} is a web page, not a Markdown file (for GitHub, use the file's address)",
                self.url
            );
        }
        Ok(text.to_string())
    }
}

fn github_raw(owner: &str, repo: &str, rest: &[&str]) -> Result<Url> {
    Ok(Url::parse(&format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{}",
        rest.join("/")
    ))?)
}

/// Make relative links and images in rendered HTML absolute, so they work
/// on a page that isn't where the document lives.
pub fn absolutize(html: &str, remote: &Remote) -> String {
    let mut out = String::with_capacity(html.len() + 256);
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        let (before, tag_start) = rest.split_at(at);
        out.push_str(before);
        let end = tag_start.find('>').map_or(tag_start.len(), |e| e + 1);
        let mut tag = tag_start[..end].to_string();
        let name = tag[1..]
            .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let attrs: &[(&str, bool)] = match name.as_str() {
            "a" => &[("href", false)],
            "img" | "source" => &[("src", true), ("srcset", true)],
            "video" => &[("src", true), ("poster", true)],
            "audio" => &[("src", true)],
            _ => &[],
        };
        for (attr, image) in attrs {
            let base = if *image {
                &remote.images
            } else {
                &remote.links
            };
            tag = rewrite_attr(&tag, attr, base);
        }
        out.push_str(&tag);
        rest = &tag_start[end..];
    }
    out.push_str(rest);
    out
}

fn rewrite_attr(tag: &str, attr: &str, base: &Url) -> String {
    let key = format!(" {attr}=\"");
    let Some(start) = tag.find(&key).map(|i| i + key.len()) else {
        return tag.to_string();
    };
    let Some(len) = tag[start..].find('"') else {
        return tag.to_string();
    };
    let value = crate::links::unescape(&tag[start..start + len]);
    let join = |u: &str| -> String {
        // Fragments stay on this page; absolute addresses are unchanged.
        if u.is_empty() || u.starts_with('#') {
            return u.to_string();
        }
        base.join(u)
            .map(|j| j.to_string())
            .unwrap_or_else(|_| u.to_string())
    };
    let new = if attr == "srcset" {
        value
            .split(',')
            .map(|item| {
                let item = item.trim();
                let (u, descriptor) = item.split_once(' ').unwrap_or((item, ""));
                format!("{} {descriptor}", join(u)).trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        join(&value)
    };
    format!(
        "{}{}{}",
        &tag[..start],
        crate::render::escape_attr(&new),
        &tag[start + len..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_web_addresses() {
        assert!(is_url("https://example.com/a.md"));
        assert!(is_url("HTTP://x"));
        assert!(!is_url("notes.md"));
        assert!(!is_url("./https.md"));
    }

    #[test]
    fn github_and_gitlab_addresses_become_raw_files() {
        let r = Remote::new("https://github.com/o/r/blob/main/docs/guide.md").unwrap();
        assert_eq!(
            r.fetch.as_str(),
            "https://raw.githubusercontent.com/o/r/main/docs/guide.md"
        );
        assert_eq!(
            r.links.as_str(),
            "https://github.com/o/r/blob/main/docs/guide.md"
        );
        assert_eq!(r.title(), "guide.md");

        let r = Remote::new("https://github.com/o/r").unwrap();
        assert_eq!(
            r.fetch.as_str(),
            "https://raw.githubusercontent.com/o/r/HEAD/README.md"
        );
        assert_eq!(
            r.links.as_str(),
            "https://github.com/o/r/blob/HEAD/README.md"
        );

        let r = Remote::new("https://github.com/o/r/tree/dev/docs").unwrap();
        assert_eq!(
            r.fetch.as_str(),
            "https://raw.githubusercontent.com/o/r/dev/docs/README.md"
        );

        let r = Remote::new("https://gitlab.com/g/p/-/blob/main/README.md").unwrap();
        assert_eq!(
            r.fetch.as_str(),
            "https://gitlab.com/g/p/-/raw/main/README.md"
        );

        let r = Remote::new("https://example.com/notes/a%20b.md").unwrap();
        assert_eq!(r.fetch, r.links);
        assert_eq!(r.title(), "a b.md");
    }

    #[test]
    fn links_point_at_the_page_and_images_at_the_raw_file() {
        let r = Remote::new("https://github.com/o/r/blob/main/docs/guide.md").unwrap();
        let html = "<p><a href=\"other.md#x\">o</a> <a href=\"#top\">t</a> \
                    <a href=\"https://x.org/\">x</a> <img src=\"img/a.png\" alt=\"\"> \
                    <source srcset=\"../b.png 2x, c.png\"></p>";
        let out = absolutize(html, &r);
        assert!(
            out.contains("href=\"https://github.com/o/r/blob/main/docs/other.md#x\""),
            "{out}"
        );
        assert!(out.contains("href=\"#top\""));
        assert!(out.contains("href=\"https://x.org/\""));
        assert!(out.contains("src=\"https://raw.githubusercontent.com/o/r/main/docs/img/a.png\""));
        assert!(out.contains(
            "srcset=\"https://raw.githubusercontent.com/o/r/main/b.png 2x, https://raw.githubusercontent.com/o/r/main/docs/c.png\""
        ), "{out}");
    }
}
