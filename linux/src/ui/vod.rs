//! Movies and series: the paged poster grid (sort, minimum rating, per page), the details
//! page (backdrop, poster, chips, plot, cast, trailer, seasons and episodes) and the in-page
//! player with "About" and the episodes below it. Twin of MainWindow.Vod.cs.

use super::app::{App, Mode};
use super::kit::{self, MenuRow};
use super::{bg, bgv, spawn};
use crate::repo::{self, DetailsInfo, Item};
use crate::settings;
use gtk::prelude::*;
use regex::Regex;
use std::rc::Rc;
use std::sync::LazyLock;

const SORT_OPTIONS: [(&str, &str); 7] = [
    ("provider", "Provider order"),
    ("added_desc", "Date added (latest)"),
    ("added_asc", "Date added (oldest)"),
    ("name_asc", "Name A–Z"),
    ("name_desc", "Name Z–A"),
    ("rating_desc", "Top rated"),
    ("rating_asc", "Lowest rated"),
];

fn sort_icon(v: &str) -> &'static str {
    match v {
        "provider" => "madhyam-view-list-symbolic",
        "added_desc" | "added_asc" => "madhyam-appointment-soon-symbolic",
        "name_asc" => "madhyam-view-sort-ascending-symbolic",
        "name_desc" => "madhyam-view-sort-descending-symbolic",
        "rating_desc" => "madhyam-starred-symbolic",
        _ => "madhyam-non-starred-symbolic",
    }
}

static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(19|20)\d{2}\b").unwrap());

impl App {
    // ================================================================ grid

    pub fn highlight_per_page(&self) {
        let n = self.per_page.get().to_string();
        for b in &self.per_buttons {
            if b.label().as_deref() == Some(n.as_str()) {
                b.add_css_class("on");
            } else {
                b.remove_css_class("on");
            }
        }
    }

    pub async fn load_grid(self: &Rc<Self>, reset_page: bool) {
        if reset_page {
            self.page.set(0);
        }
        self.highlight_per_page();
        let s = settings::snapshot();
        let kind = self.kind_now();
        self.sort_label.set_text(
            SORT_OPTIONS
                .iter()
                .find(|o| o.0 == s.vod_sort)
                .map(|o| o.1)
                .unwrap_or("Provider order"),
        );
        self.refine_label.set_text(&if s.vod_min_rating > 0.0 {
            format!("Rating {:.0}+", s.vod_min_rating)
        } else {
            "Any rating".into()
        });
        let Some(src) = self.source.borrow().clone() else {
            self.fill_grid(Vec::new());
            self.grid_title.set_text("No playlist");
            self.grid_count
                .set_text("Add a playlist with the + button to start.");
            self.update_pager();
            return;
        };
        let version = self.load_version.get();
        let per = self.per_page.get();
        let page = self.page.get();
        let items;
        let search = self.vod_search.borrow().clone();
        if let Some(q) = search {
            // Search results are paged here (the engine returns the best 300 hits).
            if self.search_hits.borrow().is_none() {
                let (repo, q2) = (self.repo.clone(), q.clone());
                let hits = bg(move || repo.search(&q2, Some(kind)))
                    .await
                    .unwrap_or_default();
                if version != self.load_version.get() {
                    return;
                }
                let hidden = self.hidden_now();
                let min = s.vod_min_rating;
                let filtered: Vec<Item> = hits
                    .into_iter()
                    .filter(|h| h.source_id == src.id && !hidden.contains(&h.group))
                    .filter(|h| {
                        min <= 0.0
                            || h.rating
                                .as_deref()
                                .and_then(|r| r.parse::<f64>().ok())
                                .is_some_and(|r| r >= min)
                    })
                    .collect();
                *self.search_hits.borrow_mut() = Some(filtered);
            }
            let hits = self.search_hits.borrow().clone().unwrap_or_default();
            self.grid_total.set(hits.len() as i64);
            items = hits.into_iter().skip(page * per).take(per).collect();
            self.grid_title.set_text(&format!("Results for “{q}”"));
        } else {
            let group = self.group.borrow().clone();
            let hidden = if group.is_none() {
                self.hidden_now()
            } else {
                Vec::new()
            };
            let (repo, id, sort, min) = (
                self.repo.clone(),
                src.id,
                s.vod_sort.clone(),
                s.vod_min_rating,
            );
            let r = bg(move || {
                repo.query(
                    kind,
                    Some(id),
                    group.as_deref(),
                    &hidden,
                    &sort,
                    min,
                    page * per,
                    per,
                    None,
                )
            })
            .await;
            if version != self.load_version.get() {
                return;
            }
            let (total, list) = r.unwrap_or_default();
            self.grid_total.set(total);
            items = list;
            self.grid_title
                .set_text(self.group.borrow().as_deref().unwrap_or("All items"));
        }
        let total = self.grid_total.get();
        self.grid_count.set_text(&if total == 0 {
            if self.vod_search.borrow().is_some() {
                "Nothing matches.".into()
            } else {
                "No items here.".into()
            }
        } else {
            format!(
                "{} {}",
                repo::group_digits(total),
                if kind == "series" { "series" } else { "movies" }
            )
        });
        self.fill_grid(items);
        self.grid_scroll.vadjustment().set_value(0.0);
        self.update_pager();
    }

    fn fill_grid(self: &Rc<Self>, items: Vec<Item>) {
        self.poster_grid.remove_all();
        let s = settings::snapshot();
        let (w, h) = match s.cover_size.as_str() {
            "Small" => (136, 202),
            "Large" => (220, 328),
            _ => (176, 262),
        };
        for it in &items {
            let (img, _frame) = kit::picture_frame(
                it.logo.as_deref(),
                w,
                h,
                "madhyam-video-x-generic-symbolic",
                false,
                "poster-frame",
            );
            if it.has_rating() {
                let b = kit::badge(
                    &format!("★ {}", it.rating.as_deref().unwrap_or("")),
                    "rating",
                );
                b.set_halign(gtk::Align::End);
                b.set_valign(gtk::Align::Start);
                kit::margins(&b, 8, 8, 8, 8);
                img.add_overlay(&b);
            }
            let stack = kit::vbox(0);
            stack.set_size_request(w, -1);
            stack.append(&img);
            if s.titles_under_covers {
                let t = kit::lines(&it.display_name(), 13.0, "", 2);
                t.set_justify(gtk::Justification::Center);
                t.set_xalign(0.5);
                t.set_max_width_chars(1);
                kit::margins(&t, 4, 9, 4, 0);
                stack.append(&t);
            }
            let (me, it2) = (Rc::downgrade(self), it.clone());
            let b = kit::bare(&stack, move || {
                if let Some(a) = me.upgrade() {
                    let it = it2.clone();
                    spawn(async move { a.show_details(it).await });
                }
            });
            b.set_tooltip_text(Some(&it.name));
            b.set_valign(gtk::Align::Start);
            kit::margins(&b, 0, 0, 18, 22);
            self.poster_grid.insert(&b, -1);
        }
        *self.grid_items.borrow_mut() = items;
    }

    fn page_count(&self) -> usize {
        ((self.grid_total.get() as f64 / self.per_page.get() as f64).ceil() as usize).max(1)
    }

    fn update_pager(&self) {
        self.page_text
            .set_text(&format!("{} / {}", self.page.get() + 1, self.page_count()));
        self.prev_page.set_sensitive(self.page.get() > 0);
        self.next_page
            .set_sensitive(self.page.get() + 1 < self.page_count());
    }

    pub async fn search_vod(self: &Rc<Self>, q: String) {
        self.close_details_silently();
        *self.vod_search.borrow_mut() = if q.is_empty() { None } else { Some(q) };
        *self.search_hits.borrow_mut() = None;
        self.next_version();
        self.load_grid(true).await;
    }

    pub fn sort_menu(self: &Rc<Self>) {
        let cur = settings::get().vod_sort.clone();
        let rows = SORT_OPTIONS
            .iter()
            .map(|(v, l)| {
                let (me, v2) = (self.clone(), v.to_string());
                MenuRow::new(sort_icon(v), l, *v == cur, move || {
                    settings::update(|s| s.vod_sort = v2.clone());
                    *me.search_hits.borrow_mut() = None;
                    let me = me.clone();
                    spawn(async move { me.load_grid(true).await });
                })
            })
            .collect();
        kit::menu(&self.sort_btn, "Sort by", rows);
    }

    pub fn refine_menu(self: &Rc<Self>) {
        let cur = settings::get().vod_min_rating;
        let options = [
            (0.0, "Any rating"),
            (9.0, "9 and up"),
            (8.0, "8 and up"),
            (7.0, "7 and up"),
            (6.0, "6 and up"),
            (5.0, "5 and up"),
        ];
        let rows = options
            .iter()
            .map(|(v, l)| {
                let (me, v2) = (self.clone(), *v);
                MenuRow::new(
                    if *v == 0.0 {
                        "madhyam-edit-clear-all-symbolic"
                    } else {
                        "madhyam-starred-symbolic"
                    },
                    l,
                    (v - cur).abs() < 0.01,
                    move || {
                        settings::update(|s| s.vod_min_rating = v2);
                        *me.search_hits.borrow_mut() = None;
                        let me = me.clone();
                        spawn(async move { me.load_grid(true).await });
                    },
                )
            })
            .collect();
        kit::menu(&self.refine_btn, "Minimum rating", rows);
    }

    pub fn set_per_page(self: &Rc<Self>, n: usize) {
        self.per_page.set(n);
        settings::update(|s| s.per_page = n as i32);
        let me = self.clone();
        spawn(async move { me.load_grid(true).await });
    }

    pub fn change_page(self: &Rc<Self>, delta: i64) {
        let p = self.page.get() as i64 + delta;
        if p < 0 || p as usize >= self.page_count() {
            return;
        }
        self.page.set(p as usize);
        let me = self.clone();
        spawn(async move { me.load_grid(false).await });
    }

    // ================================================================ details page

    /// Open a film or series page: shows at once, then fills in details and episodes.
    pub async fn show_details(self: &Rc<Self>, it: Item) {
        if self.vod_playing.get() {
            self.stop_playback();
        }
        let (repo, id) = (self.repo.clone(), it.id.clone());
        let fresh = bgv(move || repo.item(&id)).await.unwrap_or(it);
        *self.details.borrow_mut() = Some(fresh.clone());
        *self.details_info.borrow_mut() = None;
        *self.ratings.borrow_mut() = None;
        self.episodes.borrow_mut().clear();
        *self.season.borrow_mut() = None;
        self.details_loading.set(true);
        self.layout();
        self.vod_header_title.set_text(&fresh.display_name());
        self.render_vod_page();
        self.scroll_vod_top();

        let (repo, f2) = (self.repo.clone(), fresh.clone());
        let info = match bg(move || {
            if f2.is_series() {
                repo.series_details(&f2)
            } else {
                repo.movie_details(&f2)
            }
        })
        .await
        {
            Ok(i) => i,
            Err(e) => {
                crate::log(&format!("details failed for {}: {e}", fresh.name));
                None
            }
        };
        let mut eps = Vec::new();
        if fresh.is_series() {
            let (repo, f2) = (self.repo.clone(), fresh.clone());
            match bg(move || repo.episodes(&f2)).await {
                Ok(e) => eps = e,
                Err(e) => self.set_status(&format!("Episodes could not be loaded: {e}")),
            }
        }
        if self.details.borrow().as_ref().map(|d| &d.id) != Some(&fresh.id) {
            return;
        }
        *self.details_info.borrow_mut() = info.clone();
        *self.episodes.borrow_mut() = eps;
        let season = self.resume_episode().and_then(|e| e.season).or_else(|| {
            self.episodes
                .borrow()
                .first()
                .and_then(|e| e.season.clone())
        });
        *self.season.borrow_mut() = season;
        self.details_loading.set(false);
        self.render_vod_page();

        // IMDb / Rotten Tomatoes (only shown when found).
        let year = fresh
            .year
            .clone()
            .or_else(|| YEAR.find(&fresh.name).map(|m| m.as_str().to_string()));
        let title = crate::ratings::clean_title(&fresh.name);
        let imdb = info.and_then(|i| i.imdb_id);
        let series = fresh.is_series();
        let ext =
            bgv(move || crate::ratings::fetch(&title, year.as_deref(), imdb.as_deref(), series))
                .await;
        if self.details.borrow().as_ref().map(|d| &d.id) != Some(&fresh.id) || ext.is_none() {
            return;
        }
        *self.ratings.borrow_mut() = ext;
        self.render_vod_page();
    }

    pub fn close_details(self: &Rc<Self>) {
        self.close_details_silently();
        self.update_search_pill();
    }

    /// Leave the details page (and its player) without touching the grid.
    pub fn close_details_silently(self: &Rc<Self>) {
        if self.details.borrow().is_none() {
            return;
        }
        *self.details.borrow_mut() = None;
        if self.vod_playing.get() {
            self.stop_playback();
        }
        *self.details_info.borrow_mut() = None;
        self.episodes.borrow_mut().clear();
        kit::clear(&self.vod_content);
        kit::clear(&self.vod_backdrop);
        *self.backdrop_key.borrow_mut() = None;
        self.layout();
    }

    /// Episode to offer as "Resume": started but unfinished, else the first unwatched one.
    pub fn resume_episode(&self) -> Option<Item> {
        let eps = self.episodes.borrow();
        let mut started: Vec<&Item> = eps
            .iter()
            .filter(|e| e.position > 0.0 && !e.is_watched())
            .collect();
        started.sort_by_key(|e| std::cmp::Reverse(e.played_at));
        if let Some(e) = started.first() {
            return Some((*e).clone());
        }
        if eps.iter().any(|e| e.is_watched()) {
            return eps.iter().find(|e| !e.is_watched()).cloned();
        }
        None
    }

    pub async fn reload_details_progress(self: &Rc<Self>, delay_ms: u64) {
        let Some(d) = self.details.borrow().clone() else {
            return;
        };
        if delay_ms > 0 {
            super::sleep_ms(delay_ms).await; // let the player's last progress write land
        }
        let (repo, d2) = (self.repo.clone(), d.clone());
        let old = self.episodes.borrow().clone();
        let (fresh, eps) = bgv(move || {
            (
                repo.item(&d2.id),
                if d2.is_series() {
                    repo.episodes(&d2).unwrap_or(old)
                } else {
                    old
                },
            )
        })
        .await;
        if self.details.borrow().as_ref().map(|x| &x.id) != Some(&d.id) {
            return;
        }
        if let Some(f) = fresh {
            *self.details.borrow_mut() = Some(f);
        }
        *self.episodes.borrow_mut() = eps;
        self.render_vod_page();
    }

    pub async fn play_movie(self: &Rc<Self>, start_over: bool) {
        let Some(it) = self.details.borrow().clone().filter(|d| !d.is_series()) else {
            return;
        };
        if start_over {
            let (repo, id, dur) = (self.repo.clone(), it.id.clone(), it.duration.max(1.0));
            bgv(move || repo.set_progress(&id, 0.0, dur)).await;
        }
        *self.playlist.borrow_mut() = vec![it];
        self.progress_stale.set(true);
        self.play_at(0).await;
    }

    pub async fn play_episode(self: &Rc<Self>, ep: Item) {
        let list = self.episodes.borrow().clone();
        let i = list.iter().position(|e| e.id == ep.id);
        *self.season.borrow_mut() = ep.season.clone();
        self.progress_stale.set(true);
        match i {
            Some(i) => {
                *self.playlist.borrow_mut() = list;
                self.play_at(i as i64).await;
            }
            None => {
                *self.playlist.borrow_mut() = vec![ep];
                self.play_at(0).await;
            }
        }
    }

    /// Open a film, series or episode from anywhere (dashboard, search, favorites, recent).
    pub async fn go_to_vod(self: &Rc<Self>, it: Item, play: bool) {
        let mut episode = None;
        let mut target = it.clone();
        if it.kind == "episode" {
            episode = Some(it.clone());
            if let Some(sid) = it.series_id.clone() {
                let repo = self.repo.clone();
                if let Some(series) = bgv(move || repo.item(&sid)).await {
                    target = series;
                }
            }
        }
        self.stop_playback();
        self.search.set_text("");
        // Clearing the box arms the filter-as-you-type timer, which would reload the grid over the page.
        self.stop_search_timer();
        *self.vod_search.borrow_mut() = None;
        *self.search_hits.borrow_mut() = None;
        self.set_nav(if target.is_series() || target.kind == "episode" {
            Mode::Series
        } else {
            Mode::Movies
        });
        *self.details.borrow_mut() = None;
        *self.group.borrow_mut() = None;
        let other = self
            .source
            .borrow()
            .as_ref()
            .is_none_or(|s| s.id != target.source_id);
        if other {
            let src = self
                .sources
                .borrow()
                .iter()
                .find(|s| s.id == target.source_id)
                .cloned();
            if let Some(src) = src {
                *self.source.borrow_mut() = Some(src);
                self.update_playlist_header().await;
            }
        }
        self.layout();
        let v = self.next_version();
        self.load_categories(v, None).await;
        if target.kind == "episode" {
            // Series item not found: play the episode on its own page.
            *self.details.borrow_mut() = Some(target.clone());
            *self.details_info.borrow_mut() = None;
            *self.episodes.borrow_mut() = vec![target.clone()];
            self.layout();
            self.play_episode(target).await;
            return;
        }
        self.show_details(target.clone()).await;
        if self.details.borrow().as_ref().map(|d| &d.id) != Some(&target.id) {
            return;
        }
        if let Some(ep) = episode {
            let found = self
                .episodes
                .borrow()
                .iter()
                .find(|e| e.id == ep.id)
                .cloned()
                .unwrap_or(ep);
            self.play_episode(found).await;
        } else if play && !target.is_series() {
            self.play_movie(false).await;
        } else if play {
            if let Some(next) = self.resume_episode() {
                self.play_episode(next).await;
            }
        }
    }

    // ================================================================ page rendering

    /// (Re)build the page: hero (details) or About (while playing), plus episodes.
    /// Back to the top of the details page, once the new content has its height.
    pub fn scroll_vod_top(&self) {
        let adj = self.vod_scroll.vadjustment();
        adj.set_value(0.0);
        gtk::glib::idle_add_local_once(move || adj.set_value(0.0));
    }

    pub fn render_vod_page(self: &Rc<Self>) {
        kit::clear(&self.vod_content);
        let Some(it) = self.details.borrow().clone() else {
            return;
        };
        if !self.vod_playing.get() && self.progress_stale.get() {
            self.progress_stale.set(false);
            let me = self.clone();
            spawn(async move { me.reload_details_progress(300).await });
        }
        let info = self.details_info.borrow().clone();
        // The list's own image is the reliable one; the details endpoint's poster is a fallback.
        let poster = it
            .logo
            .clone()
            .or_else(|| info.as_ref().and_then(|i| i.poster.clone()));
        let key = info
            .as_ref()
            .and_then(|i| i.backdrop.clone())
            .or_else(|| poster.clone())
            .unwrap_or_default();
        if self.backdrop_key.borrow().as_deref() != Some(key.as_str()) {
            *self.backdrop_key.borrow_mut() = Some(key.clone());
            kit::clear(&self.vod_backdrop);
            let b = kit::backdrop(
                Some(&key).filter(|k| !k.is_empty()).map(|k| k.as_str()),
                0.4,
            );
            b.set_vexpand(true);
            self.vod_backdrop.append(&b);
        }
        if !self.vod_playing.get() {
            self.vod_header_title.set_text("");
            self.vod_content
                .append(&self.hero(&it, info.as_ref(), poster.as_deref()));
        } else {
            if let Some(ep) = self.playing().filter(|p| p.kind == "episode") {
                if it.is_series() {
                    self.vod_content.append(&now_playing_episode(&ep));
                }
            }
            self.vod_content
                .append(&kit::section_header("About", None, None, ""));
            self.vod_content
                .append(&self.about(&it, info.as_ref(), poster.as_deref()));
        }
        // Seasons and episodes first, the trailer after them.
        if it.is_series() {
            self.vod_content.append(&self.seasons(&it));
        }
        if let Some(yt) = info.as_ref().and_then(|i| i.trailer.clone()) {
            self.vod_content.append(&trailer_section(&it, &yt, self));
        }
        if self.details_loading.get() {
            let t = kit::text(
                if it.is_series() {
                    "Loading details and episodes…"
                } else {
                    "Loading details…"
                },
                13.0,
                "muted",
                400,
                false,
            );
            t.set_margin_top(18);
            self.vod_content.append(&t);
        }
    }

    /// IMDb and Rotten Tomatoes chips, only when OMDb had a score.
    fn add_external_ratings(&self, chips: &gtk::Box) {
        let Some(r) = self.ratings.borrow().clone() else {
            return;
        };
        if let Some(imdb) = &r.imdb {
            let row = kit::hbox(7);
            let logo = gtk::Label::new(Some("IMDb"));
            logo.add_css_class("imdb");
            row.append(&logo);
            row.append(&kit::text(&format!("{imdb}/10"), 13.0, "", 600, false));
            let c = kit::hbox(0);
            c.add_css_class("chip");
            c.append(&row);
            c.set_tooltip_text(Some(
                &r.imdb_votes
                    .as_ref()
                    .map(|v| format!("IMDb · {v} votes"))
                    .unwrap_or_else(|| "IMDb".into()),
            ));
            chips.append(&c);
        }
        if let Some(rt) = &r.rotten {
            let fresh = rt
                .trim_end_matches('%')
                .parse::<i32>()
                .is_ok_and(|p| p >= 60);
            let row = kit::hbox(7);
            let dot = gtk::Label::new(None);
            dot.set_markup(&format!(
                "<span foreground='{}'>●</span>",
                if fresh { "#FA320A" } else { "#2E9E4F" }
            ));
            row.append(&dot);
            row.append(&kit::text(rt, 13.0, "", 600, false));
            let c = kit::hbox(0);
            c.add_css_class("chip");
            c.append(&row);
            c.set_tooltip_text(Some(if fresh {
                "Rotten Tomatoes · Fresh"
            } else {
                "Rotten Tomatoes · Rotten"
            }));
            chips.append(&c);
        }
    }

    /// One row of chips (rating, IMDb, year, genres, length, language, country). A plain row:
    /// a FlowBox here measured its height wrongly and drew over the plot below it.
    fn chips(&self, it: &Item, info: Option<&DetailsInfo>) -> gtk::Box {
        let chips = kit::hbox(8);
        chips.set_halign(gtk::Align::Start);
        let rating = info.and_then(|i| i.rating.clone()).or_else(|| {
            it.has_rating()
                .then(|| it.rating.clone().unwrap_or_default())
        });
        if let Some(r) = rating {
            chips.append(&kit::chip(&format!("★ {r}"), "rating"));
        }
        self.add_external_ratings(&chips);
        for c in chip_texts(it, info) {
            chips.append(&kit::chip(&c, ""));
        }
        chips
    }

    fn hero(
        self: &Rc<Self>,
        it: &Item,
        info: Option<&DetailsInfo>,
        poster: Option<&str>,
    ) -> gtk::Widget {
        let g = kit::hbox(34);
        g.set_margin_top(6);
        let (img, _) = kit::picture_frame(
            poster,
            230,
            340,
            "madhyam-video-x-generic-symbolic",
            false,
            "poster-frame",
        );
        img.set_valign(gtk::Align::Start);
        g.append(&img);
        let right = kit::vbox(0);
        right.set_hexpand(true);
        right.set_margin_top(4);
        let title = kit::text(&it.name, 32.0, "", 600, true);
        right.append(&title);
        let chips = self.chips(it, info);
        kit::margins(&chips, 0, 14, 0, 4);
        right.append(&chips);
        if let Some(plot) = info
            .and_then(|i| i.plot.clone())
            .or_else(|| it.plot.clone())
            .filter(|p| !p.trim().is_empty())
        {
            let p = kit::text(&plot, 14.5, "", 400, true);
            p.set_margin_top(8);
            p.set_max_width_chars(110);
            right.append(&p);
        }
        add_credit(&right, "Actors", info.and_then(|i| i.cast.as_deref()));
        add_credit(&right, "Director", info.and_then(|i| i.director.as_deref()));
        if !it.is_series() && it.duration > 0.0 && it.position > 0.0 && !it.is_watched() {
            let row = kit::hbox(10);
            row.set_margin_top(18);
            let bar = kit::progress(it.watched_ratio());
            bar.set_size_request(220, -1);
            row.append(&bar);
            row.append(&kit::text(
                &format!(
                    "{} · {:.0}% watched",
                    it.time_left_text(),
                    it.watched_ratio() * 100.0
                ),
                12.5,
                "muted",
                400,
                false,
            ));
            right.append(&row);
        }
        let buttons = kit::hbox(10);
        buttons.set_margin_top(22);
        if it.is_series() {
            let first = self.episodes.borrow().first().cloned();
            if let Some(ep) = self.resume_episode() {
                let (me, e) = (self.clone(), ep.clone());
                buttons.append(&kit::primary(
                    &format!("Resume {}", ep.se()),
                    Some("madhyam-media-playback-start-symbolic"),
                    move || {
                        let (me, e) = (me.clone(), e.clone());
                        spawn(async move { me.play_episode(e).await });
                    },
                    false,
                ));
            } else if let Some(ep) = first {
                let (me, e) = (self.clone(), ep.clone());
                buttons.append(&kit::primary(
                    &format!("Play first episode {}", ep.se()),
                    Some("madhyam-media-playback-start-symbolic"),
                    move || {
                        let (me, e) = (me.clone(), e.clone());
                        spawn(async move { me.play_episode(e).await });
                    },
                    false,
                ));
            }
        } else {
            let started = it.position > 0.0 && !it.is_watched();
            let me = self.clone();
            buttons.append(&kit::primary(
                if started { "Resume" } else { "Play" },
                Some("madhyam-media-playback-start-symbolic"),
                move || {
                    let me = me.clone();
                    spawn(async move { me.play_movie(false).await });
                },
                false,
            ));
            if started {
                let me = self.clone();
                buttons.append(&kit::primary(
                    "Start over",
                    Some("madhyam-media-skip-backward-symbolic"),
                    move || {
                        let me = me.clone();
                        spawn(async move { me.play_movie(true).await });
                    },
                    true,
                ));
            }
        }
        let fav = self.is_favorite(&it.id);
        let (me, i2) = (self.clone(), it.clone());
        buttons.append(&kit::primary(
            if fav {
                "Remove from favorites"
            } else {
                "Add to favorites"
            },
            Some("madhyam-emote-love-symbolic"),
            move || {
                let (me, i) = (me.clone(), i2.clone());
                spawn(async move { me.toggle_favorite(i).await });
            },
            true,
        ));
        if !it.is_series() {
            let (me, i2) = (self.clone(), it.clone());
            buttons.append(&kit::primary(
                if it.is_watched() {
                    "Watched"
                } else {
                    "Mark as watched"
                },
                Some("madhyam-object-select-symbolic"),
                move || {
                    let (me, i) = (me.clone(), i2.clone());
                    spawn(async move {
                        let repo = me.repo.clone();
                        let watched = !i.is_watched();
                        bgv(move || repo.set_watched_many(&[i], watched)).await;
                        me.progress_stale.set(false);
                        me.reload_details_progress(0).await;
                    });
                },
                true,
            ));
        }
        if let Some(yt) = info.and_then(|i| i.trailer.clone()) {
            let me = self.clone();
            buttons.append(&kit::primary(
                "Trailer",
                Some("madhyam-media-playback-start-symbolic"),
                move || me.play_trailer(&yt),
                true,
            ));
        }
        right.append(&buttons);
        g.append(&right);
        g.upcast()
    }

    /// Compact details shown under the player.
    fn about(
        self: &Rc<Self>,
        it: &Item,
        info: Option<&DetailsInfo>,
        poster: Option<&str>,
    ) -> gtk::Widget {
        let g = kit::hbox(24);
        let (img, _) = kit::picture_frame(
            poster,
            140,
            208,
            "madhyam-video-x-generic-symbolic",
            false,
            "poster-frame",
        );
        img.set_valign(gtk::Align::Start);
        g.append(&img);
        let right = kit::vbox(0);
        right.set_hexpand(true);
        right.append(&kit::text(&it.name, 22.0, "", 600, true));
        let chips = self.chips(it, info);
        chips.set_margin_top(10);
        right.append(&chips);
        if let Some(plot) = info
            .and_then(|i| i.plot.clone())
            .or_else(|| it.plot.clone())
            .filter(|p| !p.trim().is_empty())
        {
            let p = kit::text(&plot, 13.5, "", 400, true);
            p.set_margin_top(4);
            p.set_max_width_chars(110);
            right.append(&p);
        }
        add_credit(&right, "Actors", info.and_then(|i| i.cast.as_deref()));
        add_credit(&right, "Director", info.and_then(|i| i.director.as_deref()));
        let fav = self.is_favorite(&it.id);
        let (me, i2) = (self.clone(), it.clone());
        let b = kit::primary(
            if fav {
                "Remove from favorites"
            } else {
                "Add to favorites"
            },
            Some("madhyam-emote-love-symbolic"),
            move || {
                let (me, i) = (me.clone(), i2.clone());
                spawn(async move { me.toggle_favorite(i).await });
            },
            true,
        );
        b.set_halign(gtk::Align::Start);
        b.set_margin_top(16);
        right.append(&b);
        g.append(&right);
        g.upcast()
    }

    // ---------------------------------------------------------------- seasons and episodes

    fn seasons(self: &Rc<Self>, series: &Item) -> gtk::Widget {
        let s = kit::vbox(0);
        let all = self.episodes.borrow().clone();
        if all.is_empty() {
            if !self.details_loading.get() {
                s.append(&kit::section_header("Seasons and episodes", None, None, ""));
                s.append(&kit::text(
                    "This series has no episodes on the server.",
                    13.0,
                    "muted",
                    400,
                    false,
                ));
            }
            return s.upcast();
        }
        let mut seasons: Vec<String> = Vec::new();
        for e in &all {
            let k = e.season.clone().unwrap_or_else(|| "1".into());
            if !seasons.contains(&k) {
                seasons.push(k);
            }
        }
        let current = self
            .season
            .borrow()
            .clone()
            .filter(|x| seasons.contains(x))
            .unwrap_or_else(|| seasons[0].clone());
        let eps: Vec<Item> = all
            .into_iter()
            .filter(|e| e.season.clone().unwrap_or_else(|| "1".into()) == current)
            .collect();
        s.append(&kit::section_header(
            "Seasons and episodes",
            Some(self.episodes.borrow().len() as i64),
            None,
            "",
        ));
        let bar = kit::hbox(10);
        bar.set_margin_bottom(14);
        let labels: Vec<(String, String)> = seasons
            .iter()
            .map(|x| (x.clone(), format!("Season {x}")))
            .collect();
        let opts: Vec<(&str, &str)> = labels
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let me = Rc::downgrade(self);
        let seg = kit::segments(&opts, &current, move |v| {
            if let Some(a) = me.upgrade() {
                *a.season.borrow_mut() = Some(v);
                a.render_vod_page();
            }
        });
        seg.set_hexpand(true);
        bar.append(&seg);
        let all_watched = eps.iter().all(|e| e.is_watched());
        let (me, eps2) = (self.clone(), eps.clone());
        bar.append(&kit::primary(
            if all_watched {
                "Mark season as unwatched"
            } else {
                "Mark season as watched"
            },
            Some("madhyam-object-select-symbolic"),
            move || {
                let (me, list) = (me.clone(), eps2.clone());
                spawn(async move {
                    let repo = me.repo.clone();
                    bgv(move || repo.set_watched_many(&list, !all_watched)).await;
                    me.progress_stale.set(false);
                    me.reload_details_progress(0).await;
                });
            },
            true,
        ));
        s.append(&bar);
        s.append(&kit::wrap(
            eps.iter().map(|e| self.episode_card(series, e)).collect(),
        ));
        s.upcast()
    }

    fn episode_card(self: &Rc<Self>, series: &Item, ep: &Item) -> gtk::Widget {
        const W: i32 = 260;
        const H: i32 = 146;
        let playing = self.playing().is_some_and(|p| p.id == ep.id);
        let (img, frame) = kit::picture_frame(
            ep.logo.as_deref().or(series.logo.as_deref()),
            W,
            H,
            "madhyam-video-x-generic-symbolic",
            false,
            "episode-frame",
        );
        if playing {
            frame.add_css_class("playing");
        }
        let num = kit::badge(&format!("E{}", ep.episode.as_deref().unwrap_or("")), "");
        num.set_halign(gtk::Align::Start);
        num.set_valign(gtk::Align::Start);
        kit::margins(&num, 7, 7, 7, 7);
        img.add_overlay(&num);
        if !ep.duration_text().is_empty() {
            let d = kit::badge(&ep.duration_text(), "");
            d.set_halign(gtk::Align::End);
            d.set_valign(gtk::Align::End);
            kit::margins(&d, 7, 7, 7, 12);
            img.add_overlay(&d);
        }
        if !ep.is_watched() && ep.watched_ratio() > 0.0 {
            let p = kit::progress(ep.watched_ratio());
            p.set_valign(gtk::Align::End);
            kit::margins(&p, 8, 0, 8, 6);
            img.add_overlay(&p);
        }
        // Watched toggle: filled when watched, an outline that shows on hover otherwise.
        let watch = gtk::Button::new();
        let tick = kit::icon("madhyam-object-select-symbolic", 12);
        watch.set_child(Some(&tick));
        watch.add_css_class("bare");
        watch.add_css_class("watched-dot");
        if ep.is_watched() {
            watch.add_css_class("on");
        } else {
            watch.set_opacity(0.0);
        }
        watch.set_halign(gtk::Align::End);
        watch.set_valign(gtk::Align::Start);
        kit::margins(&watch, 7, 7, 7, 7);
        watch.set_tooltip_text(Some(if ep.is_watched() {
            "Watched — click to mark as unwatched"
        } else {
            "Mark as watched"
        }));
        let (me, e2) = (self.clone(), ep.clone());
        watch.connect_clicked(move |_| {
            let (me, e) = (me.clone(), e2.clone());
            spawn(async move {
                let repo = me.repo.clone();
                let w = !e.is_watched();
                bgv(move || repo.set_watched_many(&[e], w)).await;
                me.reload_details_progress(0).await;
            });
        });
        img.add_overlay(&watch);
        let hint = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hint.add_css_class("play-hint");
        hint.set_halign(gtk::Align::Center);
        hint.set_valign(gtk::Align::Center);
        let pi = kit::icon("madhyam-media-playback-start-symbolic", 16);
        pi.set_halign(gtk::Align::Center);
        hint.set_homogeneous(true);
        hint.append(&pi);
        hint.set_opacity(0.0);
        hint.set_can_target(false);
        if playing {
            let p = kit::badge("▶ PLAYING", "");
            p.set_halign(gtk::Align::Center);
            p.set_valign(gtk::Align::Center);
            img.add_overlay(&p);
        } else {
            img.add_overlay(&hint);
        }
        let stack = kit::vbox(0);
        stack.set_size_request(W, -1);
        stack.append(&img);
        let title = kit::text(
            &format!("{}. {}", ep.episode.as_deref().unwrap_or(""), ep.name),
            13.5,
            if playing { "accent" } else { "" },
            600,
            false,
        );
        title.set_max_width_chars(1);
        kit::margins(&title, 2, 8, 2, 0);
        stack.append(&title);
        if let Some(plot) = ep.plot.as_ref().filter(|p| !p.trim().is_empty()) {
            let p = kit::lines(plot, 12.0, "muted", 2);
            p.set_max_width_chars(1);
            kit::margins(&p, 2, 3, 2, 0);
            stack.append(&p);
        }
        let (me, e2) = (self.clone(), ep.clone());
        let card = kit::bare(&stack, move || {
            let (me, e) = (me.clone(), e2.clone());
            spawn(async move { me.play_episode(e).await });
        });
        card.set_tooltip_text(Some(&ep.name));
        kit::margins(&card, 0, 0, 18, 20);
        let motion = gtk::EventControllerMotion::new();
        let (w1, h1, watched) = (watch.clone(), hint.clone(), ep.is_watched());
        motion.connect_enter(move |_, _, _| {
            w1.set_opacity(1.0);
            h1.set_opacity(1.0);
        });
        let (w2, h2) = (watch.clone(), hint.clone());
        motion.connect_leave(move |_| {
            if !watched {
                w2.set_opacity(0.0);
            }
            h2.set_opacity(0.0);
        });
        card.add_controller(motion);
        let click = gtk::GestureClick::new();
        click.set_button(3);
        let (me, e2, cw) = (self.clone(), ep.clone(), card.downgrade());
        click.connect_pressed(move |_, _, _, _| {
            let Some(card) = cw.upgrade() else { return };
            let (m1, e1) = (me.clone(), e2.clone());
            let (m2, ee) = (me.clone(), e2.clone());
            kit::menu(
                &card,
                &e2.se(),
                vec![
                    MenuRow::new(
                        "madhyam-media-playback-start-symbolic",
                        "Play",
                        false,
                        move || {
                            let (m, e) = (m1.clone(), e1.clone());
                            spawn(async move { m.play_episode(e).await });
                        },
                    ),
                    MenuRow::new(
                        "madhyam-object-select-symbolic",
                        if e2.is_watched() {
                            "Mark as unwatched"
                        } else {
                            "Mark as watched"
                        },
                        false,
                        move || {
                            let (m, e) = (m2.clone(), ee.clone());
                            spawn(async move {
                                let repo = m.repo.clone();
                                let w = !e.is_watched();
                                bgv(move || repo.set_watched_many(&[e], w)).await;
                                m.reload_details_progress(300).await;
                            });
                        },
                    ),
                ],
            );
        });
        card.add_controller(click);
        card.upcast()
    }
}

fn chip_texts(it: &Item, info: Option<&DetailsInfo>) -> Vec<String> {
    let mut out = Vec::new();
    let year = info
        .and_then(|i| i.released.clone())
        .filter(|r| r.len() >= 4)
        .map(|r| r.chars().take(4).collect())
        .or_else(|| it.year.clone());
    if let Some(y) = year.filter(|y: &String| !y.is_empty()) {
        out.push(y);
    }
    if let Some(genre) = info
        .and_then(|i| i.genre.clone())
        .or_else(|| it.genre.clone())
    {
        out.extend(
            genre
                .split([',', '/', '|'])
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .take(3)
                .map(str::to_string),
        );
    }
    if let Some(d) = info
        .and_then(|i| i.duration.clone())
        .filter(|d| !d.is_empty())
    {
        out.push(d);
    }
    if let Some(l) = info
        .and_then(|i| i.language.clone())
        .filter(|l| !l.is_empty())
    {
        out.push(if l.len() <= 3 { l.to_uppercase() } else { l });
    }
    if let Some(c) = info
        .and_then(|i| i.country.clone())
        .filter(|c| !c.is_empty())
    {
        out.push(c);
    }
    out
}

fn add_credit(parent: &gtk::Box, label: &str, value: Option<&str>) {
    let Some(v) = value.filter(|v| !v.trim().is_empty()) else {
        return;
    };
    let l = gtk::Label::new(None);
    l.set_markup(&format!(
        "<b>{}:</b>  <span alpha='75%'>{}</span>",
        gtk::glib::markup_escape_text(label),
        gtk::glib::markup_escape_text(v)
    ));
    l.set_wrap(true);
    l.set_xalign(0.0);
    l.set_margin_top(10);
    l.set_max_width_chars(110);
    parent.append(&l);
}

fn now_playing_episode(ep: &Item) -> gtk::Widget {
    let s = kit::vbox(0);
    s.set_margin_top(4);
    s.append(&kit::text(
        &format!("{} · {}", ep.se(), ep.name),
        19.0,
        "",
        600,
        true,
    ));
    if let Some(p) = ep.plot.as_ref().filter(|p| !p.trim().is_empty()) {
        let l = kit::text(p, 13.5, "muted", 400, true);
        l.set_margin_top(6);
        l.set_max_width_chars(110);
        s.append(&l);
    }
    s.upcast()
}

/// Trailers open on YouTube in the default browser (the Windows app embeds WebView2).
/// mpv plays YouTube through yt-dlp (or youtube-dl) when one is installed.
fn yt_dlp_available() -> bool {
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p)
            .any(|d| d.join("yt-dlp").is_file() || d.join("youtube-dl").is_file())
    })
}

impl App {
    /// Play a trailer in the page's own player card, like the Windows app. Without yt-dlp
    /// mpv cannot open YouTube, so the trailer opens in the browser instead.
    pub fn play_trailer(self: &Rc<Self>, youtube_id: &str) {
        let url = format!("https://www.youtube.com/watch?v={youtube_id}");
        if !yt_dlp_available() {
            self.set_status(
                "Install yt-dlp to watch trailers inside Madhyam; opening it in your browser.",
            );
            crate::ui::dialogs::open_uri(&self.window, &url);
            return;
        }
        let name = self
            .details
            .borrow()
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "Trailer".into());
        // No id: a trailer is not in the catalog, so it gets no history or resume position.
        let trailer = Item {
            kind: "movie".into(),
            name,
            url,
            ..Default::default()
        };
        *self.playlist.borrow_mut() = vec![trailer];
        let me = self.clone();
        spawn(async move {
            me.play_at(0).await;
            me.vod_card_label.set_text("TRAILER");
            me.vod_card_sub.set_text("YouTube");
        });
    }
}

fn trailer_section(it: &Item, youtube_id: &str, app: &Rc<App>) -> gtk::Widget {
    let s = kit::vbox(0);
    s.append(&kit::section_header("Trailer", None, None, ""));
    let (thumb, frame) = kit::picture_frame(
        Some(&format!(
            "https://img.youtube.com/vi/{youtube_id}/hqdefault.jpg"
        )),
        360,
        202,
        "madhyam-media-playback-start-symbolic",
        false,
        "episode-frame",
    );
    let _ = frame;
    let play = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    play.set_size_request(64, 44);
    play.set_halign(gtk::Align::Center);
    play.set_valign(gtk::Align::Center);
    let lbl = gtk::Label::new(None);
    lbl.set_markup("<span foreground='white' size='large'>▶</span>");
    lbl.set_halign(gtk::Align::Center);
    play.set_homogeneous(true);
    play.append(&lbl);
    play.add_css_class("yt-play");
    thumb.add_overlay(&play);
    let label = kit::badge(&format!("{} · YouTube", it.name), "");
    label.set_halign(gtk::Align::Start);
    label.set_valign(gtk::Align::End);
    kit::margins(&label, 10, 10, 10, 10);
    thumb.add_overlay(&label);
    let (me, id) = (app.clone(), youtube_id.to_string());
    let b = kit::bare(&thumb, move || me.play_trailer(&id));
    b.set_halign(gtk::Align::Start);
    b.set_tooltip_text(Some("Play the trailer"));
    s.append(&b);
    s.upcast()
}
