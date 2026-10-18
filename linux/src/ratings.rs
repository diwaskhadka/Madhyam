//! IMDb rating and Rotten Tomatoes score from OMDb (omdbapi.com, the user's free key).
//! Nothing is shown when there is no key, no match, or no score.

use crate::settings;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

#[derive(Clone, Debug)]
pub struct ExternalRatings {
    pub imdb: Option<String>,
    pub imdb_votes: Option<String>,
    pub rotten: Option<String>,
}

static CACHE: LazyLock<Mutex<HashMap<String, Option<ExternalRatings>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub fn url_encode(s: &str) -> String {
    encode(s)
}

/// Blocking: call from a worker thread.
pub fn fetch(
    title: &str,
    year: Option<&str>,
    imdb_id: Option<&str>,
    series: bool,
) -> Option<ExternalRatings> {
    let key = settings::get().omdb_api_key.trim().to_string();
    if key.is_empty() {
        return None;
    }
    let cache_key = imdb_id
        .map(str::to_string)
        .unwrap_or_else(|| format!("{title}|{}|{series}", year.unwrap_or("")));
    if let Some(hit) = CACHE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&cache_key)
    {
        return hit.clone();
    }
    let q = match imdb_id {
        Some(id) => format!("i={}", encode(id)),
        None => format!(
            "t={}{}&type={}",
            encode(title),
            year.filter(|y| y.len() == 4)
                .map(|y| format!("&y={y}"))
                .unwrap_or_default(),
            if series { "series" } else { "movie" }
        ),
    };
    let result = (|| {
        let body = crate::http::get_quick(
            &format!("https://www.omdbapi.com/?apikey={}&{q}", encode(&key)),
            None,
        )
        .ok()?;
        let r: Value = serde_json::from_slice(&body).ok()?;
        if r.get("Response").and_then(Value::as_str) != Some("True") {
            return None;
        }
        let val = |e: &Value, k: &str| {
            e.get(k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && *s != "N/A")
                .map(str::to_string)
        };
        let rotten = r
            .get("Ratings")
            .and_then(Value::as_array)
            .and_then(|l| {
                l.iter()
                    .find(|x| val(x, "Source").as_deref() == Some("Rotten Tomatoes"))
            })
            .and_then(|x| val(x, "Value"));
        let imdb = val(&r, "imdbRating");
        (imdb.is_some() || rotten.is_some()).then(|| ExternalRatings {
            imdb,
            imdb_votes: val(&r, "imdbVotes"),
            rotten,
        })
    })();
    CACHE
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(cache_key, result.clone());
    result
}

static LEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:4K-)?[A-Z0-9]{2,4}\s*[-|:]\s+").unwrap());
static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\(\[].*?[\)\]]").unwrap());

/// "EN - The Movie (2024) [4K]" → "The Movie".
pub fn clean_title(name: &str) -> String {
    let s = LEAD.replace(name, "");
    let s = BRACKETS.replace_all(&s, "");
    s.trim_matches(|c: char| c == ' ' || c == '-' || c == '|')
        .to_string()
}
