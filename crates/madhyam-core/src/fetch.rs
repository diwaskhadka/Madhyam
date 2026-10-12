//! The one seam between the core and the network.
//!
//! The core never does HTTP itself. A host implements [`Fetcher`] with its
//! native stack and passes it in. Tests implement it with a `HashMap`.

use crate::error::Result;

pub trait Fetcher {
    /// GET `url` and return the response body. Non-2xx statuses must be
    /// returned as `Err(Error::Api(...))`.
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>>;
}

/// Closures work as fetchers too: `|url, _| my_http_get(url)`.
impl<F> Fetcher for F
where
    F: Fn(&str, &[(&str, &str)]) -> Result<Vec<u8>>,
{
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
        self(url, headers)
    }
}

/// Percent-encode a query component (RFC 3986 unreserved set kept as-is).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes() {
        assert_eq!(super::url_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }
}
