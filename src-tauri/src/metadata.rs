//! Fetches a page's title and favicon. Icons are saved as `<content hash>.<ext>` in the
//! favicons dir, so identical icons are stored once.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::{Client, Url};
use scraper::{Html, Selector};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ICON_BYTES: usize = 512 * 1024;
/// Icon URLs tried per page, best first; each can take up to `TIMEOUT`.
const MAX_ICON_ATTEMPTS: usize = 3;
const MAX_TITLE_CHARS: usize = 300;
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (compatible; LinkOpener/",
    env!("CARGO_PKG_VERSION"),
    ")"
);

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub title: Option<String>,
    /// File name in the favicons dir.
    pub favicon: Option<String>,
}

/// HTTP client and icon storage, held in Tauri state. Cheap to clone.
#[derive(Clone)]
pub struct Fetcher {
    client: Client,
    pub favicon_dir: PathBuf,
}

impl Fetcher {
    pub fn new(favicon_dir: PathBuf) -> AppResult<Self> {
        // On Linux reqwest uses rustls without a bundled crypto provider; install ring.
        // Errors if one is already installed, which is fine.
        #[cfg(target_os = "linux")]
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .connect_timeout(TIMEOUT)
            .build()
            .map_err(|e| AppError::Fetch(e.to_string()))?;
        fs::create_dir_all(&favicon_dir)?;
        Ok(Self {
            client,
            favicon_dir,
        })
    }

    /// Title and favicon for an http(s) URL. A page that can't be fetched still gets the
    /// site's `/favicon.ico` tried.
    pub async fn fetch(&self, url: &str) -> AppResult<PageMeta> {
        let url = Url::parse(url).map_err(|e| AppError::Invalid(format!("invalid URL: {e}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Ok(PageMeta::default());
        }

        let (title, page_icons, final_url) = match self.get_page(&url).await {
            Ok((final_url, html)) => {
                let page = parse_page(&html, &final_url);
                (page.title, page.icons, Some(final_url))
            }
            Err(_) => (None, Vec::new(), None),
        };
        let icons = icon_attempts(page_icons, &url, final_url.as_ref());

        let mut favicon = None;
        for icon in &icons {
            if let Ok(Some(name)) = self.get_icon(icon).await {
                favicon = Some(name);
                break;
            }
        }
        Ok(PageMeta { title, favicon })
    }

    async fn get_page(&self, url: &Url) -> AppResult<(Url, String)> {
        let resp = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(fetch_err)?;
        if !resp.status().is_success() {
            return Err(AppError::Fetch(format!("HTTP {}", resp.status())));
        }
        let is_html = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_none_or(|ct| ct.contains("html"));
        if !is_html {
            return Err(AppError::Fetch("not an HTML page".into()));
        }
        let final_url = resp.url().clone();
        let body = read_limited(resp, MAX_PAGE_BYTES).await?;
        Ok((final_url, String::from_utf8_lossy(&body).into_owned()))
    }

    /// Downloads and stores an icon; `None` if it isn't a recognisable image.
    async fn get_icon(&self, url: &Url) -> AppResult<Option<String>> {
        let resp = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(fetch_err)?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let bytes = read_limited(resp, MAX_ICON_BYTES).await?;
        store_icon(&self.favicon_dir, &bytes)
    }
}

fn fetch_err(e: reqwest::Error) -> AppError {
    AppError::Fetch(e.to_string())
}

/// Reads the body, stopping (with what was read) once `limit` bytes have arrived.
async fn read_limited(mut resp: reqwest::Response, limit: usize) -> AppResult<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(fetch_err)? {
        body.extend_from_slice(&chunk);
        if body.len() >= limit {
            body.truncate(limit);
            break;
        }
    }
    Ok(body)
}

#[derive(Debug, PartialEq)]
pub struct ParsedPage {
    pub title: Option<String>,
    /// Icon URLs, best first.
    pub icons: Vec<Url>,
}

/// `og:title` (usually without the " | Site" suffix), else `<title>`; icon links ranked.
pub fn parse_page(html: &str, base: &Url) -> ParsedPage {
    let doc = Html::parse_document(html);
    let sel = |s: &str| Selector::parse(s).expect("valid selector");

    let og_title = doc
        .select(&sel(r#"meta[property="og:title"], meta[name="og:title"]"#))
        .find_map(|m| m.value().attr("content"))
        .map(clean_title);
    let title = og_title.flatten().or_else(|| {
        doc.select(&sel("title"))
            .next()
            .and_then(|t| clean_title(&t.text().collect::<String>()))
    });

    // `<base href>` changes how relative links resolve.
    let base = doc
        .select(&sel("base[href]"))
        .next()
        .and_then(|b| base.join(b.value().attr("href")?).ok())
        .unwrap_or_else(|| base.clone());

    let mut icons: Vec<(i32, Url)> = doc
        .select(&sel("link[rel][href]"))
        .filter_map(|link| {
            let el = link.value();
            let rel = el.attr("rel")?.to_ascii_lowercase();
            let href = el.attr("href")?.trim();
            if href.is_empty() || href.starts_with("data:") {
                return None;
            }
            let score = icon_score(&rel, el.attr("sizes"), el.attr("type"), href)?;
            Some((score, base.join(href).ok()?))
        })
        .collect();
    // Stable: equal scores keep document order.
    icons.sort_by_key(|(score, _)| -score);
    let mut seen = HashSet::new();
    let icons = icons
        .into_iter()
        .map(|(_, url)| url)
        .filter(|u| seen.insert(u.clone()))
        .collect();

    ParsedPage { title, icons }
}

/// The best few page icons, then `/favicon.ico` on the site the page redirected to (e.g.
/// gmail.com → accounts.google.com) and on the requested site.
fn icon_attempts(page_icons: Vec<Url>, requested: &Url, final_url: Option<&Url>) -> Vec<Url> {
    let mut icons: Vec<Url> = page_icons.into_iter().take(MAX_ICON_ATTEMPTS).collect();
    for site in final_url.into_iter().chain([requested]) {
        if let Ok(fallback) = site.join("/favicon.ico") {
            if !icons.contains(&fallback) {
                icons.push(fallback);
            }
        }
    }
    icons
}

/// Higher is better; `None` if the link isn't an icon. Aims for roughly 32–128px.
fn icon_score(rel: &str, sizes: Option<&str>, mime: Option<&str>, href: &str) -> Option<i32> {
    let tokens: Vec<&str> = rel.split_ascii_whitespace().collect();
    let apple = tokens.iter().any(|t| t.starts_with("apple-touch-icon"));
    if !apple && !tokens.contains(&"icon") {
        return None;
    }
    let svg = mime.is_some_and(|m| m.contains("svg"))
        || href
            .split(['?', '#'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .ends_with(".svg");
    let largest = sizes
        .unwrap_or("")
        .split_ascii_whitespace()
        .filter_map(|s| {
            s.to_ascii_lowercase()
                .split_once('x')?
                .0
                .parse::<i32>()
                .ok()
        })
        .max();
    Some(match largest {
        Some(px) if px >= 32 => 100 - ((px - 64).abs() / 16).min(40),
        Some(px) => 30 + px,
        None if svg => 90,
        None if apple => 70, // usually 180px
        None => 60,
    })
}

fn clean_title(raw: &str) -> Option<String> {
    let title: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
    (!title.is_empty()).then_some(title)
}

/// File extension for a supported image format, from its magic bytes.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Some("ico")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
        let head = head.trim_start_matches('\u{feff}').trim_start();
        ((head.starts_with("<svg") || head.starts_with("<?xml")) && head.contains("<svg"))
            .then_some("svg")
    }
}

/// Saves image bytes in the favicons dir and returns the file name; `None` if the bytes
/// aren't a supported image or are too large.
pub fn store_icon(dir: &Path, bytes: &[u8]) -> AppResult<Option<String>> {
    if bytes.len() > MAX_ICON_BYTES {
        return Ok(None);
    }
    let Some(ext) = sniff_image(bytes) else {
        return Ok(None);
    };
    let name = icon_file_name(bytes, ext);
    let path = dir.join(&name);
    if !path.exists() {
        fs::write(&path, bytes)?;
    }
    Ok(Some(name))
}

fn icon_file_name(bytes: &[u8], ext: &str) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("{hex}.{ext}")
}

/// Whether `name` looks like a file this module wrote (so it is safe to join to the dir).
pub fn is_favicon_name(name: &str) -> bool {
    let Some((hash, ext)) = name.split_once('.') else {
        return false;
    };
    hash.len() == 32
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && ["png", "ico", "gif", "jpg", "webp", "svg"].contains(&ext)
}

/// Deletes icon files no bookmark refers to.
pub fn remove_unused(dir: &Path, used: &HashSet<String>) -> AppResult<usize> {
    let mut removed = 0;
    for entry in fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_favicon_name(&name) && !used.contains(&name) && fs::remove_file(entry.path()).is_ok()
        {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR fake png body";

    fn base() -> Url {
        Url::parse("https://example.com/docs/page.html").unwrap()
    }

    #[test]
    fn og_title_preferred_and_cleaned() {
        let html = r#"<html><head>
            <title>  Rust   Book | Docs  </title>
            <meta property="og:title" content="The Rust Book">
        </head></html>"#;
        assert_eq!(
            parse_page(html, &base()).title.as_deref(),
            Some("The Rust Book")
        );

        let html = "<title>\n  Plain &amp; Simple\n</title>";
        assert_eq!(
            parse_page(html, &base()).title.as_deref(),
            Some("Plain & Simple")
        );
        assert_eq!(parse_page("<p>no title</p>", &base()).title, None);
        assert_eq!(parse_page("<title>   </title>", &base()).title, None);
    }

    #[test]
    fn icons_ranked_and_resolved() {
        let html = r#"<head>
            <link rel="stylesheet" href="/style.css">
            <link rel="icon" href="tiny.png" sizes="16x16">
            <link rel="apple-touch-icon" href="/apple.png">
            <link rel="icon" type="image/png" href="https://cdn.example.com/i64.png" sizes="64x64">
            <link rel="mask-icon" href="/mask.svg">
            <link rel="shortcut icon" href="/favicon.ico">
            <link rel="icon" href="data:image/png;base64,AAAA">
            <link rel="icon" href="/favicon.ico">
        </head>"#;
        let icons: Vec<String> = parse_page(html, &base())
            .icons
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(
            icons,
            [
                "https://cdn.example.com/i64.png",
                "https://example.com/apple.png",
                "https://example.com/favicon.ico",
                "https://example.com/docs/tiny.png",
            ]
        );
    }

    #[test]
    fn icon_attempt_order() {
        let u = |s: &str| Url::parse(s).unwrap();
        let page: Vec<Url> = (1..=5)
            .map(|i| u(&format!("https://cdn.example/{i}.png")))
            .collect();
        let attempts: Vec<String> = icon_attempts(
            page,
            &u("https://gmail.com/"),
            Some(&u("https://accounts.google.com/signin")),
        )
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            attempts,
            [
                "https://cdn.example/1.png",
                "https://cdn.example/2.png",
                "https://cdn.example/3.png",
                "https://accounts.google.com/favicon.ico",
                "https://gmail.com/favicon.ico",
            ]
        );
        // No redirect, page unreachable: just the site's favicon.ico, once.
        let attempts = icon_attempts(vec![], &u("https://a.example/x"), None);
        assert_eq!(attempts, [u("https://a.example/favicon.ico")]);
        let attempts = icon_attempts(
            vec![],
            &u("https://a.example/x"),
            Some(&u("https://a.example/y")),
        );
        assert_eq!(attempts, [u("https://a.example/favicon.ico")]);
    }

    #[test]
    fn base_href_applies() {
        let html =
            r#"<base href="https://static.example.org/assets/"><link rel="icon" href="f.ico">"#;
        let icons = parse_page(html, &base()).icons;
        assert_eq!(icons[0].as_str(), "https://static.example.org/assets/f.ico");
    }

    #[test]
    fn sniffing() {
        assert_eq!(sniff_image(PNG), Some("png"));
        assert_eq!(sniff_image(&[0, 0, 1, 0, 1, 0]), Some("ico"));
        assert_eq!(sniff_image(b"GIF89a..."), Some("gif"));
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(sniff_image(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(
            sniff_image(b"\xEF\xBB\xBF <svg xmlns='...'></svg>"),
            Some("svg")
        );
        assert_eq!(
            sniff_image(b"<?xml version='1.0'?><svg></svg>"),
            Some("svg")
        );
        assert_eq!(sniff_image(b"<!doctype html><html>"), None);
        assert_eq!(sniff_image(b""), None);
    }

    #[test]
    fn file_names() {
        let name = icon_file_name(PNG, "png");
        assert!(is_favicon_name(&name), "{name}");
        assert_eq!(name, icon_file_name(PNG, "png"));
        assert!(!is_favicon_name("../../etc/passwd"));
        assert!(!is_favicon_name("0123456789abcdef0123456789abcdef.exe"));
        assert!(!is_favicon_name("0123456789ABCDEF0123456789abcdef.png"));
    }

    #[test]
    fn remove_unused_keeps_referenced() {
        let dir = tempfile::tempdir().unwrap();
        let keep = icon_file_name(b"a", "png");
        let drop = icon_file_name(b"b", "png");
        for name in [&keep, &drop, &"notes.txt".to_string()] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        let used = HashSet::from([keep.clone()]);
        assert_eq!(remove_unused(dir.path(), &used).unwrap(), 1);
        assert!(dir.path().join(&keep).exists());
        assert!(!dir.path().join(&drop).exists());
        assert!(dir.path().join("notes.txt").exists());
    }

    /// Serves `routes` (path → (content type, body)) over plain HTTP; 404 otherwise.
    fn serve(routes: Vec<(&'static str, &'static str, &'static [u8])>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(&stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                // Drain headers.
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("/")
                    .to_string();
                let mut stream = &stream;
                match routes.iter().find(|(p, _, _)| *p == path) {
                    Some((_, ctype, body)) => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    }
                }
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn fetches_title_and_icon_from_server() {
        let origin = serve(vec![
            (
                "/",
                "text/html; charset=utf-8",
                br#"<title>Local Test</title><link rel="icon" href="/missing.png" sizes="64x64"><link rel="icon" href="/icon.png">"#,
            ),
            ("/icon.png", "image/png", PNG),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let fetcher = Fetcher::new(dir.path().join("favicons")).unwrap();
        let meta = tauri::async_runtime::block_on(fetcher.fetch(&format!("{origin}/"))).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Local Test"));
        // The better-ranked icon 404s, so the next one is used.
        let name = meta.favicon.unwrap();
        assert_eq!(fs::read(fetcher.favicon_dir.join(&name)).unwrap(), PNG);
    }

    #[test]
    fn falls_back_to_favicon_ico() {
        let origin = serve(vec![("/favicon.ico", "image/x-icon", &[0, 0, 1, 0, 1, 0])]);
        let dir = tempfile::tempdir().unwrap();
        let fetcher = Fetcher::new(dir.path().to_path_buf()).unwrap();
        // The page itself 404s.
        let meta =
            tauri::async_runtime::block_on(fetcher.fetch(&format!("{origin}/article"))).unwrap();
        assert_eq!(meta.title, None);
        assert!(meta.favicon.unwrap().ends_with(".ico"));
    }

    /// Needs the internet: `cargo test live_fetch -- --ignored --nocapture`.
    /// Set `LIVE_URLS` (space-separated) to try other pages.
    #[test]
    #[ignore]
    fn live_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let fetcher = Fetcher::new(dir.path().to_path_buf()).unwrap();
        let urls = std::env::var("LIVE_URLS").unwrap_or_else(|_| {
            "https://www.rust-lang.org/ https://github.com/tauri-apps/tauri".into()
        });
        for url in urls.split_whitespace() {
            let meta = tauri::async_runtime::block_on(fetcher.fetch(url)).unwrap();
            println!("{url}: {meta:?}");
            assert!(
                meta.title.is_some() && meta.favicon.is_some(),
                "{url}: {meta:?}"
            );
        }
    }

    #[test]
    fn non_http_urls_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let fetcher = Fetcher::new(dir.path().to_path_buf()).unwrap();
        let meta = tauri::async_runtime::block_on(fetcher.fetch("mailto:me@example.com")).unwrap();
        assert_eq!(meta, PageMeta::default());
    }
}
