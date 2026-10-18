//! Small text helpers shared by the parsers and the search index.

/// 64-bit FNV-1a hash.
///
/// `std::collections::hash_map::DefaultHasher` is *not* guaranteed to be stable
/// across Rust versions or platforms, so it must never be used for IDs that are
/// persisted (favorites, watch progress). FNV-1a is tiny and fully specified.
pub fn fnv1a64(parts: &[&[u8]]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            // Separator so ("ab","c") and ("a","bc") hash differently.
            h ^= 0x1f;
            h = h.wrapping_mul(PRIME);
        }
        for &b in *part {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
    }
    h
}

/// Fold a character for search: lowercase and strip common Latin diacritics,
/// so "Télé" matches "tele" and "MÜNCHEN" matches "munchen".
fn fold_char(c: char, out: &mut String) {
    let base = match c {
        'À'..='Å' | 'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
        'Ç' | 'ç' | 'ć' | 'č' => 'c',
        'È'..='Ë' | 'è'..='ë' | 'ē' | 'ę' | 'ě' => 'e',
        'Ì'..='Ï' | 'ì'..='ï' | 'ı' => 'i',
        'Ñ' | 'ñ' | 'ń' | 'ň' => 'n',
        'Ò'..='Ö' | 'Ø' | 'ò'..='ö' | 'ø' | 'ő' => 'o',
        'Ù'..='Ü' | 'ù'..='ü' | 'ů' | 'ű' => 'u',
        'Ý' | 'ý' | 'ÿ' => 'y',
        'ß' => {
            out.push_str("ss");
            return;
        }
        'ł' | 'Ł' => 'l',
        'ś' | 'š' | 'Š' => 's',
        'ž' | 'ź' | 'ż' | 'Ž' => 'z',
        'ř' => 'r',
        _ => {
            out.extend(c.to_lowercase());
            return;
        }
    };
    out.push(base);
}

/// Normalise text and split it into search tokens (alphanumeric runs).
/// Non-Latin scripts (Arabic, Cyrillic, CJK...) are kept as-is, lowercased.
pub fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            fold_char(c, &mut cur);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Standard base64 (RFC 4648) decoder, tolerant of whitespace and missing
/// padding. Xtream `get_short_epg` returns titles base64-encoded.
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    }
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &c in input.as_bytes() {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Decode base64 to text, falling back to the original if it was not base64.
pub fn maybe_base64_text(s: &str) -> String {
    base64_decode(s)
        .and_then(|b| String::from_utf8(b).ok())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_is_stable() {
        // Known FNV-1a 64 vector for "a".
        assert_eq!(fnv1a64(&[b"a"]), 0xaf63dc4c8601ec8c);
        assert_ne!(fnv1a64(&[b"ab", b"c"]), fnv1a64(&[b"a", b"bc"]));
    }

    #[test]
    fn tokenizer_folds() {
        assert_eq!(tokens("Télé-Matin HD"), ["tele", "matin", "hd"]);
        assert_eq!(tokens("STRAßE  München!"), ["strasse", "munchen"]);
        assert_eq!(tokens("Первый канал"), ["первый", "канал"]);
    }

    #[test]
    fn base64() {
        assert_eq!(base64_decode("SGVsbG8gV29ybGQ=").unwrap(), b"Hello World");
        assert_eq!(base64_decode("SGVsbG8").unwrap(), b"Hello");
        assert_eq!(maybe_base64_text("TmV3cw=="), "News");
        assert_eq!(maybe_base64_text("Not base64!"), "Not base64!");
    }
}

/// A fast, non-cryptographic hasher for the catalog's own maps (FxHash's mixing step). The
/// standard SipHash resists crafted keys, but costs several times more on the 32-bit ARM CPUs
/// of cheap TVs, where building the maps of a large catalog is a good part of the start-up.
#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);

impl FastHasher {
    #[inline]
    fn add(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl std::hash::Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes(c.try_into().unwrap_or_default()));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(b));
        }
    }
    fn write_u8(&mut self, n: u8) {
        self.add(u64::from(n));
    }
    fn write_u32(&mut self, n: u32) {
        self.add(u64::from(n));
    }
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

pub type FastBuild = std::hash::BuildHasherDefault<FastHasher>;
/// `HashMap` with [`FastHasher`].
pub type FastMap<K, V> = std::collections::HashMap<K, V, FastBuild>;
/// `HashSet` with [`FastHasher`].
pub type FastSet<K> = std::collections::HashSet<K, FastBuild>;
