//! User preferences in settings.json in the data folder. Same keys and defaults as the Windows
//! app (AppSettings.cs), so the file means the same thing on both. Every field has a default,
//! so an old or missing file always loads.

use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    // ---- General
    /// Dark (Navy & Crimson) | Light | System | Custom
    pub theme: String,
    pub custom_color: String,
    pub custom_light: bool,
    /// Small | Medium | Large
    pub cover_size: String,
    pub titles_under_covers: bool,
    /// Dashboard | Live | Movies | Series | Favorites | Recent | Last
    pub startup_view: String,
    pub last_view: String,
    /// Last | Maximized | Fullscreen
    pub window_startup: String,
    pub win_width: Option<f64>,
    pub win_height: Option<f64>,
    pub win_maximized: bool,
    pub strip_prefixes: bool,
    pub show_numbers: bool,

    // ---- Playback
    /// Auto | TS | HLS
    pub live_format: String,
    pub network_caching_ms: i32,
    pub hardware_decoding: bool,
    pub resume_playback: bool,
    pub show_subtitles: bool,
    pub audio_language: String,
    pub subtitle_language: String,
    pub os_api_key: String,
    pub os_username: String,
    /// Not used on Linux (the password lives in the keyring); kept so the key round-trips.
    pub os_password: String,
    pub os_languages: String,
    pub omdb_api_key: String,

    // ---- Subtitle style (YouTube-like caption options)
    pub sub_font_size_pt: i32,
    /// Sans-serif | Serif | Monospace | Casual | Small caps
    pub sub_font: String,
    pub sub_text_color: String,
    pub sub_text_opacity: i32,
    pub sub_back_color: String,
    pub sub_back_opacity: i32,
    pub sub_window_color: String,
    pub sub_window_opacity: i32,
    /// None | Drop shadow | Raised | Depressed | Outline
    pub sub_edge: String,
    /// Bottom | Lower third | Middle | Top
    pub sub_position: String,
    /// Fine position in %, positive = up.
    pub sub_pos_offset: i32,

    // ---- Parental controls
    pub parental_enabled: bool,
    /// SHA-256 (hex) of salt + PIN; the PIN itself is never stored.
    pub parental_pin_hash: String,
    pub parental_pin_salt: String,
    pub parental_auto_adult: bool,
    pub parental_lock_settings: bool,
    /// 0 = any age, else 7 / 12 / 16 / 18.
    pub parental_max_age: i32,
    /// Locked / explicitly unlocked category names per "sourceId:kind" (like HiddenCategories).
    pub locked_categories: HashMap<String, Vec<String>>,
    pub unlocked_categories: HashMap<String, Vec<String>>,

    // ---- Guide
    /// Timeline | List
    pub guide_view: String,
    pub epg_offset_minutes: i32,
    pub epg_refresh_hours: i32,
    pub extra_epg_urls: Vec<String>,

    // ---- Dashboard
    pub dash_hero: bool,
    pub dash_continue: bool,
    pub dash_live_favorites: bool,
    pub dash_recent_live: bool,
    pub dash_favorite_vod: bool,
    pub dash_recently_added: bool,

    // ---- Browsing state
    /// Hidden categories per "sourceId:kind".
    pub hidden_categories: HashMap<String, Vec<String>>,
    /// Category order per kind: provider | name_asc | name_desc.
    pub category_sort: HashMap<String, String>,
    pub live_sort_az: bool,
    pub vod_sort: String,
    pub vod_min_rating: f64,
    pub per_page: i32,

    // ---- Sources
    pub source_added: HashMap<i64, i64>,
    pub source_user_agent: HashMap<i64, String>,
    pub free_tv_source_id: Option<i64>,
    pub free_tv_group_by: String,
    pub free_tv_preinstalled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme: "Dark".into(),
            custom_color: crate::theme::CRIMSON.into(),
            custom_light: false,
            cover_size: "Medium".into(),
            titles_under_covers: true,
            startup_view: "Dashboard".into(),
            last_view: "Dashboard".into(),
            window_startup: "Last".into(),
            win_width: None,
            win_height: None,
            win_maximized: false,
            strip_prefixes: false,
            show_numbers: false,
            live_format: "Auto".into(),
            network_caching_ms: 1500,
            hardware_decoding: true,
            resume_playback: true,
            show_subtitles: true,
            audio_language: String::new(),
            subtitle_language: String::new(),
            os_api_key: String::new(),
            os_username: String::new(),
            os_password: String::new(),
            os_languages: "en".into(),
            omdb_api_key: String::new(),
            sub_font_size_pt: 24,
            sub_font: "Sans-serif".into(),
            sub_text_color: "White".into(),
            sub_text_opacity: 100,
            sub_back_color: "Black".into(),
            sub_back_opacity: 50,
            sub_window_color: "Black".into(),
            sub_window_opacity: 0,
            sub_edge: "Drop shadow".into(),
            sub_position: "Bottom".into(),
            sub_pos_offset: 0,
            parental_enabled: false,
            parental_pin_hash: String::new(),
            parental_pin_salt: String::new(),
            parental_auto_adult: true,
            parental_lock_settings: false,
            parental_max_age: 0,
            locked_categories: HashMap::new(),
            unlocked_categories: HashMap::new(),
            guide_view: "Timeline".into(),
            epg_offset_minutes: 0,
            epg_refresh_hours: 12,
            extra_epg_urls: Vec::new(),
            dash_hero: true,
            dash_continue: true,
            dash_live_favorites: true,
            dash_recent_live: true,
            dash_favorite_vod: true,
            dash_recently_added: true,
            hidden_categories: HashMap::new(),
            category_sort: HashMap::new(),
            live_sort_az: false,
            vod_sort: "provider".into(),
            vod_min_rating: 0.0,
            per_page: 50,
            source_added: HashMap::new(),
            source_user_agent: HashMap::new(),
            free_tv_source_id: None,
            free_tv_group_by: "category".into(),
            free_tv_preinstalled: false,
        }
    }
}

impl AppSettings {
    pub fn hidden_key(source: i64, kind: &str) -> String {
        format!("{source}:{kind}")
    }

    pub fn hidden(&self, source: i64, kind: &str) -> Vec<String> {
        self.hidden_categories
            .get(&Self::hidden_key(source, kind))
            .cloned()
            .unwrap_or_default()
    }

    /// Dark or light after resolving "System" and "Custom".
    pub fn is_light(&self) -> bool {
        match self.theme.as_str() {
            "Light" => true,
            "System" => !crate::theme::system_prefers_dark(),
            "Custom" => self.custom_light,
            _ => false,
        }
    }
}

static CURRENT: LazyLock<Mutex<AppSettings>> = LazyLock::new(|| Mutex::new(AppSettings::default()));

/// The live settings. Keep the guard short: never hold it across an await or engine call.
pub fn get() -> MutexGuard<'static, AppSettings> {
    CURRENT.lock().unwrap_or_else(|p| p.into_inner())
}

/// A copy, for code that reads several fields.
pub fn snapshot() -> AppSettings {
    get().clone()
}

/// Change something and write the file.
pub fn update(f: impl FnOnce(&mut AppSettings)) {
    f(&mut get());
    save();
}

pub fn load() {
    let path = paths::data_dir().join("settings.json");
    let loaded = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<AppSettings>(&text).unwrap_or_else(|e| {
            crate::log(&format!("settings unreadable, using defaults: {e}"));
            AppSettings::default()
        }),
        Err(_) => AppSettings::default(),
    };
    *get() = loaded;
}

pub fn save() {
    let path = paths::data_dir().join("settings.json");
    let text = match serde_json::to_string_pretty(&*get()) {
        Ok(t) => t,
        Err(e) => return crate::log(&format!("settings not saved: {e}")),
    };
    if let Err(e) = paths::write_atomic(&path, text.as_bytes()) {
        crate::log(&format!("settings not saved: {e}"));
    }
}
