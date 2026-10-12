//! Streaming XMLTV guide parser built on [`crate::xml`].
//!
//! ```no_run
//! # use madhyam_core::xmltv::{XmltvReader, XmltvItem};
//! let file = std::io::BufReader::new(std::fs::File::open("guide.xml")?);
//! for item in XmltvReader::new(file) {
//!     match item? {
//!         XmltvItem::Channel(c) => println!("channel {}", c.id),
//!         XmltvItem::Programme(p) => println!("{} @ {}", p.title, p.start),
//!     }
//! }
//! # Ok::<(), madhyam_core::Error>(())
//! ```
//!
//! Gzipped guides (`.xml.gz`) should be decompressed by the host before being
//! handed over (Android: `GZIPInputStream`, .NET: `GZipStream`).

use crate::error::Result;
use crate::model::{EpgChannel, Programme};
use crate::time::parse_xmltv_time;
use crate::xml::{Event, Tag, XmlReader};
use std::io::BufRead;

#[derive(Debug, Clone, PartialEq)]
pub enum XmltvItem {
    Channel(EpgChannel),
    Programme(Programme),
}

pub struct XmltvReader<R> {
    xml: XmlReader<R>,
    done: bool,
    /// Programmes skipped because their times could not be parsed.
    pub skipped: usize,
}

impl<R: BufRead> XmltvReader<R> {
    pub fn new(r: R) -> Self {
        XmltvReader {
            xml: XmlReader::new(r),
            done: false,
            skipped: 0,
        }
    }

    fn read_channel(&mut self, tag: &Tag) -> Result<EpgChannel> {
        let mut ch = EpgChannel {
            id: tag.attr("id").unwrap_or_default().trim().to_string(),
            ..Default::default()
        };
        loop {
            match self.xml.next_event()? {
                Event::Start(t) => match t.name.as_str() {
                    "display-name" => {
                        let n = self.xml.read_text("display-name")?;
                        if !n.is_empty() {
                            ch.names.push(n);
                        }
                    }
                    "icon" => {
                        if ch.icon.is_empty() {
                            ch.icon = t.attr("src").unwrap_or_default().to_string();
                        }
                        self.xml.skip("icon")?;
                    }
                    other => self.xml.skip(other)?,
                },
                Event::End(n) if n == "channel" => return Ok(ch),
                Event::Eof => return Ok(ch),
                _ => {}
            }
        }
    }

    fn read_programme(&mut self, tag: &Tag) -> Result<Option<Programme>> {
        let start = tag.attr("start").and_then(parse_xmltv_time);
        // A missing stop is legal XMLTV; the catalog fills it from the next show.
        let stop = tag.attr("stop").and_then(parse_xmltv_time).unwrap_or(0);
        let mut p = Programme {
            channel: tag.attr("channel").unwrap_or_default().trim().to_string(),
            start: start.unwrap_or(0),
            stop,
            ..Default::default()
        };
        let mut onscreen = String::new();
        let mut xmltv_ns = String::new();
        loop {
            match self.xml.next_event()? {
                Event::Start(t) => {
                    let name = t.name.clone();
                    match name.as_str() {
                        // Keep the first title/desc (usually the primary language).
                        "title" if p.title.is_empty() => p.title = self.xml.read_text("title")?,
                        "sub-title" if p.sub_title.is_empty() => {
                            p.sub_title = self.xml.read_text("sub-title")?
                        }
                        "desc" if p.desc.is_empty() => p.desc = self.xml.read_text("desc")?,
                        "category" => {
                            let c = self.xml.read_text("category")?;
                            if !c.is_empty() && !p.categories.contains(&c) {
                                p.categories.push(c);
                            }
                        }
                        "icon" => {
                            if p.icon.is_empty() {
                                p.icon = t.attr("src").unwrap_or_default().to_string();
                            }
                            self.xml.skip("icon")?;
                        }
                        "episode-num" => {
                            let v = self.xml.read_text("episode-num")?;
                            match t.attr("system") {
                                Some("onscreen") => onscreen = v,
                                Some("xmltv_ns") => xmltv_ns = v,
                                _ => {}
                            }
                        }
                        "rating" => {
                            // <rating><value>PG-13</value></rating>
                            let v = self.xml.read_text("rating")?;
                            if p.rating.is_empty() {
                                p.rating = v;
                            }
                        }
                        _ => self.xml.skip(&name)?,
                    }
                }
                Event::End(n) if n == "programme" => break,
                Event::Eof => break,
                _ => {}
            }
        }
        p.episode = if !onscreen.is_empty() {
            onscreen
        } else {
            xmltv_ns_to_onscreen(&xmltv_ns)
        };
        if start.is_none() || p.channel.is_empty() || (p.stop != 0 && p.stop <= p.start) {
            self.skipped += 1;
            return Ok(None);
        }
        Ok(Some(p))
    }
}

impl<R: BufRead> Iterator for XmltvReader<R> {
    type Item = Result<XmltvItem>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            let ev = match self.xml.next_event() {
                Ok(e) => e,
                Err(e) => {
                    self.done = true;
                    return Some(Err(e));
                }
            };
            match ev {
                Event::Start(t) if t.name == "channel" => {
                    return Some(self.read_channel(&t).map(XmltvItem::Channel));
                }
                Event::Start(t) if t.name == "programme" => match self.read_programme(&t) {
                    Ok(Some(p)) => return Some(Ok(XmltvItem::Programme(p))),
                    Ok(None) => continue,
                    Err(e) => {
                        self.done = true;
                        return Some(Err(e));
                    }
                },
                Event::Eof => {
                    self.done = true;
                    return None;
                }
                _ => {} // <tv>, whitespace, unknown top-level elements
            }
        }
    }
}

/// `xmltv_ns` is zero-based `season.episode.part` with optional `/total`:
/// `"0.4/12.0"` → `"S1E5"`.
pub fn xmltv_ns_to_onscreen(ns: &str) -> String {
    let mut parts = ns.split('.');
    let num = |s: Option<&str>| -> Option<u32> {
        s.map(|x| x.split('/').next().unwrap_or("").trim())
            .filter(|x| !x.is_empty())
            .and_then(|x| x.parse().ok())
    };
    let season = num(parts.next());
    let episode = num(parts.next());
    match (season, episode) {
        (Some(s), Some(e)) => format!("S{}E{}", s + 1, e + 1),
        (None, Some(e)) => format!("E{}", e + 1),
        (Some(s), None) => format!("S{}", s + 1),
        (None, None) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUIDE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE tv SYSTEM "xmltv.dtd">
<tv generator-info-name="test">
  <channel id="bbc1.uk">
    <display-name lang="en">BBC One</display-name>
    <display-name>BBC1</display-name>
    <icon src="http://l/bbc.png" />
    <url>http://bbc.co.uk</url>
  </channel>
  <programme start="20261012180000 +0100" stop="20261012190000 +0100" channel="bbc1.uk">
    <title lang="en">News &amp; Weather</title>
    <title lang="cy">Newyddion</title>
    <desc>Headlines.</desc>
    <category>News</category><category>News</category>
    <episode-num system="xmltv_ns">2.9/20.</episode-num>
    <rating system="BBFC"><value>U</value></rating>
    <credits><director>X</director></credits>
  </programme>
  <programme start="garbage" channel="bbc1.uk"><title>Bad</title></programme>
  <programme start="20261012190000 +0100" channel="bbc1.uk"><title>No stop</title></programme>
</tv>"#;

    #[test]
    fn parses_guide() {
        let mut r = XmltvReader::new(GUIDE.as_bytes());
        let items: Vec<_> = r.by_ref().map(|i| i.unwrap()).collect();
        assert_eq!(items.len(), 3);
        assert_eq!(r.skipped, 1);
        let XmltvItem::Channel(c) = &items[0] else {
            panic!()
        };
        assert_eq!(c.names, ["BBC One", "BBC1"]);
        assert_eq!(c.icon, "http://l/bbc.png");
        let XmltvItem::Programme(p) = &items[1] else {
            panic!()
        };
        assert_eq!(p.title, "News & Weather");
        assert_eq!(p.stop - p.start, 3600);
        assert_eq!(p.categories, ["News"]);
        assert_eq!(p.episode, "S3E10");
        assert_eq!(p.rating, "U");
        let XmltvItem::Programme(p2) = &items[2] else {
            panic!()
        };
        assert_eq!(p2.stop, 0);
    }
}
