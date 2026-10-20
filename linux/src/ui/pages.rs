//! Full-width pages: Dashboard, Advanced search, Favorites, Recently viewed and Recently
//! added. They never host the player: opening an item jumps to its section. Twin of
//! MainWindow.Pages.cs.

use super::app::{App, Mode};
use super::kit;
use super::{bg, bgv, spawn};
use crate::repo::{self, Item, Source};
use crate::settings;
use gtk::prelude::*;
use std::rc::Rc;

type Open = Rc<dyn Fn(Item)>;

impl App {
    pub async fn show_page(self: &Rc<Self>, version: u64) {
        let body = kit::vbox(0);
        kit::margins(&body, 26, 4, 26, 34);
        let result = match self.mode.get() {
            Mode::Dashboard => self.build_dashboard(&body, version).await,
            Mode::Search => self.build_search(&body, version).await,
            Mode::Favorites => self.build_favorites(&body, version).await,
            Mode::Recent => self.build_recent(&body, version).await,
            Mode::Added => self.build_added(&body, version).await,
            _ => Ok(()),
        };
        if let Err(e) = result {
            crate::log(&format!("page: {e:?}"));
            self.set_status(&e.to_string());
        }
        if version != self.load_version.get() {
            return;
        }
        kit::clear(&self.page_host);
        self.page_host.append(&kit::page_scroll(&body));
    }

    fn opener(self: &Rc<Self>) -> Open {
        let me = Rc::downgrade(self);
        Rc::new(move |it: Item| {
            if let Some(a) = me.upgrade() {
                if it.is_live() {
                    spawn(async move { a.go_to_live(it).await });
                } else {
                    spawn(async move { a.go_to_vod(it, false).await });
                }
            }
        })
    }

    fn matches(&self, it: &Item) -> bool {
        let q = self.search.text().trim().to_lowercase();
        q.is_empty() || it.name.to_lowercase().contains(&q) || it.group.to_lowercase().contains(&q)
    }

    fn no_source(self: &Rc<Self>) -> gtk::Widget {
        let s = kit::vbox(0);
        s.set_halign(gtk::Align::Center);
        s.append(&kit::empty(
            "madhyam-view-list-symbolic",
            "No playlist yet",
            "Add an Xtream Codes account or an M3U playlist from your provider to start watching.",
        ));
        let me = self.clone();
        let b = kit::primary(
            "Add playlist",
            Some("madhyam-list-add-symbolic"),
            move || me.open_add_source(),
            false,
        );
        b.set_halign(gtk::Align::Center);
        b.set_margin_top(20);
        s.append(&b);
        s.upcast()
    }

    /// The series' own poster; falls back to the details endpoint's cover.
    async fn series_poster(self: &Rc<Self>, series: &Item) -> Option<String> {
        if series.logo.as_deref().is_some_and(|l| !l.trim().is_empty()) {
            return series.logo.clone();
        }
        let (repo, s) = (self.repo.clone(), series.clone());
        bgv(move || {
            repo.series_details(&s)
                .ok()
                .flatten()
                .and_then(|d| d.poster)
        })
        .await
    }

    fn see(self: &Rc<Self>, mode: Mode, kind: Option<&str>) -> Rc<dyn Fn()> {
        let (me, kind) = (Rc::downgrade(self), kind.map(str::to_string));
        Rc::new(move || {
            if let Some(a) = me.upgrade() {
                if let Some(k) = &kind {
                    *a.page_kind.borrow_mut() = k.clone();
                }
                a.nav_button(mode).set_active(true);
            }
        })
    }

    // ================================================================ dashboard

    async fn build_dashboard(
        self: &Rc<Self>,
        body: &gtk::Box,
        version: u64,
    ) -> crate::http::Res<()> {
        let Some(src) = self.source.borrow().clone() else {
            body.append(&self.no_source());
            return Ok(());
        };
        let s = settings::snapshot();
        let repo = self.repo.clone();
        let cont: Vec<Item> = bg(move || repo.continue_watching(20))
            .await?
            .into_iter()
            .filter(|i| !i.is_live() && i.duration > 0.0 && !i.is_watched())
            .filter(|i| !crate::parental::hide_in_lists(&s, i))
            .collect();
        if version != self.load_version.get() {
            return Ok(());
        }
        let open = self.opener();
        if s.dash_hero {
            body.append(&self.dashboard_hero(cont.first().cloned()).await);
        }
        if s.dash_continue && !cont.is_empty() {
            body.append(&kit::section_header(
                "Continue watching",
                Some(cont.len() as i64),
                Some(self.see(Mode::Recent, None)),
                "History",
            ));
            let mut shown = Vec::new();
            for i in &cont {
                if i.kind == "episode" {
                    if let Some(sid) = i.series_id.clone() {
                        let repo = self.repo.clone();
                        if let Some(sr) = bgv(move || repo.item(&sid)).await {
                            let poster = self.series_poster(&sr).await.or(i.logo.clone());
                            shown.push(i.with_logo(poster));
                            continue;
                        }
                    }
                }
                shown.push(i.clone());
            }
            let me = Rc::downgrade(self);
            let play: Open = Rc::new(move |it| {
                if let Some(a) = me.upgrade() {
                    spawn(async move { a.go_to_vod(it, true).await });
                }
            });
            body.append(&kit::rail(
                shown
                    .iter()
                    .map(|i| {
                        kit::poster(
                            i,
                            play.clone(),
                            Some(progress_subtitle(i)),
                            false,
                            None,
                            kit::POSTER_W,
                            kit::POSTER_H,
                        )
                    })
                    .collect(),
            ));
        }
        if s.dash_live_favorites {
            let repo = self.repo.clone();
            let mut fav = bg(move || repo.favorites(Some("live")))
                .await
                .unwrap_or_default();
            fav.retain(|i| !crate::parental::hide_in_lists(&s, i));
            if !fav.is_empty() {
                self.update_now_next(&fav).await;
                body.append(&kit::section_header(
                    "Favorite channels",
                    Some(fav.len() as i64),
                    Some(self.see(Mode::Favorites, Some("live"))),
                    "See all",
                ));
                body.append(&kit::rail(
                    fav.iter()
                        .take(30)
                        .map(|i| kit::live_card(i, self.now_of(&i.id).as_ref(), open.clone(), None))
                        .collect(),
                ));
            }
        }
        if s.dash_recent_live {
            let repo = self.repo.clone();
            let mut recent = bg(move || repo.recent(Some("live"), 20))
                .await
                .unwrap_or_default();
            recent.retain(|i| !crate::parental::hide_in_lists(&s, i));
            if !recent.is_empty() {
                self.update_now_next(&recent).await;
                body.append(&kit::section_header(
                    "Recently watched channels",
                    Some(recent.len() as i64),
                    Some(self.see(Mode::Recent, Some("live"))),
                    "See all",
                ));
                body.append(&kit::rail(
                    recent
                        .iter()
                        .map(|i| kit::live_card(i, self.now_of(&i.id).as_ref(), open.clone(), None))
                        .collect(),
                ));
            }
        }
        if s.dash_favorite_vod {
            let repo = self.repo.clone();
            let fav: Vec<Item> = bg(move || repo.favorites(None))
                .await
                .unwrap_or_default()
                .into_iter()
                .filter(|i| !i.is_live() && !crate::parental::hide_in_lists(&s, i))
                .collect();
            if !fav.is_empty() {
                body.append(&kit::section_header(
                    "Favorite movies and series",
                    Some(fav.len() as i64),
                    Some(self.see(Mode::Favorites, Some("vod"))),
                    "See all",
                ));
                body.append(&kit::rail(
                    fav.iter()
                        .take(30)
                        .map(|i| {
                            kit::poster(
                                i,
                                open.clone(),
                                i.year.clone(),
                                true,
                                None,
                                kit::POSTER_W,
                                kit::POSTER_H,
                            )
                        })
                        .collect(),
                ));
            }
        }
        if s.dash_recently_added {
            for (kind, label) in [
                ("movie", "Recently added movies"),
                ("series", "Recently added series"),
            ] {
                let mut hidden = s.hidden(src.id, kind);
                hidden.extend(self.locked_groups(src.id, kind).await);
                let (repo, id) = (self.repo.clone(), src.id);
                let (total, items) = bg(move || {
                    repo.query(
                        kind,
                        Some(id),
                        None,
                        &hidden,
                        "added_desc",
                        0.0,
                        0,
                        20,
                        None,
                    )
                })
                .await
                .unwrap_or_default();
                if version != self.load_version.get() {
                    return Ok(());
                }
                if items.is_empty() || items.iter().all(|i| i.added_at == 0) {
                    continue;
                }
                body.append(&kit::section_header(
                    label,
                    Some(total),
                    Some(self.see(Mode::Added, None)),
                    "Browse all",
                ));
                body.append(&kit::rail(
                    items
                        .iter()
                        .map(|i| {
                            kit::poster(
                                i,
                                open.clone(),
                                Some(i.added_text()),
                                false,
                                None,
                                kit::POSTER_W,
                                kit::POSTER_H,
                            )
                        })
                        .collect(),
                ));
            }
        }
        let sources = self.sources.borrow().clone();
        if sources.len() > 1 {
            let me = Rc::downgrade(self);
            let manage: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(a) = me.upgrade() {
                    a.open_settings("Sources");
                }
            });
            body.append(&kit::section_header(
                "Your playlists",
                Some(sources.len() as i64),
                Some(manage),
                "Manage",
            ));
            body.append(&kit::rail(
                sources.iter().map(|s| self.source_card(s)).collect(),
            ));
        }
        Ok(())
    }

    /// Big banner: the film or episode to continue, or a welcome card with shortcuts.
    async fn dashboard_hero(self: &Rc<Self>, it: Option<Item>) -> gtk::Widget {
        let grid = gtk::Overlay::new();
        grid.add_css_class("hero");
        grid.set_overflow(gtk::Overflow::Hidden);
        grid.set_size_request(-1, 300);
        kit::margins(&grid, 0, 8, 0, 4);
        let content = kit::hbox(28);
        kit::margins(&content, 30, 26, 30, 26);
        let right = kit::vbox(0);
        right.set_valign(gtk::Align::Center);
        right.set_hexpand(true);
        match it {
            Some(it) => {
                let mut poster = it.logo.clone();
                let mut title = it.name.clone();
                if it.kind == "episode" {
                    if let Some(sid) = it.series_id.clone() {
                        let repo = self.repo.clone();
                        if let Some(series) = bgv(move || repo.item(&sid)).await {
                            title = series.name.clone();
                            poster = self.series_poster(&series).await.or(poster);
                        }
                    }
                }
                grid.set_child(Some(&kit::backdrop(poster.as_deref(), 0.45)));
                let (img, _) = kit::picture_frame(
                    poster.as_deref(),
                    168,
                    248,
                    "madhyam-video-x-generic-symbolic",
                    false,
                    "poster-frame",
                );
                img.set_valign(gtk::Align::Center);
                content.append(&img);
                right.append(&kit::text("CONTINUE WATCHING", 11.5, "accent", 700, false));
                let t = kit::lines(&title, 30.0, "", 2);
                t.set_attributes(Some(&{
                    let a = gtk::pango::AttrList::new();
                    a.insert(gtk::pango::AttrSize::new_size_absolute(
                        30 * gtk::pango::SCALE,
                    ));
                    a.insert(gtk::pango::AttrInt::new_weight(
                        gtk::pango::Weight::Semibold,
                    ));
                    a
                }));
                t.set_margin_top(6);
                right.append(&t);
                if it.kind == "episode" {
                    right.append(&kit::text(
                        &super::player::episode_label(&title, &it),
                        14.0,
                        "muted",
                        400,
                        false,
                    ));
                }
                let row = kit::hbox(12);
                row.set_margin_top(16);
                let bar = kit::progress(it.watched_ratio());
                bar.set_size_request(260, -1);
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
                let buttons = kit::hbox(10);
                buttons.set_margin_top(22);
                let (me, i2) = (self.clone(), it.clone());
                buttons.append(&kit::primary(
                    "Continue watching",
                    Some("madhyam-media-playback-start-symbolic"),
                    move || {
                        let (me, i) = (me.clone(), i2.clone());
                        spawn(async move { me.go_to_vod(i, true).await });
                    },
                    false,
                ));
                let (me, i2, t2) = (self.clone(), it.clone(), title.clone());
                buttons.append(&kit::primary(
                    "Details",
                    Some("madhyam-help-about-symbolic"),
                    move || {
                        let (me, i) = (me.clone(), i2.clone());
                        let target = match (&i.kind[..], &i.series_id) {
                            ("episode", Some(sid)) => Item {
                                id: sid.clone(),
                                kind: "series".into(),
                                name: t2.clone(),
                                source_id: i.source_id,
                                ..Default::default()
                            },
                            _ => i,
                        };
                        spawn(async move { me.go_to_vod(target, false).await });
                    },
                    true,
                ));
                right.append(&buttons);
            }
            None => {
                grid.set_child(Some(&kit::backdrop(None, 0.0)));
                let icon_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                icon_box.add_css_class("chip-icon");
                icon_box.add_css_class("welcome-icon");
                icon_box.set_size_request(120, 120);
                icon_box.set_valign(gtk::Align::Center);
                let i = kit::icon("madhyam-tv-symbolic", 46);
                i.add_css_class("accent");
                i.set_halign(gtk::Align::Center);
                icon_box.set_homogeneous(true);
                icon_box.append(&i);
                content.append(&icon_box);
                right.append(&kit::text("WELCOME BACK", 11.5, "accent", 700, false));
                let name = self
                    .source
                    .borrow()
                    .as_ref()
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| crate::brand::NAME.to_string());
                let t = kit::text(&name, 30.0, "", 600, false);
                t.set_margin_top(6);
                right.append(&t);
                right.append(&kit::text(
                    &self.playlist_count.text(),
                    14.0,
                    "muted",
                    400,
                    false,
                ));
                let buttons = kit::hbox(10);
                buttons.set_margin_top(22);
                for (label, mode, icon, secondary) in [
                    ("Live TV", Mode::Live, "madhyam-tv-symbolic", false),
                    (
                        "Movies",
                        Mode::Movies,
                        "madhyam-video-x-generic-symbolic",
                        true,
                    ),
                    ("Series", Mode::Series, "madhyam-view-grid-symbolic", true),
                ] {
                    let me = Rc::downgrade(self);
                    buttons.append(&kit::primary(
                        label,
                        Some(icon),
                        move || {
                            if let Some(a) = me.upgrade() {
                                a.nav_button(mode).set_active(true);
                            }
                        },
                        secondary,
                    ));
                }
                right.append(&buttons);
            }
        }
        content.append(&right);
        grid.add_overlay(&content);
        grid.upcast()
    }

    fn source_card(self: &Rc<Self>, s: &Source) -> gtk::Widget {
        let active = self.source.borrow().as_ref().is_some_and(|x| x.id == s.id);
        let stack = kit::vbox(0);
        stack.set_size_request(230, -1);
        let top = kit::hbox(10);
        let ib = kit::hbox(0);
        ib.add_css_class("chip-icon");
        ib.set_size_request(36, 36);
        let ic = kit::icon(
            if s.kind == "xtream" {
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
        top.append(&ib);
        let names = kit::vbox(0);
        names.set_valign(gtk::Align::Center);
        let n = kit::text(&s.name, 14.0, "", 600, false);
        n.set_max_width_chars(1);
        names.append(&n);
        names.append(&kit::text(
            &if active {
                format!("{} · in use", s.kind_label())
            } else {
                s.kind_label().to_string()
            },
            11.5,
            if active { "accent" } else { "muted" },
            400,
            false,
        ));
        names.set_hexpand(true);
        top.append(&names);
        stack.append(&top);
        if s.updated_at > 0 {
            let u = kit::text(
                &format!("Updated {}", repo::local(s.updated_at, "%-d %b, %H:%M")),
                11.5,
                "muted",
                400,
                false,
            );
            u.set_margin_top(10);
            stack.append(&u);
        }
        let border = kit::hbox(0);
        border.add_css_class("source-card");
        if active {
            border.add_css_class("active");
        }
        border.append(&stack);
        let (me, s2) = (self.clone(), s.clone());
        let card = kit::bare(&border, move || {
            let (me, s) = (me.clone(), s2.clone());
            spawn(async move { me.switch_source(s).await });
        });
        kit::margins(&card, 0, 0, 12, 12);
        card.set_valign(gtk::Align::Start);
        card.upcast()
    }

    // ================================================================ advanced search

    fn page_title(title: &str, subtitle: Option<&str>, right: Option<gtk::Widget>) -> gtk::Widget {
        let g = kit::hbox(0);
        kit::margins(&g, 0, 10, 0, 6);
        let left = kit::vbox(0);
        left.set_hexpand(true);
        left.append(&kit::text(title, 26.0, "", 600, false));
        if let Some(s) = subtitle {
            let l = kit::text(s, 13.0, "muted", 400, false);
            l.set_margin_top(3);
            left.append(&l);
        }
        g.append(&left);
        if let Some(r) = right {
            r.set_valign(gtk::Align::Center);
            g.append(&r);
        }
        g.upcast()
    }

    fn rerun(self: &Rc<Self>) {
        let me = self.clone();
        spawn(async move {
            let v = me.next_version();
            me.show_page(v).await
        });
    }

    async fn build_search(self: &Rc<Self>, body: &gtk::Box, version: u64) -> crate::http::Res<()> {
        let q = self.search.text().trim().to_string();
        let filters = gtk::FlowBox::new();
        filters.set_selection_mode(gtk::SelectionMode::None);
        filters.set_max_children_per_line(10);
        filters.set_halign(gtk::Align::Start);
        kit::margins(&filters, 0, 12, 0, 4);
        let toggle = |label: &str, cell: fn(&App) -> &std::cell::Cell<bool>| {
            let me = Rc::downgrade(self);
            kit::filter(label, cell(self).get(), move |v| {
                if let Some(a) = me.upgrade() {
                    cell(&a).set(v);
                    a.rerun();
                }
            })
        };
        filters.insert(&toggle("Live TV", |a| &a.search_live), -1);
        filters.insert(&toggle("Movies", |a| &a.search_movies), -1);
        filters.insert(&toggle("Series", |a| &a.search_series), -1);
        filters.insert(
            &toggle("Visible categories only", |a| &a.search_visible_only),
            -1,
        );
        if self.sources.borrow().len() > 1 {
            filters.insert(&toggle("All playlists", |a| &a.search_all_sources), -1);
        }
        if q.is_empty() {
            body.append(&Self::page_title(
                "Advanced search",
                Some("Search channels, movies and series at once."),
                None,
            ));
            body.append(&filters);
            body.append(&kit::empty(
                "madhyam-system-search-symbolic",
                "Type to search",
                "Use the search box at the top. Press Enter there from any page to come here.",
            ));
            return Ok(());
        }
        let mut kinds = Vec::new();
        if self.search_live.get() {
            kinds.push("live");
        }
        if self.search_movies.get() {
            kinds.push("movie");
        }
        if self.search_series.get() {
            kinds.push("series");
        }
        let (repo, q2) = (self.repo.clone(), q.clone());
        let mut hits = bg(move || {
            let mut all = Vec::new();
            for k in kinds {
                all.extend(repo.search(&q2, Some(k))?);
            }
            Ok(all)
        })
        .await?;
        if version != self.load_version.get() {
            return Ok(());
        }
        if !self.search_all_sources.get() {
            if let Some(src) = self.source.borrow().as_ref() {
                hits.retain(|h| h.source_id == src.id);
            }
        }
        if self.search_visible_only.get() {
            let s = settings::snapshot();
            hits.retain(|h| !s.hidden(h.source_id, &h.kind).contains(&h.group));
        }
        {
            // Parental controls: locked categories and adult channels stay out of results.
            let s = settings::snapshot();
            hits.retain(|h| !crate::parental::hide_in_lists(&s, h));
        }
        let sub = if hits.is_empty() {
            "Nothing found.".to_string()
        } else {
            format!("Found {} results", repo::group_digits(hits.len() as i64))
        };
        body.append(&Self::page_title(
            &format!("Results for “{q}”"),
            Some(&sub),
            None,
        ));
        body.append(&filters);
        if hits.is_empty() {
            body.append(&kit::empty(
                "madhyam-system-search-symbolic",
                "No results",
                "Try fewer words, or turn on more types above.",
            ));
            return Ok(());
        }
        let open = self.opener();
        let live: Vec<Item> = hits.iter().filter(|h| h.is_live()).cloned().collect();
        if !live.is_empty() {
            self.update_now_next(&live).await;
            body.append(&kit::section_header(
                "Live TV",
                Some(live.len() as i64),
                None,
                "",
            ));
            body.append(&kit::wrap(
                live.iter()
                    .take(60)
                    .map(|i| kit::live_card(i, self.now_of(&i.id).as_ref(), open.clone(), None))
                    .collect(),
            ));
        }
        for (kind, label) in [("movie", "Movies"), ("series", "Series")] {
            let part: Vec<&Item> = hits.iter().filter(|h| h.kind == kind).collect();
            if part.is_empty() {
                continue;
            }
            body.append(&kit::section_header(
                label,
                Some(part.len() as i64),
                None,
                "",
            ));
            body.append(&kit::wrap(
                part.iter()
                    .take(200)
                    .map(|i| {
                        kit::poster(
                            i,
                            open.clone(),
                            Some(i.group.clone()),
                            false,
                            None,
                            kit::POSTER_W,
                            kit::POSTER_H,
                        )
                    })
                    .collect(),
            ));
        }
        Ok(())
    }

    // ================================================================ favorites / recent

    fn kind_and_source_bar(self: &Rc<Self>) -> gtk::Widget {
        let bar = kit::hbox(24);
        kit::margins(&bar, 0, 12, 0, 4);
        let cur = self.page_kind.borrow().clone();
        let cur = if cur == "vod" { "all".to_string() } else { cur };
        let me = Rc::downgrade(self);
        bar.append(&kit::segments(
            &[
                ("all", "All"),
                ("live", "Live TV"),
                ("movie", "Movies"),
                ("series", "Series"),
            ],
            &cur,
            move |v| {
                if let Some(a) = me.upgrade() {
                    *a.page_kind.borrow_mut() = v;
                    a.rerun();
                }
            },
        ));
        if self.sources.borrow().len() > 1 {
            let me = Rc::downgrade(self);
            bar.append(&kit::segments(
                &[("this", "This playlist"), ("all", "All playlists")],
                if self.page_all_sources.get() {
                    "all"
                } else {
                    "this"
                },
                move |v| {
                    if let Some(a) = me.upgrade() {
                        a.page_all_sources.set(v == "all");
                        a.rerun();
                    }
                },
            ));
        }
        bar.upcast()
    }

    fn page_filter(&self, items: Vec<Item>) -> Vec<Item> {
        let kind = self.page_kind.borrow().clone();
        let src = self.source.borrow().as_ref().map(|s| s.id);
        items
            .into_iter()
            .filter(|i| match kind.as_str() {
                "live" => i.is_live(),
                "movie" => i.kind == "movie",
                "series" => i.kind == "series" || i.kind == "episode",
                "vod" => !i.is_live(),
                _ => true,
            })
            .filter(|i| self.page_all_sources.get() || src.is_none_or(|s| i.source_id == s))
            .filter(|i| self.matches(i))
            .collect()
    }

    async fn build_favorites(
        self: &Rc<Self>,
        body: &gtk::Box,
        version: u64,
    ) -> crate::http::Res<()> {
        let repo = self.repo.clone();
        let all = bg(move || repo.favorites(None)).await?;
        if version != self.load_version.get() {
            return Ok(());
        }
        let total = all.len();
        let items = self.page_filter(all);
        let sub = (total > 0).then(|| {
            format!(
                "{} of {}",
                repo::group_digits(items.len() as i64),
                repo::group_digits(total as i64)
            )
        });
        body.append(&Self::page_title("Favorites", sub.as_deref(), None));
        body.append(&self.kind_and_source_bar());
        if items.is_empty() {
            body.append(&kit::empty(
                "madhyam-emote-love-symbolic",
                if total == 0 {
                    "No favorites yet"
                } else {
                    "Nothing here"
                },
                if total == 0 {
                    "Use the star next to a channel, or “Add to favorites” on a movie or series."
                } else {
                    "Try another filter."
                },
            ));
            return Ok(());
        }
        let me = Rc::downgrade(self);
        let remove: Open = Rc::new(move |it| {
            if let Some(a) = me.upgrade() {
                spawn(async move { a.toggle_favorite(it).await });
            }
        });
        self.add_live_and_vod(body, items, remove, |i| i.group.clone())
            .await;
        Ok(())
    }

    async fn build_recent(self: &Rc<Self>, body: &gtk::Box, version: u64) -> crate::http::Res<()> {
        let repo = self.repo.clone();
        let all = bg(move || repo.recent(None, 200)).await?;
        if version != self.load_version.get() {
            return Ok(());
        }
        let total = all.len();
        let items = self.page_filter(all);
        let clear = (total > 0).then(|| {
            let me = self.clone();
            kit::primary("Clear history", Some("madhyam-edit-clear-all-symbolic"), move || {
                let me = me.clone();
                spawn(async move {
                    if !super::dialogs::confirm(&me.window, "Clear Recently viewed?", "Everything is removed from Recently viewed. Favorites and resume positions are kept.", "Clear history", true).await {
                        return;
                    }
                    let repo = me.repo.clone();
                    let _ = bg(move || repo.clear_history()).await;
                    me.rerun();
                });
            }, true)
            .upcast::<gtk::Widget>()
        });
        let sub = (total > 0).then(|| format!("{} items", repo::group_digits(items.len() as i64)));
        body.append(&Self::page_title("Recently viewed", sub.as_deref(), clear));
        body.append(&self.kind_and_source_bar());
        if items.is_empty() {
            body.append(&kit::empty(
                "madhyam-document-open-recent-symbolic",
                if total == 0 {
                    "Nothing watched yet"
                } else {
                    "Nothing here"
                },
                if total == 0 {
                    "Channels, movies and episodes you play appear here."
                } else {
                    "Try another filter."
                },
            ));
            return Ok(());
        }
        let me = Rc::downgrade(self);
        let remove: Open = Rc::new(move |it: Item| {
            if let Some(a) = me.upgrade() {
                spawn(async move {
                    let repo = a.repo.clone();
                    let _ = bg(move || repo.remove_from_history(&it.id)).await;
                    a.rerun();
                });
            }
        });
        self.add_live_and_vod(body, items, remove, |i| i.played_text())
            .await;
        Ok(())
    }

    async fn add_live_and_vod(
        self: &Rc<Self>,
        body: &gtk::Box,
        items: Vec<Item>,
        remove: Open,
        subtitle: fn(&Item) -> String,
    ) {
        let open = self.opener();
        let live: Vec<Item> = items.iter().filter(|i| i.is_live()).cloned().collect();
        if !live.is_empty() {
            self.update_now_next(&live).await;
            body.append(&kit::section_header(
                "Live TV",
                Some(live.len() as i64),
                None,
                "",
            ));
            body.append(&kit::wrap(
                live.iter()
                    .map(|i| {
                        kit::live_card(
                            i,
                            self.now_of(&i.id).as_ref(),
                            open.clone(),
                            Some(remove.clone()),
                        )
                    })
                    .collect(),
            ));
        }
        for (kinds, label) in [
            (&["movie"][..], "Movies"),
            (&["series", "episode"][..], "Series"),
        ] {
            let part: Vec<&Item> = items
                .iter()
                .filter(|i| kinds.contains(&i.kind.as_str()))
                .collect();
            if part.is_empty() {
                continue;
            }
            body.append(&kit::section_header(
                label,
                Some(part.len() as i64),
                None,
                "",
            ));
            body.append(&kit::wrap(
                part.iter()
                    .map(|i| {
                        let sub = if i.kind == "episode" {
                            progress_subtitle(i)
                        } else {
                            subtitle(i)
                        };
                        kit::poster(
                            i,
                            open.clone(),
                            Some(sub),
                            i.kind == "episode",
                            Some(remove.clone()),
                            kit::POSTER_W,
                            kit::POSTER_H,
                        )
                    })
                    .collect(),
            ));
        }
    }

    // ================================================================ recently added

    async fn build_added(self: &Rc<Self>, body: &gtk::Box, version: u64) -> crate::http::Res<()> {
        let Some(src) = self.source.borrow().clone() else {
            body.append(&self.no_source());
            return Ok(());
        };
        body.append(&Self::page_title(
            "Recently added",
            Some(&format!("Newest first in {}", src.name)),
            None,
        ));
        let open = self.opener();
        let mut any = false;
        let s = settings::snapshot();
        for (kind, label, mode) in [
            ("movie", "Movies", Mode::Movies),
            ("series", "Series", Mode::Series),
            ("live", "Live TV", Mode::Live),
        ] {
            let mut hidden = s.hidden(src.id, kind);
            hidden.extend(self.locked_groups(src.id, kind).await);
            let (repo, id) = (self.repo.clone(), src.id);
            let (total, items) = bg(move || {
                repo.query(
                    kind,
                    Some(id),
                    None,
                    &hidden,
                    "added_desc",
                    0.0,
                    0,
                    30,
                    None,
                )
            })
            .await
            .unwrap_or_default();
            if version != self.load_version.get() {
                return Ok(());
            }
            let items: Vec<Item> = items.into_iter().filter(|i| i.added_at > 0).collect();
            if items.is_empty() {
                continue;
            }
            any = true;
            let me = Rc::downgrade(self);
            let browse: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(a) = me.upgrade() {
                    if mode != Mode::Live {
                        settings::update(|s| s.vod_sort = "added_desc".into());
                    }
                    a.nav_button(mode).set_active(true);
                }
            });
            body.append(&kit::section_header(
                label,
                Some(total),
                Some(browse),
                "Browse all",
            ));
            if kind == "live" {
                self.update_now_next(&items).await;
                body.append(&kit::rail(
                    items
                        .iter()
                        .map(|i| kit::live_card(i, self.now_of(&i.id).as_ref(), open.clone(), None))
                        .collect(),
                ));
            } else {
                body.append(&kit::rail(
                    items
                        .iter()
                        .map(|i| {
                            kit::poster(
                                i,
                                open.clone(),
                                Some(i.added_text()),
                                false,
                                None,
                                kit::POSTER_W,
                                kit::POSTER_H,
                            )
                        })
                        .collect(),
                ));
            }
        }
        if !any {
            body.append(&kit::empty("madhyam-appointment-soon-symbolic", "No dates from this provider", "Recently added uses the dates an Xtream server sends. M3U playlists do not include them."));
        }
        Ok(())
    }
}

fn progress_subtitle(it: &Item) -> String {
    let left = it.time_left_text();
    if it.kind == "episode" {
        if left.is_empty() {
            it.se()
        } else {
            format!("{} · {left}", it.se())
        }
    } else {
        left
    }
}
