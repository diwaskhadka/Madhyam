//! Parental controls without the UI: which categories and items are locked, the PIN (stored
//! only as a salted SHA-256), age ratings, and the "unlocked for this session" window.
//! Same rules as the Windows and Android TV apps (spec 1.1.1).

use crate::repo::Item;
use crate::settings::{self, AppSettings};
use gtk::glib;
use regex::Regex;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// A PIN stays valid this long after the last PIN-gated action.
const SESSION: Duration = Duration::from_secs(10 * 60);
/// Wrong PINs before the entry dialog makes you wait, and how long.
pub const MAX_TRIES: u32 = 5;
pub const WAIT: Duration = Duration::from_secs(30);

/// Names treated as adult content: whole words, or set apart by separators ("XXX|", "[18+]").
static ADULT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:^|[^\p{L}\p{N}])(?:xxx|adults?|18\+|\+18|porn|erotica?|playboy|hustler|brazzers|redlight|private)(?:$|[^\p{L}\p{N}])",
    )
    .unwrap()
});

pub fn is_adult(name: &str) -> bool {
    ADULT.is_match(name)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    /// Locked by hand in Parental controls.
    Category,
    /// Locked because the name looks like adult content.
    Adult,
}

/// "sourceId:kind" for the lock lists; episodes belong to their series' categories.
pub fn key(source: i64, kind: &str) -> String {
    let kind = if kind == "episode" { "series" } else { kind };
    AppSettings::hidden_key(source, kind)
}

/// Why a category is locked, or None (parental controls off, or the category is open).
pub fn category_lock(s: &AppSettings, source: i64, kind: &str, group: &str) -> Option<Lock> {
    if !s.parental_enabled {
        return None;
    }
    let k = key(source, kind);
    let has = |m: &std::collections::HashMap<String, Vec<String>>| {
        m.get(&k).is_some_and(|v| v.iter().any(|g| g == group))
    };
    if has(&s.unlocked_categories) {
        return None; // an explicit unlock wins over the adult auto-lock
    }
    if has(&s.locked_categories) {
        return Some(Lock::Category);
    }
    (s.parental_auto_adult && is_adult(group)).then_some(Lock::Adult)
}

/// Locked by its category, or (live channels) by an adult-looking name. Age limits are separate:
/// they need the details, see `age_blocked`.
pub fn item_lock(s: &AppSettings, it: &Item) -> Option<Lock> {
    if !s.parental_enabled {
        return None;
    }
    category_lock(s, it.source_id, &it.kind, &it.group).or_else(|| {
        (it.is_live() && s.parental_auto_adult && is_adult(&it.name)).then_some(Lock::Adult)
    })
}

/// True when this item should be left out of lists right now (locked and not unlocked).
pub fn hide_in_lists(s: &AppSettings, it: &Item) -> bool {
    item_lock(s, it).is_some() && !unlocked()
}

/// The names in `groups` that are locked for this source and kind (empty while unlocked).
pub fn locked_names<'a>(
    s: &AppSettings,
    source: i64,
    kind: &str,
    groups: impl Iterator<Item = &'a str>,
) -> Vec<String> {
    if !s.parental_enabled || unlocked() {
        return Vec::new();
    }
    groups
        .filter(|g| category_lock(s, source, kind, g).is_some())
        .map(str::to_string)
        .collect()
}

/// An age from a provider rating: "PG-13" → 12, "R" → 16, "12+", "FSK 16", "TV-MA" → 18…
pub fn parse_age(rating: &str) -> Option<i32> {
    let r = rating.trim().to_uppercase();
    if r.is_empty() {
        return None;
    }
    let named = match r.replace(' ', "").as_str() {
        "G" | "PG" | "TV-Y" | "TV-G" | "TV-Y7" | "TV-PG" => Some(7),
        "PG-13" | "PG13" => Some(12),
        "R" => Some(16),
        "NC-17" | "NC17" | "TV-MA" => Some(18),
        _ => None,
    };
    if named.is_some() {
        return named;
    }
    static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{1,2})").unwrap());
    NUM.captures(&r)
        .and_then(|c| c[1].parse::<i32>().ok())
        .filter(|n| (0..=21).contains(n))
}

/// The film or series is above the "Allow up to" limit (no rating → allowed).
pub fn age_blocked(s: &AppSettings, age: Option<i32>) -> bool {
    s.parental_enabled && s.parental_max_age > 0 && age.is_some_and(|a| a > s.parental_max_age)
}

// ------------------------------------------------------------------ PIN

pub fn has_pin(s: &AppSettings) -> bool {
    !s.parental_pin_hash.is_empty()
}

/// Lowercase hex of SHA-256(salt bytes ‖ UTF-8 PIN), as on Windows and Android TV.
fn hash(salt_hex: &str, pin: &str) -> String {
    let salt: Vec<u8> = (0..salt_hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(salt_hex.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect();
    let mut c = glib::Checksum::new(glib::ChecksumType::Sha256).expect("SHA-256");
    c.update(&salt);
    c.update(pin.as_bytes());
    c.string().map(|s| s.to_lowercase()).unwrap_or_default()
}

pub fn check_pin(pin: &str) -> bool {
    let s = settings::get();
    has_pin(&s) && hash(&s.parental_pin_salt, pin) == s.parental_pin_hash
}

pub fn set_pin(pin: &str) {
    // 16 random bytes, stored as lowercase hex.
    let salt: String = (0..4)
        .map(|_| format!("{:08x}", glib::random_int()))
        .collect();
    debug_assert_eq!(salt.len(), 32);
    let h = hash(&salt, pin);
    settings::update(|s| {
        s.parental_pin_salt = salt;
        s.parental_pin_hash = h;
    });
}

/// Forgotten PIN: remove the PIN and every lock.
pub fn reset() {
    settings::update(|s| {
        s.parental_enabled = false;
        s.parental_pin_hash.clear();
        s.parental_pin_salt.clear();
        s.parental_lock_settings = false;
        s.parental_max_age = 0;
        s.parental_auto_adult = true;
        s.locked_categories.clear();
        s.unlocked_categories.clear();
    });
    lock_now();
    crate::log("parental controls reset (PIN and all locks removed)");
}

// ------------------------------------------------------------------ session and wrong tries

struct State {
    until: Option<Instant>,
    wrong: u32,
    wait_until: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    until: None,
    wrong: 0,
    wait_until: None,
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|p| p.into_inner())
}

/// The PIN was entered less than 10 minutes after the last gated action.
pub fn unlocked() -> bool {
    state().until.is_some_and(|t| Instant::now() < t)
}

/// A gated action while unlocked: the 10 minutes start again.
pub fn touch() {
    let mut st = state();
    if st.until.is_some_and(|t| Instant::now() < t) {
        st.until = Some(Instant::now() + SESSION);
    }
}

pub fn unlock() {
    let mut st = state();
    st.until = Some(Instant::now() + SESSION);
    st.wrong = 0;
    st.wait_until = None;
}

pub fn lock_now() {
    state().until = None;
}

/// A wrong PIN; true when that was one too many and entry now waits.
pub fn wrong_pin() -> bool {
    let mut st = state();
    st.wrong += 1;
    if st.wrong >= MAX_TRIES {
        st.wrong = 0;
        st.wait_until = Some(Instant::now() + WAIT);
        return true;
    }
    false
}

/// Seconds left before the PIN can be tried again.
pub fn wait_left() -> u64 {
    state()
        .wait_until
        .map(|t| {
            t.saturating_duration_since(Instant::now())
                .as_secs_f64()
                .ceil() as u64
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adult_names() {
        for n in [
            "XXX",
            "Adult",
            "ADULTS | VOD",
            "[18+] Night",
            "+18 channels",
            "US: Playboy TV",
            "EN - Private",
            "erotica",
            "XXX: Redlight",
            "porn_hd",
        ] {
            assert!(is_adult(n), "{n}");
        }
        for n in [
            "Adulthood",
            "Kids",
            "Sport 18",
            "Privateer",
            "Pornographic",
            "News 180",
        ] {
            assert!(!is_adult(n), "{n}");
        }
    }

    #[test]
    fn pin_hash_matches_other_platforms() {
        // SHA-256(salt bytes ‖ UTF-8 PIN), lowercase hex (checked with Python's hashlib).
        assert_eq!(
            hash("000102030405060708090a0b0c0d0e0f", "1234"),
            "bad0b4f7cae08eb7c1b5acc763a8ed25242e2c9173e81f3f624b1fa5303cf6ce"
        );
    }

    #[test]
    fn ages() {
        assert_eq!(parse_age("PG-13"), Some(12));
        assert_eq!(parse_age("R"), Some(16));
        assert_eq!(parse_age("NC-17"), Some(18));
        assert_eq!(parse_age("G"), Some(7));
        assert_eq!(parse_age("PG"), Some(7));
        assert_eq!(parse_age("12+"), Some(12));
        assert_eq!(parse_age("FSK 16"), Some(16));
        assert_eq!(parse_age(""), None);
        assert_eq!(parse_age("Unrated"), None);
    }
}
