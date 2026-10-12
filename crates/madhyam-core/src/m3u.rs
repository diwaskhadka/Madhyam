//! Streaming M3U / M3U8 (extended) playlist parser.
//!
//! IPTV playlists can be hundreds of megabytes, so the parser is an
//! [`Iterator`] over any [`BufRead`]: it holds one entry in memory at a time
//! and never needs the whole file.
//!
//! Supported syntax (what real-world providers actually emit):
//!
//! ```text
//! #EXTM3U url-tvg="http://epg.xml" catchup="shift" catchup-days="3"
//! #KODIPROP:inputstream.adaptive.license_type=clearkey
//! #EXTINF:-1 tvg-id="bbc1.uk" tvg-logo="http://l.png" group-title="UK, News",BBC One
//! #EXTGRP:Fallback group
//! #EXTVLCOPT:http-user-agent=Mozilla/5.0
//! #EXTVLCOPT:http-referrer=http://site/
//! http://host/live/1.ts|User-Agent=Other&Referer=http://x/
//! ```
//!
//! Garbage tolerance is deliberate: a broken entry is skipped (or yielded as
//! an error you may ignore); it never aborts the whole playlist.

use crate::error::{Error, Result};
use crate::model::{Catchup, M3uEntry};
use std::io::BufRead;

/// Attributes on the `#EXTM3U` header line.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct M3uHeader {
    pub attrs: Vec<(String, String)>,
}

impl M3uHeader {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// EPG URLs announced by the playlist (`url-tvg`, `x-tvg-url`), comma separated.
    pub fn epg_urls(&self) -> Vec<String> {
        ["url-tvg", "x-tvg-url", "tvg-url"]
            .iter()
            .filter_map(|k| self.attr(k))
            .flat_map(|v| v.split(','))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// Parse the part after `#EXTINF:` into `(duration, attributes, title)`.
///
/// The title is everything after the first comma that is *outside* quotes, so
/// `group-title="News, Sport",CNN` yields the group `News, Sport` and title `CNN`.
/// Exposed publicly because it is a nice unit to test and reuse.
pub fn parse_extinf(rest: &str) -> (f64, Vec<(String, String)>, String) {
    let b = rest.as_bytes();
    let mut i = 0;
    let n = b.len();

    // 1. Duration: up to whitespace or comma.
    while i < n && !b[i].is_ascii_whitespace() && b[i] != b',' {
        i += 1;
    }
    let duration = rest[..i].trim().parse::<f64>().unwrap_or(-1.0);

    // 2. key=value pairs.
    let mut attrs = Vec::new();
    loop {
        while i < n && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= n {
            return (duration, attrs, String::new());
        }
        if b[i] == b',' {
            let title = rest[i + 1..].trim().to_string();
            return (duration, attrs, title);
        }
        // Key: up to '=', whitespace or ','.
        let ks = i;
        while i < n && b[i] != b'=' && b[i] != b',' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = rest[ks..i].trim().to_ascii_lowercase();
        if i < n && b[i] == b'=' {
            i += 1;
            let value = if i < n && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                let vs = i;
                while i < n && b[i] != q {
                    i += 1;
                }
                let v = &rest[vs..i];
                if i < n {
                    i += 1; // closing quote
                }
                v
            } else {
                let vs = i;
                while i < n && !b[i].is_ascii_whitespace() && b[i] != b',' {
                    i += 1;
                }
                &rest[vs..i]
            };
            if !key.is_empty() {
                attrs.push((key, value.trim().to_string()));
            }
        } else if !key.is_empty() {
            // Bare flag attribute, e.g. `radio`.
            attrs.push((key, String::new()));
        }
    }
}

/// Split Kodi-style `url|Header=Value&Header2=Value2` options off a URL.
fn split_pipe_headers(url: &str) -> (&str, Vec<(String, String)>) {
    match url.split_once('|') {
        Some((u, opts)) => {
            let hs = opts
                .split('&')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
                .collect();
            (u.trim(), hs)
        }
        None => (url, Vec::new()),
    }
}

/// Iterator over the entries of a playlist.
pub struct M3uReader<R> {
    reader: R,
    buf: Vec<u8>,
    line_no: usize,
    header: M3uHeader,
    saw_first_line: bool,
    /// Entry being assembled between `#EXTINF` and its URL.
    pending: Option<M3uEntry>,
    /// `#KODIPROP`/`#EXTVLCOPT` lines may come *before* `#EXTINF`; stash them.
    pre_props: Vec<(String, String)>,
    pre_vlc: Vec<(String, String)>,
    pending_grp: String,
}

impl<R: BufRead> M3uReader<R> {
    pub fn new(reader: R) -> Self {
        M3uReader {
            reader,
            buf: Vec::with_capacity(512),
            line_no: 0,
            header: M3uHeader::default(),
            saw_first_line: false,
            pending: None,
            pre_props: Vec::new(),
            pre_vlc: Vec::new(),
            pending_grp: String::new(),
        }
    }

    /// Header attributes. Complete once the first entry has been read.
    pub fn header(&self) -> &M3uHeader {
        &self.header
    }

    /// Read one line (lossy UTF-8, CR/LF and BOM stripped). `None` at EOF.
    fn next_line(&mut self) -> Result<Option<String>> {
        self.buf.clear();
        let n = self.reader.read_until(b'\n', &mut self.buf)?;
        if n == 0 {
            return Ok(None);
        }
        self.line_no += 1;
        let mut s = String::from_utf8_lossy(&self.buf).into_owned();
        if !self.saw_first_line {
            self.saw_first_line = true;
            if let Some(stripped) = s.strip_prefix('\u{feff}') {
                s = stripped.to_string();
            }
        }
        // trim_end handles "\r\n" too.
        let t = s.trim();
        Ok(Some(if t.len() == s.len() { s } else { t.to_string() }))
    }

    fn apply_vlc_opt(e: &mut M3uEntry, key: &str, val: &str) {
        match key {
            "http-user-agent" => e.user_agent = val.to_string(),
            "http-referrer" | "http-referer" => e.referrer = val.to_string(),
            _ => {}
        }
    }

    fn start_entry(&mut self, rest: &str) -> M3uEntry {
        let (duration, attrs, title) = parse_extinf(rest);
        let mut e = M3uEntry {
            duration,
            line: self.line_no,
            ..Default::default()
        };
        for (k, v) in &attrs {
            match k.as_str() {
                "tvg-id" => e.tvg_id = v.clone(),
                "tvg-name" => e.tvg_name = v.clone(),
                "tvg-logo" | "logo" => e.logo = v.clone(),
                "group-title" => e.group = v.clone(),
                "radio" => e.radio = v.is_empty() || v == "true" || v == "1",
                "user-agent" | "http-user-agent" => e.user_agent = v.clone(),
                "referrer" | "http-referrer" => e.referrer = v.clone(),
                _ => {}
            }
        }
        e.name = if !title.is_empty() {
            title
        } else {
            e.tvg_name.clone()
        };
        e.catchup = self.catchup_for(&attrs);
        e.attrs = attrs;
        e.kodi_props = std::mem::take(&mut self.pre_props);
        for (k, v) in std::mem::take(&mut self.pre_vlc) {
            Self::apply_vlc_opt(&mut e, &k, &v);
        }
        e
    }

    /// Catch-up from entry attributes, falling back to header defaults.
    fn catchup_for(&self, attrs: &[(String, String)]) -> Option<Catchup> {
        let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
        let kind = get("catchup")
            .or(get("catchup-type"))
            .or(self.header.attr("catchup"));
        let source = get("catchup-source").or(self.header.attr("catchup-source"));
        let days = get("catchup-days")
            .or(get("timeshift"))
            .or(get("tvg-rec"))
            .or(self.header.attr("catchup-days"))
            .and_then(|d| d.trim().parse::<u32>().ok())
            .unwrap_or(0);
        if kind.is_none() && days == 0 {
            return None;
        }
        Some(Catchup {
            kind: kind.unwrap_or("default").to_string(),
            source: source.unwrap_or("").to_string(),
            days,
        })
    }

    fn finish_entry(&mut self, mut e: M3uEntry, url_line: &str) -> M3uEntry {
        let (url, headers) = split_pipe_headers(url_line);
        e.url = url.to_string();
        for (k, v) in headers {
            match k.as_str() {
                "user-agent" => e.user_agent = v,
                "referer" | "referrer" => e.referrer = v,
                _ => e.kodi_props.push((format!("header.{k}"), v)),
            }
        }
        if e.group.is_empty() {
            e.group = std::mem::take(&mut self.pending_grp);
        } else {
            self.pending_grp.clear();
        }
        if e.name.is_empty() {
            e.name = e.url.rsplit('/').next().unwrap_or(&e.url).to_string();
        }
        e
    }
}

impl<R: BufRead> Iterator for M3uReader<R> {
    type Item = Result<M3uEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = match self.next_line() {
                Ok(Some(l)) => l,
                Ok(None) => {
                    // EOF: an #EXTINF without URL is reported, not silently lost.
                    return self.pending.take().map(|e| {
                        Err(Error::parse(
                            "M3U",
                            e.line,
                            format!("entry '{}' has no stream URL", e.name),
                        ))
                    });
                }
                Err(e) => return Some(Err(e)),
            };
            if line.is_empty() {
                continue;
            }
            if let Some(tag) = line.strip_prefix('#') {
                // Tag name ends at ':' (`#EXTINF:...`) or whitespace (`#EXTM3U url-tvg=...`).
                let end = tag
                    .find(|c: char| c == ':' || c.is_whitespace())
                    .unwrap_or(tag.len());
                let (name, rest) = (&tag[..end], tag.get(end + 1..).unwrap_or(""));
                match name.to_ascii_uppercase().as_str() {
                    "EXTM3U" => {
                        // Reuse the EXTINF attribute scanner with a dummy duration.
                        let (_, attrs, _) = parse_extinf(&format!("0 {rest}"));
                        self.header.attrs.extend(attrs);
                    }
                    "EXTINF" => {
                        let e = self.start_entry(rest);
                        if let Some(prev) = self.pending.replace(e) {
                            return Some(Err(Error::parse(
                                "M3U",
                                prev.line,
                                format!("entry '{}' has no stream URL", prev.name),
                            )));
                        }
                    }
                    "EXTGRP" => self.pending_grp = rest.trim().to_string(),
                    "EXTVLCOPT" => {
                        if let Some((k, v)) = rest.split_once('=') {
                            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_string());
                            match self.pending.as_mut() {
                                Some(e) => Self::apply_vlc_opt(e, &k, &v),
                                None => self.pre_vlc.push((k, v)),
                            }
                        }
                    }
                    "KODIPROP" => {
                        if let Some((k, v)) = rest.split_once('=') {
                            let kv = (k.trim().to_string(), v.trim().to_string());
                            match self.pending.as_mut() {
                                Some(e) => e.kodi_props.push(kv),
                                None => self.pre_props.push(kv),
                            }
                        }
                    }
                    _ => {} // comments, #EXT-X-*, #PLAYLIST... ignored
                }
                continue;
            }
            // A non-comment line is a URL.
            let entry = match self.pending.take() {
                Some(e) => e,
                // Bare URL with no #EXTINF: still playable, so keep it.
                None => {
                    let mut e = M3uEntry {
                        duration: -1.0,
                        line: self.line_no,
                        ..Default::default()
                    };
                    e.kodi_props = std::mem::take(&mut self.pre_props);
                    for (k, v) in std::mem::take(&mut self.pre_vlc) {
                        Self::apply_vlc_opt(&mut e, &k, &v);
                    }
                    e
                }
            };
            return Some(Ok(self.finish_entry(entry, &line)));
        }
    }
}

/// Parse a whole playlist held in memory, skipping malformed entries.
pub fn parse_str(text: &str) -> (M3uHeader, Vec<M3uEntry>) {
    let mut r = M3uReader::new(text.as_bytes());
    let entries: Vec<_> = r.by_ref().filter_map(|e| e.ok()).collect();
    (r.header().clone(), entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ItemKind;

    const SAMPLE: &str = "\u{feff}#EXTM3U url-tvg=\"http://epg/a.xml,http://epg/b.xml\" catchup=\"shift\"\r\n\
#EXTINF:-1 tvg-id=\"bbc1.uk\" tvg-name=\"BBC One\" tvg-logo=\"http://l/bbc.png\" group-title=\"UK, News\" catchup-days=\"7\",BBC One HD\r\n\
#EXTVLCOPT:http-user-agent=Mozilla/5.0\r\n\
http://host/live/u/p/1.ts\r\n\
\r\n\
#KODIPROP:inputstream.adaptive.license_type=clearkey\n\
#KODIPROP:inputstream.adaptive.license_key=0123:abcd\n\
#EXTINF:-1 tvg-id=drm.ch group-title=DRM,Locked Channel\n\
https://cdn/x.mpd|User-Agent=Kodi&Referer=http://r/&X-Token=abc\n\
#EXTINF:5400 tvg-logo=\"m.jpg\",A Film (2024)\n\
#EXTGRP:Movies\n\
http://host/movie/u/p/77.mkv\n\
#EXTINF:-1,Orphan without url\n\
#EXTINF:-1 radio=\"true\",Radio 1\n\
http://radio/stream\n\
http://bare/url/channel.m3u8\n";

    #[test]
    fn parses_sample() {
        let mut r = M3uReader::new(SAMPLE.as_bytes());
        let items: Vec<_> = r.by_ref().collect();
        let ok: Vec<_> = items.iter().filter_map(|i| i.as_ref().ok()).collect();
        let errs = items.iter().filter(|i| i.is_err()).count();
        assert_eq!(ok.len(), 5);
        assert_eq!(errs, 1, "orphan #EXTINF reported once");

        assert_eq!(
            r.header().epg_urls(),
            ["http://epg/a.xml", "http://epg/b.xml"]
        );

        let bbc = ok[0];
        assert_eq!(bbc.name, "BBC One HD");
        assert_eq!(bbc.tvg_id, "bbc1.uk");
        assert_eq!(bbc.group, "UK, News");
        assert_eq!(bbc.user_agent, "Mozilla/5.0");
        assert_eq!(bbc.url, "http://host/live/u/p/1.ts");
        let cu = bbc.catchup.as_ref().unwrap();
        assert_eq!((cu.kind.as_str(), cu.days), ("shift", 7));
        assert_eq!(bbc.guess_kind(), ItemKind::Live);

        let drm = ok[1];
        assert_eq!(drm.tvg_id, "drm.ch");
        assert_eq!(drm.url, "https://cdn/x.mpd");
        assert_eq!(drm.user_agent, "Kodi");
        assert_eq!(drm.referrer, "http://r/");
        assert_eq!(drm.kodi_props.len(), 3);
        assert_eq!(drm.kodi_props[0].1, "clearkey");

        let film = ok[2];
        assert_eq!(film.group, "Movies");
        assert_eq!(film.duration, 5400.0);
        assert_eq!(film.guess_kind(), ItemKind::Movie);

        assert!(ok[3].radio);
        assert_eq!(ok[4].name, "channel.m3u8");
    }

    #[test]
    fn extinf_edge_cases() {
        let (d, a, t) = parse_extinf("-1 tvg-name='Single Q' empty=\"\" flag,Title, with comma");
        assert_eq!(d, -1.0);
        assert_eq!(t, "Title, with comma");
        assert_eq!(a[0], ("tvg-name".into(), "Single Q".into()));
        assert_eq!(a[1], ("empty".into(), "".into()));
        assert_eq!(a[2], ("flag".into(), "".into()));

        let (d, a, t) = parse_extinf("0,Just a name");
        assert_eq!((d, a.len(), t.as_str()), (0.0, 0, "Just a name"));

        // Unterminated quote must not panic.
        let (_, a, t) = parse_extinf("-1 tvg-name=\"oops,Name");
        assert_eq!(a[0].1, "oops,Name");
        assert_eq!(t, "");
    }

    #[test]
    fn invalid_utf8_is_lossy_not_fatal() {
        let data = b"#EXTM3U\n#EXTINF:-1,Caf\xe9\nhttp://x/1\n";
        let (_, e) = parse_str(&String::from_utf8_lossy(data));
        assert_eq!(e.len(), 1);
        let mut r = M3uReader::new(&data[..]);
        assert!(r.next().unwrap().unwrap().name.starts_with("Caf"));
    }
}
