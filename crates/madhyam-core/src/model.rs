//! Plain data types shared by every module and exposed over FFI.

use crate::json::{ToJson, Value, obj};

/// Stable identifier of a catalog item. Derived by hashing
/// `(source, kind, remote id)`, so it survives playlist refreshes and can be
/// stored by hosts (favorites, watch history).
pub type ItemId = u64;
pub type SourceId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemKind {
    Live,
    Movie,
    Series,
    /// A single episode of a series (only stored when a series is opened).
    Episode,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Live => "live",
            ItemKind::Movie => "movie",
            ItemKind::Series => "series",
            ItemKind::Episode => "episode",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "live" => ItemKind::Live,
            "movie" | "vod" => ItemKind::Movie,
            "series" => ItemKind::Series,
            "episode" => ItemKind::Episode,
            _ => return None,
        })
    }
}

/// Catch-up (TV archive) settings of a live channel.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catchup {
    /// `default`, `append`, `shift`, `flussonic`, `xc`... (M3U `catchup=`).
    pub kind: String,
    /// URL template with `{utc}`, `${start}`, `{duration}`... placeholders.
    pub source: String,
    pub days: u32,
}

/// One entry of an M3U playlist, exactly as written in the file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct M3uEntry {
    pub name: String,
    pub url: String,
    /// `#EXTINF` duration; `-1` (or 0) for live streams.
    pub duration: f64,
    pub tvg_id: String,
    pub tvg_name: String,
    pub logo: String,
    pub group: String,
    pub user_agent: String,
    pub referrer: String,
    pub catchup: Option<Catchup>,
    pub radio: bool,
    /// `#KODIPROP:key=value` lines (DRM licence type/key, manifest type...).
    pub kodi_props: Vec<(String, String)>,
    /// Every `key="value"` attribute on the `#EXTINF` line, including unknown ones.
    pub attrs: Vec<(String, String)>,
    /// 1-based line number of the `#EXTINF` (or URL) line, for diagnostics.
    pub line: usize,
}

impl M3uEntry {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// Guess what kind of content this is. M3U has no field for it, so we use
    /// the same heuristics as most players: Xtream-style paths and file
    /// extensions.
    pub fn guess_kind(&self) -> ItemKind {
        let url = self.url.to_ascii_lowercase();
        let path = url.split(['?', '|']).next().unwrap_or(&url);
        if path.contains("/series/") {
            return ItemKind::Episode;
        }
        if path.contains("/movie/") {
            return ItemKind::Movie;
        }
        const VOD_EXT: [&str; 8] = [
            ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".m4v", ".webm", ".flv",
        ];
        if VOD_EXT.iter().any(|e| path.ends_with(e)) || self.duration > 0.0 {
            return ItemKind::Movie;
        }
        ItemKind::Live
    }
}

/// A channel declared in an XMLTV file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpgChannel {
    pub id: String,
    pub names: Vec<String>,
    pub icon: String,
}

/// One programme (show) in the guide.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Programme {
    /// XMLTV channel id; matches `tvg-id` / Xtream `epg_channel_id`. Read back from a
    /// catalog, the (lowercase) guide key.
    pub channel: String,
    /// Unix seconds, UTC.
    pub start: i64,
    pub stop: i64,
    pub title: String,
    pub sub_title: String,
    pub desc: String,
    pub categories: Vec<String>,
    pub icon: String,
    pub episode: String,
    pub rating: String,
}

impl Programme {
    pub fn is_live_at(&self, t: i64) -> bool {
        self.start <= t && t < self.stop
    }
}

/// The unified, playable thing the UI shows: a channel, a film, a series, an episode.
///
/// A catalog holds hundreds of thousands of these on TVs with 1–2 GB of RAM, so an item keeps
/// all its text in **one** allocation (remote id, name, logo, URL, EPG id, then the extra
/// fields), and shares its group name and the directories of its logo and stream URLs
/// (`https://image.tmdb.org/t/p/w600_and_h900_bestv2/`) with the other items. Read the text
/// with the accessors ([`Item::name`], [`Item::extra`]…), change it with the setters.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub source: SourceId,
    pub kind: ItemKind,
    /// Catch-up days available (0 = none).
    pub archive_days: u32,
    /// Position within the source, so the UI can keep the provider's order.
    pub order: u32,
    /// Provider category; the catalog shares one string per group name.
    pub group: Group,
    /// Logo and stream URLs up to their last `/` (shared); `text` holds the rest.
    logo_dir: Group,
    url_dir: Group,
    /// `remote_id name logo url epg_id` back to back, then the extra fields as
    /// `key \0 value \0` pairs (well-known keys as one control character, see [`EXTRA_KEYS`]).
    text: Box<str>,
    /// End offsets of the five fixed fields in `text`.
    ends: [u32; 5],
}

/// A group (category) name, shared by every item of the group.
pub type Group = std::sync::Arc<str>;

/// The empty [`Group`], without an allocation per item.
fn no_group() -> Group {
    static EMPTY: std::sync::OnceLock<Group> = std::sync::OnceLock::new();
    EMPTY.get_or_init(|| Group::from("")).clone()
}

/// `(directory, file)` of a URL: everything up to the last `/`, and the rest.
fn split_dir(url: &str) -> (&str, &str) {
    url.split_at(url.rfind('/').map_or(0, |i| i + 1))
}

fn dir_of(dir: &str) -> Group {
    if dir.is_empty() {
        no_group()
    } else {
        Group::from(dir)
    }
}

fn joined<'a>(dir: &str, file: &'a str) -> std::borrow::Cow<'a, str> {
    if dir.is_empty() {
        std::borrow::Cow::Borrowed(file)
    } else {
        std::borrow::Cow::Owned(format!("{dir}{file}"))
    }
}

const REMOTE_ID: usize = 0;
const NAME: usize = 1;
const LOGO: usize = 2;
const URL: usize = 3;
const EPG_ID: usize = 4;

/// Extra keys stored as one character (`\u{1}` + index) instead of their name.
const EXTRA_KEYS: [&str; 24] = [
    "added",
    "container",
    "rating",
    "tmdb",
    "plot",
    "cast",
    "genre",
    "year",
    "trailer",
    "last_modified",
    "direct_source",
    "series_id",
    "season",
    "episode",
    "duration_secs",
    "air_date",
    "tvg_name",
    "user_agent",
    "referrer",
    "radio",
    "duration",
    "catchup",
    "catchup_source",
    "country",
];

fn push_extra_to(out: &mut String, key: &str, value: &str) {
    match EXTRA_KEYS.iter().position(|k| *k == key) {
        Some(i) => out.push(char::from(1 + i as u8)),
        None => out.push_str(key),
    }
    out.push('\0');
    // The separator cannot be part of a value.
    out.extend(value.chars().map(|c| if c == '\0' { ' ' } else { c }));
    out.push('\0');
}

fn decode_key(k: &str) -> &str {
    match k.as_bytes() {
        [b] if (1..=EXTRA_KEYS.len() as u8).contains(b) => EXTRA_KEYS[(*b - 1) as usize],
        _ => k,
    }
}

/// Stable [`ItemId`] for `(source, kind, remote_id)`.
pub fn item_id(source: SourceId, kind: ItemKind, remote_id: &str) -> ItemId {
    crate::textutil::fnv1a64(&[
        &source.to_le_bytes(),
        kind.as_str().as_bytes(),
        remote_id.as_bytes(),
    ])
}

impl Item {
    /// A blank item with its id computed; fill the rest with the setters.
    pub fn new(
        source: SourceId,
        kind: ItemKind,
        remote_id: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        let remote_id: String = remote_id.into();
        let name: String = name.into();
        let r = remote_id.len() as u32;
        let n = r + name.len() as u32;
        Item {
            id: item_id(source, kind, &remote_id),
            source,
            kind,
            archive_days: 0,
            order: 0,
            group: no_group(),
            logo_dir: no_group(),
            url_dir: no_group(),
            text: (remote_id + &name).into_boxed_str(),
            ends: [r, n, n, n, n],
        }
    }

    fn field(&self, i: usize) -> &str {
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        &self.text[start..self.ends[i] as usize]
    }

    fn set_field(&mut self, i: usize, value: &str) {
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        let end = self.ends[i] as usize;
        let mut t = String::with_capacity(self.text.len() - (end - start) + value.len());
        t.push_str(&self.text[..start]);
        t.push_str(value);
        t.push_str(&self.text[end..]);
        self.text = t.into_boxed_str();
        let delta = value.len() as i64 - (end - start) as i64;
        for e in &mut self.ends[i..] {
            *e = (*e as i64 + delta) as u32;
        }
    }

    /// ID inside the source (Xtream stream_id / series_id, or a hash for M3U).
    pub fn remote_id(&self) -> &str {
        self.field(REMOTE_ID)
    }
    pub fn name(&self) -> &str {
        self.field(NAME)
    }
    pub fn logo(&self) -> std::borrow::Cow<'_, str> {
        joined(&self.logo_dir, self.field(LOGO))
    }
    /// Direct stream URL. Empty for series (open them to get episodes).
    pub fn url(&self) -> std::borrow::Cow<'_, str> {
        joined(&self.url_dir, self.field(URL))
    }
    /// EPG channel id for live items.
    pub fn epg_id(&self) -> &str {
        self.field(EPG_ID)
    }

    pub fn set_name(&mut self, v: &str) {
        self.set_field(NAME, v)
    }
    pub fn set_logo(&mut self, v: &str) {
        let (dir, file) = split_dir(v);
        self.logo_dir = dir_of(dir);
        self.set_field(LOGO, file)
    }
    pub fn set_url(&mut self, v: &str) {
        let (dir, file) = split_dir(v);
        self.url_dir = dir_of(dir);
        self.set_field(URL, file)
    }
    pub fn set_epg_id(&mut self, v: &str) {
        self.set_field(EPG_ID, v)
    }
    pub fn set_group(&mut self, v: &str) {
        self.group = Group::from(v);
    }

    /// Set several fixed fields at once, with one allocation (importers and the snapshot).
    pub fn set_text(&mut self, logo: &str, url: &str, epg_id: &str) {
        let (dir, logo) = split_dir(logo);
        self.logo_dir = dir_of(dir);
        let (dir, url) = split_dir(url);
        self.url_dir = dir_of(dir);
        let n = self.ends[NAME] as usize;
        let mut t = String::with_capacity(n + logo.len() + url.len() + epg_id.len());
        t.push_str(&self.text[..n]);
        t.push_str(logo);
        t.push_str(url);
        t.push_str(epg_id);
        let l = (n + logo.len()) as u32;
        let u = l + url.len() as u32;
        self.ends[LOGO] = l;
        self.ends[URL] = u;
        self.ends[EPG_ID] = u + epg_id.len() as u32;
        self.text = t.into_boxed_str();
    }

    /// An item from all its fields at once: one allocation for the text, and the group and
    /// URL directories looked up in `shared` (the fast path for loading a snapshot).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        source: SourceId,
        kind: ItemKind,
        remote_id: &str,
        name: &str,
        group: &str,
        logo: &str,
        url: &str,
        epg_id: &str,
        extras: &[(&str, &str)],
        shared: &mut crate::textutil::FastSet<Group>,
    ) -> Self {
        let mut share = |s: &str| match shared.get(s) {
            Some(g) => g.clone(),
            None => {
                let g = Group::from(s);
                shared.insert(g.clone());
                g
            }
        };
        let (logo_dir, logo) = split_dir(logo);
        let (url_dir, url) = split_dir(url);
        let fixed = [remote_id, name, logo, url, epg_id];
        let len = fixed.iter().map(|f| f.len()).sum::<usize>()
            + extras
                .iter()
                .map(|(k, v)| k.len() + v.len() + 2)
                .sum::<usize>();
        let mut text = String::with_capacity(len);
        let mut ends = [0u32; 5];
        for (i, f) in fixed.iter().enumerate() {
            text.push_str(f);
            ends[i] = text.len() as u32;
        }
        for (k, v) in extras {
            push_extra_to(&mut text, k, v);
        }
        Item {
            id: item_id(source, kind, remote_id),
            source,
            kind,
            archive_days: 0,
            order: 0,
            group: share(group),
            logo_dir: share(logo_dir),
            url_dir: share(url_dir),
            text: text.into_boxed_str(),
            ends,
        }
    }

    /// The item as a snapshot line (no newline): the same text as its JSON with `t` and
    /// `order` added, written directly. Saving builds no JSON tree per item, which on a TV
    /// took longer than writing the file.
    pub(crate) fn write_snapshot_line(&self, out: &mut String) {
        use crate::json::write_escaped;
        use std::fmt::Write as _;
        let _ = write!(out, "{{\"archive_days\":{},\"epg_id\":", self.archive_days);
        write_escaped(out, self.epg_id());
        out.push_str(",\"extra\":{");
        // Keys in order, the last of a repeated key winning: as a JSON object (BTreeMap) has it.
        let mut extras: Vec<(&str, &str)> = self.extras().collect();
        extras.sort_by(|a, b| a.0.cmp(b.0));
        let mut first = true;
        for (i, (k, v)) in extras.iter().enumerate() {
            if extras.get(i + 1).is_some_and(|n| n.0 == *k) {
                continue;
            }
            if !first {
                out.push(',');
            }
            first = false;
            write_escaped(out, k);
            out.push(':');
            write_escaped(out, v);
        }
        out.push_str("},\"group\":");
        write_escaped(out, &self.group);
        let _ = write!(out, ",\"id\":\"{}\",\"kind\":", self.id);
        write_escaped(out, self.kind.as_str());
        out.push_str(",\"logo\":");
        write_escaped(out, &self.logo());
        out.push_str(",\"name\":");
        write_escaped(out, self.name());
        let _ = write!(out, ",\"order\":{},\"remote_id\":", self.order);
        write_escaped(out, self.remote_id());
        let _ = write!(out, ",\"source\":{},\"t\":\"item\",\"url\":", self.source);
        write_escaped(out, &self.url());
        out.push('}');
    }

    /// Share the group and URL directory strings with other items (`seen` collects them).
    pub(crate) fn intern(&mut self, seen: &mut crate::textutil::FastSet<Group>) {
        for s in [&mut self.group, &mut self.logo_dir, &mut self.url_dir] {
            match seen.get(&**s) {
                Some(g) => *s = g.clone(),
                None => {
                    seen.insert(s.clone());
                }
            }
        }
    }

    /// Everything else: plot, rating, year, container extension, user-agent…, in insertion order.
    pub fn extras(&self) -> impl Iterator<Item = (&str, &str)> {
        let tail = &self.text[self.ends[EPG_ID] as usize..];
        let mut parts = tail.split('\0');
        std::iter::from_fn(move || {
            let k = parts.next().filter(|k| !k.is_empty())?;
            Some((decode_key(k), parts.next().unwrap_or("")))
        })
    }

    pub fn extra(&self, key: &str) -> Option<&str> {
        self.extras().find(|(k, _)| *k == key).map(|(_, v)| v)
    }

    /// Append an extra field (one with the same key stays; see [`Item::remove_extra`]).
    pub fn push_extra(&mut self, key: &str, value: &str) {
        let mut t = String::with_capacity(self.text.len() + key.len() + value.len() + 2);
        t.push_str(&self.text);
        push_extra_to(&mut t, key, value);
        self.text = t.into_boxed_str();
    }

    /// Replace every extra field.
    pub fn set_extras<'a>(&mut self, extras: impl IntoIterator<Item = (&'a str, &'a str)>) {
        let mut t = String::from(&self.text[..self.ends[EPG_ID] as usize]);
        for (k, v) in extras {
            push_extra_to(&mut t, k, v);
        }
        self.text = t.into_boxed_str();
    }

    pub fn remove_extra(&mut self, key: &str) {
        if self.extra(key).is_none() {
            return;
        }
        let kept: Vec<(String, String)> = self
            .extras()
            .filter(|(k, _)| *k != key)
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.set_extras(kept.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    }

    /// Add an `extra` field if the value is present and non-empty.
    pub fn with_extra(mut self, key: &str, value: Option<String>) -> Self {
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            self.push_extra(key, &v);
        }
        self
    }
}

// ---------------------------------------------------------------------------
// JSON representations (used by the FFI layer and the CLI).
// IDs are sent as strings: a u64 does not fit in a JavaScript/JSON double.
// ---------------------------------------------------------------------------

fn pairs_json(p: &[(String, String)]) -> Value {
    Value::Object(
        p.iter()
            .map(|(k, v)| (k.clone(), Value::from(v.as_str())))
            .collect(),
    )
}

impl ToJson for Catchup {
    fn to_json(&self) -> Value {
        obj([
            ("kind", self.kind.as_str().into()),
            ("source", self.source.as_str().into()),
            ("days", self.days.into()),
        ])
    }
}

impl ToJson for M3uEntry {
    fn to_json(&self) -> Value {
        obj([
            ("name", self.name.as_str().into()),
            ("url", self.url.as_str().into()),
            ("duration", self.duration.into()),
            ("tvg_id", self.tvg_id.as_str().into()),
            ("tvg_name", self.tvg_name.as_str().into()),
            ("logo", self.logo.as_str().into()),
            ("group", self.group.as_str().into()),
            ("user_agent", self.user_agent.as_str().into()),
            ("referrer", self.referrer.as_str().into()),
            ("catchup", self.catchup.to_json()),
            ("radio", self.radio.into()),
            ("kodi_props", pairs_json(&self.kodi_props)),
            ("attrs", pairs_json(&self.attrs)),
            ("kind", self.guess_kind().as_str().into()),
        ])
    }
}

impl ToJson for EpgChannel {
    fn to_json(&self) -> Value {
        obj([
            ("id", self.id.as_str().into()),
            ("names", self.names.clone().into()),
            ("icon", self.icon.as_str().into()),
        ])
    }
}

impl ToJson for Programme {
    fn to_json(&self) -> Value {
        obj([
            ("channel", self.channel.as_str().into()),
            ("start", self.start.into()),
            ("stop", self.stop.into()),
            ("title", self.title.as_str().into()),
            ("sub_title", self.sub_title.as_str().into()),
            ("desc", self.desc.as_str().into()),
            ("categories", self.categories.clone().into()),
            ("icon", self.icon.as_str().into()),
            ("episode", self.episode.as_str().into()),
            ("rating", self.rating.as_str().into()),
        ])
    }
}

impl ToJson for Item {
    fn to_json(&self) -> Value {
        obj([
            ("id", self.id.to_string().into()),
            ("source", self.source.into()),
            ("kind", self.kind.as_str().into()),
            ("remote_id", self.remote_id().into()),
            ("name", self.name().into()),
            ("group", (*self.group).into()),
            ("logo", self.logo().as_ref().into()),
            ("url", self.url().as_ref().into()),
            ("epg_id", self.epg_id().into()),
            ("archive_days", self.archive_days.into()),
            (
                "extra",
                Value::Object(
                    self.extras()
                        .map(|(k, v)| (k.to_string(), Value::from(v)))
                        .collect(),
                ),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_item_fields_and_extras() {
        let mut it = Item::new(3, ItemKind::Movie, "42", "Heat");
        it.set_text("https://img/t/p/heat.jpg", "http://h/movie/u/p/42.mkv", "");
        it.set_group("Action");
        it = it
            .with_extra("rating", Some("8.3".into()))
            .with_extra("kodi.license", Some("a\0b".into()))
            .with_extra("plot", Some(String::new()));
        assert_eq!((it.remote_id(), it.name(), it.epg_id()), ("42", "Heat", ""));
        assert_eq!(it.logo(), "https://img/t/p/heat.jpg");
        assert_eq!(it.url(), "http://h/movie/u/p/42.mkv");
        assert_eq!(it.extra("rating"), Some("8.3"));
        assert_eq!(it.extra("kodi.license"), Some("a b"));
        assert_eq!(it.extra("plot"), None);
        it.set_name("Heat (1995)");
        it.set_epg_id("heat.us");
        it.set_logo("plain");
        assert_eq!(
            (it.name(), it.epg_id(), &*it.logo()),
            ("Heat (1995)", "heat.us", "plain")
        );
        assert_eq!(it.url(), "http://h/movie/u/p/42.mkv");
        it.remove_extra("rating");
        assert_eq!(
            it.extras().collect::<Vec<_>>(),
            vec![("kodi.license", "a b")]
        );
        assert_eq!(it.id, item_id(3, ItemKind::Movie, "42"));
    }
}
