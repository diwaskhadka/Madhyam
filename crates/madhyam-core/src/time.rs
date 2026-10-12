//! Date/time without a date library.
//!
//! All times inside the core are **Unix seconds (UTC) as `i64`**. Hosts convert
//! to local time for display; the core never needs to know the user's zone.
//!
//! The calendar maths uses Howard Hinnant's `days_from_civil` algorithm, which
//! is exact for the proleptic Gregorian calendar and branch-light.

/// Days since 1970-01-01 for a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400; // [0, 399]
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]: `(year, month, day)`.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn unix_from_parts(y: i64, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
    days_from_civil(y, mo, d) * 86_400 + i64::from(h) * 3600 + i64::from(mi) * 60 + i64::from(s)
}

/// `(year, month, day, hour, minute, second)` in UTC.
pub fn parts_from_unix(t: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    (
        y,
        m,
        d,
        (secs / 3600) as u32,
        (secs % 3600 / 60) as u32,
        (secs % 60) as u32,
    )
}

fn digits(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// Parse an XMLTV timestamp: `YYYYMMDDhhmmss ±hhmm`.
///
/// XMLTV allows truncation (`YYYYMMDDhhmm`, `YYYYMMDD`) and the offset is
/// optional (UTC assumed). Some generators write `+02:00` or `Z`; accept those.
pub fn parse_xmltv_time(s: &str) -> Option<i64> {
    let s = s.trim();
    let (dt, tz) = match s.find([' ', '+', '-', 'Z']) {
        Some(i) => (&s[..i], s[i..].trim()),
        None => (s, ""),
    };
    // Timestamps are pure ASCII; rejecting anything else up front makes the
    // byte-index slicing below safe (slicing inside a UTF-8 char panics).
    if dt.len() < 8 || !s.is_ascii() {
        return None;
    }
    let y = i64::from(digits(&dt[0..4])?);
    let mo = digits(&dt[4..6])?;
    let d = digits(&dt[6..8])?;
    let field = |a: usize| dt.get(a..a + 2).and_then(digits).unwrap_or(0);
    let (h, mi, sec) = (field(8), field(10), field(12));
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let local = unix_from_parts(y, mo, d, h, mi, sec);
    Some(local - parse_offset(tz)?)
}

/// Offset in seconds east of UTC from `+0200`, `-05:30`, `Z` or empty.
fn parse_offset(tz: &str) -> Option<i64> {
    let tz = tz.trim();
    if tz.is_empty()
        || tz.eq_ignore_ascii_case("z")
        || tz.eq_ignore_ascii_case("utc")
        || tz.eq_ignore_ascii_case("gmt")
    {
        return Some(0);
    }
    let sign = match tz.as_bytes()[0] {
        b'+' => 1,
        b'-' => -1,
        _ => return Some(0), // Unknown zone names: treat as UTC rather than drop the programme.
    };
    let rest: String = tz[1..].chars().filter(char::is_ascii_digit).collect();
    let (hh, mm) = match rest.len() {
        1 | 2 => (digits(&rest)?, 0),
        4 => (digits(&rest[..2])?, digits(&rest[2..])?),
        _ => return None,
    };
    Some(sign * (i64::from(hh) * 3600 + i64::from(mm) * 60))
}

/// Parse `YYYY-MM-DD HH:MM:SS` (Xtream EPG `start`/`end` fields), as UTC.
pub fn parse_sql_datetime(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if !s.is_ascii() || b.len() < 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y = i64::from(digits(&s[0..4])?);
    let mo = digits(&s[5..7])?;
    let d = digits(&s[8..10])?;
    let f = |a: usize| s.get(a..a + 2).and_then(digits).unwrap_or(0);
    Some(unix_from_parts(y, mo, d, f(11), f(14), f(17)))
}

/// Current Unix time. The only place the core reads the clock; hosts may also
/// pass `now` explicitly to keep functions pure and testable.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        for z in [-800_000, -1, 0, 1, 11_016, 20_000, 2_000_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
    }

    #[test]
    fn xmltv_times() {
        // 2026-10-12 13:00:00 +0200 == 11:00:00 UTC
        let t = parse_xmltv_time("20261012130000 +0200").unwrap();
        assert_eq!(parts_from_unix(t), (2026, 10, 12, 11, 0, 0));
        assert_eq!(parse_xmltv_time("20261012110000"), Some(t));
        assert_eq!(parse_xmltv_time("20261012110000 Z"), Some(t));
        assert_eq!(parse_xmltv_time("20261012073000 -03:30"), Some(t));
        assert_eq!(parse_xmltv_time("202610121100"), Some(t));
        assert!(parse_xmltv_time("2026").is_none());
        assert!(parse_xmltv_time("20261332000000").is_none());
        // Regression (found by tests/no_panic.rs): multi-byte char inside the date.
        assert!(parse_xmltv_time("2026\u{fffd}012").is_none());
        assert!(parse_sql_datetime("2026-1\u{e9}-12 11:00").is_none());
    }

    #[test]
    fn sql_time() {
        let t = parse_sql_datetime("2026-10-12 11:00:00").unwrap();
        assert_eq!(Some(t), parse_xmltv_time("20261012110000"));
    }
}
