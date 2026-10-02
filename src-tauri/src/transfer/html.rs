//! Browser bookmark files (the "Netscape bookmark file" format that Chrome, Edge, Firefox
//! and Safari import and export).
//!
//! It is loose HTML (unclosed `<DT>` and `<p>`), so this is a small tag scanner rather than
//! an HTML parser: `<H3>` names the next `<DL>`'s folder, `<A>` is a bookmark in the
//! innermost open `<DL>`, and a `<DD>` right after a bookmark is its description.

use std::collections::HashMap;
use std::fmt::Write as _;

use super::{BookmarkRecord, FolderRecord, Library};

/// URL schemes that only mean something inside the browser that exported them.
const SKIPPED_SCHEMES: [&str; 2] = ["place:", "javascript:"];

pub fn parse(html: &str) -> Library {
    // Same byte offsets as `html`: ASCII lowercasing doesn't change lengths.
    let lower = html.to_ascii_lowercase();
    let mut lib = Library::default();
    // The folder of each open <DL>; `None` = top level.
    let mut stack: Vec<Option<i64>> = Vec::new();
    let mut pending_folder: Option<String> = None;
    let mut last_bookmark: Option<usize> = None;
    let mut folder_counts: HashMap<Option<i64>, i64> = HashMap::new();
    let mut bookmark_counts: HashMap<Option<i64>, i64> = HashMap::new();
    let mut pos = 0;

    while let Some(rel) = html[pos..].find('<') {
        let start = pos + rel;
        let Some(end_rel) = html[start..].find('>') else {
            break;
        };
        let end = start + end_rel;
        let tag = &html[start + 1..end];
        pos = end + 1;
        let (name, attrs) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        let current = stack.last().copied().flatten();

        match name.to_ascii_lowercase().as_str() {
            "h3" => {
                let (text, after) = text_until(html, &lower, pos, "</h3");
                pos = after;
                pending_folder = Some(decode(text).trim().to_string());
                last_bookmark = None;
            }
            "dl" => {
                if let Some(name) = pending_folder.take() {
                    let id = lib.folders.len() as i64 + 1;
                    let position = next(&mut folder_counts, current);
                    let name = if name.is_empty() {
                        "Untitled folder".to_string()
                    } else {
                        name
                    };
                    lib.folders.push(FolderRecord {
                        id,
                        parent_id: current,
                        name,
                        position,
                        default_target: None,
                    });
                    stack.push(Some(id));
                } else {
                    // The outermost list (or an unnamed one) belongs to the enclosing folder.
                    stack.push(current);
                }
                last_bookmark = None;
            }
            "/dl" => {
                stack.pop();
                last_bookmark = None;
            }
            "a" => {
                let (text, after) = text_until(html, &lower, pos, "</a");
                pos = after;
                last_bookmark = None;
                let attrs = parse_attrs(attrs);
                let Some(url) = attrs.get("href").map(|u| u.trim().to_string()) else {
                    continue;
                };
                let lower_url = url.to_ascii_lowercase();
                if url.is_empty() || SKIPPED_SCHEMES.iter().any(|s| lower_url.starts_with(s)) {
                    continue;
                }
                let created = attrs.get("add_date").and_then(|d| timestamp_ms(d));
                lib.bookmarks.push(BookmarkRecord {
                    folder_id: current,
                    title: decode(text).trim().to_string(),
                    url,
                    tags: attrs
                        .get("tags")
                        .map(|t| {
                            t.split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect()
                        })
                        .unwrap_or_default(),
                    // A data: URI here; the importer stores it as a favicon file.
                    favicon: attrs
                        .get("icon")
                        .filter(|i| i.starts_with("data:"))
                        .cloned(),
                    position: next(&mut bookmark_counts, current),
                    created_at: created,
                    updated_at: attrs
                        .get("last_modified")
                        .and_then(|d| timestamp_ms(d))
                        .or(created),
                    ..Default::default()
                });
                last_bookmark = Some(lib.bookmarks.len() - 1);
            }
            "dd" => {
                // The description runs to the next tag.
                let text_end = html[pos..].find('<').map_or(html.len(), |i| pos + i);
                if let Some(i) = last_bookmark.take() {
                    lib.bookmarks[i].notes = decode(&html[pos..text_end]).trim().to_string();
                }
                pos = text_end;
            }
            _ => {}
        }
    }
    lib
}

/// Writes `lib` as a bookmark file browsers can import. Unfiled bookmarks go at the top level.
pub fn write(lib: &Library) -> String {
    let mut out = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n\
         <!-- This is an automatically generated file. Exported from Link Opener. -->\n\
         <META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n\
         <TITLE>Bookmarks</TITLE>\n\
         <H1>Bookmarks</H1>\n\
         <DL><p>\n",
    );
    write_level(&mut out, lib, None, 1, 0);
    out.push_str("</DL><p>\n");
    out
}

fn write_level(out: &mut String, lib: &Library, parent: Option<i64>, indent: usize, depth: usize) {
    let pad = "    ".repeat(indent);
    let mut bookmarks: Vec<&BookmarkRecord> = lib
        .bookmarks
        .iter()
        .filter(|b| b.folder_id == parent)
        .collect();
    bookmarks.sort_by_key(|b| b.position);
    for b in bookmarks {
        let _ = write!(out, "{pad}<DT><A HREF=\"{}\"", escape(&b.url));
        if let Some(created) = b.created_at {
            let _ = write!(out, " ADD_DATE=\"{}\"", created / 1000);
        }
        if let Some(updated) = b.updated_at {
            let _ = write!(out, " LAST_MODIFIED=\"{}\"", updated / 1000);
        }
        if !b.tags.is_empty() {
            let _ = write!(out, " TAGS=\"{}\"", escape(&b.tags.join(",")));
        }
        let title = if b.title.is_empty() { &b.url } else { &b.title };
        let _ = writeln!(out, ">{}</A>", escape(title));
        if !b.notes.trim().is_empty() {
            let _ = writeln!(out, "{pad}<DD>{}", escape(b.notes.trim()));
        }
    }
    // Depth bound guards against a parent cycle in a hand-edited backup.
    if depth > 100 {
        return;
    }
    let mut folders: Vec<&FolderRecord> = lib
        .folders
        .iter()
        .filter(|f| f.parent_id == parent)
        .collect();
    folders.sort_by_key(|f| f.position);
    for f in folders {
        let _ = writeln!(out, "{pad}<DT><H3>{}</H3>", escape(&f.name));
        let _ = writeln!(out, "{pad}<DL><p>");
        write_level(out, lib, Some(f.id), indent + 1, depth + 1);
        let _ = writeln!(out, "{pad}</DL><p>");
    }
}

fn next(counts: &mut HashMap<Option<i64>, i64>, key: Option<i64>) -> i64 {
    let n = counts.entry(key).or_insert(0);
    *n += 1;
    *n - 1
}

/// Text from `pos` to the closing tag `needle` (lowercase, e.g. "</a"), and the offset just
/// past that tag. Without a closing tag, the rest of the input.
fn text_until<'a>(html: &'a str, lower: &str, pos: usize, needle: &str) -> (&'a str, usize) {
    match lower[pos..].find(needle) {
        Some(i) => {
            let close = pos + i;
            let after = html[close..]
                .find('>')
                .map_or(html.len(), |j| close + j + 1);
            (&html[pos..close], after)
        }
        None => (&html[pos..], html.len()),
    }
}

/// `NAME="value"`, `name='value'` or `name=value`; names lowercased, values decoded.
fn parse_attrs(src: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let mut rest = src.trim_start();
    while !rest.is_empty() {
        let name_end = rest
            .find(|c: char| c == '=' || c.is_whitespace())
            .unwrap_or(rest.len());
        let name = rest[..name_end].to_ascii_lowercase();
        rest = rest[name_end..].trim_start();
        let value = if let Some(after_eq) = rest.strip_prefix('=') {
            let after_eq = after_eq.trim_start();
            let (value, remaining) = match after_eq.chars().next() {
                Some(q @ ('"' | '\'')) => {
                    let body = &after_eq[1..];
                    let close = body.find(q).unwrap_or(body.len());
                    (&body[..close], body.get(close + 1..).unwrap_or(""))
                }
                _ => {
                    let end = after_eq.find(char::is_whitespace).unwrap_or(after_eq.len());
                    (&after_eq[..end], &after_eq[end..])
                }
            };
            rest = remaining;
            decode(value)
        } else {
            String::new()
        };
        if !name.is_empty() {
            attrs.insert(name, value);
        }
        rest = rest.trim_start();
    }
    attrs
}

/// Unix seconds (or ms/µs, which some exporters use) to unix ms.
fn timestamp_ms(value: &str) -> Option<i64> {
    let n: i64 = value.trim().parse().ok().filter(|n| *n > 0)?;
    Some(match n {
        n if n < 100_000_000_000 => n * 1000, // seconds
        n if n < 100_000_000_000_000 => n,    // milliseconds
        n => n / 1000,                        // microseconds
    })
}

/// Decodes the entities that appear in bookmark files: `&amp;` `&lt;` `&gt;` `&quot;`
/// `&apos;` and numeric ones. Anything else is left as is.
pub fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let semi = rest
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(j, _)| j);
        let decoded = semi.and_then(|j| {
            let entity = &rest[1..j];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => {
                    let code = entity.strip_prefix('#')?;
                    let n = match code.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => code.parse().ok()?,
                    };
                    char::from_u32(n)
                }
            };
            c.map(|c| (c, j))
        });
        match decoded {
            Some((c, j)) => {
                out.push(c);
                rest = &rest[j + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a real Chrome export, plus Firefox's TAGS/DD and some awkward bits.
    const CHROME: &str = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<!-- This is an automatically generated file. -->
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1700000000" LAST_MODIFIED="1700000100" PERSONAL_TOOLBAR_FOLDER="true">Bookmarks bar</H3>
    <DL><p>
        <DT><A HREF="https://www.rust-lang.org/" ADD_DATE="1700000200" ICON="data:image/png;base64,iVBORw0KGgo=">Rust &amp; Cargo</A>
        <DT><H3>Work &lt;stuff&gt;</H3>
        <DL><p>
            <DT><A HREF="https://example.com/a?x=1&amp;y=2" TAGS="docs,Work">Example A</A>
            <DD>First line of notes &#8212; with an em dash
            <DT><A HREF="javascript:alert(1)">Bookmarklet</A>
            <DT><A HREF="place:sort=8&maxResults=10">Most visited</A>
        </DL><p>
        <HR>
        <DT><A href='https://lower.example/' add_date=1700000300000>lowercase attrs</A>
    </DL><p>
    <DT><H3>Other bookmarks</H3>
    <DL><p>
        <DT><A HREF="https://other.example/">  Other   </A>
    </DL><p>
    <DT><A HREF="https://top.example/">Top level</A>
</DL><p>
"#;

    #[test]
    fn parses_chrome_export() {
        let lib = parse(CHROME);
        let folders: Vec<(i64, Option<i64>, &str, i64)> = lib
            .folders
            .iter()
            .map(|f| (f.id, f.parent_id, f.name.as_str(), f.position))
            .collect();
        assert_eq!(
            folders,
            [
                (1, None, "Bookmarks bar", 0),
                (2, Some(1), "Work <stuff>", 0),
                (3, None, "Other bookmarks", 1)
            ]
        );

        let urls: Vec<(&str, Option<i64>, i64)> = lib
            .bookmarks
            .iter()
            .map(|b| (b.url.as_str(), b.folder_id, b.position))
            .collect();
        assert_eq!(
            urls,
            [
                ("https://www.rust-lang.org/", Some(1), 0),
                ("https://example.com/a?x=1&y=2", Some(2), 0),
                ("https://lower.example/", Some(1), 1),
                ("https://other.example/", Some(3), 0),
                ("https://top.example/", None, 0),
            ]
        );

        let rust = &lib.bookmarks[0];
        assert_eq!(rust.title, "Rust & Cargo");
        assert_eq!(rust.created_at, Some(1_700_000_200_000));
        assert_eq!(
            rust.favicon.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );

        let a = &lib.bookmarks[1];
        assert_eq!(a.tags, ["docs", "Work"]);
        assert_eq!(a.notes, "First line of notes \u{2014} with an em dash");

        assert_eq!(lib.bookmarks[2].created_at, Some(1_700_000_300_000)); // already ms
        assert_eq!(lib.bookmarks[3].title, "Other");
    }

    #[test]
    fn round_trip() {
        let original = parse(CHROME);
        let again = parse(&write(&original));
        let shape = |lib: &Library| -> Vec<(String, Vec<String>, String, String)> {
            lib.bookmarks
                .iter()
                .map(|b| {
                    let folder = b
                        .folder_id
                        .and_then(|id| lib.folders.iter().find(|f| f.id == id))
                        .map(|f| f.name.clone())
                        .unwrap_or_default();
                    (b.url.clone(), b.tags.clone(), b.notes.clone(), folder)
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        assert_eq!(shape(&again), shape(&original));
        assert_eq!(again.folders.len(), original.folders.len());
    }

    #[test]
    fn entities() {
        assert_eq!(
            decode("a &amp; b &lt;c&gt; &quot;q&quot; &#39;s&#x27; &#x1F600;"),
            "a & b <c> \"q\" 's' \u{1F600}"
        );
        // Unknown or unterminated entities stay as written.
        assert_eq!(
            decode("AT&T &nbsp; & &zz; &#xZZ;"),
            "AT&T &nbsp; & &zz; &#xZZ;"
        );
        assert_eq!(decode("é & ü"), "é & ü");
        assert_eq!(
            escape(r#"<a href="x">&</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;"
        );
    }

    #[test]
    fn timestamps() {
        assert_eq!(timestamp_ms("1700000000"), Some(1_700_000_000_000));
        assert_eq!(timestamp_ms("1700000000123"), Some(1_700_000_000_123));
        assert_eq!(timestamp_ms("1700000000123456"), Some(1_700_000_000_123));
        assert_eq!(timestamp_ms("0"), None);
        assert_eq!(timestamp_ms("soon"), None);
    }

    #[test]
    fn junk_input_does_not_panic() {
        for input in [
            "",
            "<",
            "<DL",
            "<DT><A HREF=",
            "</DL></DL></DL>",
            "<H3>no close",
            "<A HREF=\"x\">no close",
            "<DD>orphan",
        ] {
            let _ = parse(input);
        }
    }
}
