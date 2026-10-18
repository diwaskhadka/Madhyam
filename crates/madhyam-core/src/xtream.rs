//! Xtream Codes "player API" support, sans-IO.
//!
//! [`XtreamClient`] only *builds URLs* and *parses response bodies*. Fetching
//! is done by whoever holds a [`Fetcher`]: the host app, the CLI, or a test.
//! [`XtreamClient::sync`] is a convenience driver that does the usual
//! "log in and download the whole catalog" dance through a `Fetcher`.
//!
//! The API is undocumented and panel vendors disagree on types, so every
//! field is read through the lenient accessors in [`crate::json::Value`].

use crate::error::{Error, Result};
use crate::fetch::{Fetcher, url_encode};
use crate::json::{self, ToJson, Value, obj};
use crate::model::{Item, ItemKind, Programme, SourceId};
use crate::textutil::maybe_base64_text;
use crate::time::{parse_sql_datetime, parts_from_unix};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct XtreamCredentials {
    /// `http(s)://host:port` — trailing slashes and missing scheme are fixed up.
    pub server: String,
    pub username: String,
    pub password: String,
    /// Container for live streams: `"ts"` (default) or `"m3u8"`.
    pub live_ext: String,
}

impl XtreamCredentials {
    pub fn new(server: &str, username: &str, password: &str) -> Self {
        let mut s = server.trim().trim_end_matches('/').to_string();
        // People paste full URLs such as http://host:8080/player_api.php?...
        if let Some(i) = s.find("/player_api.php").or_else(|| s.find("/get.php")) {
            s.truncate(i);
        }
        if !s.contains("://") {
            s = format!("http://{s}");
        }
        XtreamCredentials {
            server: s,
            username: username.trim().to_string(),
            password: password.trim().to_string(),
            live_ext: "ts".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountInfo {
    pub status: String,
    /// Unix expiry, `None` = unlimited.
    pub expires: Option<i64>,
    pub max_connections: i64,
    pub active_connections: i64,
    pub is_trial: bool,
    pub server_timezone: String,
    /// Server clock (lets hosts compute the offset needed for catch-up URLs).
    pub server_time: Option<i64>,
    pub allowed_formats: Vec<String>,
}

impl ToJson for AccountInfo {
    fn to_json(&self) -> Value {
        obj([
            ("status", self.status.as_str().into()),
            ("expires", self.expires.into()),
            ("max_connections", self.max_connections.into()),
            ("active_connections", self.active_connections.into()),
            ("is_trial", self.is_trial.into()),
            ("server_timezone", self.server_timezone.as_str().into()),
            ("server_time", self.server_time.into()),
            ("allowed_formats", self.allowed_formats.clone().into()),
        ])
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub parent_id: String,
}

/// Everything [`XtreamClient::sync`] downloads.
#[derive(Debug, Clone, Default)]
pub struct XtreamSnapshot {
    pub account: AccountInfo,
    pub items: Vec<Item>,
    /// Lists that failed to download/parse (`"vod: ..."`); the rest still imports.
    pub warnings: Vec<String>,
}

pub struct XtreamClient {
    pub creds: XtreamCredentials,
    pub source: SourceId,
}

fn parse_json(bytes: &[u8]) -> Result<Value> {
    let v = json::parse_bytes(bytes)?;
    // Panels answer errors in many shapes; the common ones:
    if let Some(msg) = v
        .text("error")
        .or_else(|| v.get("user_info").and_then(|u| u.text("message")))
    {
        if v.get("user_info").is_none_or(|u| !u.flag("auth")) {
            return Err(Error::Api(msg));
        }
    }
    Ok(v)
}

/// Lists may arrive as `[...]`, `{"0": {...}, "1": {...}}`, or `null`.
fn as_list(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(a) => a.iter().collect(),
        Value::Object(m) => m.values().collect(),
        _ => Vec::new(),
    }
}

impl XtreamClient {
    pub fn new(creds: XtreamCredentials, source: SourceId) -> Self {
        XtreamClient { creds, source }
    }

    fn auth_query(&self) -> String {
        format!(
            "username={}&password={}",
            url_encode(&self.creds.username),
            url_encode(&self.creds.password)
        )
    }

    // ----------------------------------------------------------------- URLs

    /// `player_api.php` URL. `action = None` returns account info.
    pub fn api_url(&self, action: Option<&str>, params: &[(&str, &str)]) -> String {
        let mut u = format!("{}/player_api.php?{}", self.creds.server, self.auth_query());
        if let Some(a) = action {
            u.push_str("&action=");
            u.push_str(a);
        }
        for (k, v) in params {
            u.push_str(&format!("&{}={}", url_encode(k), url_encode(v)));
        }
        u
    }

    /// Full XMLTV guide for this account.
    pub fn xmltv_url(&self) -> String {
        format!("{}/xmltv.php?{}", self.creds.server, self.auth_query())
    }

    fn path_auth(&self) -> String {
        format!(
            "{}/{}",
            url_encode(&self.creds.username),
            url_encode(&self.creds.password)
        )
    }

    pub fn live_url(&self, stream_id: &str) -> String {
        format!(
            "{}/live/{}/{}.{}",
            self.creds.server,
            self.path_auth(),
            stream_id,
            self.creds.live_ext
        )
    }
    pub fn movie_url(&self, stream_id: &str, ext: &str) -> String {
        format!(
            "{}/movie/{}/{}.{}",
            self.creds.server,
            self.path_auth(),
            stream_id,
            ext
        )
    }
    pub fn episode_url(&self, episode_id: &str, ext: &str) -> String {
        format!(
            "{}/series/{}/{}.{}",
            self.creds.server,
            self.path_auth(),
            episode_id,
            ext
        )
    }

    /// Catch-up URL. Panels interpret `start` in **server local time**, so pass
    /// the server's UTC offset (seconds east). Many panels run on UTC (0).
    pub fn timeshift_url(
        &self,
        stream_id: &str,
        start: i64,
        duration_min: u32,
        server_offset_secs: i64,
    ) -> String {
        let (y, mo, d, h, mi, _) = parts_from_unix(start + server_offset_secs);
        format!(
            "{}/timeshift/{}/{}/{y:04}-{mo:02}-{d:02}:{h:02}-{mi:02}/{}.ts",
            self.creds.server,
            self.path_auth(),
            duration_min,
            stream_id
        )
    }

    // -------------------------------------------------------------- parsing

    pub fn parse_account(&self, body: &[u8]) -> Result<AccountInfo> {
        let v = parse_json(body)?;
        let u = v
            .get("user_info")
            .ok_or_else(|| Error::Api("response has no user_info (wrong server URL?)".into()))?;
        if !u.flag("auth") {
            return Err(Error::Api("authentication failed".into()));
        }
        let s = v.get("server_info").cloned().unwrap_or_default();
        Ok(AccountInfo {
            status: u.text("status").unwrap_or_default(),
            expires: u.int("exp_date").filter(|&t| t > 0),
            max_connections: u.int("max_connections").unwrap_or(0),
            active_connections: u.int("active_cons").unwrap_or(0),
            is_trial: u.flag("is_trial"),
            server_timezone: s.text("timezone").unwrap_or_default(),
            server_time: s.int("timestamp_now"),
            allowed_formats: u
                .get("allowed_output_formats")
                .map(|f| as_list(f).into_iter().filter_map(Value::to_text).collect())
                .unwrap_or_default(),
        })
    }

    pub fn parse_categories(&self, body: &[u8]) -> Result<Vec<Category>> {
        let v = parse_json(body)?;
        Ok(as_list(&v)
            .into_iter()
            .filter_map(|c| {
                Some(Category {
                    id: c.text("category_id")?,
                    name: c.text("category_name").unwrap_or_default(),
                    parent_id: c.text("parent_id").unwrap_or_default(),
                })
            })
            .collect())
    }

    fn group_name(cats: &HashMap<String, String>, s: &Value) -> String {
        // Newer panels send `category_ids: [..]` in addition to `category_id`.
        let id = s.text("category_id").or_else(|| {
            s.get("category_ids")
                .and_then(|a| a.as_array())
                .and_then(|a| a.first())
                .and_then(Value::to_text)
        });
        id.and_then(|i| cats.get(&i).cloned())
            .unwrap_or_else(|| "Uncategorized".into())
    }

    pub fn parse_live_streams(
        &self,
        body: &[u8],
        cats: &HashMap<String, String>,
    ) -> Result<Vec<Item>> {
        let v = parse_json(body)?;
        Ok(as_list(&v)
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let id = s.text("stream_id")?;
                let archive = if s.flag("tv_archive") {
                    s.int("tv_archive_duration").unwrap_or(0).max(0) as u32
                } else {
                    0
                };
                let mut it = Item::new(
                    self.source,
                    ItemKind::Live,
                    &id,
                    s.text("name").unwrap_or_default(),
                );
                it.set_group(&Self::group_name(cats, s));
                it.set_text(
                    &s.text("stream_icon").unwrap_or_default(),
                    &self.live_url(&id),
                    &s.text("epg_channel_id").unwrap_or_default(),
                );
                it.archive_days = archive;
                it.order = s.int("num").map_or(i as u32, |n| n as u32);
                Some(
                    it.with_extra("added", s.text("added"))
                        .with_extra("direct_source", s.text("direct_source")),
                )
            })
            .collect())
    }

    pub fn parse_vod_streams(
        &self,
        body: &[u8],
        cats: &HashMap<String, String>,
    ) -> Result<Vec<Item>> {
        let v = parse_json(body)?;
        Ok(as_list(&v)
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let id = s.text("stream_id")?;
                let ext = s
                    .text("container_extension")
                    .unwrap_or_else(|| "mp4".into());
                let mut it = Item::new(
                    self.source,
                    ItemKind::Movie,
                    &id,
                    s.text("name").unwrap_or_default(),
                );
                it.set_group(&Self::group_name(cats, s));
                it.set_text(
                    &s.text("stream_icon").unwrap_or_default(),
                    &self.movie_url(&id, &ext),
                    "",
                );
                it.order = s.int("num").map_or(i as u32, |n| n as u32);
                Some(
                    it.with_extra("container", Some(ext))
                        .with_extra("rating", s.text("rating"))
                        .with_extra("year", s.text("year").or_else(|| s.text("releaseDate")))
                        .with_extra("plot", s.text("plot"))
                        .with_extra("added", s.text("added"))
                        .with_extra("tmdb", s.text("tmdb").or_else(|| s.text("tmdb_id"))),
                )
            })
            .collect())
    }

    pub fn parse_series(&self, body: &[u8], cats: &HashMap<String, String>) -> Result<Vec<Item>> {
        let v = parse_json(body)?;
        Ok(as_list(&v)
            .into_iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let id = s.text("series_id")?;
                let mut it = Item::new(
                    self.source,
                    ItemKind::Series,
                    &id,
                    s.text("name").unwrap_or_default(),
                );
                it.set_group(&Self::group_name(cats, s));
                it.set_logo(&s.text("cover").unwrap_or_default());
                it.order = s.int("num").map_or(i as u32, |n| n as u32);
                Some(
                    it.with_extra("plot", s.text("plot"))
                        .with_extra("cast", s.text("cast"))
                        .with_extra("genre", s.text("genre"))
                        .with_extra("rating", s.text("rating"))
                        .with_extra(
                            "year",
                            s.text("releaseDate").or_else(|| s.text("release_date")),
                        )
                        .with_extra("trailer", s.text("youtube_trailer"))
                        .with_extra("last_modified", s.text("last_modified")),
                )
            })
            .collect())
    }

    /// Episodes of one series (`get_series_info&series_id=...`), ordered by
    /// season then episode number.
    pub fn parse_series_info(&self, body: &[u8], series: &Item) -> Result<Vec<Item>> {
        let v = parse_json(body)?;
        let mut eps = Vec::new();
        let groups: Vec<(String, &Value)> = match v.get("episodes") {
            Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v)).collect(),
            Some(Value::Array(a)) => a.iter().map(|v| (String::new(), v)).collect(),
            _ => Vec::new(),
        };
        for (season_key, list) in groups {
            for e in as_list(list) {
                let Some(id) = e.text("id") else { continue };
                let season = e
                    .int("season")
                    .or_else(|| season_key.parse().ok())
                    .unwrap_or(0);
                let num = e.int("episode_num").unwrap_or(0);
                let ext = e
                    .text("container_extension")
                    .unwrap_or_else(|| "mp4".into());
                let info = e.get("info").cloned().unwrap_or_default();
                let title = e.text("title").unwrap_or_else(|| format!("Episode {num}"));
                let mut it = Item::new(self.source, ItemKind::Episode, &id, title);
                it.set_group(&format!("Season {season}"));
                it.set_text(
                    &info
                        .text("movie_image")
                        .unwrap_or_else(|| series.logo().to_string()),
                    &self.episode_url(&id, &ext),
                    "",
                );
                it.order = (season.clamp(0, 9999) * 10_000 + num.clamp(0, 9999)) as u32;
                eps.push(
                    it.with_extra("series_id", Some(series.remote_id().to_string()))
                        .with_extra("season", Some(season.to_string()))
                        .with_extra("episode", Some(num.to_string()))
                        .with_extra("container", Some(ext))
                        .with_extra("plot", info.text("plot"))
                        .with_extra("duration_secs", info.text("duration_secs"))
                        .with_extra(
                            "air_date",
                            info.text("releasedate").or_else(|| info.text("air_date")),
                        ),
                );
            }
        }
        eps.sort_by_key(|e| e.order);
        Ok(eps)
    }

    /// `get_short_epg` / `get_simple_data_table`: titles are base64 encoded.
    pub fn parse_short_epg(&self, body: &[u8], epg_id: &str) -> Result<Vec<Programme>> {
        let v = parse_json(body)?;
        let list = v.get("epg_listings").map(as_list).unwrap_or_default();
        let mut out: Vec<Programme> = list
            .into_iter()
            .filter_map(|e| {
                let start = e
                    .int("start_timestamp")
                    .or_else(|| e.text("start").and_then(|s| parse_sql_datetime(&s)))?;
                let stop = e
                    .int("stop_timestamp")
                    .or_else(|| e.text("end").and_then(|s| parse_sql_datetime(&s)))?;
                Some(Programme {
                    channel: e
                        .text("epg_id")
                        .or_else(|| e.text("channel_id"))
                        .unwrap_or_else(|| epg_id.to_string()),
                    start,
                    stop,
                    title: maybe_base64_text(&e.text("title").unwrap_or_default()),
                    desc: maybe_base64_text(&e.text("description").unwrap_or_default()),
                    ..Default::default()
                })
            })
            .collect();
        out.sort_by_key(|p| p.start);
        Ok(out)
    }

    // ---------------------------------------------------------------- driver

    /// Log in and download live, VOD and series lists through `fetcher`.
    /// A failing list becomes a warning; failing login is an error.
    pub fn sync(&self, fetcher: &dyn Fetcher) -> Result<XtreamSnapshot> {
        let account = self.parse_account(&fetcher.get(&self.api_url(None, &[]), &[])?)?;
        let mut snap = XtreamSnapshot {
            account,
            ..Default::default()
        };

        type Parse = fn(&XtreamClient, &[u8], &HashMap<String, String>) -> Result<Vec<Item>>;
        let lists: [(&str, &str, &str, Parse); 3] = [
            (
                "live",
                "get_live_categories",
                "get_live_streams",
                XtreamClient::parse_live_streams,
            ),
            (
                "vod",
                "get_vod_categories",
                "get_vod_streams",
                XtreamClient::parse_vod_streams,
            ),
            (
                "series",
                "get_series_categories",
                "get_series",
                XtreamClient::parse_series,
            ),
        ];
        for (label, cat_action, list_action, parse) in lists {
            let cats: HashMap<String, String> = match fetcher
                .get(&self.api_url(Some(cat_action), &[]), &[])
                .and_then(|b| self.parse_categories(&b))
            {
                Ok(c) => c.into_iter().map(|c| (c.id, c.name)).collect(),
                Err(e) => {
                    snap.warnings.push(format!("{label} categories: {e}"));
                    HashMap::new()
                }
            };
            match fetcher
                .get(&self.api_url(Some(list_action), &[]), &[])
                .and_then(|b| parse(self, &b, &cats))
            {
                Ok(items) => snap.items.extend(items),
                Err(e) => snap.warnings.push(format!("{label}: {e}")),
            }
        }
        Ok(snap)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A fake panel: URL substring → body. Used by catalog tests too.
    pub fn fake_panel() -> impl Fn(&str, &[(&str, &str)]) -> Result<Vec<u8>> {
        let mut r: HashMap<&str, &str> = HashMap::new();
        r.insert(
            "action=get_live_categories",
            r#"[{"category_id":"1","category_name":"News","parent_id":0}]"#,
        );
        r.insert(
            "action=get_live_streams",
            r#"[{"num":1,"name":"CNN","stream_id":101,"stream_icon":"http://i/cnn.png","epg_channel_id":"cnn.us","category_id":"1","tv_archive":1,"tv_archive_duration":"3"},
                {"num":2,"name":"No Category","stream_id":"102","category_id":null}]"#,
        );
        r.insert(
            "action=get_vod_categories",
            r#"{"0":{"category_id":7,"category_name":"Action"}}"#,
        );
        r.insert(
            "action=get_vod_streams",
            r#"[{"name":"Heat (1995)","stream_id":"5001","container_extension":"mkv","category_id":"7","rating":"8.3"}]"#,
        );
        r.insert("action=get_series_categories", r#"[]"#);
        r.insert(
            "action=get_series",
            r#"[{"series_id":900,"name":"The Show","cover":"c.jpg","category_id":"2"}]"#,
        );
        r.insert(
            "action=get_series_info",
            r#"{"info":{},"episodes":{"2":[{"id":"9002","episode_num":1,"title":"S2E1","container_extension":"mp4","info":{"duration_secs":1800}}],
                                     "1":[{"id":"9001","episode_num":"1","title":"Pilot","container_extension":"mkv","season":1}]}}"#,
        );
        r.insert(
            "action=get_short_epg",
            r#"{"epg_listings":[{"epg_id":"cnn.us","title":"TmV3cw==","description":"","start":"2026-10-12 11:00:00","end":"2026-10-12 12:00:00","start_timestamp":"1791802800","stop_timestamp":"1791806400"}]}"#,
        );
        r.insert(
            "player_api.php?username=u&password=p%26x",
            r#"{"user_info":{"auth":1,"status":"Active","exp_date":"1893456000","max_connections":"2","active_cons":0,"allowed_output_formats":["m3u8","ts"]},"server_info":{"timezone":"UTC","timestamp_now":1791800000}}"#,
        );
        move |url: &str, _h: &[(&str, &str)]| {
            // `action=` keys win over the bare account URL, which every request contains.
            r.iter()
                .filter(|(k, _)| url.contains(*k))
                .max_by_key(|(k, _)| (k.starts_with("action="), k.len()))
                .map(|(_, v)| v.as_bytes().to_vec())
                .ok_or_else(|| Error::Api(format!("404 {url}")))
        }
    }

    pub fn client() -> XtreamClient {
        XtreamClient::new(XtreamCredentials::new("panel.example:8080/", "u", "p&x"), 1)
    }

    #[test]
    fn urls() {
        let c = client();
        assert_eq!(c.creds.server, "http://panel.example:8080");
        assert_eq!(
            c.api_url(Some("get_vod_info"), &[("vod_id", "5")]),
            "http://panel.example:8080/player_api.php?username=u&password=p%26x&action=get_vod_info&vod_id=5"
        );
        assert_eq!(
            c.live_url("101"),
            "http://panel.example:8080/live/u/p%26x/101.ts"
        );
        let t = crate::time::unix_from_parts(2026, 10, 12, 11, 5, 0);
        assert_eq!(
            c.timeshift_url("101", t, 60, 7200),
            "http://panel.example:8080/timeshift/u/p%26x/60/2026-10-12:13-05/101.ts"
        );
        let pasted = XtreamCredentials::new(
            "https://h.tv/player_api.php?username=a&password=b",
            "a",
            "b",
        );
        assert_eq!(pasted.server, "https://h.tv");
    }

    #[test]
    fn full_sync() {
        let c = client();
        let snap = c.sync(&fake_panel()).unwrap();
        assert!(snap.warnings.is_empty(), "{:?}", snap.warnings);
        assert_eq!(snap.account.max_connections, 2);
        assert_eq!(snap.account.expires, Some(1893456000));
        assert_eq!(snap.items.len(), 4);
        let cnn = &snap.items[0];
        assert_eq!(
            (&*cnn.group, cnn.epg_id(), cnn.archive_days),
            ("News", "cnn.us", 3)
        );
        assert_eq!(&*snap.items[1].group, "Uncategorized");
        let heat = snap
            .items
            .iter()
            .find(|i| i.kind == ItemKind::Movie)
            .unwrap();
        assert_eq!(&*heat.group, "Action");
        assert!(heat.url().ends_with("/movie/u/p%26x/5001.mkv"));
        assert_eq!(heat.extra("rating"), Some("8.3"));
    }

    #[test]
    fn bad_login() {
        let c = client();
        let err = c.parse_account(br#"{"user_info":{"auth":0}}"#).unwrap_err();
        assert!(matches!(err, Error::Api(_)));
        assert!(c.parse_account(b"<html>nginx</html>").is_err());
    }

    #[test]
    fn series_and_epg() {
        let c = client();
        let f = fake_panel();
        let series = Item::new(1, ItemKind::Series, "900", "The Show");
        let eps = c
            .parse_series_info(&f("action=get_series_info", &[]).unwrap(), &series)
            .unwrap();
        assert_eq!(
            eps.iter().map(|e| e.name()).collect::<Vec<_>>(),
            ["Pilot", "S2E1"]
        );
        assert!(eps[0].url().ends_with("/series/u/p%26x/9001.mkv"));
        assert_eq!(eps[1].extra("season"), Some("2"));

        let epg = c
            .parse_short_epg(&f("action=get_short_epg", &[]).unwrap(), "cnn.us")
            .unwrap();
        assert_eq!(epg[0].title, "News");
        assert_eq!(epg[0].stop - epg[0].start, 3600);
    }
}
