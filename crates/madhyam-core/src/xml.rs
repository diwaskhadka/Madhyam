//! A minimal streaming ("pull") XML tokenizer.
//!
//! Enough XML for XMLTV and similar feeds: elements, attributes, text, CDATA,
//! comments, processing instructions and DOCTYPE (the last three are skipped).
//! It does not validate, resolve namespaces, or expand external entities —
//! which also makes it immune to XXE / "billion laughs" attacks by design.
//!
//! Memory use is bounded by the largest single tag or text node, not the file,
//! so a 300 MB guide streams in a few kilobytes of buffer.

use crate::error::{Error, Result};
use std::io::BufRead;

#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub name: String,
    pub attrs: Vec<(String, String)>,
}

impl Tag {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `<name a="1">`. A self-closing `<name/>` produces `Start` then `End`,
    /// so consumers only handle one shape.
    Start(Tag),
    End(String),
    /// Text content with entities decoded. Whitespace-only text is skipped.
    Text(String),
    Eof,
}

/// Hard cap for one tag or text node; protects against unterminated input.
const MAX_TOKEN: usize = 16 * 1024 * 1024;

pub struct XmlReader<R> {
    r: R,
    buf: Vec<u8>,
    /// We just consumed a `<` while reading text; the next token is a tag.
    in_tag: bool,
    /// Pending `End` for a self-closing tag.
    pending_end: Option<String>,
    /// Approximate line number for error messages.
    line: usize,
}

impl<R: BufRead> XmlReader<R> {
    pub fn new(r: R) -> Self {
        XmlReader {
            r,
            buf: Vec::with_capacity(4096),
            in_tag: false,
            pending_end: None,
            line: 1,
        }
    }

    fn err(&self, msg: &str) -> Error {
        Error::parse("XML", self.line, msg)
    }

    fn read_until(&mut self, delim: u8) -> Result<usize> {
        let n = self.r.read_until(delim, &mut self.buf)?;
        if self.buf.len() > MAX_TOKEN {
            return Err(self.err("token too large (unterminated tag or text?)"));
        }
        Ok(n)
    }

    pub fn next_event(&mut self) -> Result<Event> {
        if let Some(name) = self.pending_end.take() {
            return Ok(Event::End(name));
        }
        loop {
            if !self.in_tag {
                // ---- text up to the next '<' ----
                self.buf.clear();
                if self.read_until(b'<')? == 0 {
                    return Ok(Event::Eof);
                }
                let found_lt = self.buf.last() == Some(&b'<');
                if found_lt {
                    self.buf.pop();
                    self.in_tag = true;
                }
                self.line += self.buf.iter().filter(|&&b| b == b'\n').count();
                if self.buf.iter().any(|b| !b.is_ascii_whitespace()) {
                    let text = String::from_utf8_lossy(&self.buf);
                    return Ok(Event::Text(decode_entities(&text)));
                }
                if !found_lt {
                    return Ok(Event::Eof);
                }
                continue;
            }

            // ---- a tag: we are just after '<' ----
            self.in_tag = false;
            self.buf.clear();
            if self.read_until(b'>')? == 0 {
                return Err(self.err("unexpected end of file inside a tag"));
            }
            // Keep reading while the '>' we hit is not the real end.
            loop {
                let done = self.buf.last() == Some(&b'>') && {
                    let b = &self.buf;
                    if b.starts_with(b"!--") {
                        b.ends_with(b"-->")
                    } else if b.starts_with(b"![CDATA[") {
                        b.ends_with(b"]]>")
                    } else if b.starts_with(b"!") {
                        // DOCTYPE with an internal subset: balance [ ].
                        let open = b.iter().filter(|&&c| c == b'[').count();
                        let close = b.iter().filter(|&&c| c == b']').count();
                        open <= close
                    } else {
                        quotes_balanced(b)
                    }
                };
                if done {
                    break;
                }
                if self.read_until(b'>')? == 0 {
                    return Err(self.err("unexpected end of file inside a tag"));
                }
            }
            self.line += self.buf.iter().filter(|&&b| b == b'\n').count();
            self.buf.pop(); // the '>'

            let raw = &self.buf[..];
            if raw.starts_with(b"![CDATA[") {
                let inner = &raw[8..raw.len().saturating_sub(2)];
                return Ok(Event::Text(String::from_utf8_lossy(inner).into_owned()));
            }
            if raw.starts_with(b"!") || raw.starts_with(b"?") {
                continue; // comment / DOCTYPE / <?xml ... ?>
            }
            let s = String::from_utf8_lossy(raw);
            if let Some(end) = s.strip_prefix('/') {
                return Ok(Event::End(end.trim().to_string()));
            }
            let (body, empty) = match s.trim_end().strip_suffix('/') {
                Some(b) => (b, true),
                None => (s.as_ref(), false),
            };
            let tag = parse_tag(body).ok_or_else(|| self.err("malformed tag"))?;
            if empty {
                self.pending_end = Some(tag.name.clone());
            }
            return Ok(Event::Start(tag));
        }
    }

    /// Collect all text until the matching end tag of an element whose
    /// `Start` was just returned. Nested markup (e.g. `<b>`) is flattened.
    pub fn read_text(&mut self, name: &str) -> Result<String> {
        let mut out = String::new();
        let mut depth = 0usize;
        loop {
            match self.next_event()? {
                Event::Text(t) => out.push_str(&t),
                Event::Start(_) => depth += 1,
                Event::End(n) if depth == 0 && n == name => return Ok(out.trim().to_string()),
                Event::End(_) => depth = depth.saturating_sub(1),
                Event::Eof => return Err(self.err("unexpected end of file in element text")),
            }
        }
    }

    /// Skip an element whose `Start` was just returned, including children.
    pub fn skip(&mut self, name: &str) -> Result<()> {
        self.read_text(name).map(|_| ())
    }
}

fn quotes_balanced(b: &[u8]) -> bool {
    let mut q: Option<u8> = None;
    for &c in b {
        match q {
            None if c == b'"' || c == b'\'' => q = Some(c),
            Some(open) if c == open => q = None,
            _ => {}
        }
    }
    q.is_none()
}

fn parse_tag(s: &str) -> Option<Tag> {
    let s = s.trim();
    let name_end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    let name = &s[..name_end];
    if name.is_empty() {
        return None;
    }
    let b = s.as_bytes();
    let mut i = name_end;
    let mut attrs = Vec::new();
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let ks = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let key = &s[ks..i];
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            if !key.is_empty() {
                attrs.push((key.to_string(), String::new()));
            }
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
            let q = b[i];
            i += 1;
            let vs = i;
            while i < b.len() && b[i] != q {
                i += 1;
            }
            let v = &s[vs..i];
            i = (i + 1).min(b.len());
            v
        } else {
            let vs = i;
            while i < b.len() && !b[i].is_ascii_whitespace() {
                i += 1;
            }
            &s[vs..i]
        };
        if !key.is_empty() {
            attrs.push((key.to_string(), decode_entities(value)));
        }
    }
    Some(Tag {
        name: name.to_string(),
        attrs,
    })
}

/// Decode the five XML entities, numeric references, and `&nbsp;`
/// (common in guides generated from HTML). Unknown entities stay verbatim.
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        // Search bytes, not a str slice: slicing a str at byte 12 could split
        // a multi-byte character and panic. ';' is ASCII, so the index is safe.
        let Some(semi) = rest.as_bytes()[..rest.len().min(12)]
            .iter()
            .position(|&b| b == b';')
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let decoded = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
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

#[cfg(test)]
mod tests {
    use super::*;

    fn events(xml: &str) -> Vec<Event> {
        let mut r = XmlReader::new(xml.as_bytes());
        let mut v = Vec::new();
        loop {
            let e = r.next_event().unwrap();
            if e == Event::Eof {
                return v;
            }
            v.push(e);
        }
    }

    #[test]
    fn tokenizes() {
        let ev = events(
            "<?xml version=\"1.0\"?>\n<!DOCTYPE tv SYSTEM \"xmltv.dtd\">\n<!-- a > b -->\
             <tv gen='x &amp; y'><a href=\"1>2\"/>T&lt;3 &#233;&#x41;&bogus;<![CDATA[<raw>]]></tv>",
        );
        assert_eq!(
            ev,
            vec![
                Event::Start(Tag {
                    name: "tv".into(),
                    attrs: vec![("gen".into(), "x & y".into())]
                }),
                Event::Start(Tag {
                    name: "a".into(),
                    attrs: vec![("href".into(), "1>2".into())]
                }),
                Event::End("a".into()),
                Event::Text("T<3 éA&bogus;".into()),
                Event::Text("<raw>".into()),
                Event::End("tv".into()),
            ]
        );
    }

    #[test]
    fn read_text_flattens() {
        let mut r = XmlReader::new("<desc>Hello <b>bold</b> world</desc><x/>".as_bytes());
        assert!(matches!(r.next_event().unwrap(), Event::Start(_)));
        assert_eq!(r.read_text("desc").unwrap(), "Hello bold world");
        assert!(matches!(r.next_event().unwrap(), Event::Start(t) if t.name == "x"));
    }

    #[test]
    fn truncated_input_errors() {
        let mut r = XmlReader::new("<tv><programme start=\"1".as_bytes());
        assert!(r.next_event().is_ok());
        assert!(r.next_event().is_err());
    }
}
