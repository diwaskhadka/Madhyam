//! A small, complete JSON implementation (RFC 8259).
//!
//! We need JSON twice: Xtream panels answer in JSON, and the FFI layer speaks
//! JSON to every host language. A real project would use `serde_json`; writing
//! one is a classic Rust exercise in enums, recursion and byte-level parsing.
//!
//! Highlights worth studying:
//! * [`Value`] is a recursive enum; `Box` is not needed because `Vec`/`BTreeMap`
//!   already put children on the heap.
//! * The parser works on `&[u8]` with an index, which is both fast and simple.
//! * `\uXXXX` escapes, including UTF-16 surrogate pairs, are handled.

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    /// `BTreeMap` keeps keys sorted, so output is deterministic (nice for tests).
    Object(BTreeMap<String, Value>),
}

// ---------------------------------------------------------------------------
// Lenient accessors.
//
// Xtream panels are notoriously inconsistent: the same field may arrive as
// `123`, `"123"`, `null`, `""` or be missing entirely, depending on the panel
// vendor. These helpers absorb that mess in one place.
// ---------------------------------------------------------------------------
impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(m) => m.get(key),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Object(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// Any scalar rendered as a string; `None` for null/empty/containers.
    pub fn to_text(&self) -> Option<String> {
        match self {
            Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
            Value::Number(n) => Some(fmt_number(*n)),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }

    /// Number or numeric string as `i64`.
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) if n.is_finite() => Some(*n as i64),
            Value::String(s) => {
                let s = s.trim();
                s.parse::<i64>()
                    .ok()
                    .or_else(|| s.parse::<f64>().ok().map(|f| f as i64))
            }
            Value::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }

    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }

    /// Truthiness the way PHP panels mean it: 1, "1", true, "true".
    pub fn to_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => matches!(s.trim(), "1" | "true" | "yes"),
            _ => false,
        }
    }

    /// Convenience: `obj.text("name")`.
    pub fn text(&self, key: &str) -> Option<String> {
        self.get(key).and_then(Value::to_text)
    }
    pub fn int(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Value::to_i64)
    }
    pub fn num(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Value::to_f64)
    }
    pub fn flag(&self, key: &str) -> bool {
        self.get(key).is_some_and(Value::to_bool)
    }
}

// Ergonomic constructors used throughout the FFI layer.
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n as f64)
    }
}
impl From<u32> for Value {
    fn from(n: u32) -> Self {
        Value::Number(n as f64)
    }
}
impl From<usize> for Value {
    fn from(n: usize) -> Self {
        Value::Number(n as f64)
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(o: Option<T>) -> Self {
        o.map_or(Value::Null, Into::into)
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::Array(v.into_iter().map(Into::into).collect())
    }
}

/// Build a JSON object from `(key, value)` pairs: `obj([("a", 1i64.into())])`.
pub fn obj<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// Types that can describe themselves as JSON (our tiny version of `Serialize`).
pub trait ToJson {
    fn to_json(&self) -> Value;
}

impl<T: ToJson> ToJson for [T] {
    fn to_json(&self) -> Value {
        Value::Array(self.iter().map(ToJson::to_json).collect())
    }
}
impl<T: ToJson> ToJson for Vec<T> {
    fn to_json(&self) -> Value {
        self.as_slice().to_json()
    }
}
impl<T: ToJson> ToJson for Option<T> {
    fn to_json(&self) -> Value {
        self.as_ref().map_or(Value::Null, ToJson::to_json)
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// Integers print without a trailing `.0`; non-finite numbers become `null`
/// (JSON has no NaN/Infinity).
fn fmt_number(n: f64) -> String {
    if !n.is_finite() {
        "null".to_string()
    } else if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn write_escaped(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

impl Value {
    pub fn write_to(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&fmt_number(*n)),
            Value::String(s) => write_escaped(out, s),
            Value::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write_to(out);
                }
                out.push(']');
            }
            Value::Object(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_escaped(out, k);
                    out.push(':');
                    v.write_to(out);
                }
                out.push('}');
            }
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();
        self.write_to(&mut s);
        f.write_str(&s)
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Nesting limit so a malicious `[[[[[[...` cannot overflow the stack.
const MAX_DEPTH: usize = 256;

pub fn parse(input: &str) -> Result<Value> {
    parse_bytes(input.as_bytes())
}

pub fn parse_bytes(input: &[u8]) -> Result<Value> {
    // Some PHP panels prepend a UTF-8 BOM; tolerate it.
    let input = input.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(input);
    let mut p = Parser { b: input, i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.err("trailing characters after JSON value"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, msg: &str) -> Error {
        Error::parse("JSON", 0, format!("{msg} at byte {}", self.i))
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\n' | b'\r' | b'\t') = self.peek() {
            self.i += 1;
        }
    }

    fn expect_lit(&mut self, lit: &[u8], v: Value) -> Result<Value> {
        if self.b[self.i..].starts_with(lit) {
            self.i += lit.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') => self.expect_lit(b"true", Value::Bool(true)),
            Some(b'f') => self.expect_lit(b"false", Value::Bool(false)),
            Some(b'n') => self.expect_lit(b"null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
            None => Err(self.err("unexpected end of input")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value> {
        self.i += 1; // '{'
        let mut map = BTreeMap::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected object key"));
            }
            let k = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.i += 1;
            self.ws();
            let v = self.value(depth + 1)?;
            map.insert(k, v);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Object(map));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value> {
        self.i += 1; // '['
        let mut v = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Array(v));
        }
        loop {
            self.ws();
            v.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Array(v));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn number(&mut self) -> Result<Value> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while let Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') = self.peek() {
            self.i += 1;
        }
        // The slice is ASCII by construction, so from_utf8 cannot fail.
        let s = std::str::from_utf8(&self.b[start..self.i]).unwrap_or_default();
        s.parse::<f64>()
            .map(Value::Number)
            .map_err(|_| self.err("invalid number"))
    }

    fn hex4(&mut self) -> Result<u32> {
        let h = self
            .b
            .get(self.i..self.i + 4)
            .ok_or_else(|| self.err("short \\u escape"))?;
        let s = std::str::from_utf8(h).map_err(|_| self.err("bad \\u escape"))?;
        let n = u32::from_str_radix(s, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String> {
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            // Fast path: copy a run of plain bytes at once.
            let run_start = self.i;
            while let Some(c) = self.peek() {
                if c == b'"' || c == b'\\' || c < 0x20 {
                    break;
                }
                self.i += 1;
            }
            if self.i > run_start {
                let chunk = std::str::from_utf8(&self.b[run_start..self.i])
                    .map_err(|_| self.err("invalid UTF-8 in string"))?;
                out.push_str(chunk);
            }
            match self.peek() {
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.i += 1;
                    let e = self.peek().ok_or_else(|| self.err("unterminated escape"))?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) {
                                // High surrogate: must be followed by \uDC00..DFFF.
                                if self.b[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if (0xDC00..0xE000).contains(&lo) {
                                        0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                    } else {
                                        0xFFFD
                                    }
                                } else {
                                    0xFFFD
                                }
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(self.err("invalid escape")),
                    }
                }
                Some(_) => return Err(self.err("control character in string")),
                None => return Err(self.err("unterminated string")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let src = r#"{"a":[1,2.5,-3e2,true,false,null],"b":"x\"y\\z\né😀","c":{}}"#;
        let v = parse(src).unwrap();
        assert_eq!(v.get("b").unwrap().as_str().unwrap(), "x\"y\\z\né😀");
        let again = parse(&v.to_string()).unwrap();
        assert_eq!(v, again);
        assert_eq!(
            v.get("a").unwrap().as_array().unwrap()[2],
            Value::Number(-300.0)
        );
    }

    #[test]
    fn lenient_accessors() {
        let v = parse(r#"{"id":"42","n":7,"s":"  ","f":"1","r":"4.5"}"#).unwrap();
        assert_eq!(v.int("id"), Some(42));
        assert_eq!(v.int("n"), Some(7));
        assert_eq!(v.text("s"), None);
        assert!(v.flag("f"));
        assert_eq!(v.num("r"), Some(4.5));
        assert_eq!(v.text("missing"), None);
    }

    #[test]
    fn rejects_garbage() {
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "tru", "\"abc", "[1] 2", "01x"] {
            assert!(parse(bad).is_err(), "should reject {bad:?}");
        }
        let deep = "[".repeat(1000);
        assert!(parse(&deep).is_err());
    }

    #[test]
    fn integer_formatting() {
        assert_eq!(Value::from(3i64).to_string(), "3");
        assert_eq!(Value::from(0.5).to_string(), "0.5");
        assert_eq!(Value::from(f64::NAN).to_string(), "null");
    }
}
