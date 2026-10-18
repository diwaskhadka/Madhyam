//! Free channels from iptv-org (github.com/iptv-org/iptv): public streams collected by the
//! community, added as an ordinary M3U playlist named "Free TV". Preinstalled on first start.

pub const NAME: &str = "Free TV";

/// Every channel, grouped by category (News, Sports, Kids…).
pub const ALL_URL: &str = "https://iptv-org.github.io/iptv/index.category.m3u";

/// The same channels grouped by country; used only to tag channels with their countries.
pub const COUNTRY_INDEX_URL: &str = "https://iptv-org.github.io/iptv/index.country.m3u";

/// Item field holding a channel's countries (';'-separated).
pub const COUNTRY_FIELD: &str = "country";

pub fn is_free_tv(location: &str) -> bool {
    location.eq_ignore_ascii_case(ALL_URL) || is_countries(location)
}

/// Older "only these countries" location (still read so it can be merged away).
pub fn is_countries(location: &str) -> bool {
    location
        .to_ascii_lowercase()
        .starts_with("iptvorg:countries=")
}

pub fn country_urls(location: &str) -> Vec<String> {
    location["iptvorg:countries=".len().min(location.len())..]
        .split(',')
        .map(str::trim)
        .filter(|c| c.len() == 2 && c.chars().all(|ch| ch.is_ascii_alphabetic()))
        .map(|c| {
            format!(
                "https://iptv-org.github.io/iptv/countries/{}.m3u",
                c.to_ascii_lowercase()
            )
        })
        .collect()
}
