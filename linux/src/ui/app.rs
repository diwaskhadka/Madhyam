//! The main window, the Linux twin of MainWindow.xaml(.cs):
//!   rail | top bar: playlist switcher · search pill · add · status
//!        | Dashboard / Search / Favorites / Recent / Recently added / Settings → full-width pages (pages.rs)
//!        | Live TV → categories | channel list | player + guide                   (live.rs)
//!        | Movies / Series → categories | poster grid → details page           (vod.rs)
//! One video surface (mpv in a GLArea) moves between the live card and the movie page.

use super::kit::{self, MenuRow};
use super::ratio::RatioBox;
use super::{bg, bgv, sleep_ms, spawn};
use crate::freetv;
use crate::http::AppError;
use crate::mpv::Mpv;
use crate::repo::{self, DetailsInfo, Item, Repo, Source};
use crate::settings;
use crate::ui::settings_view::SettingsView;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Dashboard,
    Search,
    Favorites,
    Recent,
    Movies,
    Live,
    Series,
    Added,
    Settings,
}

impl Mode {
    pub const ALL: [Mode; 9] = [
        Mode::Dashboard,
        Mode::Search,
        Mode::Favorites,
        Mode::Recent,
        Mode::Movies,
        Mode::Live,
        Mode::Series,
        Mode::Added,
        Mode::Settings,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mode::Dashboard => "Dashboard",
            Mode::Search => "Search",
            Mode::Favorites => "Favorites",
            Mode::Recent => "Recent",
            Mode::Movies => "Movies",
            Mode::Live => "Live",
            Mode::Series => "Series",
            Mode::Added => "Added",
            Mode::Settings => "Settings",
        }
    }

    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(s))
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Live => "Live TV",
            Mode::Movies => "Movies",
            Mode::Series => "Series",
            Mode::Favorites => "Favorites",
            Mode::Recent => "Recently viewed",
            Mode::Added => "Recently added",
            Mode::Settings => "Settings",
            _ => "Advanced search",
        }
    }

    pub fn kind(self) -> Option<&'static str> {
        match self {
            Mode::Live => Some("live"),
            Mode::Movies => Some("movie"),
            Mode::Series => Some("series"),
            _ => None,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Mode::Dashboard => "madhyam-go-home-symbolic",
            Mode::Search => "madhyam-system-search-symbolic",
            Mode::Favorites => "madhyam-emote-love-symbolic",
            Mode::Recent => "madhyam-document-open-recent-symbolic",
            Mode::Movies => "madhyam-video-x-generic-symbolic",
            Mode::Live => "madhyam-tv-symbolic",
            Mode::Series => "madhyam-view-grid-symbolic",
            Mode::Added => "madhyam-x-office-calendar-symbolic",
            Mode::Settings => "madhyam-emblem-system-symbolic",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Mode::Dashboard => "Dashboard",
            Mode::Search => "Advanced search (Ctrl+K)",
            Mode::Favorites => "Favorites",
            Mode::Recent => "Recently watched",
            Mode::Movies => "Movies",
            Mode::Live => "Live TV",
            Mode::Series => "Series",
            Mode::Added => "Recently added",
            Mode::Settings => "Settings",
        }
    }
}

/// Category list row; `all` is the synthetic "All items" entry.
#[derive(Clone, Debug)]
pub struct CategoryRow {
    pub name: String,
    pub count: i64,
    pub all: bool,
}

/// Widgets of one channel row, kept so "what's on now" can be updated in place.
pub struct LiveRow {
    pub item: Item,
    pub row: gtk::ListBoxRow,
    pub title: gtk::Label,
    pub now: gtk::Label,
    pub times: gtk::Box,
    pub start: gtk::Label,
    pub end: gtk::Label,
    pub bar: gtk::ProgressBar,
    pub star: gtk::Button,
}

pub struct App {
    pub window: gtk::ApplicationWindow,
    pub repo: Arc<Repo>,
    pub mpv: std::cell::OnceCell<Rc<Mpv>>,
    pub css: gtk::CssProvider,

    // shell
    pub rail: gtk::Box,
    pub nav: Vec<(Mode, gtk::ToggleButton)>,
    pub topbar: gtk::Box,
    pub playlist_btn: gtk::Button,
    pub playlist_name: gtk::Label,
    pub playlist_count: gtk::Label,
    pub search_pill: gtk::Box,
    pub section_label: gtk::Label,
    pub search: gtk::Entry,
    pub search_clear: gtk::Button,
    pub status: gtk::Label,
    pub unlock_btn: gtk::Button,
    pub body: gtk::Box,
    pub stack: gtk::Stack,

    // categories
    pub cats_panel: gtk::Box,
    pub cats_title: gtk::Label,
    pub cat_sort_btn: gtk::Button,
    pub cat_manage_btn: gtk::Button,
    pub cat_mode_box: gtk::Box,
    pub cat_filter_box: gtk::Box,
    pub cat_filter: gtk::Entry,
    pub group_list: gtk::ListBox,

    // live
    pub list_panel: gtk::Box,
    pub list_title: gtk::Label,
    pub list_count: gtk::Label,
    pub live_sort_btn: gtk::Button,
    pub list_empty: gtk::Label,
    pub item_list: gtk::ListBox,
    pub item_scroll: gtk::ScrolledWindow,
    pub player_panel: gtk::Box,
    pub live_card: gtk::Box,
    pub live_card_header: gtk::Box,
    pub live_badge: gtk::Label,
    pub live_card_title: gtk::Label,
    pub live_card_sub: gtk::Label,
    pub live_close_btn: gtk::Button,
    pub live_ratio: RatioBox,
    pub live_frame: gtk::Overlay,
    pub video_hint: gtk::Box,
    pub guide_panel: gtk::Box,
    pub guide_day: gtk::Label,
    pub guide_empty: gtk::Label,
    pub guide_list: gtk::Box,
    pub guide_scroll: gtk::ScrolledWindow,
    pub timeline_panel: gtk::Box,
    pub tl_channel: gtk::Label,
    pub tl_now: gtk::Label,
    pub tl_day: gtk::Label,
    pub tl_empty: gtk::Label,
    pub tl_scroll: gtk::ScrolledWindow,
    pub tl_canvas: gtk::Fixed,
    pub live_mute: gtk::Button,

    // grid
    pub grid_title: gtk::Label,
    pub grid_count: gtk::Label,
    pub sort_btn: gtk::Button,
    pub sort_label: gtk::Label,
    pub refine_btn: gtk::Button,
    pub refine_label: gtk::Label,
    pub per_buttons: Vec<gtk::Button>,
    pub page_text: gtk::Label,
    pub prev_page: gtk::Button,
    pub next_page: gtk::Button,
    pub grid_scroll: gtk::ScrolledWindow,
    pub poster_grid: gtk::FlowBox,

    // movie / series page
    pub vod_backdrop: gtk::Box,
    pub vod_header: gtk::Box,
    pub vod_header_title: gtk::Label,
    pub vod_scroll: gtk::ScrolledWindow,
    pub vod_content: gtk::Box,
    pub vod_player_box: gtk::Box,
    pub vod_card: gtk::Box,
    pub vod_card_header: gtk::Box,
    pub vod_card_label: gtk::Label,
    pub vod_card_title: gtk::Label,
    pub vod_card_sub: gtk::Label,
    pub vod_ratio: RatioBox,
    pub vod_frame: gtk::Overlay,

    // full-width pages
    pub page_host: gtk::Box,

    // video and its controls (player.rs)
    pub video: gtk::Overlay,
    pub controls: super::player::Controls,

    // ------------------------------------------------ state
    pub sources: RefCell<Vec<Source>>,
    pub source: RefCell<Option<Source>>,
    pub mode: Cell<Mode>,
    pub favorites: RefCell<HashSet<String>>,
    pub load_version: Cell<u64>,
    pub nav_silent: Cell<bool>,
    pub fullscreen: Cell<bool>,
    pub settings_view: RefCell<Option<Rc<SettingsView>>>,
    pub reset_on_exit: Cell<bool>,
    pub restart_on_exit: Cell<bool>,
    pub source_online: RefCell<HashMap<i64, bool>>,
    pub free_tv_busy: Cell<bool>,
    pub search_timer: RefCell<Option<glib::SourceId>>,
    pub ticks: Cell<u64>,

    // categories
    pub all_categories: RefCell<Vec<CategoryRow>>,
    pub shown_categories: RefCell<Vec<CategoryRow>>,
    pub group: RefCell<Option<String>>,
    pub group_silent: Cell<bool>,
    pub country_tagging: Cell<bool>,

    // live list
    pub live_rows: RefCell<Vec<LiveRow>>,
    /// Channels of the loaded pages left out by parental controls (paging counts them).
    pub live_skipped: Cell<usize>,
    pub live_total: Cell<i64>,
    pub live_loading: Cell<bool>,
    pub live_search: RefCell<Option<String>>,
    pub now_cache: RefCell<HashMap<String, repo::Programme>>,

    // playback
    pub playlist: RefCell<Vec<Item>>,
    pub play_index: Cell<i64>,
    pub vod_playing: Cell<bool>,
    pub guide_day_offset: Cell<i64>,
    pub guide_item: RefCell<Option<Item>>,

    // movies / series
    pub details: RefCell<Option<Item>>,
    pub details_info: RefCell<Option<DetailsInfo>>,
    pub ratings: RefCell<Option<crate::ratings::ExternalRatings>>,
    pub details_loading: Cell<bool>,
    pub episodes: RefCell<Vec<Item>>,
    pub season: RefCell<Option<String>>,
    pub progress_stale: Cell<bool>,
    pub backdrop_key: RefCell<Option<String>>,
    pub per_page: Cell<usize>,
    pub page: Cell<usize>,
    pub grid_total: Cell<i64>,
    pub vod_search: RefCell<Option<String>>,
    pub search_hits: RefCell<Option<Vec<Item>>>,
    pub grid_items: RefCell<Vec<Item>>,

    // pages
    pub search_live: Cell<bool>,
    pub search_movies: Cell<bool>,
    pub search_series: Cell<bool>,
    pub search_visible_only: Cell<bool>,
    pub search_all_sources: Cell<bool>,
    pub page_kind: RefCell<String>,
    pub page_all_sources: Cell<bool>,
}

pub fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

pub fn flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

fn title_label(t: &str) -> gtk::Label {
    kit::text(t, 16.0, "", 600, false)
}

impl App {
    pub fn new(application: &gtk::Application) -> Rc<App> {
        let s = settings::snapshot();
        let window = gtk::ApplicationWindow::builder()
            .application(application)
            .title(crate::brand::NAME)
            .icon_name(crate::brand::APP_ID)
            .default_width(s.win_width.filter(|w| *w > 400.0).unwrap_or(1480.0) as i32)
            .default_height(s.win_height.filter(|h| *h > 300.0).unwrap_or(900.0) as i32)
            .build();
        window.set_size_request(1040, 620);
        window.add_css_class("main-window");

        let css = gtk::CssProvider::new();
        gtk::style_context_add_provider_for_display(
            &gdk::Display::default().expect("display"),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        // ------------------------------------------------ rail
        let rail = kit::vbox(0);
        rail.add_css_class("rail");
        let logo = gtk::Image::from_paintable(Some(&crate::brand::icon_texture()));
        logo.set_pixel_size(40);
        logo.set_halign(gtk::Align::Center);
        logo.set_tooltip_text(Some(&crate::brand::full_name()));
        logo.set_margin_bottom(14);
        rail.append(&logo);
        let mut nav: Vec<(Mode, gtk::ToggleButton)> = Vec::new();
        let mut first: Option<gtk::ToggleButton> = None;
        let make = |m: Mode, first: &mut Option<gtk::ToggleButton>| {
            let b = gtk::ToggleButton::new();
            b.set_child(Some(&kit::icon(m.icon(), 20)));
            b.add_css_class("rail-button");
            b.set_tooltip_text(Some(m.tooltip()));
            b.set_halign(gtk::Align::Center);
            if let Some(f) = first {
                b.set_group(Some(f));
            } else {
                *first = Some(b.clone());
            }
            b
        };
        for m in [Mode::Dashboard, Mode::Search, Mode::Favorites, Mode::Recent] {
            let b = make(m, &mut first);
            rail.append(&b);
            nav.push((m, b));
        }
        let sep = kit::hbox(0);
        sep.add_css_class("rail-sep");
        rail.append(&sep);
        for m in [Mode::Movies, Mode::Live, Mode::Series, Mode::Added] {
            let b = make(m, &mut first);
            rail.append(&b);
            nav.push((m, b));
        }
        let fill = kit::vbox(0);
        fill.set_vexpand(true);
        rail.append(&fill);
        let settings_btn = make(Mode::Settings, &mut first);
        rail.append(&settings_btn);
        nav.push((Mode::Settings, settings_btn));

        // ------------------------------------------------ top bar
        let topbar = kit::hbox(10);
        kit::margins(&topbar, 12, 10, 12, 4);
        let playlist_btn = gtk::Button::new();
        playlist_btn.add_css_class("bare");
        let pb = kit::hbox(9);
        pb.add_css_class("topbar-box");
        pb.set_size_request(250, 46);
        kit::margins(&pb, 0, 0, 0, 0);
        let chip = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        chip.add_css_class("chip-icon");
        chip.set_valign(gtk::Align::Center);
        chip.set_margin_start(8);
        let li = kit::icon("madhyam-view-list-symbolic", 14);
        li.set_halign(gtk::Align::Center);
        chip.set_homogeneous(true);
        chip.append(&li);
        pb.append(&chip);
        let names = kit::vbox(0);
        names.set_valign(gtk::Align::Center);
        names.set_hexpand(true);
        let playlist_name = kit::text("No playlist", 14.0, "", 600, false);
        playlist_name.set_max_width_chars(1);
        let playlist_count = kit::text("Add a source to begin", 11.0, "muted", 400, false);
        playlist_count.set_max_width_chars(1);
        names.append(&playlist_name);
        names.append(&playlist_count);
        pb.append(&names);
        let dd = kit::icon("madhyam-pan-down-symbolic", 12);
        dd.add_css_class("muted");
        dd.set_margin_end(10);
        pb.append(&dd);
        playlist_btn.set_child(Some(&pb));
        playlist_btn.set_tooltip_text(Some("Playlists, playlist info and account info"));
        playlist_btn.set_cursor_from_name(Some("pointer"));
        playlist_btn.set_hexpand(false);
        topbar.append(&playlist_btn);

        let search_pill = kit::hbox(10);
        search_pill.add_css_class("topbar-box");
        search_pill.set_size_request(460, 46);
        let si = kit::icon("madhyam-system-search-symbolic", 14);
        si.add_css_class("muted");
        si.set_margin_start(12);
        search_pill.append(&si);
        let section_label = kit::text("Advanced search", 12.5, "", 600, false);
        section_label.set_max_width_chars(22);
        search_pill.append(&section_label);
        let search = gtk::Entry::new();
        search.add_css_class("bare");
        search.set_hexpand(true);
        search.set_placeholder_text(Some("Search in this playlist…"));
        search_pill.append(&search);
        let search_clear = kit::icon_button("madhyam-window-close-symbolic", "Clear", || {});
        search_clear.add_css_class("small");
        search_clear.set_visible(false);
        search_pill.append(&search_clear);
        let kbd = gtk::Label::new(Some("Ctrl+K"));
        kbd.add_css_class("kbd");
        kbd.set_valign(gtk::Align::Center);
        kbd.set_margin_end(10);
        search_pill.append(&kbd);
        search_pill.set_hexpand(false);
        topbar.append(&search_pill);
        let add_btn = kit::icon_button("madhyam-list-add-symbolic", "Add a playlist", || {});
        topbar.append(&add_btn);
        let status = gtk::Label::new(None);
        status.add_css_class("status");
        status.set_ellipsize(gtk::pango::EllipsizeMode::End);
        status.set_max_width_chars(70);
        status.set_xalign(0.0);
        status.set_hexpand(true);
        topbar.append(&status);
        // Parental controls: shown while the PIN is remembered; clicking locks again.
        let unlock_btn = kit::icon_button(
            "madhyam-changes-allow-symbolic",
            "Unlocked for this session: click to lock again",
            || {},
        );
        unlock_btn.add_css_class("unlocked-button");
        unlock_btn.set_visible(false);
        topbar.append(&unlock_btn);

        // ------------------------------------------------ categories panel
        let cats_panel = kit::vbox(0);
        cats_panel.set_size_request(250, -1);
        cats_panel.set_margin_end(12);
        let head = kit::hbox(0);
        kit::margins(&head, 8, 2, 0, 8);
        let cats_title = title_label("Categories");
        cats_title.set_hexpand(true);
        head.append(&cats_title);
        let cat_search_btn =
            kit::icon_button("madhyam-system-search-symbolic", "Search categories", || {});
        let cat_sort_btn = kit::icon_button(
            "madhyam-view-sort-ascending-symbolic",
            "Sort categories",
            || {},
        );
        let cat_manage_btn = kit::icon_button(
            "madhyam-checkbox-checked-symbolic",
            "Manage categories (show / hide)",
            || {},
        );
        for b in [&cat_search_btn, &cat_sort_btn, &cat_manage_btn] {
            b.add_css_class("small");
            head.append(b);
        }
        cats_panel.append(&head);
        let cat_mode_box = kit::hbox(0);
        kit::margins(&cat_mode_box, 4, 0, 4, 10);
        cat_mode_box.set_visible(false);
        cats_panel.append(&cat_mode_box);
        let cat_filter_box = kit::hbox(8);
        cat_filter_box.add_css_class("topbar-box");
        kit::margins(&cat_filter_box, 4, 0, 4, 10);
        cat_filter_box.set_size_request(-1, 36);
        let fi = kit::icon("madhyam-system-search-symbolic", 12);
        fi.add_css_class("muted");
        fi.set_margin_start(12);
        cat_filter_box.append(&fi);
        let cat_filter = gtk::Entry::new();
        cat_filter.add_css_class("bare");
        cat_filter.set_hexpand(true);
        cat_filter.set_placeholder_text(Some("Search categories"));
        cat_filter_box.append(&cat_filter);
        cat_filter_box.set_visible(false);
        cats_panel.append(&cat_filter_box);
        let group_list = gtk::ListBox::new();
        group_list.add_css_class("plain");
        group_list.set_selection_mode(gtk::SelectionMode::Single);
        cats_panel.append(&kit::page_scroll(&group_list));

        // ------------------------------------------------ live: channel list
        let list_panel = kit::vbox(0);
        list_panel.set_size_request(340, -1);
        list_panel.set_margin_end(12);
        let lh = kit::hbox(6);
        lh.add_css_class("list-header");
        lh.set_margin_bottom(8);
        let list_title = kit::text("All channels", 14.5, "", 600, false);
        list_title.set_hexpand(true);
        list_title.set_max_width_chars(1);
        lh.append(&list_title);
        let list_count = kit::text("", 12.0, "muted", 400, false);
        lh.append(&list_count);
        let live_sort_btn =
            kit::icon_button("madhyam-view-sort-ascending-symbolic", "Sort A–Z", || {});
        live_sort_btn.add_css_class("small");
        lh.append(&live_sort_btn);
        list_panel.append(&lh);
        let list_empty = kit::text("", 13.0, "muted", 400, true);
        kit::margins(&list_empty, 6, 10, 6, 10);
        list_empty.set_visible(false);
        list_panel.append(&list_empty);
        let item_list = gtk::ListBox::new();
        item_list.add_css_class("plain");
        item_list.add_css_class("channels");
        item_list.set_selection_mode(gtk::SelectionMode::Single);
        let item_scroll = kit::page_scroll(&item_list);
        list_panel.append(&item_scroll);

        // ------------------------------------------------ live: player card + guide
        let player_panel = kit::vbox(0);
        player_panel.set_hexpand(true);
        let live_card = kit::vbox(0);
        live_card.add_css_class("player-card");
        kit::margins(&live_card, 0, 0, 0, 0);
        live_card.set_halign(gtk::Align::Center);
        live_card.set_valign(gtk::Align::Start);
        let live_card_header = kit::hbox(10);
        kit::margins(&live_card_header, 16, 12, 14, 10);
        let live_badge = gtk::Label::new(Some("● LIVE"));
        live_badge.add_css_class("live-badge");
        live_badge.set_valign(gtk::Align::Center);
        live_badge.set_visible(false);
        live_card_header.append(&live_badge);
        let lt = kit::vbox(0);
        lt.set_hexpand(true);
        let live_card_title = kit::text("Live TV", 15.5, "", 600, false);
        live_card_title.set_max_width_chars(1);
        let live_card_sub = kit::text("", 12.0, "muted", 400, false);
        live_card_sub.set_max_width_chars(1);
        lt.append(&live_card_title);
        lt.append(&live_card_sub);
        live_card_header.append(&lt);
        let live_close_btn = gtk::Button::new();
        let lcb = kit::hbox(8);
        lcb.append(&kit::icon("madhyam-window-close-symbolic", 11));
        lcb.append(&gtk::Label::new(Some("Close player")));
        live_close_btn.set_child(Some(&lcb));
        live_close_btn.add_css_class("pill");
        live_close_btn.set_valign(gtk::Align::Center);
        live_close_btn.set_visible(false);
        live_close_btn.set_tooltip_text(Some("Stop playback"));
        live_card_header.append(&live_close_btn);
        live_card.append(&live_card_header);
        let live_frame = gtk::Overlay::new();
        live_frame.add_css_class("video-frame");
        live_frame.set_overflow(gtk::Overflow::Hidden);
        kit::margins(&live_frame, 14, 0, 14, 14);
        let video_hint = kit::vbox(12);
        video_hint.set_halign(gtk::Align::Center);
        video_hint.set_valign(gtk::Align::Center);
        let vh = kit::icon("madhyam-tv-symbolic", 40);
        vh.add_css_class("muted");
        video_hint.append(&vh);
        video_hint.append(&kit::text(
            "Select a channel to start watching",
            13.0,
            "muted",
            400,
            false,
        ));
        let black = gtk::Box::new(gtk::Orientation::Vertical, 0);
        black.set_hexpand(true);
        black.set_vexpand(true);
        live_frame.set_child(Some(&black));
        live_frame.add_overlay(&video_hint);
        let live_ratio = RatioBox::default();
        live_ratio.set_child(Some(&live_frame));
        live_card.append(&live_ratio);
        player_panel.append(&live_card);

        // guide as a list
        let guide_panel = kit::vbox(0);
        guide_panel.set_margin_top(12);
        guide_panel.set_visible(false);
        guide_panel.set_vexpand(true);
        let gh = kit::hbox(0);
        gh.set_margin_bottom(8);
        let guide_prev = kit::icon_button("madhyam-go-previous-symbolic", "Previous day", || {});
        let guide_day = kit::text("", 14.0, "", 600, false);
        guide_day.set_hexpand(true);
        guide_day.set_xalign(0.5);
        let guide_next = kit::icon_button("madhyam-go-next-symbolic", "Next day", || {});
        gh.append(&guide_prev);
        gh.append(&guide_day);
        gh.append(&guide_next);
        guide_panel.append(&gh);
        let guide_empty = kit::text(
            "No programme guide for this channel.",
            13.0,
            "muted",
            400,
            true,
        );
        guide_empty.set_visible(false);
        kit::margins(&guide_empty, 6, 4, 6, 4);
        guide_panel.append(&guide_empty);
        let guide_list = kit::vbox(6);
        let guide_scroll = kit::page_scroll(&guide_list);
        guide_panel.append(&guide_scroll);

        // guide as a timeline ribbon (default)
        let timeline_panel = kit::vbox(0);
        timeline_panel.add_css_class("panel");
        timeline_panel.set_margin_top(10);
        kit::margins(&timeline_panel, 0, 10, 0, 0);
        timeline_panel.set_valign(gtk::Align::Start);
        let th = kit::hbox(4);
        kit::margins(&th, 14, 8, 8, 4);
        let tlt = kit::vbox(0);
        tlt.set_hexpand(true);
        tlt.set_valign(gtk::Align::Center);
        let tl_channel = kit::text("", 14.0, "", 600, false);
        tl_channel.set_max_width_chars(1);
        let tl_now = kit::text("", 12.0, "muted", 400, false);
        tl_now.set_max_width_chars(1);
        tlt.append(&tl_channel);
        tlt.append(&tl_now);
        th.append(&tlt);
        let live_mute = kit::icon_button("madhyam-audio-volume-high-symbolic", "Mute", || {});
        let tl_full = kit::icon_button(
            "madhyam-view-fullscreen-symbolic",
            "Full screen (F11)",
            || {},
        );
        let tl_nowbtn = kit::icon_button("madhyam-find-location-symbolic", "Jump to now", || {});
        let tl_prev = kit::icon_button("madhyam-go-previous-symbolic", "Previous day", || {});
        let tl_day = kit::text("", 13.0, "", 600, false);
        tl_day.set_size_request(110, -1);
        tl_day.set_xalign(0.5);
        let tl_next = kit::icon_button("madhyam-go-next-symbolic", "Next day", || {});
        for w in [
            live_mute.upcast_ref::<gtk::Widget>(),
            tl_full.upcast_ref(),
            tl_nowbtn.upcast_ref(),
            tl_prev.upcast_ref(),
            tl_day.upcast_ref(),
            tl_next.upcast_ref(),
        ] {
            th.append(w);
        }
        timeline_panel.append(&th);
        let tl_empty = kit::text(
            "No programme guide for this channel.",
            13.0,
            "muted",
            400,
            false,
        );
        kit::margins(&tl_empty, 14, 6, 14, 6);
        tl_empty.set_visible(false);
        timeline_panel.append(&tl_empty);
        let tl_canvas = gtk::Fixed::new();
        tl_canvas.set_size_request((24.0 * 60.0 * super::live::PX_PER_MINUTE) as i32, 112);
        let tl_scroll = gtk::ScrolledWindow::new();
        tl_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        tl_scroll.set_child(Some(&tl_canvas));
        tl_scroll.set_min_content_height(120);
        tl_scroll.set_margin_bottom(6);
        timeline_panel.append(&tl_scroll);
        player_panel.append(&timeline_panel);
        player_panel.append(&guide_panel);

        let live_layout = kit::hbox(0);
        live_layout.append(&list_panel);
        live_layout.append(&player_panel);

        // ------------------------------------------------ grid
        let grid_panel = kit::vbox(0);
        let gbar = kit::hbox(8);
        gbar.add_css_class("panel");
        kit::margins(&gbar, 0, 0, 0, 12);
        let gtexts = kit::vbox(0);
        kit::margins(&gtexts, 16, 8, 0, 8);
        gtexts.set_hexpand(true);
        let grid_title = kit::text("All items", 15.5, "", 600, false);
        grid_title.set_max_width_chars(1);
        let grid_count = kit::text("", 12.0, "muted", 400, false);
        gtexts.append(&grid_title);
        gtexts.append(&grid_count);
        gbar.append(&gtexts);
        let chip_btn = |icon_name: &str, label: &gtk::Label| {
            let b = gtk::Button::new();
            let r = kit::hbox(8);
            r.append(&kit::icon(icon_name, 14));
            r.append(label);
            b.set_child(Some(&r));
            b.add_css_class("chip-solid");
            b.set_valign(gtk::Align::Center);
            b
        };
        let sort_label = gtk::Label::new(Some("Sort: Provider order"));
        let sort_btn = chip_btn("madhyam-view-sort-ascending-symbolic", &sort_label);
        let refine_label = gtk::Label::new(Some("Refine"));
        let refine_btn = chip_btn("madhyam-starred-symbolic", &refine_label);
        gbar.append(&sort_btn);
        gbar.append(&refine_btn);
        let pp = kit::text("Per page", 12.0, "muted", 400, false);
        pp.set_margin_start(10);
        gbar.append(&pp);
        let mut per_buttons = Vec::new();
        for n in ["25", "50", "100"] {
            let b = gtk::Button::with_label(n);
            b.add_css_class("chip-button");
            b.set_valign(gtk::Align::Center);
            gbar.append(&b);
            per_buttons.push(b);
        }
        let page_text = kit::text("1 / 1", 12.5, "", 400, false);
        page_text.set_margin_start(10);
        gbar.append(&page_text);
        let prev_page = kit::icon_button("madhyam-go-previous-symbolic", "Previous page", || {});
        let next_page = kit::icon_button("madhyam-go-next-symbolic", "Next page", || {});
        next_page.set_margin_end(10);
        gbar.append(&prev_page);
        gbar.append(&next_page);
        grid_panel.append(&gbar);
        let poster_grid = gtk::FlowBox::new();
        poster_grid.set_selection_mode(gtk::SelectionMode::None);
        poster_grid.set_homogeneous(false);
        poster_grid.set_max_children_per_line(100);
        poster_grid.set_valign(gtk::Align::Start);
        poster_grid.set_row_spacing(0);
        poster_grid.set_column_spacing(0);
        let grid_scroll = kit::page_scroll(&poster_grid);
        grid_panel.append(&grid_scroll);

        // ------------------------------------------------ movie / series page
        let vod_page = gtk::Overlay::new();
        vod_page.add_css_class("panel-lg");
        vod_page.set_overflow(gtk::Overflow::Hidden);
        let vod_backdrop = kit::vbox(0);
        vod_backdrop.set_hexpand(true);
        vod_backdrop.set_vexpand(true);
        vod_page.set_child(Some(&vod_backdrop));
        let vod_col = kit::vbox(0);
        let vod_header = kit::hbox(12);
        kit::margins(&vod_header, 18, 14, 18, 0);
        let vod_back = kit::icon_button("madhyam-go-previous-symbolic", "Back (Esc)", || {});
        vod_header.append(&vod_back);
        let vod_header_title = kit::text("", 16.0, "", 600, false);
        vod_header_title.set_max_width_chars(1);
        vod_header_title.set_hexpand(true);
        vod_header.append(&vod_header_title);
        vod_col.append(&vod_header);
        let vod_inner = kit::vbox(0);
        let vod_player_box = kit::vbox(0);
        kit::margins(&vod_player_box, 18, 6, 18, 14);
        vod_player_box.set_visible(false);
        let vod_card = kit::vbox(0);
        vod_card.add_css_class("player-card");
        vod_card.set_halign(gtk::Align::Center);
        let vod_card_header = kit::hbox(10);
        kit::margins(&vod_card_header, 18, 12, 16, 12);
        let vt = kit::vbox(0);
        vt.set_hexpand(true);
        let vod_card_label = kit::text("NOW PLAYING", 10.5, "muted", 700, false);
        let vod_card_title = kit::text("", 18.0, "", 600, false);
        vod_card_title.set_max_width_chars(1);
        let vod_card_sub = kit::text("", 12.5, "muted", 400, false);
        vod_card_sub.set_max_width_chars(1);
        vt.append(&vod_card_label);
        vt.append(&vod_card_title);
        vt.append(&vod_card_sub);
        vod_card_header.append(&vt);
        let vod_close = gtk::Button::new();
        let vcb = kit::hbox(8);
        vcb.append(&kit::icon("madhyam-window-close-symbolic", 11));
        vcb.append(&gtk::Label::new(Some("Close player")));
        vod_close.set_child(Some(&vcb));
        vod_close.add_css_class("pill");
        vod_close.set_valign(gtk::Align::Center);
        vod_close.set_tooltip_text(Some("Stop and show the details (Esc)"));
        vod_card_header.append(&vod_close);
        vod_card.append(&vod_card_header);
        let vod_frame = gtk::Overlay::new();
        vod_frame.add_css_class("video-frame");
        vod_frame.set_overflow(gtk::Overflow::Hidden);
        kit::margins(&vod_frame, 16, 0, 16, 16);
        let vblack = gtk::Box::new(gtk::Orientation::Vertical, 0);
        vblack.set_hexpand(true);
        vblack.set_vexpand(true);
        vod_frame.set_child(Some(&vblack));
        let vod_ratio = RatioBox::default();
        vod_ratio.set_child(Some(&vod_frame));
        vod_card.append(&vod_ratio);
        vod_player_box.append(&vod_card);
        vod_inner.append(&vod_player_box);
        let vod_content = kit::vbox(0);
        kit::margins(&vod_content, 34, 14, 34, 30);
        vod_inner.append(&vod_content);
        let vod_scroll = kit::page_scroll(&vod_inner);
        // The page is rebuilt under the focused button on every play: following focus would
        // scroll the player card out of view.
        if let Some(vp) = vod_scroll.child().and_downcast::<gtk::Viewport>() {
            vp.set_scroll_to_focus(false);
        }
        vod_col.append(&vod_scroll);
        vod_page.add_overlay(&vod_col);

        // ------------------------------------------------ pages + stack
        let page_host = kit::vbox(0);
        page_host.set_hexpand(true);
        page_host.set_vexpand(true);

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.add_named(&live_layout, Some("live"));
        stack.add_named(&grid_panel, Some("grid"));
        stack.add_named(&vod_page, Some("vod"));
        stack.add_named(&page_host, Some("page"));

        let body = kit::hbox(0);
        kit::margins(&body, 12, 6, 12, 12);
        body.set_vexpand(true);
        body.append(&cats_panel);
        body.append(&stack);

        let main = kit::vbox(0);
        main.set_hexpand(true);
        main.append(&topbar);
        main.append(&body);
        let root = kit::hbox(0);
        root.append(&rail);
        root.append(&main);
        window.set_child(Some(&root));

        // ------------------------------------------------ video
        let (video, controls) = super::player::build_controls();

        let app = Rc::new(App {
            window,
            repo: Arc::new(Repo::new()),
            mpv: std::cell::OnceCell::new(),
            css,
            rail,
            nav,
            topbar,
            playlist_btn,
            playlist_name,
            playlist_count,
            search_pill,
            section_label,
            search,
            search_clear,
            status,
            unlock_btn,
            body,
            stack,
            cats_panel,
            cats_title,
            cat_sort_btn,
            cat_manage_btn,
            cat_mode_box,
            cat_filter_box,
            cat_filter,
            group_list,
            list_panel,
            list_title,
            list_count,
            live_sort_btn,
            list_empty,
            item_list,
            item_scroll,
            player_panel,
            live_card,
            live_card_header,
            live_badge,
            live_card_title,
            live_card_sub,
            live_close_btn,
            live_ratio,
            live_frame,
            video_hint,
            guide_panel,
            guide_day,
            guide_empty,
            guide_list,
            guide_scroll,
            timeline_panel,
            tl_channel,
            tl_now,
            tl_day,
            tl_empty,
            tl_scroll,
            tl_canvas,
            live_mute,
            grid_title,
            grid_count,
            sort_btn,
            sort_label,
            refine_btn,
            refine_label,
            per_buttons,
            page_text,
            prev_page,
            next_page,
            grid_scroll,
            poster_grid,
            vod_backdrop,
            vod_header,
            vod_header_title,
            vod_scroll,
            vod_content,
            vod_player_box,
            vod_card,
            vod_card_header,
            vod_card_label,
            vod_card_title,
            vod_card_sub,
            vod_ratio,
            vod_frame,
            page_host,
            video,
            controls,
            sources: RefCell::new(Vec::new()),
            source: RefCell::new(None),
            mode: Cell::new(Mode::Dashboard),
            favorites: RefCell::new(HashSet::new()),
            load_version: Cell::new(0),
            nav_silent: Cell::new(false),
            fullscreen: Cell::new(false),
            settings_view: RefCell::new(None),
            reset_on_exit: Cell::new(false),
            restart_on_exit: Cell::new(false),
            source_online: RefCell::new(HashMap::new()),
            free_tv_busy: Cell::new(false),
            search_timer: RefCell::new(None),
            ticks: Cell::new(0),
            all_categories: RefCell::new(Vec::new()),
            shown_categories: RefCell::new(Vec::new()),
            group: RefCell::new(None),
            group_silent: Cell::new(false),
            country_tagging: Cell::new(false),
            live_rows: RefCell::new(Vec::new()),
            live_skipped: Cell::new(0),
            live_total: Cell::new(0),
            live_loading: Cell::new(false),
            live_search: RefCell::new(None),
            now_cache: RefCell::new(HashMap::new()),
            playlist: RefCell::new(Vec::new()),
            play_index: Cell::new(-1),
            vod_playing: Cell::new(false),
            guide_day_offset: Cell::new(0),
            guide_item: RefCell::new(None),
            details: RefCell::new(None),
            details_info: RefCell::new(None),
            ratings: RefCell::new(None),
            details_loading: Cell::new(false),
            episodes: RefCell::new(Vec::new()),
            season: RefCell::new(None),
            progress_stale: Cell::new(false),
            backdrop_key: RefCell::new(None),
            per_page: Cell::new(if matches!(s.per_page, 25 | 50 | 100) {
                s.per_page as usize
            } else {
                50
            }),
            page: Cell::new(0),
            grid_total: Cell::new(0),
            vod_search: RefCell::new(None),
            search_hits: RefCell::new(None),
            grid_items: RefCell::new(Vec::new()),
            search_live: Cell::new(true),
            search_movies: Cell::new(true),
            search_series: Cell::new(true),
            search_visible_only: Cell::new(true),
            search_all_sources: Cell::new(false),
            page_kind: RefCell::new("all".into()),
            page_all_sources: Cell::new(true),
        });
        // mpv needs the App for its event handler, so it is attached afterwards.
        let weak = Rc::downgrade(&app);
        let mpv = Mpv::new(move |e| {
            if let Some(a) = weak.upgrade() {
                a.on_player_event(e);
            }
        });
        match mpv {
            Some(m) => {
                crate::sub_style::apply(&m);
                let _ = app.mpv.set(m);
            }
            None => crate::log("libmpv could not be initialised"),
        }
        app.controls.attach_area(&app);
        app.apply_theme();
        app.wire(
            cat_search_btn,
            add_btn,
            guide_prev,
            guide_next,
            tl_full,
            tl_nowbtn,
            tl_prev,
            tl_next,
            vod_back,
            vod_close,
        );
        app.size_players();
        app
    }

    pub fn mpv(&self) -> Option<&Rc<Mpv>> {
        self.mpv.get()
    }

    /// Restyle everything from the current settings (theme and cover size apply at once).
    pub fn apply_theme(&self) {
        let s = settings::snapshot();
        let palette = crate::theme::current(&s);
        self.css
            .load_from_string(&crate::theme::css(&palette, &s.cover_size));
    }

    #[allow(clippy::too_many_arguments)]
    fn wire(
        self: &Rc<Self>,
        cat_search_btn: gtk::Button,
        add_btn: gtk::Button,
        guide_prev: gtk::Button,
        guide_next: gtk::Button,
        tl_full: gtk::Button,
        tl_nowbtn: gtk::Button,
        tl_prev: gtk::Button,
        tl_next: gtk::Button,
        vod_back: gtk::Button,
        vod_close: gtk::Button,
    ) {
        let w = Rc::downgrade(self);
        macro_rules! on {
            ($widget:expr, $body:expr) => {{
                let w = w.clone();
                $widget.connect_clicked(move |_| {
                    if let Some(a) = w.upgrade() {
                        #[allow(clippy::redundant_closure_call)]
                        ($body)(a);
                    }
                });
            }};
        }
        for (m, b) in &self.nav {
            let (w, m) = (w.clone(), *m);
            b.connect_toggled(move |b| {
                if b.is_active() {
                    if let Some(a) = w.upgrade() {
                        a.nav_checked(m);
                    }
                }
            });
        }
        on!(self.playlist_btn, |a: Rc<App>| a.playlist_menu());
        on!(add_btn, |a: Rc<App>| a.open_add_source());
        on!(self.unlock_btn, |a: Rc<App>| a.lock_again());
        on!(self.search_clear, |a: Rc<App>| {
            a.search.set_text("");
            a.search.grab_focus();
        });
        {
            let w = w.clone();
            self.search.connect_changed(move |_| {
                if let Some(a) = w.upgrade() {
                    a.search_changed();
                }
            });
        }
        {
            let w = w.clone();
            self.search.connect_activate(move |_| {
                if let Some(a) = w.upgrade() {
                    spawn(async move { a.search_enter().await });
                }
            });
        }
        {
            let focus = gtk::EventControllerFocus::new();
            let pill = self.search_pill.clone();
            focus.connect_enter(move |_| pill.add_css_class("focused"));
            let pill = self.search_pill.clone();
            focus.connect_leave(move |_| pill.remove_css_class("focused"));
            self.search.add_controller(focus);
        }
        on!(cat_search_btn, |a: Rc<App>| a.cat_search_toggle());
        on!(self.cat_sort_btn, |a: Rc<App>| a.cat_sort_menu());
        on!(self.cat_manage_btn, |a: Rc<App>| spawn(async move {
            a.cat_manage().await
        }));
        {
            let w = w.clone();
            self.cat_filter.connect_changed(move |_| {
                if let Some(a) = w.upgrade() {
                    a.apply_category_filter(None, false);
                }
            });
        }
        {
            let w = w.clone();
            self.group_list.connect_row_selected(move |_, row| {
                if let (Some(a), Some(row)) = (w.upgrade(), row) {
                    a.group_selected(row.index());
                }
            });
        }
        on!(self.live_sort_btn, |a: Rc<App>| a.live_sort_menu());
        {
            let w = w.clone();
            self.item_list.connect_row_activated(move |_, row| {
                if let Some(a) = w.upgrade() {
                    a.live_row_activated(row.index());
                }
            });
        }
        {
            let w = w.clone();
            self.item_list.connect_row_selected(move |_, row| {
                if let (Some(a), Some(row)) = (w.upgrade(), row) {
                    a.live_row_selected(row.index());
                }
            });
        }
        {
            let w = w.clone();
            self.item_scroll.connect_edge_reached(move |_, pos| {
                if pos == gtk::PositionType::Bottom {
                    if let Some(a) = w.upgrade() {
                        spawn(async move { a.load_more_live().await });
                    }
                }
            });
        }
        on!(self.live_close_btn, |a: Rc<App>| a.stop_playback());
        on!(guide_prev, |a: Rc<App>| a.guide_shift(-1));
        on!(guide_next, |a: Rc<App>| a.guide_shift(1));
        on!(tl_prev, |a: Rc<App>| a.guide_shift(-1));
        on!(tl_next, |a: Rc<App>| a.guide_shift(1));
        on!(tl_nowbtn, |a: Rc<App>| a.timeline_now());
        on!(tl_full, |a: Rc<App>| a.toggle_fullscreen(false));
        on!(self.live_mute, |a: Rc<App>| a.toggle_mute());
        {
            // The wheel over the timeline scrolls it sideways.
            let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
            let sw = self.tl_scroll.clone();
            scroll.connect_scroll(move |_, _, dy| {
                let adj = sw.hadjustment();
                adj.set_value(adj.value() + dy * 80.0);
                glib::Propagation::Stop
            });
            self.tl_scroll.add_controller(scroll);
        }
        on!(self.sort_btn, |a: Rc<App>| a.sort_menu());
        on!(self.refine_btn, |a: Rc<App>| a.refine_menu());
        for b in &self.per_buttons {
            let w = w.clone();
            b.connect_clicked(move |b| {
                if let (Some(a), Ok(n)) =
                    (w.upgrade(), b.label().unwrap_or_default().parse::<usize>())
                {
                    a.set_per_page(n);
                }
            });
        }
        on!(self.prev_page, |a: Rc<App>| a.change_page(-1));
        on!(self.next_page, |a: Rc<App>| a.change_page(1));
        on!(vod_back, |a: Rc<App>| {
            if a.vod_playing.get() {
                a.stop_playback();
            } else {
                a.close_details();
            }
        });
        on!(vod_close, |a: Rc<App>| a.stop_playback());

        // keyboard
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let w = w.clone();
            keys.connect_key_pressed(move |_, key, _, state| match w.upgrade() {
                Some(a) => a.key_pressed(key, state),
                None => glib::Propagation::Proceed,
            });
        }
        self.window.add_controller(keys);

        // window size → video limits, and closing
        {
            let w = w.clone();
            self.window.connect_default_height_notify(move |_| {
                if let Some(a) = w.upgrade() {
                    a.size_players();
                }
            });
        }
        {
            let w = w.clone();
            self.window.connect_default_width_notify(move |_| {
                if let Some(a) = w.upgrade() {
                    a.size_players();
                }
            });
        }
        for prop in ["maximized", "fullscreened"] {
            let w = w.clone();
            self.window.connect_notify_local(Some(prop), move |_, _| {
                // The new size is known once the window has been laid out again.
                let w = w.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(120), move || {
                    if let Some(a) = w.upgrade() {
                        a.size_players();
                    }
                });
            });
        }
        {
            let w = w.clone();
            self.window.connect_close_request(move |_| {
                if let Some(a) = w.upgrade() {
                    a.on_closing();
                }
                glib::Propagation::Proceed
            });
        }
        // Follow panel size changes (window resize, maximize, full screen, panels shown or
        // hidden): GTK 4 has no size-changed signal, so check five times a second; set_limits
        // only relayouts when the numbers change.
        {
            let w = w.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
                match w.upgrade() {
                    Some(a) => {
                        a.size_players();
                        glib::ControlFlow::Continue
                    }
                    None => glib::ControlFlow::Break,
                }
            });
        }
        // Once a second: progress, seek bar, guide refresh.
        let w2 = w.clone();
        glib::timeout_add_seconds_local(1, move || match w2.upgrade() {
            Some(a) => {
                a.on_tick();
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
        self.controls.wire(self);
    }

    // ================================================================ lifecycle

    pub fn start(self: &Rc<Self>) {
        let s = settings::snapshot();
        if s.window_startup == "Maximized" || (s.window_startup == "Last" && s.win_maximized) {
            self.window.maximize();
        }
        self.window.present();
        let me = self.clone();
        spawn(async move { me.loaded().await });
    }

    async fn loaded(self: Rc<Self>) {
        self.set_status("Loading…");
        let repo = self.repo.clone();
        let favs = bgv(move || {
            repo.init();
            repo.favorite_ids()
        })
        .await;
        *self.favorites.borrow_mut() = favs;
        self.reload_sources(None).await;
        self.normalize_free_tv().await;
        self.set_status("");
        self.preinstall_free_tv().await;
        let screenshot = arg("--screenshot");
        if let (Some(src), None) = (self.source.borrow().clone(), &screenshot) {
            let me = self.clone();
            spawn(async move { me.check_source(src, false).await });
        }
        let s = settings::snapshot();
        let wanted = arg("--mode").unwrap_or_else(|| {
            if s.startup_view == "Last" {
                s.last_view.clone()
            } else {
                s.startup_view.clone()
            }
        });
        let start = Mode::parse(&wanted)
            .filter(|m| *m != Mode::Settings && (*m != Mode::Search || arg("--mode").is_some()))
            .unwrap_or(Mode::Dashboard);
        self.nav_button(start).set_active(true);
        if self.nav_button(start).is_active()
            && self.mode.get() == start
            && self.load_version.get() == 0
        {
            // Already the active button (Dashboard on a fresh start): toggled did not fire.
            self.nav_checked(start);
        }
        if s.window_startup == "Fullscreen" && screenshot.is_none() {
            self.toggle_fullscreen(true);
        }
        // Test helpers (--search, --settings, --open-first, --play-first): with --screenshot the
        // window is saved to a PNG and closes; without it the app stays open for a person to look.
        let scripted = ["--search", "--settings"].iter().any(|a| arg(a).is_some())
            || flag("--open-first")
            || flag("--play-first");
        if screenshot.is_some() || scripted {
            self.capture_for_ci(screenshot.as_deref()).await;
            if screenshot.is_some() {
                return;
            }
        }
        if self.sources.borrow().is_empty() {
            self.open_add_source();
        }
        self.refresh_stale_guides().await;
    }

    pub fn nav_button(&self, m: Mode) -> &gtk::ToggleButton {
        &self
            .nav
            .iter()
            .find(|(x, _)| *x == m)
            .expect("nav button")
            .1
    }

    /// Re-download guides older than the configured interval, in the background.
    async fn refresh_stale_guides(self: &Rc<Self>) {
        let hours = settings::get().epg_refresh_hours.max(1) as u64;
        let max_age = std::time::Duration::from_secs(hours * 3600);
        let sources = self.sources.borrow().clone(); // not borrowed across the awaits below
        for src in sources {
            let repo = self.repo.clone();
            let id = src.id;
            let done = bgv(move || {
                if repo.epg_age(id).is_some_and(|a| a < max_age) || !repo.has_epg(id) {
                    return false;
                }
                if let Err(e) = repo.refresh_epg(id) {
                    crate::log(&format!(
                        "background guide refresh failed for source {id}: {e}"
                    ));
                }
                true
            })
            .await;
            if done {
                self.refresh_list_appearance();
            }
        }
        let extra = settings::get().extra_epg_urls.clone();
        if !extra.is_empty() && Repo::custom_epg_age().is_none_or(|a| a > max_age) {
            let repo = self.repo.clone();
            bgv(move || repo.refresh_custom_epg(&extra)).await;
            self.refresh_list_appearance();
        }
    }

    /// CI helper: render the window to a PNG with demo data, then quit.
    /// --mode M · --open-first · --search q · --settings Page · --play-first
    async fn capture_for_ci(self: &Rc<Self>, path: Option<&str>) {
        sleep_ms(1800).await;
        if let Some(q) = arg("--search") {
            self.search.set_text(&q);
            self.apply_search().await;
            sleep_ms(1200).await;
        }
        if let Some(page) = arg("--settings") {
            self.nav_button(Mode::Settings).set_active(true);
            sleep_ms(800).await;
            if let Some(sv) = self.settings_view.borrow().clone() {
                sv.open(&page);
            }
            sleep_ms(800).await;
        }
        if flag("--open-first") {
            let first = self.grid_items.borrow().first().cloned();
            if let Some(first) = first {
                self.show_details(first.clone()).await;
                sleep_ms(1500).await;
                if flag("--play-first") {
                    let ep = self.episodes.borrow().first().cloned();
                    if first.is_series() {
                        if let Some(ep) = ep {
                            self.play_episode(ep).await;
                        }
                    } else {
                        self.play_movie(false).await;
                    }
                    sleep_ms(2500).await;
                    if let Some(m) = self.mpv() {
                        crate::log(&format!(
                            "screenshot: time-pos={:?} {} vod_scroll {}x{} frame {}x{}",
                            m.get_f64("time-pos"),
                            crate::mpv::stats(),
                            self.vod_scroll.width(),
                            self.vod_scroll.height(),
                            self.vod_frame.width(),
                            self.vod_frame.height()
                        ));
                    }
                }
            }
        }
        if self.mode.get() == Mode::Live {
            if let Some(row) = self
                .item_list
                .row_at_index(2)
                .or_else(|| self.item_list.row_at_index(0))
            {
                self.item_list.select_row(Some(&row));
                sleep_ms(1200).await;
                if flag("--play-first") {
                    let it = self
                        .live_rows
                        .borrow()
                        .get(row.index() as usize)
                        .map(|r| r.item.clone());
                    if let Some(it) = it {
                        self.play_live(it).await;
                        sleep_ms(4000).await;
                        if let Some(m) = self.mpv() {
                            crate::log(&format!(
                                "screenshot: playing={} time-pos={:?} video={:?}x{:?} {}",
                                !m.get_flag("idle-active"),
                                m.get_f64("time-pos"),
                                m.get_i64("width"),
                                m.get_i64("height"),
                                crate::mpv::stats()
                            ));
                        }
                    }
                }
            }
        }
        let Some(path) = path else { return };
        sleep_ms(400).await;
        if let Err(e) = self.save_png(path) {
            crate::log(&format!("screenshot failed: {e}"));
            eprintln!("screenshot failed: {e}");
        }
        self.window.close();
    }

    fn save_png(&self, path: &str) -> Result<(), String> {
        let root = self.window.child().ok_or("no content")?;
        let (w, h) = (root.width() as f64, root.height() as f64);
        let paintable = gtk::WidgetPaintable::new(Some(&root));
        let snapshot = gtk::Snapshot::new();
        // The window background (the page colour), as the window itself would draw it.
        let bg = crate::theme::current(&settings::snapshot())
            .get("Bg")
            .and_then(|h| gdk::RGBA::parse(crate::theme::css_color(h)).ok())
            .unwrap_or(gdk::RGBA::BLACK);
        snapshot.append_color(&bg, &gtk::graphene::Rect::new(0.0, 0.0, w as f32, h as f32));
        paintable.snapshot(&snapshot, w, h);
        let node = snapshot.to_node().ok_or("nothing drawn")?;
        let renderer = self
            .window
            .native()
            .and_then(|n| n.renderer())
            .ok_or("no renderer")?;
        let texture = renderer.render_texture(&node, None);
        texture.save_to_png(path).map_err(|e| e.to_string())
    }

    fn on_closing(&self) {
        self.save_progress();
        if let Some(m) = self.mpv() {
            m.stop();
        }
        let screenshot = arg("--screenshot").is_some();
        if !self.reset_on_exit.get() && !screenshot {
            self.repo.save();
            settings::update(|s| {
                let fs = self.fullscreen.get();
                s.win_maximized = self.window.is_maximized() && !fs;
                if !self.window.is_maximized() && !fs {
                    s.win_width = Some(self.window.default_width() as f64);
                    s.win_height = Some(self.window.default_height() as f64);
                }
            });
        }
        if self.reset_on_exit.get() {
            let dir = crate::paths::data_dir();
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    let _ = if p.is_dir() {
                        std::fs::remove_dir_all(&p)
                    } else {
                        std::fs::remove_file(&p)
                    };
                }
            }
            crate::secrets::delete("opensubtitles");
        }
        if self.restart_on_exit.get() || self.reset_on_exit.get() {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).spawn();
            }
        }
    }

    /// Close and start again (Settings › Reset).
    pub fn restart_app(&self) {
        self.restart_on_exit.set(true);
        self.window.close();
    }

    pub fn set_status(&self, text: &str) {
        self.status.set_text(text);
        self.status
            .set_tooltip_text(if text.is_empty() { None } else { Some(text) });
    }

    /// Run a task with a status line; errors are shown, not thrown.
    pub async fn run<F, Fut>(self: &Rc<Self>, label: &str, work: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), AppError>>,
    {
        self.set_status(label);
        match work().await {
            Ok(()) => self.set_status(""),
            Err(e) => {
                crate::log(&format!("{label} {e:?}"));
                self.set_status(&e.to_string());
            }
        }
    }

    pub fn is_section(&self) -> bool {
        matches!(self.mode.get(), Mode::Live | Mode::Movies | Mode::Series)
    }

    pub fn next_version(&self) -> u64 {
        let v = self.load_version.get() + 1;
        self.load_version.set(v);
        v
    }

    // ================================================================ playlists

    pub async fn reload_sources(self: &Rc<Self>, select: Option<i64>) {
        let repo = self.repo.clone();
        let list = bg(move || repo.sources()).await.unwrap_or_default();
        let cur = self.source.borrow().as_ref().map(|s| s.id);
        let pick = list
            .iter()
            .find(|s| Some(s.id) == select)
            .or_else(|| list.iter().find(|s| Some(s.id) == cur))
            .or_else(|| list.first())
            .cloned();
        *self.sources.borrow_mut() = list;
        *self.source.borrow_mut() = pick;
        self.update_playlist_header().await;
    }

    /// Free TV playlists (normally one; older versions could leave a second).
    pub fn free_tv_sources(&self) -> Vec<Source> {
        let id = settings::get().free_tv_source_id;
        self.sources
            .borrow()
            .iter()
            .filter(|s| s.kind == "m3u" && (Some(s.id) == id || freetv::is_free_tv(&s.location)))
            .cloned()
            .collect()
    }

    /// Keep exactly one Free TV playlist, on the all-channels list.
    async fn normalize_free_tv(self: &Rc<Self>) {
        let all = self.free_tv_sources();
        if all.is_empty() {
            if settings::get().free_tv_source_id.is_some() {
                settings::update(|s| s.free_tv_source_id = None);
            }
            return;
        }
        let id = settings::get().free_tv_source_id;
        let keep = all
            .iter()
            .find(|s| s.location.eq_ignore_ascii_case(freetv::ALL_URL))
            .or_else(|| all.iter().find(|s| Some(s.id) == id))
            .unwrap_or(&all[0])
            .clone();
        let mut changed = false;
        for extra in all.iter().filter(|s| s.id != keep.id) {
            crate::log(&format!(
                "free tv: removing duplicate playlist {} ({})",
                extra.id, extra.location
            ));
            if self.playing().is_some_and(|p| p.source_id == extra.id) {
                self.stop_playback();
            }
            let (repo, eid) = (self.repo.clone(), extra.id);
            let _ = bg(move || repo.remove_source(eid)).await;
            settings::update(|s| {
                s.source_added.remove(&eid);
            });
            changed = true;
        }
        if !keep.location.eq_ignore_ascii_case(freetv::ALL_URL) {
            let (repo, kid) = (self.repo.clone(), keep.id);
            if let Err(e) = bg(move || {
                repo.update_source(kid, freetv::NAME, Some(freetv::ALL_URL), None, None, None)
            })
            .await
            {
                crate::log(&format!("free tv: could not switch to the full list: {e}"));
            }
            changed = true;
        }
        if id != Some(keep.id) {
            settings::update(|s| s.free_tv_source_id = Some(keep.id));
            changed = true;
        }
        if changed {
            let cur = self.source.borrow().as_ref().map(|s| s.id);
            self.reload_sources(cur).await;
        }
    }

    /// First start: Free TV comes preinstalled, so there is something to watch at once.
    /// Tried again on the next start if the download fails (offline).
    async fn preinstall_free_tv(self: &Rc<Self>) {
        if settings::get().free_tv_preinstalled || arg("--screenshot").is_some() {
            return;
        }
        if self.sources.borrow().is_empty() {
            self.apply_free_tv(true).await;
            if self.free_tv_sources().is_empty() {
                return;
            }
        }
        settings::update(|s| s.free_tv_preinstalled = true);
    }

    /// Settings › Sources › Free TV: add, re-download or remove the iptv-org playlist.
    pub async fn apply_free_tv(self: &Rc<Self>, enabled: bool) {
        if self.free_tv_busy.get() {
            return; // a second click while downloading must not add a second playlist
        }
        self.free_tv_busy.set(true);
        let existing = self.free_tv_sources();
        if !enabled {
            if !existing.is_empty() {
                let me = self.clone();
                self.run("Removing Free TV…", || async move {
                    for src in existing {
                        if me.playing().is_some_and(|p| p.source_id == src.id) {
                            me.stop_playback();
                        }
                        let (repo, id) = (me.repo.clone(), src.id);
                        bg(move || repo.remove_source(id)).await?;
                        settings::update(|s| {
                            s.source_added.remove(&id);
                        });
                        if me.source.borrow().as_ref().is_some_and(|s| s.id == id) {
                            *me.source.borrow_mut() = None;
                        }
                    }
                    me.reload_sources(None).await;
                    Ok(())
                })
                .await;
            }
            settings::update(|s| s.free_tv_source_id = None);
            self.refresh_settings();
            self.free_tv_busy.set(false);
            return;
        }
        let me = self.clone();
        self.run("Downloading Free TV channels…", || async move {
            if !existing.is_empty() {
                me.normalize_free_tv().await;
                let id = settings::get().free_tv_source_id;
                if let Some(id) = id.filter(|id| {
                    me.sources
                        .borrow()
                        .iter()
                        .any(|s| s.id == *id && s.location == freetv::ALL_URL)
                }) {
                    let repo = me.repo.clone();
                    bg(move || repo.refresh(id)).await?;
                }
                let cur = me.source.borrow().as_ref().map(|s| s.id);
                me.reload_sources(cur).await;
                return Ok(());
            }
            let keep = me.source.borrow().as_ref().map(|s| s.id);
            let repo = me.repo.clone();
            let popup = super::loading::Loading::show(&me.window, freetv::NAME, false);
            let sink = popup.sink();
            let added = bg(move || {
                let report = move |s| sink.lock().unwrap_or_else(|e| e.into_inner()).push(s);
                repo.add_m3u(freetv::NAME, freetv::ALL_URL, &report)
            })
            .await;
            let id = match added {
                Ok(id) => id,
                Err(e) => {
                    popup.failed(&e.to_string());
                    return Err(e);
                }
            };
            settings::update(|s| s.free_tv_source_id = Some(id));
            popup.report(crate::repo::LoadStatus::new(crate::repo::Step::Pages, ""));
            me.reload_sources(keep.or(Some(id))).await;
            popup.done();
            Ok(())
        })
        .await;
        self.refresh_settings();
        if (self.mode.get() == Mode::Live && self.is_free_tv_source())
            || self.mode.get() == Mode::Dashboard
        {
            self.show_mode().await;
        }
        self.free_tv_busy.set(false);
    }

    pub fn refresh_settings(&self) {
        if let Some(sv) = self.settings_view.borrow().clone() {
            sv.refresh();
        }
    }

    pub async fn update_playlist_header(self: &Rc<Self>) {
        let Some(src) = self.source.borrow().clone() else {
            self.playlist_name.set_text("No playlist");
            self.playlist_count.set_text("Add a source to begin");
            return;
        };
        self.playlist_name.set_text(&src.name);
        let repo = self.repo.clone();
        let id = src.id;
        let (live, movies, series) = bgv(move || {
            (
                repo.count_kind(id, "live"),
                repo.count_kind(id, "movie"),
                repo.count_kind(id, "series"),
            )
        })
        .await;
        let mut parts = Vec::new();
        if live > 0 {
            parts.push(format!("{} channels", repo::compact(live)));
        }
        if movies > 0 {
            parts.push(format!("{} movies", repo::compact(movies)));
        }
        if series > 0 {
            parts.push(format!("{} series", repo::compact(series)));
        }
        self.playlist_count.set_text(&if parts.is_empty() {
            src.kind_label().to_string()
        } else {
            parts.join(" · ")
        });
    }

    pub async fn check_source(self: &Rc<Self>, src: Source, announce: bool) {
        let (repo, id) = (self.repo.clone(), src.id);
        match bg(move || repo.test_connection(id)).await {
            Ok(msg) => {
                self.source_online.borrow_mut().insert(src.id, true);
                if announce {
                    self.set_status(&format!("{}: {msg}", src.name));
                }
            }
            Err(e) => {
                self.source_online.borrow_mut().insert(src.id, false);
                if announce {
                    self.set_status(&format!("{}: {e}", src.name));
                }
            }
        }
    }

    /// Playlist menu: playlists with a status dot and a ⋮ menu, then shortcuts.
    fn playlist_menu(self: &Rc<Self>) {
        let mut fixed = Vec::new();
        if let Some(cur) = self.source.borrow().clone() {
            let (me, c) = (self.clone(), cur.clone());
            fixed.push(MenuRow::new(
                "madhyam-help-about-symbolic",
                "Playlist info",
                false,
                move || {
                    let (me, c) = (me.clone(), c.clone());
                    spawn(async move { me.show_playlist_info(c).await });
                },
            ));
            if cur.kind == "xtream" {
                let (me, c) = (self.clone(), cur.clone());
                fixed.push(MenuRow::new(
                    "madhyam-dialog-password-symbolic",
                    "Account info",
                    false,
                    move || {
                        let (me, c) = (me.clone(), c.clone());
                        spawn(async move { me.show_account_info(c).await });
                    },
                ));
            }
        }
        let me = self.clone();
        fixed.push(MenuRow::new(
            "madhyam-list-add-symbolic",
            "Add playlist",
            false,
            move || me.open_add_source(),
        ));

        let header = kit::hbox(10);
        kit::margins(&header, 10, 6, 4, 6);
        let hi = kit::icon("madhyam-view-list-symbolic", 13);
        hi.add_css_class("muted");
        header.append(&hi);
        let ht = kit::text("MY PLAYLISTS", 11.0, "muted", 700, false);
        ht.set_hexpand(true);
        header.append(&ht);
        let me = self.clone();
        let manage = kit::icon_button(
            "madhyam-preferences-system-symbolic",
            "Manage sources",
            move || {
                kit::close_popover();
                me.open_settings("Sources");
            },
        );
        manage.add_css_class("small");
        header.append(&manage);
        let list = kit::vbox(0);
        for src in self.sources.borrow().clone() {
            list.append(&self.playlist_row(&src));
        }
        let sep = kit::line();
        kit::margins(&sep, 6, 6, 6, 4);
        kit::pop_menu(
            &self.playlist_btn,
            kit::Menu {
                prefix: vec![header.upcast(), list.upcast(), sep.upcast()],
                fixed,
                width: 320,
                ..Default::default()
            },
        );
    }

    fn playlist_row(self: &Rc<Self>, src: &Source) -> gtk::Widget {
        let active = self
            .source
            .borrow()
            .as_ref()
            .is_some_and(|s| s.id == src.id);
        let g = kit::hbox(12);
        kit::margins(&g, 8, 6, 4, 6);
        let icon_box = gtk::Overlay::new();
        let ib = kit::hbox(0);
        ib.add_css_class("chip-icon");
        ib.set_size_request(36, 36);
        let ic = kit::icon(
            if src.kind == "xtream" {
                "madhyam-network-server-symbolic"
            } else {
                "madhyam-view-list-symbolic"
            },
            15,
        );
        ic.set_halign(gtk::Align::Center);
        ib.set_homogeneous(true);
        if active {
            ic.add_css_class("accent");
        }
        ib.append(&ic);
        icon_box.set_child(Some(&ib));
        let online = self.source_online.borrow().get(&src.id).copied();
        let dot = gtk::Label::new(Some("●"));
        dot.set_markup(&format!(
            "<span foreground='{}' size='small'>●</span>",
            match online {
                Some(true) => "#22C55E",
                Some(false) => "#EF4444",
                None => "#9CA3AF",
            }
        ));
        dot.set_halign(gtk::Align::End);
        dot.set_valign(gtk::Align::End);
        dot.set_tooltip_text(Some(match online {
            Some(true) => "Online",
            Some(false) => "Not reachable",
            None => "Not checked yet",
        }));
        icon_box.add_overlay(&dot);
        g.append(&icon_box);
        let t = kit::vbox(0);
        t.set_hexpand(true);
        t.set_valign(gtk::Align::Center);
        let name = kit::text(
            &src.name,
            14.0,
            if active { "accent" } else { "" },
            600,
            false,
        );
        name.set_max_width_chars(1);
        t.append(&name);
        let when = if src.updated_at > 0 {
            format!(" · {}", repo::local(src.updated_at, "%b %-d, %H:%M"))
        } else {
            String::new()
        };
        let sub = kit::text(
            &format!("{}{when}", src.kind_label()),
            11.5,
            if active { "accent" } else { "muted" },
            400,
            false,
        );
        sub.set_max_width_chars(1);
        t.append(&sub);
        g.append(&t);
        let (me, s2) = (self.clone(), src.clone());
        let more = kit::icon_button("madhyam-view-more-symbolic", "More", move || {
            kit::close_popover();
            let (me, s3) = (me.clone(), s2.clone());
            glib::idle_add_local_once(move || me.playlist_more_menu(s3));
        });
        more.add_css_class("small");
        g.append(&more);
        let (me, s2) = (self.clone(), src.clone());
        let row = kit::bare(&g, move || {
            kit::close_popover();
            if !active {
                let (me, s3) = (me.clone(), s2.clone());
                spawn(async move { me.switch_source(s3).await });
            }
        });
        if active {
            row.add_css_class("menu-row");
            row.add_css_class("checked");
        } else {
            row.add_css_class("menu-row");
        }
        row.upcast()
    }

    fn playlist_more_menu(self: &Rc<Self>, src: Source) {
        let mut rows = Vec::new();
        let (me, s) = (self.clone(), src.clone());
        rows.push(MenuRow::new(
            "madhyam-help-about-symbolic",
            "Playlist info",
            false,
            move || {
                let (me, s) = (me.clone(), s.clone());
                spawn(async move { me.show_playlist_info(s).await });
            },
        ));
        if src.kind == "xtream" {
            let (me, s) = (self.clone(), src.clone());
            rows.push(MenuRow::new(
                "madhyam-dialog-password-symbolic",
                "Account info",
                false,
                move || {
                    let (me, s) = (me.clone(), s.clone());
                    spawn(async move { me.show_account_info(s).await });
                },
            ));
        }
        let (me, s) = (self.clone(), src.clone());
        rows.push(MenuRow::new(
            "madhyam-network-transmit-receive-symbolic",
            "Check again",
            false,
            move || {
                let (me, s) = (me.clone(), s.clone());
                spawn(async move { me.check_source(s, true).await });
            },
        ));
        let (me, s) = (self.clone(), src.clone());
        rows.push(MenuRow::new(
            "madhyam-view-refresh-symbolic",
            "Refresh playlist",
            false,
            move || {
                let (me, s) = (me.clone(), s.clone());
                spawn(async move { me.refresh_source(s).await });
            },
        ));
        rows.push(MenuRow::sep());
        let (me, s) = (self.clone(), src.clone());
        rows.push(
            MenuRow::new(
                "madhyam-edit-delete-symbolic",
                "Remove playlist",
                false,
                move || {
                    let (me, s) = (me.clone(), s.clone());
                    spawn(async move { me.remove_source(s).await });
                },
            )
            .danger(),
        );
        kit::pop_menu(
            &self.playlist_btn,
            kit::Menu {
                title: Some(&src.name),
                rows,
                width: 300,
                ..Default::default()
            },
        );
    }

    pub async fn switch_source(self: &Rc<Self>, s: Source) {
        *self.source.borrow_mut() = Some(s);
        self.update_playlist_header().await;
        if self.is_section() {
            self.stop_playback();
        }
        self.show_mode().await;
    }

    pub async fn show_playlist_info(self: &Rc<Self>, s: Source) {
        let (repo, id) = (self.repo.clone(), s.id);
        let (live, movies, series, epg) = bgv(move || {
            (
                repo.count_kind(id, "live"),
                repo.count_kind(id, "movie"),
                repo.count_kind(id, "series"),
                repo.epg_age(id),
            )
        })
        .await;
        let mut rows = vec![
            ("Name".to_string(), s.name.clone()),
            ("Type".into(), s.kind_label().into()),
            (
                if s.kind == "xtream" {
                    "Server"
                } else {
                    "Location"
                }
                .into(),
                s.location.clone(),
            ),
        ];
        if s.kind == "xtream" {
            rows.push(("Username".into(), s.username.clone()));
        }
        rows.push(("Live channels".into(), repo::group_digits(live)));
        rows.push(("Movies".into(), repo::group_digits(movies)));
        rows.push(("Series".into(), repo::group_digits(series)));
        if let Some(added) = settings::get().source_added.get(&s.id) {
            rows.push(("Added".into(), repo::local(*added, "%-d %b %Y, %H:%M")));
        }
        if s.updated_at > 0 {
            rows.push((
                "Last updated".into(),
                repo::local(s.updated_at, "%-d %b %Y, %H:%M"),
            ));
        }
        rows.push((
            "Guide cached".into(),
            epg.map(|a| format!("{:.1} hours ago", a.as_secs_f64() / 3600.0))
                .unwrap_or_else(|| "no".into()),
        ));
        let online = self.source_online.borrow().get(&s.id).copied();
        rows.push((
            "Connection".into(),
            match online {
                Some(true) => "Online".into(),
                Some(false) => "Not reachable".into(),
                None => "Not checked yet".into(),
            },
        ));
        super::dialogs::info_window(&self.window, "Playlist info", rows);
    }

    pub async fn show_account_info(self: &Rc<Self>, s: Source) {
        let me = self.clone();
        self.run("Checking account…", || async move {
            let (repo, id) = (me.repo.clone(), s.id);
            let a = bg(move || repo.account(id)).await?;
            let date = |t: Option<i64>, f: &str, none: &str| {
                t.map(|t| repo::local(t, f))
                    .unwrap_or_else(|| none.to_string())
            };
            super::dialogs::info_window(
                &me.window,
                "Account info",
                vec![
                    ("Status".into(), a.status.clone()),
                    (
                        "Expires".into(),
                        date(a.expires, "%-d %b %Y, %H:%M", "Never / unlimited"),
                    ),
                    (
                        "Connections".into(),
                        format!(
                            "{} active of {} allowed",
                            a.active_connections, a.max_connections
                        ),
                    ),
                    ("Trial".into(), if a.is_trial { "Yes" } else { "No" }.into()),
                    ("Created".into(), date(a.created, "%-d %b %Y", "—")),
                    (
                        "Server time zone".into(),
                        if a.timezone.is_empty() {
                            "—".into()
                        } else {
                            a.timezone.clone()
                        },
                    ),
                    (
                        "Stream formats".into(),
                        if a.formats.is_empty() {
                            "—".into()
                        } else {
                            a.formats.clone()
                        },
                    ),
                ],
            );
            Ok(())
        })
        .await;
    }

    pub fn open_add_source(self: &Rc<Self>) {
        let me = self.clone();
        spawn(async move {
            if me.settings_allowed().await {
                me.open_add_source_now();
            }
        });
    }

    fn open_add_source_now(self: &Rc<Self>) {
        let me = self.clone();
        super::dialogs::add_source(&self.window, move |r| {
            let me = me.clone();
            spawn(async move {
                let label = if r.xtream {
                    "Logging in and downloading the catalog…"
                } else {
                    "Downloading playlist…"
                };
                let me2 = me.clone();
                let popup = super::loading::Loading::show(&me.window, &r.name, r.xtream);
                let ok = Rc::new(Cell::new(false));
                let (p, ok2) = (popup.clone(), ok.clone());
                me.run(label, || async move {
                    let repo = me2.repo.clone();
                    let sink = p.sink();
                    let added = bg(move || {
                        let report = move |s| sink.lock().unwrap_or_else(|e| e.into_inner()).push(s);
                        if r.xtream {
                            repo.add_xtream(&r.name, &r.server, &r.username, &r.password, &report)
                        } else {
                            repo.add_m3u(&r.name, &r.url, &report)
                        }
                    })
                    .await;
                    let id = match added {
                        Ok(id) => id,
                        Err(e) => {
                            p.failed(&e.to_string());
                            return Err(e);
                        }
                    };
                    p.report(crate::repo::LoadStatus::new(crate::repo::Step::Pages, ""));
                    me2.reload_sources(Some(id)).await;
                    me2.show_mode().await;
                    me2.refresh_settings();
                    ok2.set(true);
                    Ok(())
                })
                .await;
                if ok.get() {
                    popup.done();
                }
            });
        });
    }

    pub async fn refresh_source(self: &Rc<Self>, s: Source) {
        let me = self.clone();
        self.run(&format!("Refreshing {}…", s.name), || async move {
            let (repo, id) = (me.repo.clone(), s.id);
            bg(move || repo.refresh(id)).await?;
            let cur = me.source.borrow().as_ref().map(|s| s.id);
            me.reload_sources(cur).await;
            if me.mode.get() != Mode::Settings {
                me.show_mode().await;
            }
            Ok(())
        })
        .await;
    }

    pub async fn remove_source(self: &Rc<Self>, s: Source) {
        if !self.settings_allowed().await {
            return;
        }
        let ok = super::dialogs::confirm(
            &self.window,
            &format!("Remove “{}”?", s.name),
            &format!("Its channels, movies and series are removed from {}. You can add the playlist again later.", crate::brand::NAME),
            "Remove",
            true,
        )
        .await;
        if !ok {
            return;
        }
        let me = self.clone();
        self.run(&format!("Removing {}…", s.name), || async move {
            me.stop_playback();
            let (repo, id) = (me.repo.clone(), s.id);
            bg(move || repo.remove_source(id)).await?;
            settings::update(|st| {
                st.source_added.remove(&id);
                st.source_user_agent.remove(&id);
            });
            if me.source.borrow().as_ref().is_some_and(|x| x.id == id) {
                *me.source.borrow_mut() = None;
            }
            me.reload_sources(None).await;
            if me.mode.get() != Mode::Settings {
                me.show_mode().await;
            }
            Ok(())
        })
        .await;
        self.refresh_settings();
    }

    pub fn open_settings(self: &Rc<Self>, page: &str) {
        self.nav_button(Mode::Settings).set_active(true);
        if let Some(sv) = self.settings_view.borrow().clone() {
            sv.open(page);
        }
    }

    // ================================================================ layout

    /// Show the panels that belong to the current mode.
    pub fn layout(&self) {
        let mode = self.mode.get();
        let page = matches!(
            mode,
            Mode::Dashboard
                | Mode::Search
                | Mode::Favorites
                | Mode::Recent
                | Mode::Added
                | Mode::Settings
        );
        let vod = matches!(mode, Mode::Movies | Mode::Series);
        let live = mode == Mode::Live;
        let details = vod && self.details.borrow().is_some();
        let playing = self.playing();
        let fs = self.fullscreen.get() && playing.is_some();

        self.rail.set_visible(!fs);
        self.topbar.set_visible(!fs);
        if fs {
            kit::margins(&self.body, 0, 0, 0, 0);
        } else {
            kit::margins(&self.body, 12, 6, 12, 12);
        }
        self.cats_panel.set_visible(!page && !fs);
        let child = if page && !fs {
            "page"
        } else if live || (fs && playing.as_ref().is_some_and(|p| p.is_live())) {
            "live"
        } else if details || (fs && self.vod_playing.get()) {
            "vod"
        } else {
            "grid"
        };
        self.stack.set_visible_child_name(child);

        // live
        self.list_panel.set_visible(live && !fs);
        let timeline = settings::get().guide_view != "List";
        self.guide_panel.set_visible(live && !fs && !timeline);
        self.timeline_panel.set_visible(live && !fs && timeline);
        self.live_card_header.set_visible(!fs);
        let live_now = playing.as_ref().is_some_and(|p| p.is_live());
        self.live_close_btn.set_visible(live_now);
        self.live_badge.set_visible(live_now);

        // movie page
        self.vod_player_box.set_visible(self.vod_playing.get());
        self.vod_header.set_visible(!fs);
        self.vod_card_header.set_visible(!fs);
        self.vod_header_title.set_visible(!self.vod_playing.get());
        self.vod_content.set_visible(!fs);
        if fs {
            kit::margins(&self.vod_player_box, 0, 0, 0, 0);
        } else {
            kit::margins(&self.vod_player_box, 18, 6, 18, 14);
        }
        for (card, frame) in [
            (&self.live_card, &self.live_frame),
            (&self.vod_card, &self.vod_frame),
        ] {
            if fs {
                card.add_css_class("fullscreen");
                frame.add_css_class("fullscreen");
                kit::margins(frame, 0, 0, 0, 0);
                card.set_halign(gtk::Align::Fill);
                card.set_valign(gtk::Align::Fill);
                card.set_vexpand(true);
            } else {
                card.remove_css_class("fullscreen");
                frame.remove_css_class("fullscreen");
                if card == &self.live_card {
                    kit::margins(frame, 14, 0, 14, 14);
                } else {
                    kit::margins(frame, 16, 0, 16, 16);
                }
                card.set_halign(gtk::Align::Center);
                card.set_valign(gtk::Align::Start);
                card.set_vexpand(false);
            }
        }
        self.vod_scroll.set_policy(
            gtk::PolicyType::Never,
            if fs {
                gtk::PolicyType::Never
            } else {
                gtk::PolicyType::Automatic
            },
        );
        self.size_players();
        self.update_search_pill();
    }

    /// Keep the video 16:9 inside its card: limit it by the room left in the window.
    pub fn size_players(&self) {
        let fs = self.fullscreen.get() && self.playing().is_some();
        let (w, h) = (
            self.window.width().max(self.window.default_width()),
            self.window.height().max(self.window.default_height()),
        );
        if fs {
            // Full screen: let the ratio box take the whole screen width (mpv letterboxes).
            let (sw, sh) = (self.window.width().max(320), self.window.height().max(180));
            self.live_ratio.set_limits(sw, sh);
            self.vod_ratio.set_limits(sw, sh);
            return;
        }
        let guide = if settings::get().guide_view != "List" {
            190
        } else {
            240
        };
        // From the panels' real sizes when they are shown (as the Windows app does), else
        // estimated from the window: panel − card padding − card header − guide below.
        let (pw, ph) = (self.player_panel.width(), self.player_panel.height());
        let (live_w, live_h) = if pw > 0 && ph > 0 {
            (pw - 28, ph - 62 - 28 - guide)
        } else {
            (
                w - 64 - 24 - 250 - 12 - 340 - 12 - 28,
                h - 62 - 18 - 62 - 28 - guide,
            )
        };
        self.live_ratio.set_limits(live_w.max(300), live_h.max(170));
        let (vw, vh) = (self.vod_scroll.width(), self.vod_scroll.height());
        let (vod_w, vod_h) = if vw > 0 && vh > 0 {
            (vw - 36 - 34, vh - 20 - 70 - 34 - 90)
        } else {
            (
                w - 64 - 24 - 250 - 12 - 36 - 34,
                h - 62 - 18 - 70 - 70 - 34 - 90,
            )
        };
        self.vod_ratio
            .set_limits(vod_w.clamp(320, 1280), vod_h.max(180));
    }

    pub fn update_search_pill(&self) {
        let mode = self.mode.get();
        let label = match mode {
            Mode::Movies | Mode::Series if self.details.borrow().is_some() => {
                format!(
                    "{} / {}",
                    mode.label(),
                    self.details
                        .borrow()
                        .as_ref()
                        .map(|d| d.display_name())
                        .unwrap_or_default()
                )
            }
            Mode::Live | Mode::Movies | Mode::Series => match self.group.borrow().as_ref() {
                Some(g) => format!("{} / {g}", mode.label()),
                None => format!("{} / All items", mode.label()),
            },
            Mode::Favorites | Mode::Recent | Mode::Settings | Mode::Added => {
                mode.label().to_string()
            }
            _ => "Advanced search".to_string(),
        };
        self.section_label.set_text(&label);
        self.search.set_placeholder_text(Some(match mode {
            Mode::Live | Mode::Movies | Mode::Series => "Search in this section…",
            Mode::Favorites | Mode::Recent => "Filter this section…",
            Mode::Settings => "Search everything…",
            _ => "Search in this playlist…",
        }));
        self.search_clear
            .set_visible(!self.search.text().is_empty());
    }

    // ================================================================ navigation

    pub fn nav_checked(self: &Rc<Self>, mode: Mode) {
        if self.nav_silent.get() {
            return;
        }
        // Parental controls can lock Settings: stay where we are until the PIN is entered.
        if mode == Mode::Settings && mode != self.mode.get() {
            let s = settings::snapshot();
            if s.parental_enabled && s.parental_lock_settings && !crate::parental::unlocked() {
                self.set_nav(self.mode.get());
                let me = self.clone();
                spawn(async move {
                    if me.settings_allowed().await {
                        me.nav_button(Mode::Settings).set_active(true);
                    }
                });
                return;
            }
            crate::parental::touch();
        }
        // One video surface: leaving the section that is playing stops playback.
        if self.playing().is_some() && mode != self.mode.get() {
            self.stop_playback();
        }
        self.mode.set(mode);
        *self.details.borrow_mut() = None;
        *self.group.borrow_mut() = None;
        // A search belongs to the section it was typed in.
        *self.vod_search.borrow_mut() = None;
        *self.search_hits.borrow_mut() = None;
        *self.live_search.borrow_mut() = None;
        self.cat_filter.set_text("");
        self.cat_filter_box.set_visible(false);
        if mode != Mode::Search {
            self.search.set_text("");
        }
        self.stop_search_timer();
        if !matches!(mode, Mode::Search | Mode::Settings) {
            settings::update(|s| s.last_view = mode.name().to_string());
        }
        let me = self.clone();
        spawn(async move {
            me.show_mode().await;
            if mode == Mode::Search {
                me.search.grab_focus();
            }
        });
    }

    /// Select a rail item from code without reloading.
    pub fn set_nav(&self, m: Mode) {
        self.nav_silent.set(true);
        self.nav_button(m).set_active(true);
        self.nav_silent.set(false);
        self.mode.set(m);
    }

    /// (Re)build whatever the current mode shows.
    pub async fn show_mode(self: &Rc<Self>) {
        let version = self.next_version();
        self.layout();
        match self.mode.get() {
            Mode::Live | Mode::Movies | Mode::Series => self.load_categories(version, None).await,
            Mode::Settings => {
                let sv = self.settings_view.borrow().clone();
                let sv = match sv {
                    Some(sv) => sv,
                    None => {
                        let sv = SettingsView::new(self);
                        *self.settings_view.borrow_mut() = Some(sv.clone());
                        sv
                    }
                };
                kit::clear(&self.page_host);
                if let Some(p) = sv.root.parent() {
                    if let Ok(b) = p.downcast::<gtk::Box>() {
                        b.remove(&sv.root);
                    }
                }
                self.page_host.append(&sv.root);
                sv.refresh();
            }
            _ => self.show_page(version).await,
        }
    }

    // ================================================================ search pill

    pub(super) fn stop_search_timer(&self) {
        if let Some(id) = self.search_timer.borrow_mut().take() {
            id.remove();
        }
    }

    fn search_changed(self: &Rc<Self>) {
        self.update_search_pill();
        self.stop_search_timer();
        // Sections and Favorites/Recent filter as you type; elsewhere Enter opens advanced search.
        if self.is_section()
            || matches!(
                self.mode.get(),
                Mode::Favorites | Mode::Recent | Mode::Search
            )
        {
            let w = Rc::downgrade(self);
            let id =
                glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                    if let Some(a) = w.upgrade() {
                        a.search_timer.borrow_mut().take();
                        spawn(async move { a.apply_search().await });
                    }
                });
            *self.search_timer.borrow_mut() = Some(id);
        }
    }

    async fn search_enter(self: Rc<Self>) {
        self.stop_search_timer();
        if matches!(
            self.mode.get(),
            Mode::Dashboard | Mode::Added | Mode::Settings
        ) && !self.search.text().trim().is_empty()
        {
            self.stop_playback();
            *self.details.borrow_mut() = None;
            self.set_nav(Mode::Search);
            self.show_mode().await;
            return;
        }
        self.apply_search().await;
    }

    pub async fn apply_search(self: &Rc<Self>) {
        let q = self.search.text().trim().to_string();
        match self.mode.get() {
            Mode::Live => self.search_live(q).await,
            Mode::Movies | Mode::Series => self.search_vod(q).await,
            Mode::Search | Mode::Favorites | Mode::Recent => {
                let v = self.next_version();
                self.show_page(v).await
            }
            _ => {}
        }
    }

    // ================================================================ hooks for settings / pages

    /// Re-render names, numbers and guide after a display setting changed.
    pub fn refresh_list_appearance(self: &Rc<Self>) {
        let numbers = settings::get().show_numbers;
        for (i, r) in self.live_rows.borrow().iter().enumerate() {
            r.title
                .set_text(&live_title(&r.item, if numbers { i + 1 } else { 0 }));
        }
        let me = self.clone();
        spawn(async move {
            me.update_now_next_rows().await;
            if me.guide_item.borrow().is_some() {
                me.load_guide().await;
            }
        });
    }

    pub fn refresh_guide_view(self: &Rc<Self>) {
        self.layout();
        if self.guide_item.borrow().is_some() {
            let me = self.clone();
            spawn(async move { me.load_guide().await });
        }
    }

    pub async fn reload_favorites(self: &Rc<Self>) {
        let repo = self.repo.clone();
        *self.favorites.borrow_mut() = bgv(move || repo.favorite_ids()).await;
        self.refresh_list_appearance();
    }

    /// Settings › "Refresh all guides now".
    pub async fn refresh_all_guides(self: &Rc<Self>) {
        let ids: Vec<i64> = self.sources.borrow().iter().map(|s| s.id).collect();
        let extra = settings::get().extra_epg_urls.clone();
        let repo = self.repo.clone();
        bgv(move || {
            for id in ids {
                if let Err(e) = repo.refresh_epg(id) {
                    crate::log(&format!("guide refresh failed for source {id}: {e}"));
                }
            }
            repo.refresh_custom_epg(&extra);
        })
        .await;
        self.refresh_list_appearance();
    }

    pub async fn toggle_favorite(self: &Rc<Self>, it: Item) {
        let (repo, id) = (self.repo.clone(), it.id.clone());
        let Ok(on) = bg(move || repo.toggle_favorite(&id)).await else {
            return;
        };
        if on {
            self.favorites.borrow_mut().insert(it.id.clone());
        } else {
            self.favorites.borrow_mut().remove(&it.id);
        }
        for r in self
            .live_rows
            .borrow()
            .iter()
            .filter(|r| r.item.id == it.id)
        {
            paint_star(&r.star, on);
        }
        if self
            .details
            .borrow()
            .as_ref()
            .is_some_and(|d| d.id == it.id)
        {
            self.render_vod_page();
        }
        if self.mode.get() == Mode::Favorites {
            let v = self.next_version();
            self.show_page(v).await;
        }
    }

    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.borrow().contains(id)
    }

    // ================================================================ keyboard

    fn key_pressed(self: &Rc<Self>, key: gdk::Key, state: gdk::ModifierType) -> glib::Propagation {
        let typing = gtk::prelude::GtkWindowExt::focus(&self.window).is_some_and(|f| {
            f.is::<gtk::Text>() || f.is::<gtk::Entry>() || f.is::<gtk::PasswordEntry>()
        });
        let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
        if key == gdk::Key::Escape && kit::open_popover().is_some() {
            kit::close_popover();
            return glib::Propagation::Stop;
        }
        let playing = self.playing();
        let vod = self.vod_playing.get();
        let fs = self.fullscreen.get();
        let me = self.clone();
        let handled = match key {
            gdk::Key::F11 => {
                self.toggle_fullscreen(false);
                true
            }
            gdk::Key::Escape if fs => {
                self.toggle_fullscreen(false);
                true
            }
            gdk::Key::Escape if vod => {
                self.stop_playback();
                true
            }
            gdk::Key::Escape if self.details.borrow().is_some() => {
                self.close_details();
                true
            }
            gdk::Key::Escape if typing && !self.search.text().is_empty() => {
                self.search.set_text("");
                true
            }
            gdk::Key::k | gdk::Key::K | gdk::Key::f | gdk::Key::F if ctrl => {
                self.search.grab_focus();
                self.search.select_region(0, -1);
                true
            }
            gdk::Key::space if !typing && playing.is_some() => {
                self.toggle_pause(false);
                true
            }
            gdk::Key::Up if !typing && vod => {
                self.change_volume(5);
                true
            }
            gdk::Key::Down if !typing && vod => {
                self.change_volume(-5);
                true
            }
            gdk::Key::j | gdk::Key::J if !typing && playing.is_some() => {
                self.change_audio_delay(-50);
                true
            }
            gdk::Key::k | gdk::Key::K if !typing && !ctrl && playing.is_some() => {
                self.change_audio_delay(50);
                true
            }
            gdk::Key::g | gdk::Key::G if !typing && playing.is_some() => {
                self.change_sub_delay(-50);
                true
            }
            gdk::Key::h | gdk::Key::H if !typing && playing.is_some() => {
                self.change_sub_delay(50);
                true
            }
            gdk::Key::m | gdk::Key::M if !typing && playing.is_some() => {
                self.toggle_mute();
                true
            }
            gdk::Key::Page_Up if playing.as_ref().is_some_and(|p| p.is_live()) => {
                spawn(async move { me.zap(-1).await });
                true
            }
            gdk::Key::Up if fs && playing.as_ref().is_some_and(|p| p.is_live()) => {
                spawn(async move { me.zap(-1).await });
                true
            }
            gdk::Key::Page_Down if playing.as_ref().is_some_and(|p| p.is_live()) => {
                spawn(async move { me.zap(1).await });
                true
            }
            gdk::Key::Down if fs && playing.as_ref().is_some_and(|p| p.is_live()) => {
                spawn(async move { me.zap(1).await });
                true
            }
            gdk::Key::Left if !typing && (ctrl || fs || vod) => {
                self.seek_by(-10_000);
                true
            }
            gdk::Key::Right if !typing && (ctrl || fs || vod) => {
                self.seek_by(10_000);
                true
            }
            _ => false,
        };
        if handled {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    }

    /// Full screen: the window covers the monitor and everything but the picture is hidden.
    pub fn toggle_fullscreen(self: &Rc<Self>, force: bool) {
        if !force && self.playing().is_none() && !self.fullscreen.get() {
            return; // nothing to show full screen
        }
        let on = !self.fullscreen.get();
        self.fullscreen.set(on);
        if on {
            self.window.fullscreen();
        } else {
            self.window.unfullscreen();
        }
        self.controls.set_fullscreen_icon(on);
        self.layout();
    }

    fn on_tick(self: &Rc<Self>) {
        let t = self.ticks.get() + 1;
        self.ticks.set(t);
        self.update_seek_bar();
        self.update_lock_button(); // hides itself when the 10 minutes run out
        if t % 15 == 0 {
            self.save_progress();
        }
        // MADHYAM_DEBUG=1: every 5 s, what the video pipeline is doing (for reports of a black picture).
        if t % 5 == 0 && std::env::var_os("MADHYAM_DEBUG").is_some() && self.playing().is_some() {
            if let (Some(m), Some(area)) = (self.mpv(), self.controls.area.borrow().as_ref()) {
                crate::log(&format!(
                    "debug: time-pos={:?} {} area {}x{} realized={} mapped={} fullscreen={}",
                    m.get_f64("time-pos"),
                    crate::mpv::stats(),
                    area.width(),
                    area.height(),
                    area.is_realized(),
                    area.is_mapped(),
                    self.fullscreen.get()
                ));
            }
        }
        if t % 60 == 0 && self.mode.get() == Mode::Live {
            let me = self.clone();
            spawn(async move {
                me.update_now_next_rows().await;
                if me.guide_item.borrow().is_some() {
                    me.load_guide().await;
                }
            });
        }
    }
}

/// "12. Channel" when channel numbers are on.
pub fn live_title(it: &Item, number: usize) -> String {
    if number > 0 {
        format!("{number}. {}", it.display_name())
    } else {
        it.display_name()
    }
}

pub fn paint_star(b: &gtk::Button, on: bool) {
    b.set_child(Some(&kit::icon(
        if on {
            "madhyam-starred-symbolic"
        } else {
            "madhyam-non-starred-symbolic"
        },
        14,
    )));
    if on {
        b.add_css_class("on");
    } else {
        b.remove_css_class("on");
    }
}
