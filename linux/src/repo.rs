//! All I/O for the UI, the Linux twin of Repository.cs: the engine parses, indexes and
//! searches; this module downloads bytes and turns JSON into plain structs. Every method
//! blocks, so the UI calls them through `ui::bg` (a worker thread).

use crate::engine::{Engine, EngineError};
use crate::freetv;
use crate::http::{self, AppError, Res};
use crate::paths;
use crate::secrets;
use crate::settings;
use chrono::{Local, TimeZone};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, SystemTime};

// ================================================================== models

#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub username: String,
    pub updated_at: i64,
}

impl Source {
    pub fn kind_label(&self) -> &'static str {
        if self.kind == "xtream" {
            "Xtream Code"
        } else {
            "M3U playlist"
        }
    }
}

#[derive(Clone, Debug)]
pub struct AccountInfo {
    pub status: String,
    pub expires: Option<i64>,
    pub max_connections: i64,
    pub active_connections: i64,
    pub is_trial: bool,
    pub created: Option<i64>,
    pub timezone: String,
    pub formats: String,
}

#[derive(Clone, Debug, Default)]
pub struct DetailsInfo {
    pub plot: Option<String>,
    pub cast: Option<String>,
    pub director: Option<String>,
    pub genre: Option<String>,
    pub released: Option<String>,
    pub duration: Option<String>,
    pub rating: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    /// YouTube video id.
    pub trailer: Option<String>,
    pub imdb_id: Option<String>,
    /// Age rating from the provider (MPAA / age / certification), for parental controls.
    pub age: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub name: String,
    pub count: i64,
}

#[derive(Clone, Debug)]
pub struct Programme {
    pub title: String,
    pub start: i64,
    pub stop: i64,
    pub desc: String,
}

impl Programme {
    pub fn progress(&self, now: i64) -> f64 {
        if self.stop <= self.start {
            0.0
        } else {
            ((now - self.start) as f64 / (self.stop - self.start) as f64).clamp(0.0, 1.0)
        }
    }
    pub fn is_live(&self, now: i64) -> bool {
        self.start <= now && now < self.stop
    }
    pub fn is_past(&self, now: i64) -> bool {
        self.stop <= now
    }
    pub fn time_range(&self) -> String {
        format!("{} – {}", clock(self.start), clock(self.stop))
    }
}

/// A channel, film, series or episode.
#[derive(Clone, Debug, Default)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub group: String,
    pub logo: Option<String>,
    pub url: String,
    pub user_agent: Option<String>,
    pub referrer: Option<String>,
    pub plot: Option<String>,
    pub rating: Option<String>,
    pub year: Option<String>,
    pub genre: Option<String>,
    pub season: Option<String>,
    pub episode: Option<String>,
    pub source_id: i64,
    pub series_id: Option<String>,
    pub added_at: i64,
    pub played_at: i64,
    pub position: f64,
    pub duration: f64,
    /// From the provider (episodes).
    pub duration_hint: f64,
}

static PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:[A-Z0-9]{2,4})(?:[ \-]?[A-Z]{2,3})?\s*[|:\-–]\s+").unwrap()
});

impl Item {
    pub fn is_live(&self) -> bool {
        self.kind == "live"
    }
    pub fn is_series(&self) -> bool {
        self.kind == "series"
    }
    pub fn watched_ratio(&self) -> f64 {
        if self.duration > 0.0 {
            (self.position / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
    pub fn is_watched(&self) -> bool {
        self.watched_ratio() >= 0.95
    }
    pub fn has_rating(&self) -> bool {
        self.rating
            .as_deref()
            .is_some_and(|r| !r.is_empty() && r != "0")
    }
    pub fn type_badge(&self) -> &'static str {
        match self.kind.as_str() {
            "live" => "LIVE",
            "series" => "SERIES",
            "episode" => "EPISODE",
            _ => "MOVIE",
        }
    }
    pub fn time_left_text(&self) -> String {
        if self.duration > 0.0 {
            format!("{} left", format_duration(self.duration - self.position))
        } else {
            String::new()
        }
    }
    pub fn added_text(&self) -> String {
        if self.added_at > 0 {
            local(self.added_at, "%-d %b, %H:%M")
        } else {
            String::new()
        }
    }
    pub fn played_text(&self) -> String {
        if self.played_at > 0 {
            local(self.played_at, "%-d %b, %H:%M")
        } else {
            String::new()
        }
    }
    pub fn duration_text(&self) -> String {
        if self.duration_hint > 0.0 {
            format_clock(self.duration_hint)
        } else {
            String::new()
        }
    }
    /// "CA EN: CBC Calgary" → "CBC Calgary" when Settings › Strip country prefixes is on.
    pub fn display_name(&self) -> String {
        if self.is_live() && settings::get().strip_prefixes {
            strip_prefix(&self.name)
        } else {
            self.name.clone()
        }
    }
    pub fn initials(&self) -> String {
        self.display_name()
            .chars()
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }
    /// Same item with another picture (episodes shown with their series poster).
    pub fn with_logo(&self, logo: Option<String>) -> Item {
        Item {
            logo,
            ..self.clone()
        }
    }
    /// "S01E03".
    pub fn se(&self) -> String {
        let s = self
            .season
            .as_deref()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(1);
        let e = self
            .episode
            .as_deref()
            .and_then(|e| e.parse::<i32>().ok())
            .unwrap_or(0);
        format!("S{s:02}E{e:02}")
    }
}

pub fn strip_prefix(name: &str) -> String {
    match PREFIX.find(name) {
        Some(m) if m.end() < name.len().saturating_sub(1) => name[m.end()..].to_string(),
        _ => name.to_string(),
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn local(t: i64, fmt: &str) -> String {
    Local
        .timestamp_opt(t, 0)
        .single()
        .map(|d| d.format(fmt).to_string())
        .unwrap_or_default()
}

pub fn clock(t: i64) -> String {
    local(t, "%H:%M")
}

pub fn format_duration(seconds: f64) -> String {
    let s = seconds.max(0.0) as i64;
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h >= 1 {
        format!("{h}h {m}m")
    } else {
        format!("{}m", m.max(1))
    }
}

pub fn format_clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as i64;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h >= 1 {
        format!("{h:02}:{m:02}:{sec:02}")
    } else {
        format!("{m:02}:{sec:02}")
    }
}

/// 53,324 → "53K", 950 → "950".
pub fn compact(n: i64) -> String {
    if n >= 10_000 {
        format!("{}K", group_digits(n / 1000))
    } else {
        group_digits(n)
    }
}

/// A step of loading a playlist, for the "Adding playlist" pop-up.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    SignIn,
    Live,
    Movies,
    Series,
    Download,
    Read,
    Save,
    Guide,
    Pages,
}

/// Where a playlist load is: the step and a detail (the size downloaded so far). `done` marks
/// a finished step's result, such as how many movies came in, kept on its line.
#[derive(Clone, Debug)]
pub struct LoadStatus {
    pub step: Step,
    pub detail: String,
    pub done: bool,
}

impl LoadStatus {
    pub fn new(step: Step, detail: &str) -> Self {
        LoadStatus { step, detail: detail.to_string(), done: false }
    }
}

/// Told about each step of a load; called on the worker thread.
pub type Progress<'a> = &'a (dyn Fn(LoadStatus) + Sync);

/// "12.4 MB", "640 KB".
pub fn size(bytes: u64) -> String {
    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} KB", (bytes / 1024).max(1))
    }
}

pub fn group_digits(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

// ================================================================== JSON helpers

fn s(e: &Value, key: &str) -> Option<String> {
    match e.get(key)? {
        Value::String(v) if !v.is_empty() => Some(v.clone()),
        _ => None,
    }
}

fn int(e: &Value, key: &str) -> i64 {
    match e.get(key) {
        Some(Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .unwrap_or(0),
        Some(Value::String(v)) => v.trim().parse().unwrap_or(0),
        _ => 0,
    }
}

fn num(e: &Value, key: &str) -> f64 {
    match e.get(key) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(v)) => v.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// "7.5" stays, "72" becomes "7.2", junk like "7125" is dropped (same rule as the core's sort).
fn clean_rating(raw: Option<String>) -> Option<String> {
    let mut r: f64 = raw?.replace(',', ".").trim().parse().ok()?;
    if r > 10.0 && r <= 100.0 {
        r /= 10.0;
    }
    if r <= 0.0 || r > 10.0 {
        return None;
    }
    let rounded = (r * 10.0).round() / 10.0;
    Some(if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    })
}

pub fn parse_item(o: &Value) -> Item {
    let extra = o.get("extra").cloned().unwrap_or(Value::Null);
    let progress = o.get("progress").cloned().unwrap_or(Value::Null);
    let mut year = s(&extra, "year");
    if let Some(y) = &year {
        if y.len() > 4 {
            year = Some(y.chars().take(4).collect()); // "2024-05-01" → "2024"
        }
    }
    let added = int(&extra, "added");
    let hint = num(&extra, "duration_secs");
    Item {
        id: s(o, "id").unwrap_or_default(),
        kind: s(o, "kind").unwrap_or_else(|| "live".into()),
        name: s(o, "name").unwrap_or_default(),
        group: s(o, "group").unwrap_or_default(),
        logo: s(o, "logo"),
        url: s(o, "url").unwrap_or_default(),
        user_agent: s(&extra, "user_agent"),
        referrer: s(&extra, "referrer"),
        plot: s(&extra, "plot"),
        rating: clean_rating(s(&extra, "rating")),
        year,
        genre: s(&extra, "genre"),
        season: s(&extra, "season"),
        episode: s(&extra, "episode"),
        source_id: int(o, "source"),
        series_id: s(o, "series"),
        added_at: if added > 0 {
            added
        } else {
            int(&extra, "last_modified")
        },
        played_at: int(o, "played_at"),
        position: num(&progress, "position"),
        duration: num(&progress, "duration"),
        duration_hint: if hint > 0.0 {
            hint
        } else {
            num(&extra, "duration")
        },
    }
}

fn items(arr: &Value) -> Vec<Item> {
    arr.as_array()
        .map(|a| a.iter().map(parse_item).collect())
        .unwrap_or_default()
}

fn offset() -> i64 {
    settings::get().epg_offset_minutes as i64 * 60
}

fn parse_programme(p: &Value) -> Programme {
    let off = offset();
    Programme {
        title: s(p, "title").unwrap_or_default(),
        start: int(p, "start") + off,
        stop: int(p, "stop") + off,
        desc: s(p, "desc").unwrap_or_default(),
    }
}

static TRAILER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:v=|youtu\.be/|embed/)([\w-]{11})").unwrap());
static IMDB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tt\d{5,10}").unwrap());

fn parse_details(info: &Value) -> DetailsInfo {
    let any = |keys: &[&str]| keys.iter().find_map(|k| s(info, k));
    let backdrop = match info.get("backdrop_path") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(Value::as_str)
            .find(|x| !x.is_empty())
            .map(str::to_string),
        Some(Value::String(v)) if !v.is_empty() => Some(v.clone()),
        _ => None,
    };
    let trailer = any(&["youtube_trailer", "trailer"]).and_then(|t| {
        TRAILER
            .captures(&t)
            .map(|c| c[1].to_string())
            .or_else(|| (t.len() == 11).then_some(t))
    });
    let secs = num(info, "duration_secs");
    let mut duration = if secs > 0.0 {
        Some(format_clock(secs))
    } else {
        any(&["duration", "episode_run_time"])
    };
    if let Some(d) = &duration {
        if let Ok(mins) = d.trim().parse::<i32>() {
            if mins <= 0 {
                // Providers send "0" for an unknown runtime: no chip.
                duration = None;
            } else if mins < 600 {
                duration = Some(format!("{mins} min"));
            }
        }
    }
    let rating = any(&["rating", "rating_5based"]).filter(|r| r != "0" && r != "0.0");
    let language = info
        .get("audio")
        .and_then(|a| s(a, "language"))
        .or_else(|| any(&["language"]));
    DetailsInfo {
        plot: any(&["plot", "description"]),
        cast: any(&["cast", "actors"]),
        director: any(&["director"]),
        genre: any(&["genre"]),
        released: any(&["releasedate", "releaseDate", "release_date"]),
        duration,
        rating,
        country: any(&["country"]),
        language,
        poster: any(&["movie_image", "cover_big", "cover"]),
        backdrop,
        trailer,
        imdb_id: any(&["imdb_id", "imdb"])
            .and_then(|i| IMDB.find(&i).map(|m| m.as_str().to_string())),
        age: any(&["rating_mpaa", "mpaa_rating", "mpaa", "age", "certification"])
            .and_then(|r| crate::parental::parse_age(&r)),
    }
}

// ================================================================== repository

pub struct Repo {
    core: Engine,
    save_lock: Mutex<()>,
    save_gen: AtomicU64,
    /// Plain passwords last written to the keyring, per source (avoids rewriting them).
    stored: Mutex<HashMap<i64, String>>,
    details: Mutex<HashMap<String, DetailsInfo>>,
    series_bodies: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

fn snapshot_path() -> PathBuf {
    paths::data_dir().join("catalog.jsonl")
}

/// Downloaded guides are kept here and re-imported at start: the snapshot excludes EPG.
fn epg_dir() -> PathBuf {
    paths::data_dir().join("epg")
}

fn epg_file(source: i64, index: usize) -> PathBuf {
    epg_dir().join(format!("source-{source}-{index}.xml"))
}

fn epg_files(source: i64) -> Vec<PathBuf> {
    let prefix = format!("source-{source}-");
    list_dir(&epg_dir())
        .into_iter()
        .filter(|p| file_name(p).starts_with(&prefix) && file_name(p).ends_with(".xml"))
        .collect()
}

fn custom_epg_file(index: usize) -> PathBuf {
    epg_dir().join(format!("custom-{index}.xml"))
}

fn list_dir(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default()
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn age(files: &[PathBuf]) -> Option<Duration> {
    files
        .iter()
        .filter_map(|f| f.metadata().ok()?.modified().ok())
        .min()
        .map(|t| SystemTime::now().duration_since(t).unwrap_or_default())
}

fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to).or_else(|_| {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)
    })
}

fn secret_key(source: i64) -> String {
    format!("xtream:{source}")
}

impl Repo {
    pub fn new() -> Repo {
        Repo {
            core: Engine::new(),
            save_lock: Mutex::new(()),
            save_gen: AtomicU64::new(0),
            stored: Mutex::new(HashMap::new()),
            details: Mutex::new(HashMap::new()),
            series_bodies: Mutex::new(HashMap::new()),
        }
    }

    fn call(&self, method: &str, args: Value) -> Result<Value, EngineError> {
        self.core.call(method, args)
    }

    fn user_agent(source: i64) -> Option<String> {
        settings::get()
            .source_user_agent
            .get(&source)
            .filter(|u| !u.trim().is_empty())
            .cloned()
    }

    // ---------------------------------------------------------- persistence

    /// Save a little later, once: favorites and watched marks change in bursts, and every
    /// save writes the whole catalog snapshot.
    pub fn save_soon(self: &Arc<Self>) {
        let generation = self.save_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let me = Arc::clone(self);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            if me.save_gen.load(Ordering::SeqCst) == generation {
                me.save();
            }
        });
    }

    pub fn save(&self) {
        let _g = self.save_lock.lock().unwrap_or_else(|p| p.into_inner());
        // Passwords go to the keyring (or the private secrets file), never into the catalog.
        let secrets_ok = match self.sync_secrets() {
            Ok(()) => true,
            Err(e) => {
                crate::log(&format!("secrets not saved: {e}"));
                false
            }
        };
        // Never drop a password from the catalog unless it is safely stored.
        if let Err(e) = self.call(
            "save_snapshot",
            json!({ "path": snapshot_path(), "redact_passwords": secrets_ok }),
        ) {
            crate::log(&format!("snapshot save failed: {e}"));
        }
    }

    fn sync_secrets(&self) -> Result<(), String> {
        let sources = self.call("sources", json!({})).map_err(|e| e.to_string())?;
        let mut stored = self.stored.lock().unwrap_or_else(|p| p.into_inner());
        for src in sources.as_array().into_iter().flatten() {
            if s(src, "kind").as_deref() != Some("xtream") {
                continue;
            }
            let id = int(src, "id");
            let pw = self
                .call("source_password", json!({ "source": id }))
                .map_err(|e| e.to_string())?;
            let plain = pw.as_str().unwrap_or("").to_string();
            if plain.is_empty() || stored.get(&id) == Some(&plain) {
                continue;
            }
            secrets::store(&secret_key(id), &plain);
            if secrets::lookup(&secret_key(id)).as_deref() != Some(plain.as_str()) {
                return Err(format!("password for source {id} could not be stored"));
            }
            stored.insert(id, plain);
        }
        Ok(())
    }

    /// After loading a (redacted) catalog, give the engine its passwords back.
    fn restore_secrets(&self) {
        let Ok(sources) = self.call("sources", json!({})) else {
            return;
        };
        let mut stored = self.stored.lock().unwrap_or_else(|p| p.into_inner());
        for src in sources.as_array().into_iter().flatten() {
            if s(src, "kind").as_deref() != Some("xtream") {
                continue;
            }
            let id = int(src, "id");
            let current = self
                .call("source_password", json!({ "source": id }))
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            if !current.is_empty() {
                continue; // an older, unredacted file: moved to the keyring on the next save
            }
            if let Some(plain) = secrets::lookup(&secret_key(id)).filter(|p| !p.is_empty()) {
                let _ = self.call("update_source", json!({ "source": id, "password": plain }));
                stored.insert(id, plain);
            }
        }
    }

    /// Load the cached catalog and guides (no network).
    pub fn init(&self) {
        let snap = snapshot_path();
        if snap.exists() {
            match self.call("load_snapshot", json!({ "path": snap })) {
                Ok(_) => {
                    self.restore_secrets();
                    let plain_left = std::fs::read_to_string(&snap)
                        .map(|t| {
                            t.lines().take(50).any(|l| {
                                l.contains("\"t\":\"source\"")
                                    && l.contains("\"kind\":\"xtream\"")
                                    && !l.contains("\"password\":\"\"")
                            })
                        })
                        .unwrap_or(false);
                    if plain_left {
                        self.save();
                    }
                }
                Err(e) => {
                    crate::log(&format!("snapshot unreadable, starting empty: {e}"));
                    let _ = std::fs::remove_file(&snap);
                }
            }
        }
        for f in list_dir(&epg_dir())
            .into_iter()
            .filter(|p| file_name(p).ends_with(".xml"))
        {
            if let Err(e) = self.call("import_xmltv_file", json!({ "path": f })) {
                crate::log(&format!("cached guide {} unreadable: {e}", file_name(&f)));
            }
        }
    }

    // ---------------------------------------------------------- sources

    pub fn epg_age(&self, source: i64) -> Option<Duration> {
        age(&epg_files(source))
    }

    pub fn has_epg(&self, source: i64) -> bool {
        self.call("epg_fetch_urls", json!({ "source": source }))
            .ok()
            .and_then(|v| v.as_array().map(|a| !a.is_empty()))
            .unwrap_or(false)
    }

    pub fn sources(&self) -> Res<Vec<Source>> {
        let v = self.call("sources", json!({}))?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .map(|x| Source {
                id: int(x, "id"),
                name: s(x, "name").unwrap_or_default(),
                kind: s(x, "kind").unwrap_or_else(|| "m3u".into()),
                location: s(x, "location").unwrap_or_default(),
                username: s(x, "username").unwrap_or_default(),
                updated_at: int(x, "updated_at"),
            })
            .collect())
    }

    fn source(&self, id: i64) -> Res<Source> {
        self.sources()?
            .into_iter()
            .find(|x| x.id == id)
            .ok_or_else(|| AppError::Other("That playlist no longer exists.".into()))
    }

    /// Change a source's name and connection details, then download it again.
    pub fn update_source(
        &self,
        id: i64,
        name: &str,
        location: Option<&str>,
        server: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Res<()> {
        let clean = |v: Option<&str>| {
            v.map(str::trim)
                .filter(|x| !x.is_empty())
                .map(str::to_string)
        };
        self.call(
            "update_source",
            json!({
                "source": id,
                "name": name,
                "location": clean(location),
                "server": clean(server),
                "username": clean(username),
                "password": password.filter(|p| !p.is_empty()),
            }),
        )?;
        self.refresh(id)
    }

    pub fn source_password(&self, source: i64) -> String {
        self.call("source_password", json!({ "source": source }))
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default()
    }

    pub fn account(&self, source: i64) -> Res<AccountInfo> {
        let url = self
            .call("account_url", json!({ "source": source }))?
            .as_str()
            .unwrap_or("")
            .to_string();
        let root: Value =
            serde_json::from_slice(&http::get_bytes(&url, Self::user_agent(source).as_deref())?)?;
        let u = root.get("user_info").ok_or_else(|| {
            AppError::Engine(EngineError {
                code: "api".into(),
                message: "The server did not return account information.".into(),
            })
        })?;
        let si = root.get("server_info").cloned().unwrap_or(Value::Null);
        let auth = match u.get("auth") {
            Some(Value::Number(n)) => n.as_i64() == Some(1),
            Some(Value::String(v)) => v == "1" || v == "true",
            Some(Value::Bool(b)) => *b,
            _ => false,
        };
        if !auth {
            return Err(AppError::Engine(EngineError {
                code: "api".into(),
                message: "Login rejected: check the username and password.".into(),
            }));
        }
        let formats = u
            .get("allowed_output_formats")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|x| {
                        x.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| x.to_string())
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let date = |t: i64| (t > 0).then_some(t);
        Ok(AccountInfo {
            status: s(u, "status").unwrap_or_default(),
            expires: date(int(u, "exp_date")),
            max_connections: int(u, "max_connections"),
            active_connections: int(u, "active_cons"),
            is_trial: s(u, "is_trial").as_deref() == Some("1"),
            created: date(int(u, "created_at")),
            timezone: s(&si, "timezone").unwrap_or_default(),
            formats,
        })
    }

    /// Quick reachability check: Xtream login, or the first bytes of an M3U.
    pub fn test_connection(&self, source: i64) -> Res<String> {
        let src = self.source(source)?;
        let start = std::time::Instant::now();
        if src.kind == "xtream" {
            let acc = self.account(source)?;
            let exp = acc
                .expires
                .map(|e| format!(", expires {}", local(e, "%-d %b %Y")))
                .unwrap_or_default();
            return Ok(format!(
                "Connected in {} ms · account {}{exp}",
                start.elapsed().as_millis(),
                acc.status
            ));
        }
        if Path::new(&src.location).is_file() {
            return Ok("Local playlist file found.".into());
        }
        let probe = if freetv::is_countries(&src.location) {
            freetv::country_urls(&src.location)
                .into_iter()
                .next()
                .unwrap_or_else(|| freetv::ALL_URL.into())
        } else {
            src.location.clone()
        };
        let code = http::probe(&probe, Self::user_agent(source).as_deref())?;
        Ok(if (200..300).contains(&code) {
            format!(
                "Reachable in {} ms (HTTP {code}).",
                start.elapsed().as_millis()
            )
        } else {
            format!("Server answered HTTP {code}.")
        })
    }

    pub fn add_m3u(&self, name: &str, url: &str, progress: Progress) -> Res<i64> {
        let id = int(
            &self.call("add_m3u_source", json!({ "name": name, "location": url }))?,
            "id",
        );
        settings::update(|s| {
            s.source_added.insert(id, now_unix());
        });
        self.refresh_or_remove(id, progress)
    }

    pub fn add_xtream(&self, name: &str, server: &str, username: &str, password: &str, progress: Progress) -> Res<i64> {
        let id = int(&self.call("add_xtream_source", json!({ "name": name, "server": server, "username": username, "password": password }))?, "id");
        settings::update(|s| {
            s.source_added.insert(id, now_unix());
        });
        self.refresh_or_remove(id, progress)
    }

    fn refresh_or_remove(&self, id: i64, progress: Progress) -> Res<i64> {
        match self.refresh_with(id, progress) {
            Ok(()) => Ok(id),
            Err(e) => {
                let _ = self.call("remove_source", json!({ "source": id }));
                settings::update(|s| {
                    s.source_added.remove(&id);
                });
                Err(e)
            }
        }
    }

    pub fn remove_source(&self, id: i64) -> Res<()> {
        self.call("remove_source", json!({ "source": id }))?;
        for f in epg_files(id) {
            let _ = std::fs::remove_file(f);
        }
        secrets::delete(&secret_key(id));
        self.stored
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id);
        self.save();
        Ok(())
    }

    /// Download a source's catalog again, then (best effort) its guide.
    pub fn refresh(&self, id: i64) -> Res<()> {
        self.refresh_with(id, &|_| {})
    }

    /// `refresh`, telling `progress` each step (the loading pop-up when a playlist is added).
    pub fn refresh_with(&self, id: i64, progress: Progress) -> Res<()> {
        let src = self.source(id)?;
        let ua = Self::user_agent(id);
        if src.kind == "xtream" {
            let requests = self.call("xtream_requests", json!({ "source": id }))?;
            for r in requests.as_array().into_iter().flatten() {
                let url = s(r, "url").unwrap_or_default();
                let part = s(r, "part").unwrap_or_default();
                let step = match part.as_str() {
                    "account" => Step::SignIn,
                    "live_categories" | "live_streams" => Step::Live,
                    "vod_categories" | "vod_streams" => Step::Movies,
                    _ => Step::Series,
                };
                progress(LoadStatus::new(step, ""));
                let body = http::get_bytes_counted(&url, ua.as_deref(), &|n| progress(LoadStatus::new(step, &size(n))))?;
                self.core
                    .import("xtream_part", &body, json!({ "source": id, "part": part }))?;
            }
            progress(LoadStatus::new(Step::Save, ""));
            self.call("xtream_commit", json!({ "source": id }))?;
        } else {
            progress(LoadStatus::new(Step::Download, ""));
            let file = if freetv::is_countries(&src.location) {
                self.download_joined(&freetv::country_urls(&src.location), ua.as_deref())?
            } else {
                http::download_counted(&src.location, "playlist", ua.as_deref(), &|n| progress(LoadStatus::new(Step::Download, &size(n))))?
            };
            let len = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            progress(LoadStatus::new(Step::Read, &size(len)));
            let result = self.call("import_m3u_file", json!({ "source": id, "path": file }));
            let _ = std::fs::remove_file(&file);
            result?;
            if freetv::is_free_tv(&src.location) {
                self.tag_countries(id);
            }
        }
        self.report_counts(id, progress);
        progress(LoadStatus::new(Step::Save, ""));
        self.save();
        if let Err(e) = self.refresh_epg_with(id, progress) {
            crate::log(&format!("EPG refresh failed for source {id}: {e}")); // never block watching
        }
        Ok(())
    }

    /// How many live channels, movies and series the playlist brought, each on its own line of the pop-up.
    fn report_counts(&self, id: i64, progress: Progress) {
        for (step, kind, one, many) in [
            (Step::Live, "live", "channel", "channels"),
            (Step::Movies, "movie", "movie", "movies"),
            (Step::Series, "series", "series", "series"),
        ] {
            let groups = self.groups(id, kind, None).unwrap_or_default();
            let count: i64 = groups.iter().map(|g| g.count).sum();
            let n = groups.len() as i64;
            let detail = if count == 0 {
                "None".to_string()
            } else {
                format!(
                    "{} {} · {} {}",
                    group_digits(count),
                    if count == 1 { one } else { many },
                    group_digits(n),
                    if n == 1 { "category" } else { "categories" }
                )
            };
            progress(LoadStatus { step, detail, done: true });
        }
    }

    pub fn refresh_epg(&self, id: i64) -> Res<()> {
        self.refresh_epg_with(id, &|_| {})
    }

    fn refresh_epg_with(&self, id: i64, progress: Progress) -> Res<()> {
        let urls: Vec<String> = self
            .call("epg_fetch_urls", json!({ "source": id }))?
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|u| u.as_str().map(str::to_string))
            .filter(|u| !u.is_empty())
            .collect();
        let _ = std::fs::create_dir_all(epg_dir());
        let ua = Self::user_agent(id);
        let mut index = 0;
        if urls.is_empty() {
            progress(LoadStatus::new(Step::Guide, "None offered"));
        }
        let total = urls.len();
        for (n, url) in urls.into_iter().enumerate() {
            let which = if total > 1 { format!("Guide {} of {total}", n + 1) } else { String::new() };
            progress(LoadStatus::new(Step::Guide, &which));
            let report = |b: u64| {
                let parts: Vec<String> = [which.clone(), size(b)].into_iter().filter(|s| !s.is_empty()).collect();
                progress(LoadStatus::new(Step::Guide, &parts.join(" · ")));
            };
            let file = match http::download_counted(&url, "guide", ua.as_deref(), &report) {
                Ok(f) => f,
                Err(e) => {
                    crate::log(&format!("guide download failed: {e}"));
                    continue;
                }
            };
            if self
                .call("import_xmltv_file", json!({ "path": file }))
                .is_ok()
                && move_file(&file, &epg_file(id, index)).is_ok()
            {
                index += 1;
            }
            let _ = std::fs::remove_file(&file);
        }
        let keep: HashSet<PathBuf> = (0..index).map(|i| epg_file(id, i)).collect();
        for stale in epg_files(id).into_iter().filter(|f| !keep.contains(f)) {
            let _ = std::fs::remove_file(stale);
        }
        self.save();
        Ok(())
    }

    /// Tag Free TV channels with their countries (iptv-org's index by country). Best effort.
    pub fn tag_countries(&self, id: i64) {
        let result = (|| -> Res<i64> {
            let file = http::download(freetv::COUNTRY_INDEX_URL, "countries", None)?;
            let bytes = std::fs::read(&file);
            let _ = std::fs::remove_file(&file);
            let n = self.core.import(
                "m3u_tags",
                &bytes?,
                json!({ "source": id, "field": freetv::COUNTRY_FIELD }),
            )?;
            Ok(n.as_i64().unwrap_or(0))
        })();
        match result {
            Ok(n) => crate::log(&format!("free tv: {n} channels tagged with a country")),
            Err(e) => crate::log(&format!("free tv country index failed: {e}")),
        }
    }

    fn download_joined(&self, urls: &[String], ua: Option<&str>) -> Res<PathBuf> {
        use std::io::{BufRead, Write};
        let joined = paths::temp_file("joined");
        let mut out = std::io::BufWriter::new(std::fs::File::create(&joined)?);
        writeln!(out, "#EXTM3U")?;
        let mut any = false;
        for url in urls {
            let part = match http::download(url, "playlist", ua) {
                Ok(p) => p,
                Err(e) => {
                    crate::log(&format!("free tv list {url} failed: {e}"));
                    continue;
                }
            };
            if let Ok(f) = std::fs::File::open(&part) {
                for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                    if !line.to_ascii_uppercase().starts_with("#EXTM3U") {
                        writeln!(out, "{line}")?;
                    }
                }
                any = true;
            }
            let _ = std::fs::remove_file(&part);
        }
        out.flush()?;
        if !any {
            let _ = std::fs::remove_file(&joined);
            return Err(AppError::Network(
                "None of the chosen country lists could be downloaded.".into(),
            ));
        }
        Ok(joined)
    }

    // ---------------------------------------------------------- browsing

    pub fn groups(&self, source: i64, kind: &str, group_field: Option<&str>) -> Res<Vec<Group>> {
        let v = self.call(
            "groups",
            json!({ "source": source, "kind": kind, "group_field": group_field }),
        )?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .map(|g| Group {
                name: s(g, "name").unwrap_or_default(),
                count: int(g, "count"),
            })
            .collect())
    }

    /// Sorted, filtered, paged listing. group None = all categories; source None = every playlist.
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        kind: &str,
        source: Option<i64>,
        group: Option<&str>,
        hidden: &[String],
        sort: &str,
        min_rating: f64,
        offset: usize,
        limit: usize,
        group_field: Option<&str>,
    ) -> Res<(i64, Vec<Item>)> {
        let r = self.call(
            "query",
            json!({
                "kind": kind, "source": source, "group": group, "group_field": group_field,
                "hidden_groups": hidden, "sort": sort, "min_rating": min_rating, "offset": offset, "limit": limit,
            }),
        )?;
        Ok((
            int(&r, "total"),
            items(r.get("items").unwrap_or(&Value::Null)),
        ))
    }

    pub fn continue_watching(&self, limit: usize) -> Res<Vec<Item>> {
        Ok(items(
            &self.call("continue_watching", json!({ "limit": limit }))?,
        ))
    }

    pub fn favorites(&self, kind: Option<&str>) -> Res<Vec<Item>> {
        Ok(items(&self.call("favorites", json!({ "kind": kind }))?))
    }

    pub fn recent(&self, kind: Option<&str>, limit: usize) -> Res<Vec<Item>> {
        Ok(items(
            &self.call("recent", json!({ "kind": kind, "limit": limit }))?,
        ))
    }

    pub fn remove_from_history(self: &Arc<Self>, id: &str) -> Res<()> {
        self.call("remove_history", json!({ "id": id }))?;
        self.save_soon();
        Ok(())
    }

    pub fn clear_history(self: &Arc<Self>) -> Res<()> {
        self.call("clear_history", json!({}))?;
        self.save_soon();
        Ok(())
    }

    pub fn item(&self, id: &str) -> Option<Item> {
        self.call("item", json!({ "id": id }))
            .ok()
            .map(|v| parse_item(&v))
    }

    pub fn set_watched_many(self: &Arc<Self>, list: &[Item], watched: bool) {
        for it in list {
            let dur = if it.duration > 0.0 {
                it.duration
            } else if it.duration_hint > 0.0 {
                it.duration_hint
            } else {
                1.0
            };
            let _ = self.call("set_progress", json!({ "id": it.id, "position": if watched { dur } else { 0.0 }, "duration": dur }));
        }
        self.save_soon();
    }

    // ---------------------------------------------------------- details

    pub fn movie_details(&self, movie: &Item) -> Res<Option<DetailsInfo>> {
        if let Some(d) = self
            .details
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&movie.id)
        {
            return Ok(Some(d.clone()));
        }
        let Ok(url) = self.call("vod_info_url", json!({ "id": movie.id })) else {
            return Ok(None);
        }; // M3U films
        let root: Value = serde_json::from_slice(&http::get_bytes(
            url.as_str().unwrap_or(""),
            Self::user_agent(movie.source_id).as_deref(),
        )?)?;
        let d = parse_details(root.get("info").unwrap_or(&Value::Null));
        self.details
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(movie.id.clone(), d.clone());
        Ok(Some(d))
    }

    pub fn series_details(&self, series: &Item) -> Res<Option<DetailsInfo>> {
        if let Some(d) = self
            .details
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&series.id)
        {
            return Ok(Some(d.clone()));
        }
        let body = self.series_body(series)?;
        let root: Value = serde_json::from_slice(&body)?;
        let d = parse_details(root.get("info").unwrap_or(&Value::Null));
        self.details
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(series.id.clone(), d.clone());
        Ok(Some(d))
    }

    fn series_body(&self, series: &Item) -> Res<Arc<Vec<u8>>> {
        if let Some(b) = self
            .series_bodies
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&series.id)
        {
            return Ok(Arc::clone(b));
        }
        let url = self.call("series_info_url", json!({ "id": series.id }))?;
        let body = Arc::new(http::get_bytes(
            url.as_str().unwrap_or(""),
            Self::user_agent(series.source_id).as_deref(),
        )?);
        self.series_bodies
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(series.id.clone(), Arc::clone(&body));
        Ok(body)
    }

    pub fn search(&self, query: &str, kind: Option<&str>) -> Res<Vec<Item>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        Ok(items(&self.call(
            "search",
            json!({ "query": query, "kind": kind, "limit": 300 }),
        )?))
    }

    pub fn count_kind(&self, source: i64, kind: &str) -> i64 {
        self.groups(source, kind, None)
            .map(|g| g.iter().map(|x| x.count).sum())
            .unwrap_or(0)
    }

    // ---------------------------------------------------------- guide

    /// The extra XMLTV URLs from Settings › Guide.
    pub fn refresh_custom_epg(&self, urls: &[String]) {
        let _ = std::fs::create_dir_all(epg_dir());
        let mut index = 0;
        for url in urls.iter().map(|u| u.trim()).filter(|u| !u.is_empty()) {
            let file = match http::download(url, "guide", None) {
                Ok(f) => f,
                Err(e) => {
                    crate::log(&format!("extra guide {url} failed: {e}"));
                    continue;
                }
            };
            if self
                .call("import_xmltv_file", json!({ "path": file }))
                .is_ok()
                && move_file(&file, &custom_epg_file(index)).is_ok()
            {
                index += 1;
            }
            let _ = std::fs::remove_file(&file);
        }
        let keep: HashSet<PathBuf> = (0..index).map(custom_epg_file).collect();
        for f in list_dir(&epg_dir())
            .into_iter()
            .filter(|p| file_name(p).starts_with("custom-") && !keep.contains(p))
        {
            let _ = std::fs::remove_file(f);
        }
    }

    pub fn custom_epg_age() -> Option<Duration> {
        age(&list_dir(&epg_dir())
            .into_iter()
            .filter(|p| file_name(p).starts_with("custom-"))
            .collect::<Vec<_>>())
    }

    pub fn clear_epg(&self) {
        let _ = self.call("clear_epg", json!({}));
        let _ = std::fs::remove_dir_all(epg_dir());
    }

    pub fn now_next(
        &self,
        ids: &[String],
    ) -> HashMap<String, (Option<Programme>, Option<Programme>)> {
        let mut out = HashMap::new();
        if ids.is_empty() {
            return out;
        }
        let Ok(res) = self.call(
            "now_next_many",
            json!({ "ids": ids, "now": now_unix() - offset() }),
        ) else {
            return out;
        };
        for id in ids {
            if let Some(o) = res.get(id) {
                let prog = |k: &str| o.get(k).filter(|p| p.is_object()).map(parse_programme);
                out.insert(id.clone(), (prog("now"), prog("next")));
            }
        }
        out
    }

    pub fn guide(&self, id: &str, from: i64, to: i64) -> Vec<Programme> {
        let off = offset();
        self.call(
            "guide",
            json!({ "ids": [id], "from": from - off, "to": to - off }),
        )
        .ok()
        .and_then(|r| {
            r.get(id)
                .and_then(Value::as_array)
                .map(|a| a.iter().map(parse_programme).collect())
        })
        .unwrap_or_default()
    }

    pub fn favorite_ids(&self) -> HashSet<String> {
        self.call("favorites", json!({}))
            .map(|v| items(&v).into_iter().map(|i| i.id).collect())
            .unwrap_or_default()
    }

    // ---------------------------------------------------------- backup

    pub fn export_user_state(&self) -> Res<String> {
        Ok(self.call("export_user_state", json!({}))?.to_string())
    }

    pub fn import_user_state(&self, json_bytes: &[u8]) -> Res<()> {
        self.core.import("user_state", json_bytes, json!({}))?;
        self.save();
        Ok(())
    }

    pub fn toggle_favorite(self: &Arc<Self>, id: &str) -> Res<bool> {
        let on = !self
            .call("item", json!({ "id": id }))?
            .get("favorite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.call("set_favorite", json!({ "id": id, "on": on }))?;
        self.save_soon();
        Ok(on)
    }

    /// Episodes of a series, fetched from the panel the first time.
    pub fn episodes(self: &Arc<Self>, series: &Item) -> Res<Vec<Item>> {
        let eps = items(&self.call("episodes", json!({ "id": series.id }))?);
        if !eps.is_empty() {
            return Ok(eps);
        }
        let body = self.series_body(series)?;
        self.core
            .import("series_info", &body, json!({ "id": series.id }))?;
        self.save_soon();
        Ok(items(&self.call("episodes", json!({ "id": series.id }))?))
    }

    // ---------------------------------------------------------- playback

    pub fn record_play(&self, id: &str) {
        let _ = self.call("record_play", json!({ "id": id }));
    }

    /// Resume position in seconds (0 = start over).
    pub fn resume_seconds(&self, id: &str) -> f64 {
        let Ok(it) = self.call("item", json!({ "id": id })) else {
            return 0.0;
        };
        let Some(p) = it.get("progress") else {
            return 0.0;
        };
        let (pos, dur) = (num(p, "position"), num(p, "duration"));
        if dur > 0.0 && pos / dur > 0.95 {
            0.0
        } else {
            pos
        }
    }

    pub fn set_progress(&self, id: &str, position: f64, duration: f64) {
        let _ = self.call(
            "set_progress",
            json!({ "id": id, "position": position, "duration": duration }),
        );
    }
}
