//! Minimal client for the OpenSubtitles.com REST API (the service VLsub uses). Needs the
//! user's free API key; logging in raises the daily download quota. Blocking calls.

use crate::http::{self, AppError, Res};
use crate::settings;
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct SubtitleHit {
    pub file_id: i64,
    pub language: String,
    pub release: String,
    pub file_name: String,
    pub downloads: i64,
    pub hearing_impaired: bool,
    pub title: String,
}

pub struct Client {
    base: String,
    token: Option<String>,
}

pub const KEYRING_KEY: &str = "opensubtitles";

impl Client {
    pub fn new() -> Client {
        Client {
            base: "https://api.opensubtitles.com/api/v1".into(),
            token: None,
        }
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        let mut h = vec![
            ("Api-Key", settings::get().os_api_key.trim().to_string()),
            (
                "User-Agent",
                format!("{} v{}", crate::brand::NAME, crate::brand::VERSION),
            ),
            ("Accept", "application/json".into()),
        ];
        if let Some(t) = &self.token {
            h.push(("Authorization", format!("Bearer {t}")));
        }
        h
    }

    fn check(status: u16, body: &[u8]) -> Res<Value> {
        let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        if (200..300).contains(&status) {
            return Ok(v);
        }
        let msg = v
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("HTTP {status}"));
        Err(AppError::Network(if status == 401 || status == 403 {
            format!(
                "OpenSubtitles refused the request ({msg}). Check the API key and login in Settings › Playback."
            )
        } else {
            format!("OpenSubtitles: {msg}")
        }))
    }

    fn ensure_login(&mut self) -> Res<()> {
        let user = settings::get().os_username.clone();
        if self.token.is_some() || user.is_empty() {
            return Ok(());
        }
        let Some(pass) = crate::secrets::lookup(KEYRING_KEY).filter(|p| !p.is_empty()) else {
            return Ok(());
        };
        let (status, body) = http::post_json(
            &format!("{}/login", self.base),
            &self.headers(),
            &json!({ "username": user, "password": pass }),
        )?;
        let v = Self::check(status, &body)?;
        self.token = v.get("token").and_then(Value::as_str).map(str::to_string);
        if let Some(host) = v
            .get("base_url")
            .and_then(Value::as_str)
            .filter(|h| !h.is_empty())
        {
            self.base = format!("https://{host}/api/v1");
        }
        Ok(())
    }

    pub fn search(
        &mut self,
        query: &str,
        languages: &str,
        season: Option<i32>,
        episode: Option<i32>,
    ) -> Res<Vec<SubtitleHit>> {
        self.ensure_login()?;
        let enc = crate::ratings::url_encode;
        let mut q = vec![format!("query={}", enc(&query.to_lowercase()))];
        let langs = languages.replace(' ', "").to_lowercase();
        if !langs.is_empty() {
            q.push(format!("languages={}", enc(&langs)));
        }
        if let Some(s) = season {
            q.push(format!("season_number={s}"));
        }
        if let Some(e) = episode {
            q.push(format!("episode_number={e}"));
        }
        q.sort(); // the API redirects unless parameters are sorted
        let (status, body) = http::get_with(
            &format!("{}/subtitles?{}", self.base, q.join("&")),
            &self.headers(),
        )?;
        let v = Self::check(status, &body)?;
        let mut hits = Vec::new();
        for x in v
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(a) = x.get("attributes") else {
                continue;
            };
            let title = a
                .get("feature_details")
                .and_then(|f| f.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            for file in a
                .get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                hits.push(SubtitleHit {
                    file_id: file.get("file_id").and_then(Value::as_i64).unwrap_or(0),
                    language: a
                        .get("language")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    release: a
                        .get("release")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    file_name: file
                        .get("file_name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    downloads: a.get("download_count").and_then(Value::as_i64).unwrap_or(0),
                    hearing_impaired: a
                        .get("hearing_impaired")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    title: title.clone(),
                });
            }
        }
        Ok(hits)
    }

    /// Download one subtitle file into the data folder; returns (path, downloads left).
    pub fn download(&mut self, hit: &SubtitleHit) -> Res<(String, String)> {
        self.ensure_login()?;
        let (status, body) = http::post_json(
            &format!("{}/download", self.base),
            &self.headers(),
            &json!({ "file_id": hit.file_id }),
        )?;
        let v = Self::check(status, &body)?;
        let link = v
            .get("link")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Network("OpenSubtitles gave no download link.".into()))?;
        let name = v
            .get("file_name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}.srt", hit.file_id));
        let remaining = v
            .get("remaining")
            .map(|r| r.to_string())
            .unwrap_or_else(|| "?".into());
        let dir = crate::paths::data_dir().join("subtitles");
        std::fs::create_dir_all(&dir)?;
        let safe: String = name
            .chars()
            .map(|c| if c == '/' || c == '\0' { '_' } else { c })
            .collect();
        let path = dir.join(safe);
        std::fs::write(&path, http::get_bytes(link, None)?)?;
        Ok((path.to_string_lossy().into_owned(), remaining))
    }
}
