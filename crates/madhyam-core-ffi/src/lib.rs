//! # C ABI for madhyam_core
//!
//! Six functions, JSON in and JSON out. That keeps the ABI tiny and stable:
//! new features are new *method names*, never new symbols, so a host compiled
//! against version 0.1 keeps linking against 0.9.
//!
//! ```c
//! MadhyamCore* iptv_core_new(void);
//! void      iptv_core_free(MadhyamCore*);
//! char*     iptv_call(MadhyamCore*, const char* method, const char* args_json);
//! char*     iptv_import(MadhyamCore*, const char* method, const uint8_t* data, size_t len, const char* args_json);
//! void      iptv_string_free(char*);
//! const char* iptv_version(void);
//! ```
//!
//! Every returned `char*` is `{"ok": <result>}` or
//! `{"error": {"code": "...", "message": "..."}}` and must be released with
//! `iptv_string_free`. Calls are thread-safe (one internal mutex) and
//! **blocking** — call them off the UI thread.
//!
//! The method list lives in [`dispatch`]; see `docs/API.md`.

use madhyam_core::catalog::{ListQuery, SearchFilter, Sort, SourceKind};
use madhyam_core::json::{self, ToJson, Value, obj};
use madhyam_core::xtream::{XtreamClient, XtreamCredentials, XtreamSnapshot};
use madhyam_core::{Catalog, Error, Item, ItemId, ItemKind, Result, SourceId, m3u, time};
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::io::BufReader;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

/// Raw Xtream response bodies waiting for `xtream_commit`, per source.
type Pending = HashMap<SourceId, HashMap<String, Vec<u8>>>;

/// Everything behind the opaque `MadhyamCore*` handle.
pub struct State {
    pub catalog: Catalog,
    pending_xtream: Pending,
}

impl Default for State {
    fn default() -> Self {
        State {
            catalog: Catalog::new(),
            pending_xtream: HashMap::new(),
        }
    }
}

pub struct MadhyamCore {
    state: Mutex<State>,
}

/// The Xtream lists a full sync needs, in the order hosts should fetch them.
pub const XTREAM_PARTS: [(&str, Option<&str>); 7] = [
    ("account", None),
    ("live_categories", Some("get_live_categories")),
    ("live_streams", Some("get_live_streams")),
    ("vod_categories", Some("get_vod_categories")),
    ("vod_streams", Some("get_vod_streams")),
    ("series_categories", Some("get_series_categories")),
    ("series", Some("get_series")),
];

// ============================================================ arg helpers

fn arg_str(a: &Value, k: &str) -> Result<String> {
    a.get(k)
        .and_then(Value::to_text)
        .ok_or_else(|| Error::Invalid(format!("missing string argument '{k}'")))
}
fn opt_str(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(Value::to_text)
}
fn arg_id(a: &Value, k: &str) -> Result<ItemId> {
    // Accept "123" (preferred; u64 does not fit a double) or 123.
    let v = a
        .get(k)
        .ok_or_else(|| Error::Invalid(format!("missing id argument '{k}'")))?;
    match v {
        Value::String(s) => s
            .trim()
            .parse()
            .map_err(|_| Error::Invalid(format!("'{k}' is not a valid id"))),
        Value::Number(n) if *n >= 0.0 => Ok(*n as u64),
        _ => Err(Error::Invalid(format!("'{k}' is not a valid id"))),
    }
}
fn arg_source(a: &Value) -> Result<SourceId> {
    a.int("source")
        .or_else(|| a.int("id"))
        .map(|n| n as SourceId)
        .ok_or_else(|| Error::Invalid("missing 'source'".into()))
}
fn opt_kind(a: &Value) -> Result<Option<ItemKind>> {
    match opt_str(a, "kind") {
        None => Ok(None),
        Some(k) => ItemKind::parse(&k)
            .map(Some)
            .ok_or_else(|| Error::Invalid(format!("unknown kind '{k}'"))),
    }
}
fn arg_kind(a: &Value) -> Result<ItemKind> {
    opt_kind(a)?.ok_or_else(|| Error::Invalid("missing 'kind' (live|movie|series|episode)".into()))
}
fn usize_or(a: &Value, k: &str, d: usize) -> usize {
    a.int(k).map_or(d, |n| n.max(0) as usize)
}

fn items_json(v: &[&Item]) -> Value {
    Value::Array(v.iter().map(|i| i.to_json()).collect())
}

/// Item JSON plus live state the UI almost always wants alongside it.
fn item_rich(c: &Catalog, it: &Item) -> Value {
    let mut v = it.to_json();
    if let Value::Object(m) = &mut v {
        m.insert("favorite".into(), c.is_favorite(it.id).into());
        if it.kind == ItemKind::Episode {
            if let Some(sid) = it.extra("series_id") {
                // Lets the UI open the series page from an episode (continue watching).
                let series = madhyam_core::item_id(it.source, ItemKind::Series, sid);
                m.insert("series".into(), series.to_string().into());
            }
        }
        if let Some(p) = c.progress(it.id) {
            m.insert(
                "progress".into(),
                obj([
                    ("position", p.position.into()),
                    ("duration", p.duration.into()),
                ]),
            );
        }
    }
    v
}

fn xtream_client(c: &Catalog, source: SourceId) -> Result<XtreamClient> {
    match &c
        .source(source)
        .ok_or_else(|| Error::NotFound(format!("source {source}")))?
        .kind
    {
        SourceKind::Xtream(cr) => Ok(XtreamClient::new(cr.clone(), source)),
        SourceKind::M3u { .. } => Err(Error::Invalid(format!(
            "source {source} is not an Xtream source"
        ))),
    }
}

fn open_file(path: &str) -> Result<BufReader<std::fs::File>> {
    Ok(BufReader::with_capacity(
        256 * 1024,
        std::fs::File::open(path)?,
    ))
}

// =============================================================== dispatch

/// All `iptv_call` methods. Safe Rust, so it is unit-tested directly.
pub fn dispatch(st: &mut State, method: &str, a: &Value) -> Result<Value> {
    let now = a.int("now").unwrap_or_else(time::now);
    let c = &mut st.catalog;
    Ok(match method {
        // ---------------------------------------------------------- sources
        "add_m3u_source" => {
            let id = c.add_source(
                &arg_str(a, "name")?,
                SourceKind::M3u {
                    location: opt_str(a, "location").unwrap_or_default(),
                },
            );
            obj([("id", id.into())])
        }
        "add_xtream_source" => {
            let mut cr = XtreamCredentials::new(
                &arg_str(a, "server")?,
                &arg_str(a, "username")?,
                &arg_str(a, "password")?,
            );
            if let Some(ext) = opt_str(a, "live_ext") {
                cr.live_ext = ext;
            }
            let id = c.add_source(&arg_str(a, "name")?, SourceKind::Xtream(cr));
            obj([("id", id.into())])
        }
        "sources" => Value::Array(c.sources().iter().map(|s| s.to_json()).collect()),
        "rename_source" => {
            c.rename_source(arg_source(a)?, &arg_str(a, "name")?)?;
            Value::Null
        }
        // The stored Xtream password, only for the "Edit playlist" form (never part of `sources`).
        "source_password" => match &c
            .source(arg_source(a)?)
            .ok_or_else(|| Error::NotFound("source".into()))?
            .kind
        {
            SourceKind::Xtream(cr) => cr.password.as_str().into(),
            SourceKind::M3u { .. } => Value::Null,
        },
        // Edit a source. Omitted fields (and an empty password) keep their value.
        "update_source" => {
            let id = arg_source(a)?;
            if let Some(n) = opt_str(a, "name") {
                c.rename_source(id, &n)?;
            }
            let kind = match &c
                .source(id)
                .ok_or_else(|| Error::NotFound(format!("source {id}")))?
                .kind
            {
                SourceKind::M3u { location } => SourceKind::M3u {
                    location: opt_str(a, "location").unwrap_or_else(|| location.clone()),
                },
                SourceKind::Xtream(old) => {
                    let mut cr = XtreamCredentials::new(
                        &opt_str(a, "server").unwrap_or_else(|| old.server.clone()),
                        &opt_str(a, "username").unwrap_or_else(|| old.username.clone()),
                        &opt_str(a, "password").unwrap_or_else(|| old.password.clone()),
                    );
                    cr.live_ext = opt_str(a, "live_ext").unwrap_or_else(|| old.live_ext.clone());
                    SourceKind::Xtream(cr)
                }
            };
            c.update_source(id, kind)?;
            Value::Null
        }
        // Xtream account endpoint (status, expiry, connections). Contains the login.
        "account_url" => xtream_client(c, arg_source(a)?)?.api_url(None, &[]).into(),
        "remove_source" => {
            let s = arg_source(a)?;
            c.remove_source(s)?;
            st.pending_xtream.remove(&s);
            Value::Null
        }
        // Contains credentials for Xtream: give to the HTTP layer only.
        "epg_fetch_urls" => {
            let s = arg_source(a)?;
            c.source(s)
                .ok_or_else(|| Error::NotFound(format!("source {s}")))?
                .epg_fetch_urls()
                .into()
        }
        "import_m3u_file" => c
            .import_m3u(arg_source(a)?, open_file(&arg_str(a, "path")?)?)?
            .to_json(),
        "import_xmltv_file" => {
            let keep = a.int("keep_from").unwrap_or(now - 86_400);
            c.import_xmltv(open_file(&arg_str(a, "path")?)?, keep)?
                .to_json()
        }

        // ----------------------------------------------------------- xtream
        // Step 1: which URLs to GET. Step 2: iptv_import("xtream_part", body,
        // {source, part}) for each. Step 3: xtream_commit.
        "xtream_requests" => {
            let x = xtream_client(c, arg_source(a)?)?;
            Value::Array(
                XTREAM_PARTS
                    .iter()
                    .map(|(part, action)| {
                        obj([
                            ("part", (*part).into()),
                            ("url", x.api_url(*action, &[]).into()),
                        ])
                    })
                    .collect(),
            )
        }
        "xtream_commit" => {
            let s = arg_source(a)?;
            let x = xtream_client(c, s)?;
            let parts = st.pending_xtream.remove(&s).unwrap_or_default();
            // Re-use the core's sync driver by serving it the stored bodies.
            let fetch = |url: &str, _: &[(&str, &str)]| -> Result<Vec<u8>> {
                XTREAM_PARTS
                    .iter()
                    .find(|(_, action)| x.api_url(*action, &[]) == url)
                    .and_then(|(p, _)| parts.get(*p).cloned())
                    .ok_or_else(|| Error::Api(format!("part for {url} was not imported")))
            };
            let snap: XtreamSnapshot = x.sync(&fetch)?;
            let warnings = snap.warnings.clone();
            let account = snap.account.to_json();
            let stats = c.apply_xtream(s, snap)?;
            obj([
                ("items", stats.items.into()),
                ("account", account),
                ("warnings", warnings.into()),
            ])
        }
        "series_info_url" => {
            let it = c
                .item(arg_id(a, "id")?)
                .ok_or_else(|| Error::NotFound("item".into()))?;
            let x = xtream_client(c, it.source)?;
            x.api_url(Some("get_series_info"), &[("series_id", &it.remote_id)])
                .into()
        }
        "vod_info_url" => {
            let it = c
                .item(arg_id(a, "id")?)
                .ok_or_else(|| Error::NotFound("item".into()))?;
            let x = xtream_client(c, it.source)?;
            x.api_url(Some("get_vod_info"), &[("vod_id", &it.remote_id)])
                .into()
        }
        "short_epg_url" => {
            let it = c
                .item(arg_id(a, "id")?)
                .ok_or_else(|| Error::NotFound("item".into()))?;
            let x = xtream_client(c, it.source)?;
            let limit = usize_or(a, "limit", 10).to_string();
            x.api_url(
                Some("get_short_epg"),
                &[("stream_id", &it.remote_id), ("limit", &limit)],
            )
            .into()
        }
        "timeshift_url" => {
            let it = c
                .item(arg_id(a, "id")?)
                .ok_or_else(|| Error::NotFound("item".into()))?;
            let x = xtream_client(c, it.source)?;
            let start = a
                .int("start")
                .ok_or_else(|| Error::Invalid("missing 'start'".into()))?;
            let dur = a.int("duration_min").unwrap_or(60).clamp(1, 24 * 60) as u32;
            x.timeshift_url(
                &it.remote_id,
                start,
                dur,
                a.int("server_offset").unwrap_or(0),
            )
            .into()
        }

        // ---------------------------------------------------------- browse
        "stats" => obj([
            ("sources", c.sources().len().into()),
            ("items", c.len().into()),
            ("programmes", c.epg_programme_count().into()),
        ]),
        "groups" => c
            .groups_by(
                arg_source(a)?,
                arg_kind(a)?,
                opt_str(a, "group_field").as_deref(),
            )
            .to_json(),
        "list" => {
            let g = opt_str(a, "group");
            let v = c.list(
                arg_source(a)?,
                arg_kind(a)?,
                g.as_deref(),
                usize_or(a, "offset", 0),
                usize_or(a, "limit", 100),
            );
            items_json(&v)
        }
        // Sorted/filtered/paged listing: {kind, source?, group?, hidden_groups?[],
        // sort?, min_rating?, offset?, limit?} -> {total, items}
        "query" => {
            let mut q = ListQuery::new(arg_kind(a)?);
            q.source = a.int("source").map(|s| s as SourceId);
            q.group = opt_str(a, "group");
            q.group_field = opt_str(a, "group_field");
            q.hidden_groups = a
                .get("hidden_groups")
                .and_then(Value::as_array)
                .map(|v| {
                    v.iter()
                        .filter_map(|g| g.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            if let Some(sort) = opt_str(a, "sort") {
                q.sort = Sort::parse(&sort)
                    .ok_or_else(|| Error::Invalid(format!("unknown sort '{sort}'")))?;
            }
            q.min_rating = a.num("min_rating").unwrap_or(0.0);
            q.offset = usize_or(a, "offset", 0);
            q.limit = usize_or(a, "limit", 100);
            let (total, page) = c.query(&q);
            obj([("total", total.into()), ("items", items_json(&page))])
        }
        "item" => {
            let it = c
                .item(arg_id(a, "id")?)
                .ok_or_else(|| Error::NotFound("item".into()))?;
            item_rich(c, it)
        }
        "episodes" => Value::Array(
            c.episodes(arg_id(a, "id")?)
                .into_iter()
                .map(|it| item_rich(c, it))
                .collect(),
        ),
        "search" => {
            let f = SearchFilter {
                kind: opt_kind(a)?,
                source: a.int("source").map(|s| s as SourceId),
            };
            items_json(&c.search(&arg_str(a, "query")?, f, usize_or(a, "limit", 50)))
        }

        // ------------------------------------------------------------- EPG
        "now_next" => {
            let (n, x) = c.now_next(arg_id(a, "id")?, now);
            obj([
                ("now", n.cloned().to_json()),
                ("next", x.cloned().to_json()),
            ])
        }
        // Batch form for channel lists: {"ids": [...]} -> {"<id>": {now, next}}
        "now_next_many" => {
            let ids = a.get("ids").and_then(Value::as_array).unwrap_or(&[]);
            let mut m = std::collections::BTreeMap::new();
            for v in ids {
                let id = arg_id(&obj([("id", v.clone())]), "id")?;
                let (n, x) = c.now_next(id, now);
                m.insert(
                    id.to_string(),
                    obj([
                        ("now", n.cloned().to_json()),
                        ("next", x.cloned().to_json()),
                    ]),
                );
            }
            Value::Object(m)
        }
        "guide" => {
            let from = a.int("from").unwrap_or(now - 3600);
            let to = a.int("to").unwrap_or(now + 6 * 3600);
            let ids = a.get("ids").and_then(Value::as_array).unwrap_or(&[]);
            let mut m = std::collections::BTreeMap::new();
            for v in ids {
                let id = arg_id(&obj([("id", v.clone())]), "id")?;
                m.insert(id.to_string(), c.guide(id, from, to).to_vec().to_json());
            }
            Value::Object(m)
        }
        "clear_epg" => {
            c.clear_epg();
            Value::Null
        }

        // ------------------------------------------------------ user state
        "set_favorite" => {
            c.set_favorite(
                arg_id(a, "id")?,
                a.get("on").is_none_or(Value::to_bool),
                now,
            );
            Value::Null
        }
        "favorites" => Value::Array(
            c.favorites(opt_kind(a)?)
                .into_iter()
                .map(|it| item_rich(c, it))
                .collect(),
        ),
        "set_progress" => {
            let pos = a
                .num("position")
                .ok_or_else(|| Error::Invalid("missing 'position'".into()))?;
            c.set_progress(arg_id(a, "id")?, pos, a.num("duration").unwrap_or(0.0), now);
            Value::Null
        }
        "continue_watching" => Value::Array(
            c.continue_watching(usize_or(a, "limit", 20))
                .into_iter()
                .map(|(it, _)| item_rich(c, it))
                .collect(),
        ),
        "record_play" => {
            c.record_play(arg_id(a, "id")?, now);
            Value::Null
        }
        "recent" => Value::Array(
            c.recent(opt_kind(a)?, usize_or(a, "limit", 20))
                .into_iter()
                .map(|it| {
                    let mut v = item_rich(c, it);
                    if let (Value::Object(m), Some(t)) = (&mut v, c.last_played(it.id)) {
                        m.insert("played_at".into(), t.into());
                    }
                    v
                })
                .collect(),
        ),
        "remove_history" => {
            c.remove_from_history(arg_id(a, "id")?);
            Value::Null
        }
        "clear_history" => {
            c.clear_history();
            Value::Null
        }
        "export_user_state" => c.export_user_state(),
        "import_user_state" => {
            c.import_user_state(
                a.get("state")
                    .ok_or_else(|| Error::Invalid("missing 'state'".into()))?,
            )?;
            Value::Null
        }

        // -------------------------------------------------------- snapshot
        "save_snapshot" => {
            let path = arg_str(a, "path")?;
            // Write to a temp file and rename, so a crash never leaves half a snapshot.
            let tmp = format!("{path}.tmp");
            {
                let f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
                c.save_snapshot_with(f, a.get("redact_passwords").is_some_and(Value::to_bool))?;
            }
            std::fs::rename(&tmp, &path)?;
            Value::Null
        }
        "load_snapshot" => {
            *c = Catalog::load_snapshot(open_file(&arg_str(a, "path")?)?)?;
            st.pending_xtream.clear();
            obj([("items", st.catalog.len().into())])
        }

        _ => return Err(Error::Invalid(format!("unknown method '{method}'"))),
    })
}

/// All `iptv_import` methods: bulk bytes in, so large payloads skip JSON.
pub fn dispatch_import(st: &mut State, method: &str, data: &[u8], a: &Value) -> Result<Value> {
    let now = a.int("now").unwrap_or_else(time::now);
    Ok(match method {
        "m3u" => st.catalog.import_m3u(arg_source(a)?, data)?.to_json(),
        // Tag a source's items from another playlist's group titles: {source, field}
        "m3u_tags" => st
            .catalog
            .tag_from_m3u(arg_source(a)?, &arg_str(a, "field")?, data)?
            .into(),
        "xmltv" => st
            .catalog
            .import_xmltv(data, a.int("keep_from").unwrap_or(now - 86_400))?
            .to_json(),
        "xtream_part" => {
            let s = arg_source(a)?;
            let part = arg_str(a, "part")?;
            if !XTREAM_PARTS.iter().any(|(p, _)| *p == part) {
                return Err(Error::Invalid(format!("unknown Xtream part '{part}'")));
            }
            xtream_client(&st.catalog, s)?;
            // Validate the account answer right away so the host can show "wrong password" early.
            if part == "account" {
                let acc = xtream_client(&st.catalog, s)?.parse_account(data)?;
                st.pending_xtream
                    .entry(s)
                    .or_default()
                    .insert(part, data.to_vec());
                return Ok(acc.to_json());
            }
            st.pending_xtream
                .entry(s)
                .or_default()
                .insert(part, data.to_vec());
            Value::Null
        }
        "series_info" => {
            let id = arg_id(a, "id")?;
            let series = st
                .catalog
                .item(id)
                .ok_or_else(|| Error::NotFound("item".into()))?
                .clone();
            let eps =
                xtream_client(&st.catalog, series.source)?.parse_series_info(data, &series)?;
            let n = eps.len();
            st.catalog.set_episodes(id, eps)?;
            obj([("episodes", n.into())])
        }
        "short_epg" => {
            let id = arg_id(a, "id")?;
            let it = st
                .catalog
                .item(id)
                .ok_or_else(|| Error::NotFound("item".into()))?
                .clone();
            let mut progs =
                xtream_client(&st.catalog, it.source)?.parse_short_epg(data, &it.epg_id)?;
            // Short EPG may lack epg ids; key it by the item's so now_next finds it.
            let key = if it.epg_id.is_empty() {
                Catalog::item_epg_key(it.id)
            } else {
                it.epg_id.clone()
            };
            for p in &mut progs {
                p.channel = key.clone();
            }
            let n = progs.len();
            st.catalog.add_programmes(progs);
            obj([("programmes", n.into())])
        }
        // Stateless helper: parse a playlist and return its entries.
        "parse_m3u" => {
            let mut r = m3u::M3uReader::new(data);
            let entries: Vec<_> = r.by_ref().filter_map(|e| e.ok()).collect();
            obj([
                ("epg_urls", r.header().epg_urls().into()),
                ("entries", entries.to_json()),
            ])
        }
        "user_state" => {
            let v = json::parse_bytes(data)?;
            st.catalog.import_user_state(&v)?;
            Value::Null
        }
        _ => return Err(Error::Invalid(format!("unknown import method '{method}'"))),
    })
}

// ================================================================ C ABI

fn envelope(r: std::thread::Result<Result<Value>>) -> *mut c_char {
    let v = match r {
        Ok(Ok(v)) => obj([("ok", v)]),
        Ok(Err(e)) => obj([(
            "error",
            obj([("code", e.code().into()), ("message", e.to_string().into())]),
        )]),
        Err(_) => obj([(
            "error",
            obj([
                ("code", "panic".into()),
                ("message", "internal error (panic) in madhyam_core".into()),
            ]),
        )]),
    };
    // JSON strings escape control chars, so the output never contains a NUL.
    CString::new(v.to_string()).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// # Safety
/// `p` must be NULL or a valid NUL-terminated string.
unsafe fn cstr<'a>(p: *const c_char) -> Result<&'a str> {
    if p.is_null() {
        return Ok("");
    }
    // SAFETY: guaranteed by the caller.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| Error::Invalid("argument is not valid UTF-8".into()))
}

fn parse_args(s: &str) -> Result<Value> {
    if s.trim().is_empty() {
        Ok(Value::Object(Default::default()))
    } else {
        json::parse(s)
    }
}

fn with_state<T>(core: &MadhyamCore, f: impl FnOnce(&mut State) -> T) -> T {
    // A panic while holding the lock poisons it; the data is still usable.
    let mut g = core.state.lock().unwrap_or_else(|p| p.into_inner());
    f(&mut g)
}

/// Create a core. Free it with [`iptv_core_free`].
#[unsafe(no_mangle)]
pub extern "C" fn iptv_core_new() -> *mut MadhyamCore {
    Box::into_raw(Box::new(MadhyamCore {
        state: Mutex::new(State::default()),
    }))
}

/// # Safety
/// `core` must come from [`iptv_core_new`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iptv_core_free(core: *mut MadhyamCore) {
    if !core.is_null() {
        // SAFETY: pointer was produced by Box::into_raw in iptv_core_new.
        drop(unsafe { Box::from_raw(core) });
    }
}

/// Call a method. Returns a JSON envelope to free with [`iptv_string_free`].
///
/// # Safety
/// `core` must be a live handle; `method` and `args_json` NUL-terminated UTF-8 (args may be NULL).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iptv_call(
    core: *mut MadhyamCore,
    method: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    envelope(catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: caller contract.
        let core = unsafe { core.as_ref() }.ok_or_else(|| Error::Invalid("null core".into()))?;
        let method = unsafe { cstr(method) }?;
        let args = parse_args(unsafe { cstr(args_json) }?)?;
        with_state(core, |st| dispatch(st, method, &args))
    })))
}

/// Feed raw bytes (playlist, guide, Xtream response) to an import method.
///
/// # Safety
/// As [`iptv_call`]; additionally `data` must point to `len` readable bytes (or be NULL with `len == 0`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iptv_import(
    core: *mut MadhyamCore,
    method: *const c_char,
    data: *const u8,
    len: usize,
    args_json: *const c_char,
) -> *mut c_char {
    envelope(catch_unwind(AssertUnwindSafe(|| {
        let core = unsafe { core.as_ref() }.ok_or_else(|| Error::Invalid("null core".into()))?;
        let method = unsafe { cstr(method) }?;
        let args = parse_args(unsafe { cstr(args_json) }?)?;
        let bytes: &[u8] = if data.is_null() || len == 0 {
            &[]
        } else {
            // SAFETY: caller guarantees `len` readable bytes at `data`.
            unsafe { std::slice::from_raw_parts(data, len) }
        };
        with_state(core, |st| dispatch_import(st, method, bytes, &args))
    })))
}

/// # Safety
/// `s` must be NULL or a string returned by this library, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iptv_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: produced by CString::into_raw in `envelope`.
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Library version, static string (do not free).
#[unsafe(no_mangle)]
pub extern "C" fn iptv_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(st: &mut State, m: &str, a: &str) -> Value {
        dispatch(st, m, &json::parse(a).unwrap()).unwrap_or_else(|e| panic!("{m}: {e}"))
    }

    #[test]
    fn m3u_flow_through_dispatch() {
        let mut st = State::default();
        let id = call(&mut st, "add_m3u_source", r#"{"name":"Home"}"#)
            .int("id")
            .unwrap();
        let pl = "#EXTM3U\n#EXTINF:-1 tvg-id=\"a\" group-title=\"News\",Alpha News\nhttp://x/1\n#EXTINF:-1 group-title=\"News\",Beta\nhttp://x/2\n";
        let r =
            dispatch_import(&mut st, "m3u", pl.as_bytes(), &obj([("source", id.into())])).unwrap();
        assert_eq!(r.int("items"), Some(2));
        let g = call(
            &mut st,
            "groups",
            &format!(r#"{{"source":{id},"kind":"live"}}"#),
        );
        assert_eq!(g.as_array().unwrap()[0].int("count"), Some(2));
        let hits = call(&mut st, "search", r#"{"query":"alp"}"#);
        let item_id = hits.as_array().unwrap()[0].text("id").unwrap();
        call(&mut st, "set_favorite", &format!(r#"{{"id":"{item_id}"}}"#));
        let it = call(&mut st, "item", &format!(r#"{{"id":"{item_id}"}}"#));
        assert!(it.flag("favorite"));
        let guide = "<tv><programme start=\"20261012100000\" stop=\"20261012110000\" channel=\"a\"><title>Show</title></programme></tv>";
        dispatch_import(
            &mut st,
            "xmltv",
            guide.as_bytes(),
            &obj([("keep_from", 0i64.into())]),
        )
        .unwrap();
        let now = time::unix_from_parts(2026, 10, 12, 10, 30, 0);
        let nn = call(
            &mut st,
            "now_next",
            &format!(r#"{{"id":"{item_id}","now":{now}}}"#),
        );
        assert_eq!(
            nn.get("now").unwrap().text("title").as_deref(),
            Some("Show")
        );
        assert!(dispatch(&mut st, "nope", &Value::Null).is_err());
    }

    #[test]
    fn xtream_staged_sync() {
        let mut st = State::default();
        let s = call(
            &mut st,
            "add_xtream_source",
            r#"{"name":"P","server":"h:1","username":"u","password":"p"}"#,
        )
        .int("id")
        .unwrap();
        let reqs = call(&mut st, "xtream_requests", &format!(r#"{{"source":{s}}}"#));
        assert_eq!(reqs.as_array().unwrap().len(), 7);
        let bodies: [(&str, &str); 7] = [
            (
                "account",
                r#"{"user_info":{"auth":1,"status":"Active"},"server_info":{}}"#,
            ),
            (
                "live_categories",
                r#"[{"category_id":"1","category_name":"News"}]"#,
            ),
            (
                "live_streams",
                r#"[{"stream_id":1,"name":"CNN","category_id":"1"}]"#,
            ),
            ("vod_categories", "[]"),
            ("vod_streams", "garbage"),
            ("series_categories", "[]"),
            ("series", r#"[{"series_id":3,"name":"Show"}]"#),
        ];
        for (part, body) in bodies {
            dispatch_import(
                &mut st,
                "xtream_part",
                body.as_bytes(),
                &obj([("source", s.into()), ("part", part.into())]),
            )
            .unwrap();
        }
        let r = call(&mut st, "xtream_commit", &format!(r#"{{"source":{s}}}"#));
        assert_eq!(r.int("items"), Some(2));
        assert_eq!(
            r.get("warnings").unwrap().as_array().unwrap().len(),
            1,
            "bad vod list is a warning"
        );
        let bad = dispatch_import(
            &mut st,
            "xtream_part",
            br#"{"user_info":{"auth":0}}"#,
            &obj([("source", s.into()), ("part", "account".into())]),
        );
        assert!(bad.is_err());
    }

    #[test]
    fn c_abi_roundtrip() {
        let core = iptv_core_new();
        let m = CString::new("add_m3u_source").unwrap();
        let a = CString::new(r#"{"name":"x"}"#).unwrap();
        unsafe {
            let out = iptv_call(core, m.as_ptr(), a.as_ptr());
            let s = CStr::from_ptr(out).to_str().unwrap().to_string();
            iptv_string_free(out);
            assert_eq!(s, r#"{"ok":{"id":1}}"#);
            let bad = CString::new("{not json").unwrap();
            let out = iptv_call(core, m.as_ptr(), bad.as_ptr());
            assert!(
                CStr::from_ptr(out)
                    .to_str()
                    .unwrap()
                    .contains(r#""code":"parse""#)
            );
            iptv_string_free(out);
            let out = iptv_call(std::ptr::null_mut(), m.as_ptr(), a.as_ptr());
            assert!(CStr::from_ptr(out).to_str().unwrap().contains("null core"));
            iptv_string_free(out);
            iptv_core_free(core);
            assert!(
                CStr::from_ptr(iptv_version())
                    .to_str()
                    .unwrap()
                    .starts_with("0.")
            );
        }
    }
}
