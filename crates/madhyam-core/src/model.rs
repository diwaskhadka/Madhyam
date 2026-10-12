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
    /// XMLTV channel id; matches `tvg-id` / Xtream `epg_channel_id`.
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
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub source: SourceId,
    pub kind: ItemKind,
    /// ID inside the source (Xtream stream_id / series_id, or a hash for M3U).
    pub remote_id: String,
    pub name: String,
    pub group: String,
    pub logo: String,
    /// Direct stream URL. Empty for series (open them to get episodes).
    pub url: String,
    /// EPG channel id for live items.
    pub epg_id: String,
    /// Catch-up days available (0 = none).
    pub archive_days: u32,
    /// Position within the source, so the UI can keep the provider's order.
    pub order: u32,
    /// Everything else: plot, rating, year, container extension, user-agent...
    pub extra: Vec<(String, String)>,
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
    /// A blank item with its id computed; fill the rest with struct update syntax.
    pub fn new(
        source: SourceId,
        kind: ItemKind,
        remote_id: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        let remote_id = remote_id.into();
        Item {
            id: item_id(source, kind, &remote_id),
            source,
            kind,
            remote_id,
            name: name.into(),
            group: String::new(),
            logo: String::new(),
            url: String::new(),
            epg_id: String::new(),
            archive_days: 0,
            order: 0,
            extra: Vec::new(),
        }
    }

    /// Add an `extra` field if the value is present and non-empty.
    pub fn with_extra(mut self, key: &str, value: Option<String>) -> Self {
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            self.extra.push((key.to_string(), v));
        }
        self
    }

    pub fn extra(&self, key: &str) -> Option<&str> {
        self.extra
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
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
            ("remote_id", self.remote_id.as_str().into()),
            ("name", self.name.as_str().into()),
            ("group", self.group.as_str().into()),
            ("logo", self.logo.as_str().into()),
            ("url", self.url.as_str().into()),
            ("epg_id", self.epg_id.as_str().into()),
            ("archive_days", self.archive_days.into()),
            ("extra", pairs_json(&self.extra)),
        ])
    }
}
