//! The in-memory catalog: sources, items, search, EPG and user state.
//!
//! Data layout is chosen for TV hardware (slow CPU, little RAM):
//!
//! * Items live in one dense `Vec<Item>`; everything else refers to them by
//!   `u32` index. A `HashMap<ItemId, u32>` gives O(1) lookup by stable id.
//! * Search is an inverted index of folded tokens, kept **sorted** so a prefix
//!   query ("bb" → "bbc") is a binary search plus a short scan.
//! * The guide is `HashMap<channel id, Vec<Programme>>` with each vector sorted
//!   by start time, so "what's on now" is a binary search.
//!
//! The catalog is a cache of the providers' data. User state (favorites,
//! progress, history) is the only thing that *must* be persisted; see
//! [`Catalog::export_user_state`]. A full snapshot for instant offline start
//! is also available: [`Catalog::save_snapshot`] / [`Catalog::load_snapshot`].

use crate::error::{Error, Result};
use crate::json::{self, ToJson, Value, obj};
use crate::m3u::M3uReader;
use crate::model::*;
use crate::textutil::{fnv1a64, tokens};
use crate::xmltv::{XmltvItem, XmltvReader};
use crate::xtream::{XtreamCredentials, XtreamSnapshot};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, Write};

// ======================================================================= types

#[derive(Debug, Clone, PartialEq)]
pub enum SourceKind {
    /// Playlist URL or local path; the host fetches it.
    M3u {
        location: String,
    },
    Xtream(XtreamCredentials),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Source {
    pub id: SourceId,
    pub name: String,
    pub kind: SourceKind,
    /// Guide URLs found in the playlist header (M3U) or set by the user.
    pub epg_urls: Vec<String>,
    pub updated_at: i64,
}

impl Source {
    /// URLs the host should download to fill the guide. For Xtream this
    /// includes `xmltv.php`, which embeds the password: hand it to the HTTP
    /// layer, never to logs or UI.
    pub fn epg_fetch_urls(&self) -> Vec<String> {
        let mut v = self.epg_urls.clone();
        if let SourceKind::Xtream(c) = &self.kind {
            v.push(crate::xtream::XtreamClient::new(c.clone(), self.id).xmltv_url());
        }
        v
    }
}

impl ToJson for Source {
    fn to_json(&self) -> Value {
        let (kind, location, user) = match &self.kind {
            SourceKind::M3u { location } => ("m3u", location.clone(), String::new()),
            SourceKind::Xtream(c) => ("xtream", c.server.clone(), c.username.clone()),
        };
        // Secrets are never serialised to the UI: no password, no xmltv.php URL.
        obj([
            ("id", self.id.into()),
            ("name", self.name.as_str().into()),
            ("kind", kind.into()),
            ("location", location.into()),
            ("username", user.into()),
            ("epg_urls", self.epg_urls.clone().into()),
            ("updated_at", self.updated_at.into()),
        ])
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportStats {
    pub items: usize,
    pub errors: usize,
    pub epg_urls: Vec<String>,
}

impl ToJson for ImportStats {
    fn to_json(&self) -> Value {
        obj([
            ("items", self.items.into()),
            ("errors", self.errors.into()),
            ("epg_urls", self.epg_urls.clone().into()),
        ])
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpgStats {
    pub channels: usize,
    pub programmes: usize,
    pub skipped: usize,
}

impl ToJson for EpgStats {
    fn to_json(&self) -> Value {
        obj([
            ("channels", self.channels.into()),
            ("programmes", self.programmes.into()),
            ("skipped", self.skipped.into()),
        ])
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub position: f64,
    pub duration: f64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SearchFilter {
    pub kind: Option<ItemKind>,
    pub source: Option<SourceId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroupInfo {
    pub name: String,
    pub count: usize,
}

impl ToJson for GroupInfo {
    fn to_json(&self) -> Value {
        obj([
            ("name", self.name.as_str().into()),
            ("count", self.count.into()),
        ])
    }
}

// ================================================================ search index

#[derive(Default)]
struct SearchIndex {
    /// Sorted by token; postings sorted ascending and deduplicated.
    tokens: Vec<(Box<str>, Vec<u32>)>,
}

impl SearchIndex {
    fn build(items: &[Item]) -> Self {
        let mut map: HashMap<String, Vec<u32>> = HashMap::new();
        for (i, it) in items.iter().enumerate() {
            if it.kind == ItemKind::Episode {
                continue; // episodes are reached through their series
            }
            for t in tokens(&it.name) {
                let p = map.entry(t).or_default();
                if p.last() != Some(&(i as u32)) {
                    p.push(i as u32);
                }
            }
        }
        let mut tokens: Vec<_> = map
            .into_iter()
            .map(|(k, v)| (k.into_boxed_str(), v))
            .collect();
        tokens.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        SearchIndex { tokens }
    }

    /// All item indices having a token that starts with `prefix`.
    fn prefix(&self, prefix: &str) -> Vec<u32> {
        let start = self.tokens.partition_point(|(t, _)| &**t < prefix);
        let mut out: Vec<u32> = Vec::new();
        for (t, p) in &self.tokens[start..] {
            if !t.starts_with(prefix) {
                break;
            }
            out.extend_from_slice(p);
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out
}

/// Normalised key for matching EPG ids and channel names.
fn epg_key(s: &str) -> String {
    s.trim().to_lowercase()
}
fn name_key(s: &str) -> String {
    // "BBC One HD" and "bbc one" should match: drop quality suffixes.
    tokens(s)
        .into_iter()
        .filter(|t| {
            !matches!(
                t.as_str(),
                "hd" | "fhd" | "uhd" | "sd" | "4k" | "hevc" | "h265" | "1080p" | "720p"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ======================================================================= query

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    /// Provider order (playlist order / Xtream `num`).
    #[default]
    Provider,
    AddedDesc,
    AddedAsc,
    NameAsc,
    NameDesc,
    RatingDesc,
    RatingAsc,
}

impl Sort {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "provider" | "" => Sort::Provider,
            "added_desc" => Sort::AddedDesc,
            "added_asc" => Sort::AddedAsc,
            "name_asc" => Sort::NameAsc,
            "name_desc" => Sort::NameDesc,
            "rating_desc" => Sort::RatingDesc,
            "rating_asc" => Sort::RatingAsc,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ListQuery {
    pub source: Option<SourceId>,
    pub kind: ItemKind,
    pub group: Option<String>,
    /// Group by this `extra` field (values split on ';') instead of the provider group.
    pub group_field: Option<String>,
    pub hidden_groups: HashSet<String>,
    pub sort: Sort,
    pub min_rating: f64,
    pub offset: usize,
    pub limit: usize,
}

impl ListQuery {
    pub fn new(kind: ItemKind) -> Self {
        ListQuery {
            source: None,
            kind,
            group: None,
            group_field: None,
            hidden_groups: HashSet::new(),
            sort: Sort::Provider,
            min_rating: 0.0,
            offset: 0,
            limit: 100,
        }
    }
}

/// The groups an item belongs to: its provider group, or the ';'-separated values of an
/// `extra` field.
fn item_groups<'a>(i: &'a Item, field: Option<&'a str>) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    match field {
        None => Box::new(std::iter::once(i.group.as_str())),
        // A stream listed under several categories is imported once per listing ("<hash>#2"…);
        // by country it should appear once, so only the first copy counts.
        Some(_) if i.remote_id.contains('#') => Box::new(std::iter::empty()),
        Some(f) => Box::new(
            i.extra
                .iter()
                .filter(move |(k, _)| k == f)
                .flat_map(|(_, v)| v.split(';'))
                .filter(|g| !g.is_empty()),
        ),
    }
}

/// Is the item hidden by `q.hidden_groups`? By an `extra` field, only when every one of its
/// values is hidden (a channel of Nepal and India stays while either is shown).
fn hidden_out(i: &Item, q: &ListQuery) -> bool {
    if q.hidden_groups.is_empty() {
        return false;
    }
    let mut groups = item_groups(i, q.group_field.as_deref()).peekable();
    groups.peek().is_some() && groups.all(|g| q.hidden_groups.contains(g))
}

/// When the provider added the item (Unix seconds), 0 if unknown.
/// Xtream sends `added` for live/VOD and `last_modified` for series.
pub fn item_added(i: &Item) -> i64 {
    i.extra("added")
        .or_else(|| i.extra("last_modified"))
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(0)
}

/// Provider rating (0–10), 0 if unknown.
/// Rating on a 0–10 scale. Providers sometimes send 0–100 values ("72") or junk ("7125");
/// the first are scaled down, the rest ignored so they cannot top "Top rated".
pub fn item_rating(i: &Item) -> f64 {
    normalize_rating(
        i.extra("rating")
            .and_then(|v| v.trim().replace(',', ".").parse::<f64>().ok()),
    )
}

pub fn normalize_rating(r: Option<f64>) -> f64 {
    match r {
        Some(r) if r.is_finite() && (0.0..=10.0).contains(&r) => r,
        Some(r) if r.is_finite() && r > 10.0 && r <= 100.0 => r / 10.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod rating_tests {
    use super::normalize_rating;

    #[test]
    fn ratings_are_normalised() {
        assert_eq!(normalize_rating(Some(7.5)), 7.5);
        assert_eq!(normalize_rating(Some(72.0)), 7.2);
        assert_eq!(normalize_rating(Some(7125.0)), 0.0);
        assert_eq!(normalize_rating(Some(-1.0)), 0.0);
        assert_eq!(normalize_rating(None), 0.0);
    }
}

// ===================================================================== catalog

#[derive(Default)]
pub struct Catalog {
    sources: BTreeMap<SourceId, Source>,
    next_source: SourceId,
    items: Vec<Item>,
    by_id: HashMap<ItemId, u32>,
    index: SearchIndex,

    epg: HashMap<String, Vec<Programme>>,
    epg_channels: HashMap<String, EpgChannel>,
    /// display-name key → epg channel id, for playlists without tvg-id.
    epg_by_name: HashMap<String, String>,

    favorites: BTreeMap<ItemId, i64>,
    progress: HashMap<ItemId, Progress>,
    /// Most recent first, unique, capped.
    history: Vec<(ItemId, i64)>,
}

const HISTORY_CAP: usize = 200;

impl Catalog {
    pub fn new() -> Self {
        Catalog {
            next_source: 1,
            ..Default::default()
        }
    }

    // ------------------------------------------------------------- sources

    pub fn add_source(&mut self, name: &str, kind: SourceKind) -> SourceId {
        let id = self.next_source.max(1);
        self.next_source = id + 1;
        self.sources.insert(
            id,
            Source {
                id,
                name: name.to_string(),
                kind,
                epg_urls: Vec::new(),
                updated_at: 0,
            },
        );
        id
    }

    pub fn source(&self, id: SourceId) -> Option<&Source> {
        self.sources.get(&id)
    }

    pub fn sources(&self) -> Vec<&Source> {
        self.sources.values().collect()
    }

    /// Change where a source comes from (playlist URL, or Xtream server/login).
    /// Items keep their ids; stream URLs that embed the old login are stale
    /// until the next refresh, which the host should start right away.
    pub fn update_source(&mut self, id: SourceId, kind: SourceKind) -> Result<()> {
        let s = self
            .sources
            .get_mut(&id)
            .ok_or_else(|| Error::NotFound(format!("source {id}")))?;
        let same_type = matches!(
            (&s.kind, &kind),
            (SourceKind::M3u { .. }, SourceKind::M3u { .. })
                | (SourceKind::Xtream(_), SourceKind::Xtream(_))
        );
        if !same_type {
            return Err(Error::Invalid(
                "a source cannot change between M3U and Xtream".into(),
            ));
        }
        s.kind = kind;
        Ok(())
    }

    pub fn rename_source(&mut self, id: SourceId, name: &str) -> Result<()> {
        let s = self
            .sources
            .get_mut(&id)
            .ok_or_else(|| Error::NotFound(format!("source {id}")))?;
        s.name = name.to_string();
        Ok(())
    }

    pub fn remove_source(&mut self, id: SourceId) -> Result<()> {
        self.sources
            .remove(&id)
            .ok_or_else(|| Error::NotFound(format!("source {id}")))?;
        self.replace_items(id, |_| true, Vec::new());
        Ok(())
    }

    /// Replace the items of `source` matched by `which` with `new`, then
    /// rebuild lookups. User state is keyed by stable ids, so favorites and
    /// progress of items that still exist survive a refresh.
    fn replace_items(&mut self, source: SourceId, which: impl Fn(&Item) -> bool, new: Vec<Item>) {
        self.items.retain(|it| !(it.source == source && which(it)));
        self.items.extend(new);
        self.reindex();
    }

    fn reindex(&mut self) {
        // Dedupe by id (a provider listing the same stream twice); first wins.
        let mut seen = HashSet::with_capacity(self.items.len());
        self.items.retain(|it| seen.insert(it.id));
        self.by_id = self
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| (it.id, i as u32))
            .collect();
        self.index = SearchIndex::build(&self.items);
    }

    /// Import an M3U playlist into an M3U source, replacing its previous items.
    pub fn import_m3u<R: BufRead>(&mut self, source: SourceId, reader: R) -> Result<ImportStats> {
        if !self.sources.contains_key(&source) {
            return Err(Error::NotFound(format!("source {source}")));
        }
        let mut stats = ImportStats::default();
        let mut items = Vec::new();
        let mut url_seen: HashMap<u64, u32> = HashMap::new();
        let mut parser = M3uReader::new(reader);
        for entry in parser.by_ref() {
            let e = match entry {
                Ok(e) => e,
                Err(Error::Io(io)) => return Err(Error::Io(io)),
                Err(_) => {
                    stats.errors += 1;
                    continue;
                }
            };
            // Remote id: hash of the URL, with a counter for duplicates.
            let h = fnv1a64(&[e.url.as_bytes()]);
            let n = url_seen.entry(h).or_insert(0);
            *n += 1;
            let remote = if *n == 1 {
                format!("{h:016x}")
            } else {
                format!("{h:016x}#{n}")
            };

            let kind = match e.guess_kind() {
                ItemKind::Episode => ItemKind::Movie, // M3U has no series structure; treat as VOD
                k => k,
            };
            let mut it = Item::new(source, kind, remote, e.name.clone());
            it.group = if e.group.is_empty() {
                "Uncategorized".into()
            } else {
                e.group.clone()
            };
            it.logo = e.logo.clone();
            it.url = e.url.clone();
            it.epg_id = e.tvg_id.clone();
            it.archive_days = e.catchup.as_ref().map_or(0, |c| c.days);
            it.order = items.len() as u32;
            let mut it = it
                .with_extra("tvg_name", Some(e.tvg_name.clone()))
                .with_extra("user_agent", Some(e.user_agent.clone()))
                .with_extra("referrer", Some(e.referrer.clone()))
                .with_extra("radio", e.radio.then(|| "1".to_string()))
                .with_extra(
                    "duration",
                    (e.duration > 0.0).then(|| e.duration.to_string()),
                );
            if let Some(c) = &e.catchup {
                it = it
                    .with_extra("catchup", Some(c.kind.clone()))
                    .with_extra("catchup_source", Some(c.source.clone()));
            }
            for (k, v) in &e.kodi_props {
                it.extra.push((format!("kodi.{k}"), v.clone()));
            }
            items.push(it);
        }
        stats.items = items.len();
        stats.epg_urls = parser.header().epg_urls();
        let src = self.sources.get_mut(&source).expect("checked above");
        if !stats.epg_urls.is_empty() {
            src.epg_urls = stats.epg_urls.clone();
        }
        src.updated_at = crate::time::now();
        self.replace_items(source, |_| true, items);
        Ok(stats)
    }

    /// Tag the items of an M3U source with an `extra` field taken from the group titles of a
    /// second playlist that lists the same streams (iptv-org's index by country, say). Items
    /// are matched by URL; a stream listed under several groups gets them all, joined by ';'.
    /// Returns how many items were tagged.
    pub fn tag_from_m3u<R: BufRead>(
        &mut self,
        source: SourceId,
        field: &str,
        reader: R,
    ) -> Result<usize> {
        if !self.sources.contains_key(&source) {
            return Err(Error::NotFound(format!("source {source}")));
        }
        let mut by_url: HashMap<String, Vec<String>> = HashMap::new();
        for e in M3uReader::new(reader).flatten() {
            if e.group.is_empty() {
                continue;
            }
            let v = by_url.entry(e.url).or_default();
            for g in e.group.split(';').map(str::trim).filter(|g| !g.is_empty()) {
                if !v.iter().any(|x| x == g) {
                    v.push(g.to_string());
                }
            }
        }
        let mut n = 0;
        for it in self.items.iter_mut().filter(|i| i.source == source) {
            it.extra.retain(|(k, _)| k != field);
            if let Some(g) = by_url.get(&it.url) {
                it.extra.push((field.to_string(), g.join(";")));
                n += 1;
            }
        }
        Ok(n)
    }

    /// Apply the result of [`crate::xtream::XtreamClient::sync`].
    pub fn apply_xtream(&mut self, source: SourceId, snap: XtreamSnapshot) -> Result<ImportStats> {
        let src = self
            .sources
            .get_mut(&source)
            .ok_or_else(|| Error::NotFound(format!("source {source}")))?;
        src.updated_at = crate::time::now();
        let stats = ImportStats {
            items: snap.items.len(),
            errors: snap.warnings.len(),
            epg_urls: src.epg_urls.clone(),
        }; // Keep cached episodes: they are refreshed when a series is opened.
        self.replace_items(source, |it| it.kind != ItemKind::Episode, snap.items);
        Ok(stats)
    }

    /// Store the episodes of a series (from `get_series_info`).
    pub fn set_episodes(&mut self, series: ItemId, episodes: Vec<Item>) -> Result<()> {
        let s = self
            .item(series)
            .ok_or_else(|| Error::NotFound(format!("item {series}")))?;
        let (source, rid) = (s.source, s.remote_id.clone());
        self.replace_items(
            source,
            |it| it.kind == ItemKind::Episode && it.extra("series_id") == Some(&rid),
            episodes,
        );
        Ok(())
    }

    // ------------------------------------------------------------- queries

    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn item(&self, id: ItemId) -> Option<&Item> {
        self.by_id.get(&id).map(|&i| &self.items[i as usize])
    }

    /// Groups of one source and kind, in provider order, with item counts.
    pub fn groups(&self, source: SourceId, kind: ItemKind) -> Vec<GroupInfo> {
        self.groups_by(source, kind, None)
    }

    /// Like [`Catalog::groups`], but grouped by an `extra` field when `field` is given
    /// (an item counts once in each of its ';'-separated values; untagged items are left out).
    pub fn groups_by(
        &self,
        source: SourceId,
        kind: ItemKind,
        field: Option<&str>,
    ) -> Vec<GroupInfo> {
        let mut order: Vec<GroupInfo> = Vec::new();
        let mut pos: HashMap<&str, usize> = HashMap::new();
        let mut items: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| i.source == source && i.kind == kind)
            .collect();
        items.sort_by_key(|i| i.order);
        for it in items {
            for g in item_groups(it, field) {
                match pos.get(g) {
                    Some(&p) => order[p].count += 1,
                    None => {
                        pos.insert(g, order.len());
                        order.push(GroupInfo {
                            name: g.to_string(),
                            count: 1,
                        });
                    }
                }
            }
        }
        order
    }

    /// Items of one group (or the whole kind if `group` is `None`), paged.
    pub fn list(
        &self,
        source: SourceId,
        kind: ItemKind,
        group: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Vec<&Item> {
        let mut v: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| i.source == source && i.kind == kind && group.is_none_or(|g| i.group == g))
            .collect();
        v.sort_by_key(|i| i.order);
        v.into_iter().skip(offset).take(limit).collect()
    }

    /// Filtered, sorted, paged listing for grids and long lists.
    /// Returns `(total matches, page)`.
    pub fn query(&self, q: &ListQuery) -> (usize, Vec<&Item>) {
        let mut v: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| {
                i.kind == q.kind
                    && q.source.is_none_or(|s| s == i.source)
                    && q.group
                        .as_deref()
                        .is_none_or(|g| item_groups(i, q.group_field.as_deref()).any(|x| x == g))
                    && !hidden_out(i, q)
                    // By a field, repeat listings of one stream ("<hash>#2") show once.
                    && (q.group_field.is_none() || !i.remote_id.contains('#'))
                    && (q.min_rating <= 0.0 || item_rating(i) >= q.min_rating)
            })
            .collect();
        match q.sort {
            Sort::Provider => v.sort_by_key(|i| (i.source, i.order)),
            Sort::AddedDesc => v.sort_by_key(|i| (std::cmp::Reverse(item_added(i)), i.order)),
            Sort::AddedAsc => v.sort_by_key(|i| (item_added(i), i.order)),
            Sort::NameAsc => v.sort_by_cached_key(|i| i.name.to_lowercase()),
            Sort::NameDesc => v.sort_by_cached_key(|i| std::cmp::Reverse(i.name.to_lowercase())),
            Sort::RatingDesc => v.sort_by(|a, b| item_rating(b).total_cmp(&item_rating(a))),
            Sort::RatingAsc => v.sort_by(|a, b| item_rating(a).total_cmp(&item_rating(b))),
        }
        let total = v.len();
        (total, v.into_iter().skip(q.offset).take(q.limit).collect())
    }

    /// Episodes of a series, in season/episode order.
    pub fn episodes(&self, series: ItemId) -> Vec<&Item> {
        let Some(s) = self.item(series) else {
            return Vec::new();
        };
        let mut v: Vec<&Item> = self
            .items
            .iter()
            .filter(|i| {
                i.kind == ItemKind::Episode
                    && i.source == s.source
                    && i.extra("series_id") == Some(&s.remote_id)
            })
            .collect();
        v.sort_by_key(|i| i.order);
        v
    }

    /// Ranked, accent-insensitive, prefix-matching search over item names.
    pub fn search(&self, query: &str, filter: SearchFilter, limit: usize) -> Vec<&Item> {
        let q = tokens(query);
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }
        // Rarest token first keeps intermediate sets small.
        let mut sets: Vec<Vec<u32>> = q.iter().map(|t| self.index.prefix(t)).collect();
        sets.sort_by_key(Vec::len);
        let mut hits = sets[0].clone();
        for s in &sets[1..] {
            hits = intersect(&hits, s);
            if hits.is_empty() {
                return Vec::new();
            }
        }
        let joined = q.join(" ");
        let mut scored: Vec<(i32, usize, u32, u32)> = hits
            .into_iter()
            .filter_map(|ix| {
                let it = &self.items[ix as usize];
                if filter.kind.is_some_and(|k| k != it.kind)
                    || filter.source.is_some_and(|s| s != it.source)
                {
                    return None;
                }
                let name_toks = tokens(&it.name);
                let mut score = 0;
                for t in &q {
                    score += if name_toks.iter().any(|n| n == t) {
                        3
                    } else {
                        1
                    };
                }
                if name_toks.join(" ").starts_with(&joined) {
                    score += 5;
                }
                if self.favorites.contains_key(&it.id) {
                    score += 2;
                }
                Some((-score, it.name.len(), it.order, ix))
            })
            .collect();
        scored.sort_unstable();
        scored
            .into_iter()
            .take(limit)
            .map(|(.., ix)| &self.items[ix as usize])
            .collect()
    }

    // ----------------------------------------------------------------- EPG

    /// Stream an XMLTV guide in. Channels present in this guide replace what
    /// was stored for them; other channels are untouched, so several guides
    /// can be layered. Programmes ending before `keep_from` are dropped.
    pub fn import_xmltv<R: BufRead>(&mut self, reader: R, keep_from: i64) -> Result<EpgStats> {
        let mut fresh: HashMap<String, Vec<Programme>> = HashMap::new();
        let mut stats = EpgStats::default();
        let mut parser = XmltvReader::new(reader);
        // Guides list each channel's programmes consecutively, so caching the
        // last key saves a lowercase allocation for almost every programme.
        let (mut last_raw, mut last_key) = (String::new(), String::new());
        for item in parser.by_ref() {
            match item? {
                XmltvItem::Channel(c) => {
                    stats.channels += 1;
                    let key = epg_key(&c.id);
                    for n in &c.names {
                        self.epg_by_name
                            .entry(name_key(n))
                            .or_insert_with(|| key.clone());
                    }
                    self.epg_channels.insert(key, c);
                }
                XmltvItem::Programme(p) => {
                    if p.stop != 0 && p.stop < keep_from {
                        continue;
                    }
                    if p.channel != last_raw {
                        last_key = epg_key(&p.channel);
                        last_raw.clone_from(&p.channel);
                    }
                    match fresh.get_mut(&last_key) {
                        Some(v) => v.push(p),
                        None => {
                            fresh.insert(last_key.clone(), vec![p]);
                        }
                    }
                }
            }
        }
        stats.skipped = parser.skipped;
        for (ch, mut list) in fresh {
            normalise_schedule(&mut list);
            list.retain(|p| p.stop >= keep_from);
            stats.programmes += list.len();
            self.epg.insert(ch, list);
        }
        Ok(stats)
    }

    /// Merge programmes from another source (e.g. Xtream short EPG).
    pub fn add_programmes(&mut self, programmes: Vec<Programme>) {
        let mut by_ch: HashMap<String, Vec<Programme>> = HashMap::new();
        for p in programmes {
            by_ch.entry(epg_key(&p.channel)).or_default().push(p);
        }
        for (ch, new) in by_ch {
            let list = self.epg.entry(ch).or_default();
            list.extend(new);
            normalise_schedule(list);
        }
    }

    pub fn clear_epg(&mut self) {
        self.epg.clear();
        self.epg_channels.clear();
        self.epg_by_name.clear();
    }

    pub fn epg_programme_count(&self) -> usize {
        self.epg.values().map(Vec::len).sum()
    }

    /// The guide key for an item: its `epg_id`, else a display-name match.
    pub fn epg_key_for(&self, item: &Item) -> Option<String> {
        let k = epg_key(&item.epg_id);
        if !k.is_empty() && self.epg.contains_key(&k) {
            return Some(k);
        }
        let by_name = |n: &str| self.epg_by_name.get(&name_key(n)).cloned();
        let own = Self::item_epg_key(item.id);
        item.extra("tvg_name")
            .and_then(by_name)
            .or_else(|| by_name(&item.name))
            .or_else(|| self.epg.contains_key(&own).then_some(own))
            .or((!k.is_empty()).then_some(k))
    }

    /// Guide key for programmes attached directly to one item (e.g. Xtream
    /// short EPG for a channel without an `epg_channel_id`).
    pub fn item_epg_key(id: ItemId) -> String {
        format!("item:{id}")
    }

    fn schedule(&self, item: ItemId) -> &[Programme] {
        self.item(item)
            .and_then(|it| self.epg_key_for(it))
            .and_then(|k| self.epg.get(&k))
            .map_or(&[], Vec::as_slice)
    }

    /// `(now, next)` for a channel at time `t`.
    pub fn now_next(&self, item: ItemId, t: i64) -> (Option<&Programme>, Option<&Programme>) {
        let s = self.schedule(item);
        // First programme that has not ended yet.
        let i = s.partition_point(|p| p.stop <= t);
        match s.get(i) {
            Some(p) if p.start <= t => (Some(p), s.get(i + 1)),
            Some(p) => (None, Some(p)), // gap in the schedule
            None => (None, None),
        }
    }

    /// Programmes overlapping `[from, to)`, for the guide grid.
    pub fn guide(&self, item: ItemId, from: i64, to: i64) -> &[Programme] {
        let s = self.schedule(item);
        let a = s.partition_point(|p| p.stop <= from);
        let b = s.partition_point(|p| p.start < to);
        &s[a..b.max(a)]
    }

    // ---------------------------------------------------------- user state

    pub fn set_favorite(&mut self, id: ItemId, on: bool, now: i64) {
        if on {
            self.favorites.entry(id).or_insert(now);
        } else {
            self.favorites.remove(&id);
        }
    }

    pub fn is_favorite(&self, id: ItemId) -> bool {
        self.favorites.contains_key(&id)
    }

    /// Favorites that still exist in the catalog, most recently added first.
    pub fn favorites(&self, kind: Option<ItemKind>) -> Vec<&Item> {
        let mut v: Vec<(i64, &Item)> = self
            .favorites
            .iter()
            .filter_map(|(id, t)| self.item(*id).map(|i| (*t, i)))
            .filter(|(_, i)| kind.is_none_or(|k| k == i.kind))
            .collect();
        v.sort_by_key(|(t, _)| -t);
        v.into_iter().map(|(_, i)| i).collect()
    }

    pub fn set_progress(&mut self, id: ItemId, position: f64, duration: f64, now: i64) {
        self.progress.insert(
            id,
            Progress {
                position: position.max(0.0),
                duration: duration.max(0.0),
                updated_at: now,
            },
        );
    }

    pub fn progress(&self, id: ItemId) -> Option<Progress> {
        self.progress.get(&id).copied()
    }

    /// Partially watched films/episodes, most recent first (2%–95% watched).
    pub fn continue_watching(&self, limit: usize) -> Vec<(&Item, Progress)> {
        let mut v: Vec<(&Item, Progress)> = self
            .progress
            .iter()
            .filter(|(_, p)| p.duration > 0.0 && (0.02..0.95).contains(&(p.position / p.duration)))
            .filter_map(|(id, p)| self.item(*id).map(|i| (i, *p)))
            .collect();
        v.sort_by_key(|(_, p)| -p.updated_at);
        v.truncate(limit);
        v
    }

    /// Record that an item was played (for "recent channels").
    pub fn record_play(&mut self, id: ItemId, now: i64) {
        self.history.retain(|(i, _)| *i != id);
        self.history.insert(0, (id, now));
        self.history.truncate(HISTORY_CAP);
    }

    /// Forget one item from "recently watched" (its resume position stays).
    pub fn remove_from_history(&mut self, id: ItemId) {
        self.history.retain(|(i, _)| *i != id);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// When an item was last played (Unix seconds).
    pub fn last_played(&self, id: ItemId) -> Option<i64> {
        self.history.iter().find(|(i, _)| *i == id).map(|(_, t)| *t)
    }

    pub fn recent(&self, kind: Option<ItemKind>, limit: usize) -> Vec<&Item> {
        self.history
            .iter()
            .filter_map(|(id, _)| self.item(*id))
            .filter(|i| kind.is_none_or(|k| k == i.kind))
            .take(limit)
            .collect()
    }

    /// Favorites, progress and history as JSON — the part worth syncing or
    /// backing up. IDs are strings because u64 does not fit a JSON double.
    pub fn export_user_state(&self) -> Value {
        let fav: Vec<Value> = self
            .favorites
            .iter()
            .map(|(id, t)| obj([("id", id.to_string().into()), ("at", (*t).into())]))
            .collect();
        let prog: Vec<Value> = self
            .progress
            .iter()
            .map(|(id, p)| {
                obj([
                    ("id", id.to_string().into()),
                    ("pos", p.position.into()),
                    ("dur", p.duration.into()),
                    ("at", p.updated_at.into()),
                ])
            })
            .collect();
        let hist: Vec<Value> = self
            .history
            .iter()
            .map(|(id, t)| obj([("id", id.to_string().into()), ("at", (*t).into())]))
            .collect();
        obj([
            ("version", 1i64.into()),
            ("favorites", fav.into()),
            ("progress", prog.into()),
            ("history", hist.into()),
        ])
    }

    /// Merge user state exported by [`Self::export_user_state`] (newest wins).
    pub fn import_user_state(&mut self, v: &Value) -> Result<()> {
        let id_of = |e: &Value| e.text("id").and_then(|s| s.parse::<u64>().ok());
        for e in v.get("favorites").and_then(Value::as_array).unwrap_or(&[]) {
            if let Some(id) = id_of(e) {
                self.favorites.entry(id).or_insert(e.int("at").unwrap_or(0));
            }
        }
        for e in v.get("progress").and_then(Value::as_array).unwrap_or(&[]) {
            if let Some(id) = id_of(e) {
                let p = Progress {
                    position: e.num("pos").unwrap_or(0.0),
                    duration: e.num("dur").unwrap_or(0.0),
                    updated_at: e.int("at").unwrap_or(0),
                };
                if self
                    .progress
                    .get(&id)
                    .is_none_or(|old| old.updated_at < p.updated_at)
                {
                    self.progress.insert(id, p);
                }
            }
        }
        let mut hist: Vec<(ItemId, i64)> = v
            .get("history")
            .and_then(Value::as_array)
            .unwrap_or(&[])
            .iter()
            .filter_map(|e| Some((id_of(e)?, e.int("at").unwrap_or(0))))
            .chain(self.history.iter().copied())
            .collect();
        hist.sort_by_key(|(_, t)| -t);
        let mut seen = HashSet::new();
        hist.retain(|(id, _)| seen.insert(*id));
        hist.truncate(HISTORY_CAP);
        self.history = hist;
        Ok(())
    }

    // ------------------------------------------------------------ snapshot

    /// Write sources, items and user state as JSON Lines. EPG is excluded
    /// (it goes stale within days; re-download it instead).
    pub fn save_snapshot<W: Write>(&self, w: W) -> Result<()> {
        self.save_snapshot_with(w, false)
    }

    /// Like [`Catalog::save_snapshot`]; with `redact_passwords` the Xtream
    /// passwords are written empty so a host can keep them in an OS key store.
    pub fn save_snapshot_with<W: Write>(&self, mut w: W, redact_passwords: bool) -> Result<()> {
        let mut line = String::new();
        let mut emit = |w: &mut W, v: Value| -> Result<()> {
            line.clear();
            v.write_to(&mut line);
            line.push('\n');
            w.write_all(line.as_bytes()).map_err(Error::Io)
        };
        emit(
            &mut w,
            obj([
                ("t", "header".into()),
                ("version", 1i64.into()),
                ("next_source", self.next_source.into()),
            ]),
        )?;
        for s in self.sources.values() {
            let mut v = s.to_json();
            if let (Value::Object(m), SourceKind::Xtream(c)) = (&mut v, &s.kind) {
                let pw = if redact_passwords {
                    ""
                } else {
                    c.password.as_str()
                };
                m.insert("password".into(), pw.into());
                m.insert("live_ext".into(), c.live_ext.as_str().into());
            }
            if let Value::Object(m) = &mut v {
                m.insert("t".into(), "source".into());
            }
            emit(&mut w, v)?;
        }
        for it in &self.items {
            let mut v = it.to_json();
            if let Value::Object(m) = &mut v {
                m.insert("t".into(), "item".into());
                m.insert("order".into(), it.order.into());
            }
            emit(&mut w, v)?;
        }
        let mut u = self.export_user_state();
        if let Value::Object(m) = &mut u {
            m.insert("t".into(), "user".into());
        }
        emit(&mut w, u)?;
        w.flush()?;
        Ok(())
    }

    /// Restore a catalog written by [`Self::save_snapshot`].
    pub fn load_snapshot<R: BufRead>(r: R) -> Result<Catalog> {
        let mut c = Catalog::new();
        for (n, line) in r.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let v =
                json::parse(&line).map_err(|e| Error::parse("snapshot", n + 1, e.to_string()))?;
            match v.text("t").as_deref() {
                Some("header") => c.next_source = v.int("next_source").unwrap_or(1) as SourceId,
                Some("source") => {
                    let id = v
                        .int("id")
                        .ok_or_else(|| Error::parse("snapshot", n + 1, "source without id"))?
                        as SourceId;
                    let kind = if v.text("kind").as_deref() == Some("xtream") {
                        let mut cr = XtreamCredentials::new(
                            &v.text("location").unwrap_or_default(),
                            &v.text("username").unwrap_or_default(),
                            &v.text("password").unwrap_or_default(),
                        );
                        cr.live_ext = v.text("live_ext").unwrap_or_else(|| "ts".into());
                        SourceKind::Xtream(cr)
                    } else {
                        SourceKind::M3u {
                            location: v.text("location").unwrap_or_default(),
                        }
                    };
                    let epg_urls = v
                        .get("epg_urls")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().filter_map(Value::to_text).collect())
                        .unwrap_or_default();
                    c.sources.insert(
                        id,
                        Source {
                            id,
                            name: v.text("name").unwrap_or_default(),
                            kind,
                            epg_urls,
                            updated_at: v.int("updated_at").unwrap_or(0),
                        },
                    );
                    c.next_source = c.next_source.max(id + 1);
                }
                Some("item") => {
                    let kind = v
                        .text("kind")
                        .and_then(|k| ItemKind::parse(&k))
                        .unwrap_or(ItemKind::Live);
                    let source = v.int("source").unwrap_or(0) as SourceId;
                    let mut it = Item::new(
                        source,
                        kind,
                        v.text("remote_id").unwrap_or_default(),
                        v.text("name").unwrap_or_default(),
                    );
                    it.group = v.text("group").unwrap_or_default();
                    it.logo = v.text("logo").unwrap_or_default();
                    it.url = v.text("url").unwrap_or_default();
                    it.epg_id = v.text("epg_id").unwrap_or_default();
                    it.archive_days = v.int("archive_days").unwrap_or(0) as u32;
                    it.order = v.int("order").unwrap_or(0) as u32;
                    if let Some(m) = v.get("extra").and_then(Value::as_object) {
                        it.extra = m
                            .iter()
                            .filter_map(|(k, v)| Some((k.clone(), v.to_text()?)))
                            .collect();
                    }
                    c.items.push(it);
                }
                Some("user") => c.import_user_state(&v)?,
                _ => {}
            }
        }
        c.reindex();
        Ok(c)
    }
}

/// Sort, drop exact duplicates, fill missing stop times from the next
/// programme, and trim overlaps (later programme wins its start time).
fn normalise_schedule(list: &mut Vec<Programme>) {
    list.sort_by(|a, b| a.start.cmp(&b.start).then(b.stop.cmp(&a.stop)));
    list.dedup_by(|b, a| a.start == b.start); // keep the first (longest) of same start
    for i in 0..list.len() {
        let next_start = list.get(i + 1).map(|n| n.start);
        let p = &mut list[i];
        if p.stop == 0 {
            p.stop = next_start.unwrap_or(p.start + 3600);
        }
        if let Some(ns) = next_start {
            if p.stop > ns {
                p.stop = ns;
            }
        }
    }
    list.retain(|p| p.stop > p.start);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xtream::tests::{client, fake_panel};

    const M3U: &str = "#EXTM3U url-tvg=\"http://g/x.xml\"\n\
#EXTINF:-1 tvg-id=\"BBC1.uk\" group-title=\"UK\",BBC One HD\nhttp://s/1.ts\n\
#EXTINF:-1 tvg-id=\"\" tvg-name=\"Télé Matin\" group-title=\"FR\",Télé Matin\nhttp://s/2.ts\n\
#EXTINF:-1 group-title=\"UK\",BBC Two\nhttp://s/3.ts\n\
#EXTINF:-1 group-title=\"UK\",BBC One HD\nhttp://s/1.ts\n\
#EXTINF:-1 group-title=\"Films\",Heat (1995)\nhttp://s/heat.mkv\n";

    const GUIDE: &str = r#"<tv>
<channel id="bbc1.uk"><display-name>BBC One</display-name></channel>
<channel id="tm.fr"><display-name>Télé Matin</display-name></channel>
<programme start="20261012100000 +0000" stop="20261012110000 +0000" channel="BBC1.uk"><title>Morning</title></programme>
<programme start="20261012110000 +0000" stop="20261012113000 +0000" channel="bbc1.uk"><title>News</title></programme>
<programme start="20261012113000 +0000" channel="bbc1.uk"><title>Open end</title></programme>
<programme start="20261012120000 +0000" stop="20261012130000 +0000" channel="bbc1.uk"><title>Lunch</title></programme>
<programme start="20261001000000 +0000" stop="20261001010000 +0000" channel="bbc1.uk"><title>Old</title></programme>
<programme start="20261012090000 +0000" stop="20261012120000 +0000" channel="tm.fr"><title>Matin</title></programme>
</tv>"#;

    fn t(h: u32, m: u32) -> i64 {
        crate::time::unix_from_parts(2026, 10, 12, h, m, 0)
    }

    fn sample() -> (Catalog, SourceId) {
        let mut c = Catalog::new();
        let s = c.add_source(
            "Test",
            SourceKind::M3u {
                location: "mem".into(),
            },
        );
        let st = c.import_m3u(s, M3U.as_bytes()).unwrap();
        assert_eq!(st.items, 5);
        assert_eq!(st.epg_urls, ["http://g/x.xml"]);
        let e = c.import_xmltv(GUIDE.as_bytes(), t(0, 0)).unwrap();
        assert_eq!(e.channels, 2);
        (c, s)
    }

    #[test]
    fn m3u_import_and_browse() {
        let (c, s) = sample();
        assert_eq!(c.len(), 5, "duplicate URL kept as separate entry");
        let g = c.groups(s, ItemKind::Live);
        assert_eq!(
            g.iter()
                .map(|g| (g.name.as_str(), g.count))
                .collect::<Vec<_>>(),
            [("UK", 3), ("FR", 1)]
        );
        assert_eq!(c.groups(s, ItemKind::Movie)[0].name, "Films");
        let uk = c.list(s, ItemKind::Live, Some("UK"), 1, 10);
        assert_eq!(uk[0].name, "BBC Two");
    }

    #[test]
    fn search_ranks_and_folds() {
        let (c, _) = sample();
        let r = c.search("tele", SearchFilter::default(), 10);
        assert_eq!(r[0].name, "Télé Matin");
        let r = c.search("bbc o", SearchFilter::default(), 10);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|i| i.name == "BBC One HD"));
        let r = c.search(
            "bbc",
            SearchFilter {
                kind: Some(ItemKind::Movie),
                source: None,
            },
            10,
        );
        assert!(r.is_empty());
        assert!(c.search("zzz", SearchFilter::default(), 10).is_empty());
        assert!(c.search("   ", SearchFilter::default(), 10).is_empty());
    }

    #[test]
    fn epg_now_next_and_grid() {
        let (c, s) = sample();
        let bbc = c.list(s, ItemKind::Live, Some("UK"), 0, 1)[0].id;
        let (now, next) = c.now_next(bbc, t(10, 30));
        assert_eq!(now.unwrap().title, "Morning");
        assert_eq!(next.unwrap().title, "News");
        // Missing stop filled from the next programme (12:00).
        let (now, _) = c.now_next(bbc, t(11, 45));
        assert_eq!(now.unwrap().title, "Open end");
        assert_eq!(now.unwrap().stop, t(12, 0));
        // Old programme dropped by keep_from.
        assert_eq!(c.guide(bbc, 0, i64::MAX).len(), 4);
        assert_eq!(c.guide(bbc, t(11, 15), t(12, 1)).len(), 3);
        // Name-based EPG match for the channel without tvg-id (accent folded).
        let tm = c.search("matin", SearchFilter::default(), 1)[0].id;
        assert_eq!(c.now_next(tm, t(10, 0)).0.unwrap().title, "Matin");
        // After the schedule ends.
        assert_eq!(c.now_next(bbc, t(23, 0)), (None, None));
    }

    #[test]
    fn refresh_keeps_user_state() {
        let (mut c, s) = sample();
        let bbc2 = c.search("bbc two", SearchFilter::default(), 1)[0].id;
        c.set_favorite(bbc2, true, 100);
        let heat = c.search("heat", SearchFilter::default(), 1)[0].id;
        c.set_progress(heat, 600.0, 6000.0, 200);
        c.import_m3u(s, M3U.as_bytes()).unwrap();
        assert_eq!(c.favorites(None)[0].id, bbc2);
        assert_eq!(c.continue_watching(5)[0].0.id, heat);
        // Removing the channel from the playlist hides (but keeps) the favorite.
        c.import_m3u(s, "#EXTM3U\n#EXTINF:-1,Other\nhttp://o\n".as_bytes())
            .unwrap();
        assert!(c.favorites(None).is_empty());
        assert!(c.is_favorite(bbc2));
    }

    #[test]
    fn xtream_into_catalog_with_episodes() {
        let mut c = Catalog::new();
        let xc = client();
        let s = c.add_source("Panel", SourceKind::Xtream(xc.creds.clone()));
        let xc = crate::xtream::XtreamClient::new(xc.creds, s);
        c.apply_xtream(s, xc.sync(&fake_panel()).unwrap()).unwrap();
        assert_eq!(c.len(), 4);
        let show = c.search("show", SearchFilter::default(), 1)[0].clone();
        let body = fake_panel()("action=get_series_info", &[]).unwrap();
        c.set_episodes(show.id, xc.parse_series_info(&body, &show).unwrap())
            .unwrap();
        assert_eq!(c.episodes(show.id).len(), 2);
        assert_eq!(c.len(), 6);
        // Episodes are not in search results, and survive a catalog re-sync.
        assert!(c.search("pilot", SearchFilter::default(), 5).is_empty());
        c.apply_xtream(s, xc.sync(&fake_panel()).unwrap()).unwrap();
        assert_eq!(c.episodes(show.id).len(), 2);
        assert!(c.sources()[0].epg_fetch_urls()[0].contains("/xmltv.php?username=u"));
        assert!(!c.sources()[0].to_json().to_string().contains("p%26x"));
    }

    #[test]
    fn query_sorts_filters_pages() {
        let mut c = Catalog::new();
        let xc = client();
        let s = c.add_source("Panel", SourceKind::Xtream(xc.creds.clone()));
        let mut items = Vec::new();
        for (i, (name, rating, added, group)) in [
            ("Beta", "7.5", "300", "A"),
            ("alpha", "9.1", "100", "A"),
            ("Gamma", "", "200", "B"),
            ("delta", "5", "", "A"),
        ]
        .into_iter()
        .enumerate()
        {
            let mut it = Item::new(s, ItemKind::Movie, i.to_string(), name);
            it.group = group.into();
            it.order = i as u32;
            items.push(
                it.with_extra("rating", Some(rating.into()))
                    .with_extra("added", Some(added.into())),
            );
        }
        c.apply_xtream(
            s,
            XtreamSnapshot {
                items,
                ..Default::default()
            },
        )
        .unwrap();
        let names = |q: &ListQuery| {
            c.query(q)
                .1
                .iter()
                .map(|i| i.name.clone())
                .collect::<Vec<_>>()
        };
        let mut q = ListQuery::new(ItemKind::Movie);
        assert_eq!(names(&q), ["Beta", "alpha", "Gamma", "delta"]);
        q.sort = Sort::NameAsc;
        assert_eq!(names(&q), ["alpha", "Beta", "delta", "Gamma"]);
        q.sort = Sort::AddedDesc;
        assert_eq!(names(&q), ["Beta", "Gamma", "alpha", "delta"]);
        q.sort = Sort::RatingDesc;
        q.min_rating = 7.0;
        assert_eq!(names(&q), ["alpha", "Beta"]);
        q.min_rating = 0.0;
        q.hidden_groups.insert("A".into());
        assert_eq!(names(&q), ["Gamma"]);
        q.hidden_groups.clear();
        q.offset = 1;
        q.limit = 2;
        let (total, page) = c.query(&q);
        assert_eq!((total, page.len()), (4, 2));
    }

    #[test]
    fn update_source_keeps_items() {
        let (mut c, s) = sample();
        let n = c.len();
        c.update_source(
            s,
            SourceKind::M3u {
                location: "http://new/list.m3u".into(),
            },
        )
        .unwrap();
        assert_eq!(c.len(), n);
        let SourceKind::M3u { location } = &c.source(s).unwrap().kind else {
            panic!()
        };
        assert_eq!(location, "http://new/list.m3u");
        assert!(
            c.update_source(s, SourceKind::Xtream(XtreamCredentials::new("h", "u", "p")))
                .is_err()
        );
    }

    #[test]
    fn tags_from_a_second_playlist_group_by_country() {
        let mut c = Catalog::new();
        let s = c.add_source(
            "Free",
            SourceKind::M3u {
                location: "mem".into(),
            },
        );
        let cats = "#EXTM3U\n#EXTINF:-1 group-title=\"News\",A\nhttp://a\n\
#EXTINF:-1 group-title=\"Kids\",B\nhttp://b\n#EXTINF:-1 group-title=\"News\",C\nhttp://c\n\
#EXTINF:-1 group-title=\"Kids\",A\nhttp://a\n";
        c.import_m3u(s, cats.as_bytes()).unwrap();
        let countries = "#EXTM3U\n#EXTINF:-1 group-title=\"Nepal\",A\nhttp://a\n\
#EXTINF:-1 group-title=\"India\",A\nhttp://a\n#EXTINF:-1 group-title=\"India\",B\nhttp://b\n";
        assert_eq!(
            c.tag_from_m3u(s, "country", countries.as_bytes()).unwrap(),
            3
        );
        let g = c.groups_by(s, ItemKind::Live, Some("country"));
        let names: Vec<_> = g.iter().map(|g| (g.name.as_str(), g.count)).collect();
        assert_eq!(names, [("Nepal", 1), ("India", 2)]);
        let mut q = ListQuery::new(ItemKind::Live);
        q.group = Some("India".into());
        q.group_field = Some("country".into());
        let (n, v) = c.query(&q);
        assert_eq!(n, 2);
        assert_eq!(v[0].name, "A");
        q.group = None;
        q.hidden_groups = ["India".to_string()].into();
        let (n, _) = c.query(&q);
        assert_eq!(
            n, 2,
            "A (Nepal too) and untagged C stay; B (India only) goes"
        );
        // Re-tagging replaces the old values.
        c.tag_from_m3u(s, "country", "#EXTM3U\n".as_bytes())
            .unwrap();
        assert!(c.groups_by(s, ItemKind::Live, Some("country")).is_empty());
    }

    #[test]
    fn snapshot_can_redact_passwords() {
        let mut c = Catalog::new();
        let id = c.add_source(
            "X",
            SourceKind::Xtream(XtreamCredentials::new("http://h", "u", "topsecret")),
        );
        let _ = id;
        let mut buf = Vec::new();
        c.save_snapshot_with(&mut buf, true).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(!text.contains("topsecret"));
        let mut plain = Vec::new();
        c.save_snapshot(&mut plain).unwrap();
        assert!(String::from_utf8(plain).unwrap().contains("topsecret"));
    }

    #[test]
    fn snapshot_roundtrip() {
        let (mut c, _) = sample();
        let id = c.search("heat", SearchFilter::default(), 1)[0].id;
        c.set_favorite(id, true, 5);
        c.record_play(id, 6);
        c.add_source(
            "X",
            SourceKind::Xtream(XtreamCredentials::new("h", "u", "secret")),
        );
        let mut buf = Vec::new();
        c.save_snapshot(&mut buf).unwrap();
        let d = Catalog::load_snapshot(&buf[..]).unwrap();
        assert_eq!(d.len(), c.len());
        assert_eq!(d.item(id), c.item(id));
        assert!(d.is_favorite(id));
        assert_eq!(d.recent(None, 1)[0].id, id);
        let SourceKind::Xtream(cr) = &d.sources()[1].kind else {
            panic!()
        };
        assert_eq!(cr.password, "secret");
        // The UI-facing JSON never contains the password.
        assert!(!d.sources()[1].to_json().to_string().contains("secret"));
        let mut d = d;
        assert_eq!(
            d.add_source(
                "Y",
                SourceKind::M3u {
                    location: String::new()
                }
            ),
            3
        );
    }

    #[test]
    fn user_state_merge() {
        let (mut a, _) = sample();
        a.set_progress(1, 10.0, 100.0, 50);
        a.set_favorite(2, true, 1);
        let exported = a.export_user_state();
        let mut b = Catalog::new();
        b.set_progress(1, 90.0, 100.0, 99);
        b.import_user_state(&json::parse(&exported.to_string()).unwrap())
            .unwrap();
        assert_eq!(
            b.progress(1).unwrap().position,
            90.0,
            "newer local value wins"
        );
        assert!(b.is_favorite(2));
    }
}
