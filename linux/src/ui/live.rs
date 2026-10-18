//! Categories (all sections), the Live TV list, the player core and the programme guide.
//! Twin of MainWindow.Live.cs.

use super::app::{App, CategoryRow, LiveRow, Mode, live_title, paint_star};
use super::kit::{self, MenuRow};
use super::{bg, bgv, spawn};
use crate::freetv;
use crate::repo::{self, Item, Programme};
use crate::settings;
use chrono::{Duration as CDuration, Local, NaiveTime, TimeZone};
use gtk::glib;
use gtk::prelude::*;
use std::rc::Rc;

pub const PX_PER_MINUTE: f64 = 3.2;
const LIVE_PAGE: usize = 400;

impl App {
    pub fn kind_now(&self) -> &'static str {
        self.mode.get().kind().unwrap_or("live")
    }

    /// Is the current playlist the Free TV one (iptv-org)?
    pub fn is_free_tv_source(&self) -> bool {
        let id = settings::get().free_tv_source_id;
        self.source
            .borrow()
            .as_ref()
            .is_some_and(|s| Some(s.id) == id || freetv::is_free_tv(&s.location))
    }

    /// "country" for Free TV listed by country, else None (categories).
    pub fn group_field_now(&self) -> Option<&'static str> {
        (self.mode.get() == Mode::Live
            && self.is_free_tv_source()
            && settings::get().free_tv_group_by == "country")
            .then_some(freetv::COUNTRY_FIELD)
    }

    /// Key part for hidden groups: the kind, or "live:country" for Free TV's countries.
    pub fn hidden_kind(&self) -> String {
        match self.group_field_now() {
            Some(f) => format!("{}:{f}", self.kind_now()),
            None => self.kind_now().to_string(),
        }
    }

    pub fn hidden_now(&self) -> Vec<String> {
        let kind = self.hidden_kind(); // reads the settings too: not while holding the lock
        match self.source.borrow().as_ref() {
            Some(s) => settings::get().hidden(s.id, &kind),
            None => Vec::new(),
        }
    }

    pub fn playing(&self) -> Option<Item> {
        let i = self.play_index.get();
        if i < 0 {
            return None;
        }
        self.playlist.borrow().get(i as usize).cloned()
    }

    // ================================================================ categories

    /// The Categories | Countries switch at the top of the panel (Free TV in Live TV only).
    fn update_cat_mode(self: &Rc<Self>) {
        let show = self.mode.get() == Mode::Live && self.is_free_tv_source();
        self.cat_mode_box.set_visible(show);
        self.cat_manage_btn
            .set_tooltip_text(Some(if self.group_field_now().is_none() {
                "Manage categories (show / hide)"
            } else {
                "Manage countries (show / hide)"
            }));
        kit::clear(&self.cat_mode_box);
        if !show {
            return;
        }
        let cur = if settings::get().free_tv_group_by == "country" {
            "country"
        } else {
            "category"
        };
        let me = Rc::downgrade(self);
        let seg = kit::segments(
            &[("category", "Categories"), ("country", "Countries")],
            cur,
            move |v| {
                if settings::get().free_tv_group_by == v {
                    return;
                }
                settings::update(|s| s.free_tv_group_by = v.clone());
                if let Some(a) = me.upgrade() {
                    *a.group.borrow_mut() = None;
                    a.cat_filter.set_text("");
                    let v = a.next_version();
                    spawn(async move { a.load_categories(v, None).await });
                }
            },
        );
        self.cat_mode_box.append(&seg);
    }

    pub async fn load_categories(self: &Rc<Self>, version: u64, select: Option<String>) {
        let field = self.group_field_now();
        self.cats_title.set_text(if field.is_some() {
            "Countries"
        } else {
            match self.mode.get() {
                Mode::Live => "Live categories",
                Mode::Movies => "Movie categories",
                _ => "Series categories",
            }
        });
        self.cat_filter
            .set_placeholder_text(Some(if field.is_some() {
                "Search countries"
            } else {
                "Search categories"
            }));
        self.update_cat_mode();
        self.live_sort_btn
            .set_tooltip_text(Some(if settings::get().live_sort_az {
                "Sorted A–Z (click for provider order)"
            } else {
                "Sort A–Z"
            }));
        let kind = self.kind_now();
        let source = self.source.borrow().clone();
        let mut groups = match &source {
            Some(s) => {
                let (repo, id) = (self.repo.clone(), s.id);
                bg(move || repo.groups(id, kind, field))
                    .await
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };
        if version != self.load_version.get() {
            return;
        }
        if let (Some(_), true, Some(src), false) = (
            field,
            groups.is_empty(),
            &source,
            self.country_tagging.get(),
        ) {
            // Older Free TV download without country tags: fetch them now.
            self.country_tagging.set(true);
            self.cats_title.set_text("Countries (loading…)");
            let (repo, id) = (self.repo.clone(), src.id);
            groups = bgv(move || {
                repo.tag_countries(id);
                repo.groups(id, kind, field).unwrap_or_default()
            })
            .await;
            self.country_tagging.set(false);
            if version != self.load_version.get() {
                return;
            }
            self.cats_title.set_text("Countries");
        }
        let hidden = self.hidden_now();
        let mut visible: Vec<_> = groups
            .into_iter()
            .filter(|g| !hidden.contains(&g.name))
            .collect();
        let mut order = settings::get()
            .category_sort
            .get(kind)
            .cloned()
            .unwrap_or_else(|| "provider".into());
        if field.is_some() && order == "provider" {
            order = "name_asc".into(); // countries have no provider order worth keeping
        }
        match order.as_str() {
            "name_asc" => visible.sort_by_key(|g| g.name.to_lowercase()),
            "name_desc" => {
                visible.sort_by_key(|g| g.name.to_lowercase());
                visible.reverse();
            }
            _ => {}
        }
        if field.is_some() {
            visible.sort_by_key(|g| g.name == "Undefined"); // channels with no country last
        }
        // By country a channel counts once per country, so "All items" takes the real count.
        let all_count = match (&source, field) {
            (Some(s), Some(f)) => {
                let (repo, id, h) = (self.repo.clone(), s.id, hidden.clone());
                bg(move || repo.query(kind, Some(id), None, &h, "provider", 0.0, 0, 1, Some(f)))
                    .await
                    .map(|r| r.0)
                    .unwrap_or(0)
            }
            _ => visible.iter().map(|g| g.count).sum(),
        };
        if version != self.load_version.get() {
            return;
        }
        let mut rows = vec![CategoryRow {
            name: "All items".into(),
            count: all_count,
            all: true,
        }];
        rows.extend(visible.into_iter().map(|g| CategoryRow {
            name: g.name,
            count: g.count,
            all: false,
        }));
        *self.all_categories.borrow_mut() = rows;
        self.apply_category_filter(select, true);
    }

    fn category_row(r: &CategoryRow) -> gtk::ListBoxRow {
        let g = kit::hbox(10);
        kit::margins(&g, 12, 9, 10, 9);
        let name = kit::text(&r.name, 13.5, "", 400, false);
        name.set_hexpand(true);
        name.set_max_width_chars(1);
        g.append(&name);
        g.append(&kit::count_pill(r.count));
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&g));
        row
    }

    /// Fill the category list (with the filter box applied) and select a row.
    pub fn apply_category_filter(self: &Rc<Self>, select: Option<String>, load: bool) {
        let f = self.cat_filter.text().trim().to_lowercase();
        let rows: Vec<CategoryRow> = self
            .all_categories
            .borrow()
            .iter()
            .filter(|r| r.all || f.is_empty() || r.name.to_lowercase().contains(&f))
            .cloned()
            .collect();
        self.group_silent.set(true);
        self.group_list.remove_all();
        for r in &rows {
            self.group_list.append(&Self::category_row(r));
        }
        *self.shown_categories.borrow_mut() = rows.clone();
        let target = select.clone().or_else(|| self.group.borrow().clone());
        let mut pick = rows.iter().position(|r| match &target {
            None => r.all,
            Some(t) => !r.all && &r.name == t,
        });
        if pick.is_none() && !rows.is_empty() {
            pick = Some(0);
        }
        // Live opens on the first real category (a smaller list); grids open on All items.
        if load
            && select.is_none()
            && self.group.borrow().is_none()
            && self.mode.get() == Mode::Live
            && rows.len() > 1
        {
            pick = Some(1);
        }
        if let Some(i) = pick {
            if let Some(row) = self.group_list.row_at_index(i as i32) {
                self.group_list.select_row(Some(&row));
            }
        }
        self.group_silent.set(false);
        if load {
            match pick {
                Some(i) => {
                    let r = rows[i].clone();
                    let me = self.clone();
                    spawn(
                        async move { me.show_group(if r.all { None } else { Some(r.name) }).await },
                    );
                }
                None => {
                    let me = self.clone();
                    spawn(async move { me.show_group(None).await });
                }
            }
        }
    }

    pub fn cat_search_toggle(&self) {
        let show = !self.cat_filter_box.is_visible();
        self.cat_filter_box.set_visible(show);
        if show {
            self.cat_filter.grab_focus();
        } else {
            self.cat_filter.set_text("");
        }
    }

    pub fn cat_sort_menu(self: &Rc<Self>) {
        let kind = self.kind_now();
        let cur = settings::get()
            .category_sort
            .get(kind)
            .cloned()
            .unwrap_or_else(|| "provider".into());
        let set = |order: &'static str| {
            let me = self.clone();
            move || {
                settings::update(|s| {
                    s.category_sort.insert(kind.to_string(), order.to_string());
                });
                let (me, v, g) = (me.clone(), me.next_version(), me.group.borrow().clone());
                spawn(async move { me.load_categories(v, g).await });
            }
        };
        kit::menu(
            &self.cat_sort_btn,
            "Sort categories",
            vec![
                MenuRow::new(
                    "view-list-symbolic",
                    "Provider order",
                    cur == "provider",
                    set("provider"),
                ),
                MenuRow::new(
                    "view-sort-ascending-symbolic",
                    "Name A–Z",
                    cur == "name_asc",
                    set("name_asc"),
                ),
                MenuRow::new(
                    "view-sort-descending-symbolic",
                    "Name Z–A",
                    cur == "name_desc",
                    set("name_desc"),
                ),
            ],
        );
    }

    pub async fn cat_manage(self: &Rc<Self>) {
        let Some(src) = self.source.borrow().clone() else {
            return;
        };
        let field = self.group_field_now();
        let (repo, id, kind) = (self.repo.clone(), src.id, self.kind_now());
        let mut all = bg(move || repo.groups(id, kind, field))
            .await
            .unwrap_or_default();
        if field.is_some() {
            all.sort_by_key(|g| g.name.to_lowercase());
        }
        let hidden = self.hidden_now();
        let key = settings::AppSettings::hidden_key(src.id, &self.hidden_kind());
        let me = self.clone();
        super::dialogs::manage_categories(
            &self.window,
            all,
            hidden,
            if field.is_some() {
                "countries"
            } else {
                "categories"
            },
            move |result| {
                settings::update(|s| {
                    s.hidden_categories.insert(key.clone(), result.clone());
                });
                if me
                    .group
                    .borrow()
                    .as_ref()
                    .is_some_and(|g| result.contains(g))
                {
                    *me.group.borrow_mut() = None;
                }
                let (me, v, g) = (me.clone(), me.next_version(), me.group.borrow().clone());
                spawn(async move { me.load_categories(v, g).await });
            },
        );
    }

    pub fn group_selected(self: &Rc<Self>, index: i32) {
        if self.group_silent.get() || index < 0 {
            return;
        }
        let Some(row) = self.shown_categories.borrow().get(index as usize).cloned() else {
            return;
        };
        if !self.search.text().is_empty() || self.vod_search.borrow().is_some() {
            self.search.set_text("");
            *self.vod_search.borrow_mut() = None;
            *self.search_hits.borrow_mut() = None;
            *self.live_search.borrow_mut() = None;
        }
        self.close_details_silently();
        let me = self.clone();
        spawn(async move {
            me.show_group(if row.all { None } else { Some(row.name) })
                .await
        });
    }

    pub async fn show_group(self: &Rc<Self>, group: Option<String>) {
        *self.group.borrow_mut() = group;
        self.update_search_pill();
        match self.mode.get() {
            Mode::Live => self.load_live(true).await,
            Mode::Movies | Mode::Series => self.load_grid(true).await,
            _ => {}
        }
    }

    // ================================================================ live list

    pub async fn load_live(self: &Rc<Self>, reset: bool) {
        let Some(src) = self.source.borrow().clone() else {
            self.show_live_empty("Add a playlist with the + button to start.");
            return;
        };
        if self.live_loading.get() {
            return;
        }
        self.live_loading.set(true);
        let version = self.load_version.get();
        if reset {
            self.item_list.remove_all();
            self.live_rows.borrow_mut().clear();
            self.live_total.set(0);
            *self.live_search.borrow_mut() = None;
            self.list_title
                .set_text(self.group.borrow().as_deref().unwrap_or("All channels"));
        }
        let group = self.group.borrow().clone();
        let hidden = if group.is_none() {
            self.hidden_now()
        } else {
            Vec::new()
        };
        let sort = if settings::get().live_sort_az {
            "name_asc"
        } else {
            "provider"
        };
        let offset = self.live_rows.borrow().len();
        let field = self.group_field_now();
        let (repo, id) = (self.repo.clone(), src.id);
        let result = bg(move || {
            repo.query(
                "live",
                Some(id),
                group.as_deref(),
                &hidden,
                sort,
                0.0,
                offset,
                LIVE_PAGE,
                field,
            )
        })
        .await;
        self.live_loading.set(false);
        if version != self.load_version.get() {
            return;
        }
        match result {
            Ok((total, items)) => {
                self.live_total.set(total);
                self.append_live(items).await;
            }
            Err(e) => self.set_status(&e.to_string()),
        }
    }

    pub async fn load_more_live(self: Rc<Self>) {
        if self.live_search.borrow().is_none()
            && !self.live_loading.get()
            && (self.live_rows.borrow().len() as i64) < self.live_total.get()
        {
            self.load_live(false).await;
        }
    }

    fn live_row(self: &Rc<Self>, it: &Item, number: usize) -> LiveRow {
        let g = kit::hbox(10);
        kit::margins(&g, 8, 8, 4, 8);
        g.append(&kit::logo_box(it, 40));
        let t = kit::vbox(0);
        t.set_hexpand(true);
        t.set_valign(gtk::Align::Center);
        let title = kit::text(&live_title(it, number), 13.5, "", 600, false);
        title.set_max_width_chars(1);
        t.append(&title);
        let now = kit::text("", 12.0, "muted", 400, false);
        now.set_max_width_chars(1);
        t.append(&now);
        let times = kit::hbox(8);
        times.set_margin_top(3);
        let start = kit::text("", 11.0, "muted", 400, false);
        let bar = kit::progress(0.0);
        bar.set_hexpand(true);
        let end = kit::text("", 11.0, "muted", 400, false);
        times.append(&start);
        times.append(&bar);
        times.append(&end);
        times.set_visible(false);
        t.append(&times);
        g.append(&t);
        let star = gtk::Button::new();
        star.add_css_class("icon-button");
        star.add_css_class("star");
        star.add_css_class("small");
        star.set_valign(gtk::Align::Start);
        star.set_tooltip_text(Some("Favorite"));
        paint_star(&star, self.is_favorite(&it.id));
        let (me, it2) = (Rc::downgrade(self), it.clone());
        star.connect_clicked(move |_| {
            if let Some(a) = me.upgrade() {
                let it = it2.clone();
                spawn(async move { a.toggle_favorite(it).await });
            }
        });
        g.append(&star);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&g));
        // Right-click: play or (un)favorite.
        let click = gtk::GestureClick::new();
        click.set_button(3);
        let (me, it2, rw) = (Rc::downgrade(self), it.clone(), row.downgrade());
        click.connect_pressed(move |_, _, _, _| {
            let (Some(a), Some(row)) = (me.upgrade(), rw.upgrade()) else {
                return;
            };
            let (a1, i1) = (a.clone(), it2.clone());
            let (a2, i2) = (a.clone(), it2.clone());
            kit::menu(
                &row,
                &it2.display_name(),
                vec![
                    MenuRow::new("media-playback-start-symbolic", "Play", false, move || {
                        let (a, i) = (a1.clone(), i1.clone());
                        spawn(async move { a.play_live(i).await });
                    }),
                    MenuRow::new(
                        "starred-symbolic",
                        "Add / remove favorite",
                        false,
                        move || {
                            let (a, i) = (a2.clone(), i2.clone());
                            spawn(async move { a.toggle_favorite(i).await });
                        },
                    ),
                ],
            );
        });
        row.add_controller(click);
        LiveRow {
            item: it.clone(),
            row,
            title,
            now,
            times,
            start,
            end,
            bar,
            star,
        }
    }

    async fn append_live(self: &Rc<Self>, items: Vec<Item>) {
        let numbers = settings::get().show_numbers;
        let first = self.live_rows.borrow().len() + 1;
        let mut new_rows = Vec::new();
        for (number, it) in (first..).zip(&items) {
            let r = self.live_row(it, if numbers { number } else { 0 });
            self.item_list.append(&r.row);
            new_rows.push(r);
        }
        self.live_rows.borrow_mut().extend(new_rows);
        let total = self.live_total.get();
        self.list_count.set_text(&if total > 0 {
            repo::group_digits(total)
        } else {
            String::new()
        });
        self.list_empty
            .set_text(if self.live_search.borrow().is_some() {
                "No channels match."
            } else {
                "No channels in this category."
            });
        let empty = self.live_rows.borrow().is_empty();
        self.list_empty.set_visible(empty);
        if let Some(p) = self.playing().filter(|p| p.is_live()) {
            self.select_live_row(&p.id);
        } else if self.playing().is_none() && self.guide_item.borrow().is_none() {
            let first = self.live_rows.borrow().first().map(|r| r.item.clone());
            if let Some(first) = first {
                *self.guide_item.borrow_mut() = Some(first);
                self.guide_day_offset.set(0);
                self.load_guide().await;
            }
        }
        self.update_now_next(&items).await;
    }

    fn select_live_row(&self, id: &str) {
        let row = self
            .live_rows
            .borrow()
            .iter()
            .find(|r| r.item.id == id)
            .map(|r| r.row.clone());
        if let Some(row) = row {
            self.item_list.select_row(Some(&row));
            // Scroll it into view.
            let adj = self.item_scroll.vadjustment();
            if let Some(b) = row.compute_bounds(&self.item_list) {
                let (y, h) = (b.y() as f64, b.height() as f64);
                if y < adj.value() || y + h > adj.value() + adj.page_size() {
                    adj.set_value((y - adj.page_size() / 3.0).max(0.0));
                }
            }
        }
    }

    fn show_live_empty(&self, t: &str) {
        self.item_list.remove_all();
        self.live_rows.borrow_mut().clear();
        self.list_count.set_text("");
        self.list_empty.set_text(t);
        self.list_empty.set_visible(true);
    }

    pub async fn search_live(self: &Rc<Self>, q: String) {
        if q.is_empty() {
            self.load_live(true).await;
            return;
        }
        let version = self.next_version();
        let (repo, q2) = (self.repo.clone(), q.clone());
        let mut hits = bg(move || repo.search(&q2, Some("live")))
            .await
            .unwrap_or_default();
        if version != self.load_version.get() {
            return;
        }
        if let Some(src) = self.source.borrow().as_ref() {
            hits.retain(|h| h.source_id == src.id);
        }
        self.item_list.remove_all();
        self.live_rows.borrow_mut().clear();
        *self.live_search.borrow_mut() = Some(q.clone());
        self.live_total.set(hits.len() as i64);
        self.list_title.set_text(&format!("Results for “{q}”"));
        self.append_live(hits).await;
    }

    pub fn live_sort_menu(self: &Rc<Self>) {
        let az = settings::get().live_sort_az;
        let set = |v: bool| {
            let me = self.clone();
            move || {
                settings::update(|s| s.live_sort_az = v);
                me.live_sort_btn.set_tooltip_text(Some(if v {
                    "Sorted A–Z"
                } else {
                    "Provider order"
                }));
                let me = me.clone();
                spawn(async move { me.load_live(true).await });
            }
        };
        kit::menu(
            &self.live_sort_btn,
            "Sort channels",
            vec![
                MenuRow::new("view-list-symbolic", "Provider order", !az, set(false)),
                MenuRow::new("view-sort-ascending-symbolic", "Name A–Z", az, set(true)),
            ],
        );
    }

    /// What's on now for some channels (also used by pages for their live cards).
    pub async fn update_now_next(self: &Rc<Self>, items: &[Item]) {
        let ids: Vec<String> = items
            .iter()
            .filter(|i| i.is_live())
            .take(600)
            .map(|i| i.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        let repo = self.repo.clone();
        let nn = bgv(move || repo.now_next(&ids)).await;
        {
            let mut cache = self.now_cache.borrow_mut();
            for it in items.iter().filter(|i| i.is_live()) {
                match nn.get(&it.id).and_then(|(now, _)| now.clone()) {
                    Some(p) => {
                        cache.insert(it.id.clone(), p);
                    }
                    None => {
                        cache.remove(&it.id);
                    }
                }
            }
        }
        self.paint_now_rows();
    }

    pub async fn update_now_next_rows(self: &Rc<Self>) {
        let items: Vec<Item> = self
            .live_rows
            .borrow()
            .iter()
            .map(|r| r.item.clone())
            .collect();
        self.update_now_next(&items).await;
    }

    fn paint_now_rows(&self) {
        let now = repo::now_unix();
        let cache = self.now_cache.borrow();
        for r in self.live_rows.borrow().iter() {
            match cache.get(&r.item.id) {
                Some(p) => {
                    r.now.set_text(&p.title);
                    r.start.set_text(&repo::clock(p.start));
                    r.end.set_text(&repo::clock(p.stop));
                    r.bar.set_fraction(p.progress(now));
                    r.times.set_visible(true);
                }
                None => {
                    r.now.set_text("No programme information");
                    r.times.set_visible(false);
                }
            }
        }
    }

    pub fn now_of(&self, id: &str) -> Option<Programme> {
        self.now_cache.borrow().get(id).cloned()
    }

    pub fn live_row_activated(self: &Rc<Self>, index: i32) {
        let it = self
            .live_rows
            .borrow()
            .get(index as usize)
            .map(|r| r.item.clone());
        if let Some(it) = it {
            let me = self.clone();
            spawn(async move { me.play_live(it).await });
        }
    }

    pub fn live_row_selected(self: &Rc<Self>, index: i32) {
        // The guide follows the selected channel.
        let it = self
            .live_rows
            .borrow()
            .get(index as usize)
            .map(|r| r.item.clone());
        if let Some(it) = it {
            if self.guide_item.borrow().as_ref().map(|g| &g.id) != Some(&it.id) {
                *self.guide_item.borrow_mut() = Some(it);
                self.guide_day_offset.set(0);
                let me = self.clone();
                spawn(async move { me.load_guide().await });
            }
        }
    }

    pub async fn play_live(self: &Rc<Self>, it: Item) {
        let list: Vec<Item> = self
            .live_rows
            .borrow()
            .iter()
            .map(|r| r.item.clone())
            .collect();
        let i = list.iter().position(|x| x.id == it.id);
        match i {
            Some(i) => {
                *self.playlist.borrow_mut() = list;
                self.play_at(i as i64).await;
            }
            None => {
                *self.playlist.borrow_mut() = vec![it];
                self.play_at(0).await;
            }
        }
    }

    /// Open a live channel from anywhere: go to Live TV, its category, and play it.
    pub async fn go_to_live(self: &Rc<Self>, it: Item) {
        self.stop_playback();
        self.search.set_text("");
        *self.vod_search.borrow_mut() = None;
        *self.search_hits.borrow_mut() = None;
        self.set_nav(Mode::Live);
        *self.details.borrow_mut() = None;
        let other = self
            .source
            .borrow()
            .as_ref()
            .is_none_or(|s| s.id != it.source_id);
        if other {
            let src = self
                .sources
                .borrow()
                .iter()
                .find(|s| s.id == it.source_id)
                .cloned();
            if let Some(src) = src {
                *self.source.borrow_mut() = Some(src);
                self.update_playlist_header().await;
            }
        }
        self.layout();
        let hidden = self
            .source
            .borrow()
            .as_ref()
            .map(|s| settings::get().hidden(s.id, "live"))
            .unwrap_or_default();
        let version = self.next_version();
        let select = if hidden.contains(&it.group) || it.group.is_empty() {
            None
        } else {
            Some(it.group.clone())
        };
        *self.group.borrow_mut() = select.clone();
        self.load_categories(version, select).await;
        for _ in 0..50 {
            if !self.live_loading.get() && !self.live_rows.borrow().is_empty() {
                break;
            }
            super::sleep_ms(60).await;
        }
        let target = self
            .live_rows
            .borrow()
            .iter()
            .find(|r| r.item.id == it.id)
            .map(|r| r.item.clone())
            .unwrap_or(it);
        self.select_live_row(&target.id);
        self.play_live(target).await;
    }

    // ================================================================ player core

    /// The single video surface lives either in the live card or in the movie page.
    fn move_video(&self, live: bool) {
        let target = if live {
            &self.live_frame
        } else {
            &self.vod_frame
        };
        if let Some(parent) = self.video.parent() {
            if &parent == target.upcast_ref::<gtk::Widget>() {
                return;
            }
            if let Ok(o) = parent.downcast::<gtk::Overlay>() {
                o.remove_overlay(&self.video);
            }
        }
        target.add_overlay(&self.video);
    }

    pub async fn play_at(self: &Rc<Self>, index: i64) {
        self.save_progress();
        let n = self.playlist.borrow().len() as i64;
        if n == 0 {
            return;
        }
        let i = ((index % n) + n) % n;
        self.play_index.set(i);
        let it = self.playlist.borrow()[i as usize].clone();
        let Some(mpv) = self.mpv().cloned() else {
            self.set_status("The video engine (libmpv) could not be started. Install mpv / libmpv2 and start Madhyam again.");
            return;
        };
        let s = settings::snapshot();
        let mut url = it.url.clone();
        if it.is_live() && s.live_format != "Auto" {
            // Xtream live URLs end in .ts or .m3u8; the server offers both.
            let (from, to) = if s.live_format == "HLS" {
                (".ts", ".m3u8")
            } else {
                (".m3u8", ".ts")
            };
            if url.to_lowercase().ends_with(from) && url.contains("/live/") {
                url = format!("{}{to}", &url[..url.len() - from.len()]);
            }
        }
        if !(url.contains("://") || std::path::Path::new(&url).is_file()) {
            self.set_status(&format!(
                "“{}” has no valid stream address.",
                it.display_name()
            ));
            return;
        }
        mpv.stop();
        self.vod_playing.set(!it.is_live());
        self.move_video(it.is_live());
        self.video.set_visible(true);
        self.video_hint.set_visible(false);

        let ua = it
            .user_agent
            .clone()
            .or_else(|| {
                s.source_user_agent
                    .get(&it.source_id)
                    .filter(|u| !u.is_empty())
                    .cloned()
            })
            .unwrap_or_else(crate::http::default_user_agent);
        mpv.set("user-agent", &ua);
        mpv.set("referrer", it.referrer.as_deref().unwrap_or(""));
        let secs = (s.network_caching_ms.max(300) as f64) / 1000.0;
        mpv.set("cache", "yes");
        mpv.set("cache-secs", &format!("{secs:.1}"));
        mpv.set("demuxer-readahead-secs", &format!("{secs:.1}"));
        mpv.set(
            "hwdec",
            if s.hardware_decoding {
                "auto-safe"
            } else {
                "no"
            },
        );
        mpv.set("alang", &crate::ui::player::mpv_lang(&s.audio_language));
        mpv.set("slang", &crate::ui::player::mpv_lang(&s.subtitle_language));
        mpv.set("sid", if s.show_subtitles { "auto" } else { "no" });
        mpv.set("start", "none");
        if !it.is_live() && s.resume_playback {
            let (repo, id) = (self.repo.clone(), it.id.clone());
            let resume = bgv(move || repo.resume_seconds(&id)).await;
            if resume > 0.0 {
                mpv.set("start", &format!("{resume:.0}"));
            }
        }
        self.layout();
        mpv.load(&url);
        mpv.set("speed", "1");
        self.on_new_media(&it);
        {
            let (repo, id) = (self.repo.clone(), it.id.clone());
            spawn(async move { bgv(move || repo.record_play(&id)).await });
        }
        self.set_status("");

        if it.is_live() {
            self.select_live_row(&it.id);
            if self.guide_item.borrow().as_ref().map(|g| &g.id) != Some(&it.id) {
                *self.guide_item.borrow_mut() = Some(it.clone());
                self.guide_day_offset.set(0);
                self.load_guide().await;
            }
        } else {
            let next = it.kind == "episode" && (i as usize) < self.playlist.borrow().len() - 1;
            self.controls.next_ep.set_visible(next);
            self.render_vod_page();
            self.vod_scroll.vadjustment().set_value(0.0);
        }
    }

    pub fn stop_playback(self: &Rc<Self>) {
        self.save_progress();
        if let Some(m) = self.mpv() {
            m.stop();
        }
        self.play_index.set(-1);
        let was_vod = self.vod_playing.get();
        self.vod_playing.set(false);
        self.video.set_visible(false);
        kit::close_popover();
        self.video_hint.set_visible(true);
        self.move_video(true);
        if self.fullscreen.get() {
            self.toggle_fullscreen(false);
        }
        self.layout();
        if was_vod && self.details.borrow().is_some() {
            self.render_vod_page();
        }
    }

    pub fn save_progress(&self) {
        let Some(it) = self.playing().filter(|p| !p.is_live()) else {
            return;
        };
        let Some(m) = self.mpv() else { return };
        let len = m.length_ms();
        if len > 0 {
            let (pos, repo) = (m.time_ms(), self.repo.clone());
            std::thread::spawn(move || {
                repo.set_progress(&it.id, pos as f64 / 1000.0, len as f64 / 1000.0)
            });
        }
    }

    /// Films: mark finished. Episodes: continue with the next one.
    pub async fn on_end_reached(self: &Rc<Self>) {
        let Some(it) = self.playing().filter(|p| !p.is_live()) else {
            return;
        };
        let len = self.mpv().map(|m| m.length_ms()).unwrap_or(0).max(1000) as f64 / 1000.0;
        let (repo, id) = (self.repo.clone(), it.id.clone());
        bgv(move || repo.set_progress(&id, len, len)).await;
        let i = self.play_index.get();
        if it.kind == "episode" && (i as usize) + 1 < self.playlist.borrow().len() {
            self.play_at(i + 1).await;
        }
    }

    // ================================================================ guide (list + timeline)

    fn guide_day_start(&self) -> chrono::DateTime<Local> {
        let today = Local::now().date_naive() + CDuration::days(self.guide_day_offset.get());
        Local
            .from_local_datetime(&today.and_time(NaiveTime::MIN))
            .earliest()
            .unwrap_or_else(Local::now)
    }

    pub fn guide_shift(self: &Rc<Self>, days: i64) {
        self.guide_day_offset
            .set(self.guide_day_offset.get() + days);
        let me = self.clone();
        spawn(async move { me.load_guide().await });
    }

    pub async fn load_guide(self: &Rc<Self>) {
        let Some(it) = self.guide_item.borrow().clone() else {
            return;
        };
        if self.playing().filter(|p| p.is_live()).is_none() {
            self.live_card_title.set_text(&it.display_name());
            self.live_card_sub
                .set_text("Click the channel to start watching");
        }
        let offset = self.guide_day_offset.get();
        let day = self.guide_day_start();
        self.guide_day.set_text(&if offset == 0 {
            format!("Today, {}", day.format("%A %-d %B"))
        } else {
            day.format("%A, %-d %B").to_string()
        });
        self.tl_day.set_text(&if offset == 0 {
            format!("Today · {}", day.format("%a %-d %b"))
        } else {
            day.format("%a %-d %b").to_string()
        });
        let from = day.timestamp();
        let to = from + 86400;
        let (repo, id) = (self.repo.clone(), it.id.clone());
        let progs = bgv(move || repo.guide(&id, from, to)).await;
        if self.guide_item.borrow().as_ref().map(|g| &g.id) != Some(&it.id)
            || self.guide_day_offset.get() != offset
        {
            return;
        }
        self.render_timeline(&it, from, &progs);
        self.render_guide_list(&progs);
    }

    fn render_guide_list(&self, progs: &[Programme]) {
        kit::clear(&self.guide_list);
        self.guide_empty.set_visible(progs.is_empty());
        let now = repo::now_unix();
        let mut live_row: Option<gtk::Box> = None;
        for p in progs {
            let card = kit::vbox(0);
            card.add_css_class("guide-row");
            if p.is_past(now) {
                card.add_css_class("past");
            }
            let top = kit::hbox(12);
            top.append(&kit::text(&p.time_range(), 12.0, "muted", 400, false));
            if p.is_live(now) {
                let l = gtk::Label::new(Some("LIVE"));
                l.add_css_class("live-outline");
                top.append(&l);
            }
            card.append(&top);
            let t = kit::text(&p.title, 14.5, "", 600, false);
            t.set_margin_top(6);
            card.append(&t);
            if p.is_live(now) {
                let bar = kit::progress(p.progress(now));
                bar.set_margin_top(8);
                card.append(&bar);
                live_row = Some(card.clone());
            }
            if !p.desc.trim().is_empty() {
                let d = kit::lines(&p.desc, 12.0, "muted", 2);
                d.set_margin_top(6);
                card.append(&d);
            }
            self.guide_list.append(&card);
        }
        if let Some(row) = live_row {
            let sw = self.guide_scroll.clone();
            glib::idle_add_local_once(move || {
                if let Some(b) = row.compute_bounds(&sw) {
                    let adj = sw.vadjustment();
                    adj.set_value((adj.value() + b.y() as f64 - 10.0).max(0.0));
                }
            });
        }
    }

    /// Horizontal ribbon: hour ruler, one block per programme, a red "now" line.
    fn render_timeline(&self, channel: &Item, day_start: i64, progs: &[Programme]) {
        self.tl_channel.set_text(&channel.display_name());
        let now = repo::now_unix();
        let live = progs.iter().find(|p| p.is_live(now));
        self.tl_now.set_text(
            &live
                .map(|p| format!("Now: {}  ·  {}", p.title, p.time_range()))
                .unwrap_or_default(),
        );
        self.tl_empty.set_visible(progs.is_empty());
        while let Some(c) = self.tl_canvas.first_child() {
            self.tl_canvas.remove(&c);
        }
        let canvas = &self.tl_canvas;
        for h in 0..24 {
            let x = h as f64 * 60.0 * PX_PER_MINUTE;
            let label = kit::text(&format!("{h:02}:00"), 11.0, "muted", 400, false);
            label.set_ellipsize(gtk::pango::EllipsizeMode::None);
            canvas.put(&label, x + 4.0, 0.0);
            let tick = kit::hbox(0);
            tick.add_css_class("tl-tick");
            tick.set_size_request(1, 8);
            canvas.put(&tick, x, 16.0);
        }
        let day_end = day_start + 86400;
        for p in progs {
            let (start, stop) = (p.start.max(day_start), p.stop.min(day_end));
            if stop <= start {
                continue;
            }
            let x = (start - day_start) as f64 / 60.0 * PX_PER_MINUTE;
            let w = ((stop - start) as f64 / 60.0 * PX_PER_MINUTE - 4.0).max(6.0);
            let block = kit::vbox(0);
            block.add_css_class("tl-block");
            if p.is_live(now) {
                block.add_css_class("live");
            }
            if p.is_past(now) {
                block.add_css_class("past");
            }
            block.set_size_request(w as i32, 78);
            block.set_overflow(gtk::Overflow::Hidden);
            block.set_tooltip_text(Some(&if p.desc.trim().is_empty() {
                format!("{}\n{}", p.time_range(), p.title)
            } else {
                format!("{}\n{}\n\n{}", p.time_range(), p.title, p.desc)
            }));
            let tr = kit::text(&p.time_range(), 10.5, "muted", 400, false);
            tr.set_max_width_chars(1);
            block.append(&tr);
            let t = kit::lines(
                &p.title,
                13.0,
                if p.is_live(now) { "accent" } else { "" },
                2,
            );
            t.set_max_width_chars(1);
            t.set_attributes(Some(&{
                let a = gtk::pango::AttrList::new();
                a.insert(gtk::pango::AttrSize::new_size_absolute(
                    13 * gtk::pango::SCALE,
                ));
                a.insert(gtk::pango::AttrInt::new_weight(
                    gtk::pango::Weight::Semibold,
                ));
                a
            }));
            t.set_margin_top(2);
            block.append(&t);
            if p.is_live(now) {
                let on = gtk::Label::new(Some("● ON NOW"));
                on.add_css_class("on-now");
                on.set_xalign(0.0);
                block.append(&on);
            }
            canvas.put(&block, x, 26.0);
        }
        if now >= day_start && now < day_end {
            let x = (now - day_start) as f64 / 60.0 * PX_PER_MINUTE;
            let line = kit::hbox(0);
            line.add_css_class("tl-now");
            line.set_size_request(2, 90);
            canvas.put(&line, x - 1.0, 18.0);
            let tag = gtk::Label::new(Some(&Local::now().format("%H:%M").to_string()));
            tag.add_css_class("tl-now-tag");
            canvas.put(&tag, (x - 16.0).max(0.0), 0.0);
        }
        let sw = self.tl_scroll.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(60), move || {
            scroll_timeline_to(&sw, day_start)
        });
    }

    pub fn timeline_now(self: &Rc<Self>) {
        if self.guide_day_offset.get() != 0 {
            self.guide_day_offset.set(0);
            let me = self.clone();
            spawn(async move { me.load_guide().await });
        } else {
            scroll_timeline_to(&self.tl_scroll, self.guide_day_start().timestamp());
        }
    }
}

fn scroll_timeline_to(sw: &gtk::ScrolledWindow, day_start: i64) {
    let now = repo::now_unix();
    let minutes = if now >= day_start && now < day_start + 86400 {
        (now - day_start) as f64 / 60.0
    } else {
        18.0 * 60.0
    };
    let adj = sw.hadjustment();
    adj.set_value((minutes * PX_PER_MINUTE - adj.page_size() * 0.3).max(0.0));
}
