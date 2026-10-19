//! Settings: section list on the left, rows of "title + description … control" on the right.
//! Same sections and rows as SettingsView.cs on Windows.

use super::app::App;
use super::kit;
use super::{bg, bgv, spawn};
use crate::repo::{Repo, Source};
use crate::settings;
use crate::theme;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

const SECTIONS: [(&str, &str, &str); 7] = [
    ("General", "madhyam-preferences-system-symbolic", "General"),
    ("Sources", "madhyam-view-list-symbolic", "Sources"),
    ("Dashboard", "madhyam-go-home-symbolic", "Dashboard"),
    (
        "Playback",
        "madhyam-media-playback-start-symbolic",
        "Playback",
    ),
    (
        "Guide",
        "madhyam-appointment-soon-symbolic",
        "Programme guide",
    ),
    ("Data", "madhyam-folder-symbolic", "Backup & data"),
    ("About", "madhyam-help-about-symbolic", "About"),
];

pub struct SettingsView {
    pub root: gtk::Box,
    app: Weak<App>,
    nav: gtk::ListBox,
    content: gtk::Box,
    scroll: gtk::ScrolledWindow,
    current: RefCell<String>,
    src_filter: RefCell<String>,
    src_sort: RefCell<String>,
}

fn row_widget(
    title: &str,
    desc: &str,
    control: &impl IsA<gtk::Widget>,
    below: bool,
) -> gtk::Widget {
    let text = kit::vbox(3);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    let t = gtk::Label::new(Some(title));
    t.add_css_class("row-title");
    t.set_xalign(0.0);
    t.set_wrap(true);
    text.append(&t);
    if !desc.is_empty() {
        let d = gtk::Label::new(Some(desc));
        d.add_css_class("row-desc");
        d.set_xalign(0.0);
        d.set_wrap(true);
        d.set_max_width_chars(80);
        text.append(&d);
    }
    if below {
        let s = kit::vbox(10);
        s.append(&text);
        control.set_halign(gtk::Align::Start);
        s.append(control);
        s.upcast()
    } else {
        let g = kit::hbox(24);
        g.append(&text);
        control.set_valign(gtk::Align::Center);
        g.append(control);
        g.upcast()
    }
}

fn toggle(value: bool, changed: impl Fn(bool) + 'static) -> gtk::Switch {
    let s = gtk::Switch::new();
    s.set_active(value);
    s.set_valign(gtk::Align::Center);
    s.connect_active_notify(move |s| changed(s.is_active()));
    s
}

fn choice(options: &[(&str, &str)], current: &str, changed: impl Fn(String) + 'static) -> gtk::Box {
    kit::segments(options, current, changed)
}

fn btn(label: &str, primary: bool, f: impl Fn() + 'static) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class(if primary { "primary" } else { "secondary" });
    b.connect_clicked(move |_| f());
    b
}

fn buttons(list: &[&gtk::Button]) -> gtk::Box {
    let p = kit::hbox(8);
    for b in list {
        p.append(*b);
    }
    p
}

fn note(t: &str) -> gtk::Label {
    let l = kit::text(t, 13.0, "note", 400, true);
    l.set_margin_top(8);
    l
}

fn entry(text: &str, on_done: impl Fn(String) + 'static) -> gtk::Entry {
    let e = gtk::Entry::new();
    e.set_text(text);
    e.set_size_request(320, -1);
    let f = Rc::new(on_done);
    let f2 = f.clone();
    e.connect_activate(move |e| f2(e.text().trim().to_string()));
    let focus = gtk::EventControllerFocus::new();
    let ew = e.downgrade();
    focus.connect_leave(move |_| {
        if let Some(e) = ew.upgrade() {
            f(e.text().trim().to_string());
        }
    });
    e.add_controller(focus);
    e
}

impl SettingsView {
    pub fn new(app: &Rc<App>) -> Rc<SettingsView> {
        let root = kit::hbox(14);
        root.set_hexpand(true);
        root.set_vexpand(true);
        let left = kit::vbox(0);
        left.set_size_request(230, -1);
        let t = kit::text("Settings", 17.0, "", 600, false);
        kit::margins(&t, 12, 4, 0, 12);
        left.append(&t);
        let nav = gtk::ListBox::new();
        nav.add_css_class("plain");
        for (_, icon, label) in SECTIONS {
            let r = kit::hbox(10);
            kit::margins(&r, 12, 9, 10, 9);
            r.append(&kit::icon(icon, 15));
            r.append(&kit::text(label, 14.0, "", 400, false));
            nav.append(&r);
        }
        left.append(&nav);
        root.append(&left);
        let content = kit::vbox(0);
        kit::margins(&content, 40, 8, 40, 40);
        content.set_size_request(-1, -1);
        let clamp = kit::vbox(0);
        clamp.append(&content);
        content.set_hexpand(true);
        let scroll = kit::page_scroll(&clamp);
        scroll.add_css_class("settings-panel");
        root.append(&scroll);
        let sv = Rc::new(SettingsView {
            root,
            app: Rc::downgrade(app),
            nav,
            content,
            scroll,
            current: RefCell::new("General".into()),
            src_filter: RefCell::new("all".into()),
            src_sort: RefCell::new("added".into()),
        });
        let w = Rc::downgrade(&sv);
        sv.nav.connect_row_selected(move |_, r| {
            if let (Some(sv), Some(r)) = (w.upgrade(), r) {
                sv.show(SECTIONS[r.index().max(0) as usize].0);
            }
        });
        if let Some(r) = sv.nav.row_at_index(0) {
            sv.nav.select_row(Some(&r));
        }
        sv
    }

    /// Rebuild the visible section (after sources changed).
    pub fn refresh(self: &Rc<Self>) {
        let cur = self.current.borrow().clone();
        if cur == "Sources" || cur == "About" {
            self.show(&cur);
        }
    }

    /// Open a section by key (General, Sources, Dashboard, Playback, Guide, Data, About).
    pub fn open(self: &Rc<Self>, key: &str) {
        if let Some(i) = SECTIONS.iter().position(|s| s.0.eq_ignore_ascii_case(key)) {
            if let Some(r) = self.nav.row_at_index(i as i32) {
                self.nav.select_row(Some(&r));
            }
        }
    }

    fn header(&self, title: &str, subtitle: &str) {
        let t = kit::text(title, 24.0, "", 600, false);
        kit::margins(&t, 0, 22, 0, 2);
        self.content.append(&t);
        let s = kit::text(subtitle, 13.0, "muted", 400, true);
        s.set_margin_bottom(10);
        self.content.append(&s);
    }

    fn add_row(&self, w: &impl IsA<gtk::Widget>) {
        let l = kit::line();
        kit::margins(&l, 0, 14, 0, 14);
        self.content.append(&l);
        self.content.append(w);
    }

    fn row(&self, title: &str, desc: &str, control: &impl IsA<gtk::Widget>, below: bool) {
        self.add_row(&row_widget(title, desc, control, below));
    }

    fn show(self: &Rc<Self>, key: &str) {
        *self.current.borrow_mut() = key.to_string();
        kit::clear(&self.content);
        self.scroll.vadjustment().set_value(0.0);
        let Some(app) = self.app.upgrade() else {
            return;
        };
        match key {
            "General" => self.general(&app),
            "Sources" => self.sources_page(&app),
            "Dashboard" => self.dashboard_page(),
            "Playback" => self.playback(&app),
            "Guide" => self.guide(&app),
            "Data" => self.data(&app),
            _ => self.about(&app),
        }
    }

    // ================================================================ general

    fn general(self: &Rc<Self>, app: &Rc<App>) {
        let name = crate::brand::NAME;
        self.header("General", &format!("How {name} looks and starts."));
        let s = settings::snapshot();
        let theme_box = kit::vbox(10);
        let swatch = gtk::Label::new(None);
        let custom_name = kit::text("", 13.0, "muted", 400, false);
        let show_custom = {
            let (swatch, custom_name) = (swatch.clone(), custom_name.clone());
            move || {
                let s = settings::snapshot();
                swatch.set_markup(&format!(
                    "<span foreground='{}'>●</span>",
                    theme::hex(theme::parse(&s.custom_color).unwrap_or(theme::Rgb(220, 20, 60)))
                ));
                custom_name.set_text(&format!(
                    "{} · {}",
                    theme::name_of(&s.custom_color),
                    if s.custom_light { "light" } else { "dark" }
                ));
            }
        };
        show_custom();
        let w = Rc::downgrade(app);
        let choice_box = choice(
            &[
                ("Dark", "☾  Navy & Crimson"),
                ("Light", "☀  Light"),
                ("System", "🖥  System"),
                ("Custom", "🎨  Custom"),
            ],
            &s.theme,
            move |v| {
                settings::update(|s| s.theme = v.clone());
                if let Some(a) = w.upgrade() {
                    a.apply_theme();
                }
            },
        );
        theme_box.append(&choice_box);
        let custom_row = kit::hbox(8);
        custom_row.append(&swatch);
        custom_row.append(&custom_name);
        let (w, sc, cb) = (Rc::downgrade(app), Rc::new(show_custom), choice_box.clone());
        let pick = btn("Pick colour…", false, move || {
            let Some(a) = w.upgrade() else { return };
            let s = settings::snapshot();
            let (w2, sc2, cb2) = (Rc::downgrade(&a), sc.clone(), cb.clone());
            super::dialogs::color_picker(
                &a.window,
                &s.custom_color,
                s.custom_light,
                move |hex, light| {
                    settings::update(|s| {
                        s.custom_color = hex.clone();
                        s.custom_light = light;
                        s.theme = "Custom".into();
                    });
                    sc2();
                    // Select the Custom segment (the last one).
                    if let Some(last) = cb2
                        .last_child()
                        .and_then(|c| c.downcast::<gtk::ToggleButton>().ok())
                    {
                        last.set_active(true);
                    }
                    if let Some(a) = w2.upgrade() {
                        a.apply_theme();
                    }
                },
            );
        });
        pick.set_margin_start(4);
        custom_row.append(&pick);
        theme_box.append(&custom_row);
        self.row("Visual theme", "Navy & Crimson is the dark theme: deep navy with crimson highlights. System follows your desktop's light or dark style. Custom builds a theme from any colour you pick.", &theme_box, true);

        let w = Rc::downgrade(app);
        self.row(
            "Cover size",
            "Size of posters in the movie and series grids. Larger is easier to see from a distance; smaller shows more per row.",
            &choice(&[("Small", "Small"), ("Medium", "Medium"), ("Large", "Large")], &s.cover_size, move |v| {
                settings::update(|s| s.cover_size = v.clone());
                if let Some(a) = w.upgrade() {
                    a.apply_theme();
                }
            }),
            true,
        );
        self.row(
            "Show titles under covers",
            "Turn off for a posters-only wall; the title still shows when you point at a poster.",
            &toggle(s.titles_under_covers, |v| {
                settings::update(|s| s.titles_under_covers = v)
            }),
            false,
        );
        self.row(
            "Startup view",
            &format!("The section {name} opens on."),
            &choice(
                &[
                    ("Dashboard", "Dashboard"),
                    ("Live", "Live TV"),
                    ("Movies", "Movies"),
                    ("Series", "Series"),
                    ("Favorites", "Favorites"),
                    ("Last", "Where I left off"),
                ],
                &s.startup_view,
                |v| settings::update(|s| s.startup_view = v),
            ),
            true,
        );
        self.row(
            "Window on startup",
            "Open at the last size, maximized, or full screen. F11 toggles full screen at any time.",
            &choice(&[("Last", "Last size"), ("Maximized", "Maximized"), ("Fullscreen", "Full screen")], &s.window_startup, |v| settings::update(|s| s.window_startup = v)),
            true,
        );
        let w = Rc::downgrade(app);
        self.row(
            "Strip country prefixes from channel names",
            "Shows “CBC Calgary HD” instead of “CA EN: CBC Calgary HD”. Search still finds both.",
            &toggle(s.strip_prefixes, move |v| {
                settings::update(|s| s.strip_prefixes = v);
                if let Some(a) = w.upgrade() {
                    a.refresh_list_appearance();
                }
            }),
            false,
        );
        let w = Rc::downgrade(app);
        self.row(
            "Show channel numbers",
            "Number channels in the order the provider lists them.",
            &toggle(s.show_numbers, move |v| {
                settings::update(|s| s.show_numbers = v);
                if let Some(a) = w.upgrade() {
                    a.refresh_list_appearance();
                }
            }),
            false,
        );
    }

    // ================================================================ sources

    fn sources_page(self: &Rc<Self>, app: &Rc<App>) {
        self.header(
            "Sources",
            "Your playlists and Xtream accounts. Switch, test, refresh, edit or remove them here.",
        );
        let tools = kit::hbox(18);
        tools.set_margin_top(6);
        let sv = Rc::downgrade(self);
        tools.append(&choice(
            &[("all", "All"), ("m3u", "M3U"), ("xtream", "Xtream")],
            &self.src_filter.borrow(),
            move |v| {
                if let Some(sv) = sv.upgrade() {
                    *sv.src_filter.borrow_mut() = v;
                    sv.show("Sources");
                }
            },
        ));
        let sv = Rc::downgrade(self);
        tools.append(&choice(
            &[("added", "Newest first"), ("name", "Name A–Z")],
            &self.src_sort.borrow(),
            move |v| {
                if let Some(sv) = sv.upgrade() {
                    *sv.src_sort.borrow_mut() = v;
                    sv.show("Sources");
                }
            },
        ));
        tools.append(&kit::spacer());
        let w = Rc::downgrade(app);
        tools.append(&btn("Add source", true, move || {
            if let Some(a) = w.upgrade() {
                a.open_add_source();
            }
        }));
        self.content.append(&tools);
        self.add_row(&self.free_tv_card(app));
        let filter = self.src_filter.borrow().clone();
        let all = app.sources.borrow().clone();
        let mut list: Vec<Source> = all
            .iter()
            .filter(|s| filter == "all" || s.kind == filter)
            .cloned()
            .collect();
        if *self.src_sort.borrow() == "name" {
            list.sort_by_key(|s| s.name.to_lowercase());
        } else {
            let added = settings::get().source_added.clone();
            list.sort_by_key(|s| std::cmp::Reverse(added.get(&s.id).copied().unwrap_or(s.id)));
        }
        if list.is_empty() {
            self.add_row(&kit::text(
                if all.is_empty() {
                    "No sources yet. Use “Add source” to add an Xtream account or an M3U playlist."
                } else {
                    "No sources of this type."
                },
                13.0,
                "muted",
                400,
                true,
            ));
            return;
        }
        for s in list {
            self.add_row(&self.source_row(app, &s));
        }
    }

    /// Free TV (iptv-org): on/off and re-download.
    fn free_tv_card(self: &Rc<Self>, app: &Rc<App>) -> gtk::Widget {
        let id = settings::get().free_tv_source_id;
        let on = id.is_some_and(|id| app.sources.borrow().iter().any(|s| s.id == id));
        let bx = kit::vbox(0);
        let status = note("");
        status.set_visible(false);
        let options = kit::vbox(0);
        options.set_margin_top(12);
        options.set_visible(on);
        let (w, opt, st) = (Rc::downgrade(app), options.clone(), status.clone());
        let sw = toggle(on, move |v| {
            opt.set_visible(v);
            st.set_visible(true);
            st.set_text(if v {
                "Downloading the channel list…"
            } else {
                "Removing Free TV…"
            });
            if let Some(a) = w.upgrade() {
                let st = st.clone();
                spawn(async move {
                    a.apply_free_tv(v).await;
                    st.set_text(if v {
                        "Free TV is on. Pick it from the playlist menu at the top."
                    } else {
                        "Free TV removed."
                    });
                });
            }
        });
        sw.set_valign(gtk::Align::Start);
        bx.append(&row_widget(
            "Free TV channels (iptv-org)",
            "Thousands of public channels collected by the iptv-org project, added as a playlist called “Free TV”. Many streams are offline or only work in their own country; turn it off to remove them again.",
            &sw,
            false,
        ));
        options.append(&note("One playlist with every channel. In Live TV, switch the left panel between Categories and Countries."));
        let (w, st) = (Rc::downgrade(app), status.clone());
        let upd = btn("Update channel list", true, move || {
            st.set_visible(true);
            st.set_text("Downloading the channel list…");
            if let Some(a) = w.upgrade() {
                let st = st.clone();
                spawn(async move {
                    a.apply_free_tv(true).await;
                    st.set_text("Free TV updated.");
                });
            }
        });
        upd.set_halign(gtk::Align::Start);
        upd.set_margin_top(12);
        options.append(&upd);
        bx.append(&options);
        bx.append(&status);
        bx.upcast()
    }

    fn source_row(self: &Rc<Self>, app: &Rc<App>, src: &Source) -> gtk::Widget {
        let active = app.source.borrow().as_ref().is_some_and(|s| s.id == src.id);
        let outer = kit::vbox(0);
        let g = kit::hbox(14);
        let ib = kit::hbox(0);
        ib.add_css_class("chip-icon");
        ib.set_size_request(42, 42);
        ib.set_valign(gtk::Align::Start);
        let ic = kit::icon(
            if src.kind == "xtream" {
                "madhyam-network-server-symbolic"
            } else {
                "madhyam-view-list-symbolic"
            },
            16,
        );
        ic.set_halign(gtk::Align::Center);
        ib.set_homogeneous(true);
        if active {
            ic.add_css_class("accent");
        }
        ib.append(&ic);
        g.append(&ib);
        let text = kit::vbox(2);
        text.set_hexpand(true);
        let name_row = kit::hbox(10);
        name_row.append(&kit::text(&src.name, 15.0, "", 600, false));
        if active {
            let u = gtk::Label::new(Some("IN USE"));
            u.add_css_class("in-use");
            u.set_valign(gtk::Align::Center);
            name_row.append(&u);
        }
        text.append(&name_row);
        let when = |t: i64| crate::repo::local(t, "%-d %b %Y, %H:%M");
        let mut meta = vec![src.kind_label().to_string()];
        if let Some(a) = settings::get().source_added.get(&src.id) {
            meta.push(format!("Added {}", when(*a)));
        }
        if src.updated_at > 0 {
            meta.push(format!("Updated {}", when(src.updated_at)));
        }
        text.append(&kit::text(&meta.join("  |  "), 12.5, "muted", 400, true));
        let loc = kit::text(
            &if src.kind == "xtream" {
                format!("{}  ·  user {}", src.location, src.username)
            } else {
                src.location.clone()
            },
            12.0,
            "muted",
            400,
            false,
        );
        loc.set_max_width_chars(1);
        text.append(&loc);
        g.append(&text);
        let status = kit::text("", 13.0, "note", 400, true);
        kit::margins(&status, 56, 8, 0, 0);
        status.set_visible(false);
        let say = {
            let status = status.clone();
            Rc::new(move |msg: &str, bad: bool| {
                status.set_text(msg);
                if bad {
                    status.add_css_class("error");
                } else {
                    status.remove_css_class("error");
                }
                status.set_visible(true);
            })
        };
        let actions = kit::hbox(0);
        actions.set_valign(gtk::Align::Center);
        let action = |icon: &str,
                      tip: &str,
                      f: Rc<
            dyn Fn(
                Rc<App>,
                Rc<dyn Fn(&str, bool)>,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<(), crate::http::AppError>>>,
            >,
        >| {
            let (w, say) = (Rc::downgrade(app), say.clone());
            let b = gtk::Button::new();
            b.set_child(Some(&kit::icon(icon, 15)));
            b.add_css_class("icon-button");
            b.set_tooltip_text(Some(tip));
            b.connect_clicked(move |b| {
                let Some(a) = w.upgrade() else { return };
                let (b, say, f) = (b.clone(), say.clone(), f.clone());
                b.set_sensitive(false);
                spawn(async move {
                    let s2: Rc<dyn Fn(&str, bool)> = say.clone();
                    if let Err(e) = f(a, s2).await {
                        say(&e.to_string(), true);
                    }
                    b.set_sensitive(true);
                });
            });
            b
        };
        let sv = Rc::downgrade(self);
        let s1 = src.clone();
        if !active {
            let sv = sv.clone();
            actions.append(&action(
                "madhyam-object-select-symbolic",
                "Use this playlist",
                Rc::new(move |a, _| {
                    let (s, sv) = (s1.clone(), sv.clone());
                    Box::pin(async move {
                        a.switch_source(s).await;
                        if let Some(sv) = sv.upgrade() {
                            sv.show("Sources");
                        }
                        Ok(())
                    })
                }),
            ));
        }
        let s1 = src.clone();
        actions.append(&action(
            "madhyam-network-transmit-receive-symbolic",
            "Test connection",
            Rc::new(move |a, say| {
                let s = s1.clone();
                Box::pin(async move {
                    say("Testing…", false);
                    let repo = a.repo.clone();
                    let msg = bg(move || repo.test_connection(s.id)).await?;
                    say(&msg, false);
                    Ok(())
                })
            }),
        ));
        let (s1, sv1) = (src.clone(), sv.clone());
        actions.append(&action(
            "madhyam-view-refresh-symbolic",
            "Refresh (download again)",
            Rc::new(move |a, say| {
                let (s, sv) = (s1.clone(), sv1.clone());
                Box::pin(async move {
                    say("Refreshing…", false);
                    a.refresh_source(s).await;
                    if let Some(sv) = sv.upgrade() {
                        sv.show("Sources");
                    }
                    Ok(())
                })
            }),
        ));
        let (s1, sv1) = (src.clone(), sv.clone());
        actions.append(&action(
            "madhyam-document-edit-symbolic",
            "Edit",
            Rc::new(move |a, say| {
                let (s, sv) = (s1.clone(), sv1.clone());
                Box::pin(async move {
                    let ua = settings::get()
                        .source_user_agent
                        .get(&s.id)
                        .cloned()
                        .unwrap_or_default();
                    let pw = if s.kind == "xtream" {
                        let (repo, id) = (a.repo.clone(), s.id);
                        bgv(move || repo.source_password(id)).await
                    } else {
                        String::new()
                    };
                    let (a2, s2, sv2, say2) = (a.clone(), s.clone(), sv.clone(), say.clone());
                    super::dialogs::edit_source(&a.window, &s, &ua, &pw, move |r| {
                        settings::update(|st| {
                            if r.user_agent.is_empty() {
                                st.source_user_agent.remove(&s2.id);
                            } else {
                                st.source_user_agent.insert(s2.id, r.user_agent.clone());
                            }
                        });
                        say2("Saving and downloading again…", false);
                        let (a3, sv3, say3, id) = (a2.clone(), sv2.clone(), say2.clone(), s2.id);
                        spawn(async move {
                            let repo = a3.repo.clone();
                            let res = bg(move || {
                                repo.update_source(
                                    id,
                                    &r.name,
                                    r.location.as_deref(),
                                    r.server.as_deref(),
                                    r.username.as_deref(),
                                    r.password.as_deref(),
                                )
                            })
                            .await;
                            match res {
                                Ok(()) => {
                                    let cur = a3.source.borrow().as_ref().map(|s| s.id);
                                    a3.reload_sources(cur).await;
                                    if let Some(sv) = sv3.upgrade() {
                                        sv.show("Sources");
                                    }
                                }
                                Err(e) => say3(&e.to_string(), true),
                            }
                        });
                    });
                    Ok(())
                })
            }),
        ));
        let s1 = src.clone();
        actions.append(&action(
            "madhyam-help-about-symbolic",
            "Playlist info",
            Rc::new(move |a, _| {
                let s = s1.clone();
                Box::pin(async move {
                    a.show_playlist_info(s).await;
                    Ok(())
                })
            }),
        ));
        if src.kind == "xtream" {
            let s1 = src.clone();
            actions.append(&action(
                "madhyam-dialog-password-symbolic",
                "Account info",
                Rc::new(move |a, _| {
                    let s = s1.clone();
                    Box::pin(async move {
                        a.show_account_info(s).await;
                        Ok(())
                    })
                }),
            ));
        }
        let (s1, sv1) = (src.clone(), sv.clone());
        actions.append(&action(
            "madhyam-edit-delete-symbolic",
            "Remove",
            Rc::new(move |a, _| {
                let (s, sv) = (s1.clone(), sv1.clone());
                Box::pin(async move {
                    a.remove_source(s).await;
                    if let Some(sv) = sv.upgrade() {
                        sv.show("Sources");
                    }
                    Ok(())
                })
            }),
        ));
        g.append(&actions);
        outer.append(&g);
        outer.append(&status);
        outer.upcast()
    }

    // ================================================================ dashboard

    fn dashboard_page(self: &Rc<Self>) {
        self.header("Dashboard", "Choose what the start page shows.");
        let s = settings::snapshot();
        let rows: [(&str, &str, bool, fn(&mut settings::AppSettings, bool)); 6] = [
            (
                "Banner",
                "A large banner with the film or episode you were watching, or shortcuts when there is none.",
                s.dash_hero,
                |s, v| s.dash_hero = v,
            ),
            (
                "Continue watching",
                "Films and episodes you started but did not finish.",
                s.dash_continue,
                |s, v| s.dash_continue = v,
            ),
            (
                "Favorite channels",
                "Your starred live channels with what is on now.",
                s.dash_live_favorites,
                |s, v| s.dash_live_favorites = v,
            ),
            (
                "Recently watched channels",
                "The live channels you opened last.",
                s.dash_recent_live,
                |s, v| s.dash_recent_live = v,
            ),
            (
                "Favorite movies and series",
                "",
                s.dash_favorite_vod,
                |s, v| s.dash_favorite_vod = v,
            ),
            (
                "Recently added",
                "The newest movies and series on your Xtream server.",
                s.dash_recently_added,
                |s, v| s.dash_recently_added = v,
            ),
        ];
        for (title, desc, value, set) in rows {
            self.row(
                title,
                desc,
                &toggle(value, move |v| settings::update(|s| set(s, v))),
                false,
            );
        }
    }

    // ================================================================ playback

    fn playback(self: &Rc<Self>, app: &Rc<App>) {
        self.header(
            "Playback",
            "How streams are opened and played (mpv engine).",
        );
        let s = settings::snapshot();
        self.row(
            "Live stream format",
            "Auto uses what the provider gives. If live channels stutter or fail, try the other format. Applies to Xtream channels.",
            &choice(&[("Auto", "Auto"), ("TS", "MPEG-TS"), ("HLS", "HLS (m3u8)")], &s.live_format, |v| settings::update(|s| s.live_format = v)),
            true,
        );
        self.row(
            "Network buffer",
            "More buffer means fewer freezes on a slow connection but a longer wait when switching channels.",
            &choice(&[("800", "0.8 s"), ("1500", "1.5 s"), ("3000", "3 s"), ("6000", "6 s")], &s.network_caching_ms.to_string(), |v| settings::update(|s| s.network_caching_ms = v.parse().unwrap_or(1500))),
            true,
        );
        self.row("Hardware decoding", "Uses the graphics card to decode video (lower CPU use, smoother 4K). Turn off if you see green or black frames.", &toggle(s.hardware_decoding, |v| settings::update(|s| s.hardware_decoding = v)), false);
        self.row(
            "Resume movies and episodes",
            "Continue where you stopped last time.",
            &toggle(s.resume_playback, |v| {
                settings::update(|s| s.resume_playback = v)
            }),
            false,
        );
        self.row(
            "Show subtitles by default",
            "When a film has subtitles, turn them on automatically.",
            &toggle(s.show_subtitles, |v| {
                settings::update(|s| s.show_subtitles = v)
            }),
            false,
        );
        let langs: [(&str, &str); 9] = [
            ("", "Stream default"),
            ("eng", "English"),
            ("fre", "French"),
            ("spa", "Spanish"),
            ("ger", "German"),
            ("ara", "Arabic"),
            ("hin", "Hindi"),
            ("por", "Portuguese"),
            ("tur", "Turkish"),
        ];
        let lang_flow = |current: &str, set: fn(&mut settings::AppSettings, String)| {
            let f = gtk::FlowBox::new();
            f.set_selection_mode(gtk::SelectionMode::None);
            f.set_max_children_per_line(5);
            f.set_halign(gtk::Align::Start);
            f.insert(
                &choice(&langs[..5], current, move |v| {
                    settings::update(|s| set(s, v))
                }),
                -1,
            );
            f.insert(
                &choice(&langs[5..], current, move |v| {
                    settings::update(|s| set(s, v))
                }),
                -1,
            );
            f
        };
        self.row(
            "Preferred audio language",
            "Chosen automatically when a stream has several audio tracks.",
            &lang_flow(&s.audio_language, |s, v| s.audio_language = v),
            true,
        );
        self.row(
            "Preferred subtitle language",
            "",
            &lang_flow(&s.subtitle_language, |s, v| s.subtitle_language = v),
            true,
        );

        // Online subtitles (VLsub-like search)
        let os = gtk::Grid::new();
        os.set_row_spacing(8);
        os.set_column_spacing(12);
        let pair = |row: i32, label: &str, w: &gtk::Widget| {
            let l = kit::text(label, 13.0, "muted", 400, false);
            l.set_size_request(150, -1);
            os.attach(&l, 0, row, 1, 1);
            os.attach(w, 1, row, 1, 1);
        };
        let key = entry(&s.os_api_key, |v| settings::update(|s| s.os_api_key = v));
        let user = entry(&s.os_username, |v| settings::update(|s| s.os_username = v));
        let pass = gtk::PasswordEntry::new();
        pass.set_show_peek_icon(true);
        pass.set_size_request(320, -1);
        pass.set_placeholder_text(Some("Saved in your keyring. Type to replace."));
        {
            let focus = gtk::EventControllerFocus::new();
            let pw = pass.downgrade();
            focus.connect_leave(move |_| {
                if let Some(p) = pw.upgrade() {
                    let t = p.text().to_string();
                    if !t.is_empty() {
                        std::thread::spawn(move || {
                            crate::secrets::store(crate::subtitles::KEYRING_KEY, &t)
                        });
                    }
                }
            });
            pass.add_controller(focus);
        }
        let langs_e = entry(&s.os_languages, |v| {
            settings::update(|s| s.os_languages = v)
        });
        pair(0, "API key", key.upcast_ref());
        pair(1, "Username (optional)", user.upcast_ref());
        pair(2, "Password (optional)", pass.upcast_ref());
        pair(3, "Languages", langs_e.upcast_ref());
        let osb = kit::vbox(10);
        osb.append(&os);
        let w = Rc::downgrade(app);
        let get_key = btn("Get a free API key", false, move || {
            if let Some(a) = w.upgrade() {
                super::dialogs::open_uri(&a.window, "https://www.opensubtitles.com/en/consumers");
            }
        });
        let (u2, p2) = (user.clone(), pass.clone());
        let forget = btn("Forget login", false, move || {
            settings::update(|s| {
                s.os_username.clear();
                s.os_password.clear();
            });
            u2.set_text("");
            p2.set_text("");
            std::thread::spawn(|| crate::secrets::delete(crate::subtitles::KEYRING_KEY));
        });
        osb.append(&buttons(&[&get_key, &forget]));
        self.row(
            "Online subtitles (OpenSubtitles.com)",
            "Search and download subtitles from the player's subtitles menu, like VLC's VLsub. Needs a free API key; logging in gives more downloads per day. Languages: comma-separated codes such as en, fr, es.",
            &osb,
            true,
        );
        let omdb = entry(&s.omdb_api_key, |v| {
            settings::update(|s| s.omdb_api_key = v)
        });
        let ob = kit::vbox(8);
        ob.append(&omdb);
        let w = Rc::downgrade(app);
        ob.append(&buttons(&[&btn("Get a free OMDb key", false, move || {
            if let Some(a) = w.upgrade() {
                super::dialogs::open_uri(&a.window, "https://www.omdbapi.com/apikey.aspx");
            }
        })]));
        self.row("IMDb and Rotten Tomatoes ratings", "Shown on movie and series pages when found. Uses OMDb (free key, 1,000 look-ups a day). Leave empty to hide them.", &ob, true);
        self.row("Keyboard", "Space or click the picture: pause · ←/→ 10 s · ↑/↓ volume · M mute · J/K audio delay · G/H subtitle delay · double-click or F11 full screen", &gtk::Box::new(gtk::Orientation::Horizontal, 0), false);
        self.content.append(&note(
            "Playback changes apply to the next channel or film you open.",
        ));
    }

    // ================================================================ guide

    fn guide(self: &Rc<Self>, app: &Rc<App>) {
        self.header("Programme guide", "The EPG shown next to live channels.");
        let s = settings::snapshot();
        let w = Rc::downgrade(app);
        self.row(
            "Guide view",
            "Timeline is a horizontal ribbon under the video (like a TV guide); List is a vertical list of today's programmes.",
            &choice(&[("Timeline", "Timeline"), ("List", "List")], &s.guide_view, move |v| {
                settings::update(|s| s.guide_view = v.clone());
                if let Some(a) = w.upgrade() {
                    a.refresh_guide_view();
                }
            }),
            true,
        );
        let label = kit::text("", 14.0, "", 400, false);
        label.set_size_request(70, -1);
        label.set_xalign(0.5);
        let show = {
            let label = label.clone();
            Rc::new(move || {
                let m = settings::get().epg_offset_minutes;
                label.set_text(&if m == 0 {
                    "0 min".into()
                } else {
                    format!("{m:+} min")
                });
            })
        };
        show();
        let offset = kit::hbox(4);
        for (text, delta) in [
            ("−1 h", -60),
            ("−15", -15),
            ("", 0),
            ("+15", 15),
            ("+1 h", 60),
        ] {
            if delta == 0 {
                offset.append(&label);
                continue;
            }
            let b = gtk::Button::with_label(text);
            b.add_css_class("chip-solid");
            let (show, w) = (show.clone(), Rc::downgrade(app));
            b.connect_clicked(move |_| {
                settings::update(|s| {
                    s.epg_offset_minutes = (s.epg_offset_minutes + delta).clamp(-720, 720)
                });
                show();
                if let Some(a) = w.upgrade() {
                    a.refresh_list_appearance();
                }
            });
            offset.append(&b);
        }
        self.row(
            "Time offset",
            "Shift programme times if your provider's guide is consistently early or late.",
            &offset,
            true,
        );
        self.row(
            "Refresh guides every",
            "Guides are downloaded in the background when they are older than this.",
            &choice(
                &[
                    ("6", "6 hours"),
                    ("12", "12 hours"),
                    ("24", "24 hours"),
                    ("72", "3 days"),
                ],
                &s.epg_refresh_hours.to_string(),
                |v| settings::update(|s| s.epg_refresh_hours = v.parse().unwrap_or(12)),
            ),
            true,
        );
        // Extra XMLTV sources
        let list = kit::vbox(6);
        let render: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let r: Rc<dyn Fn()> = {
            let (list, render) = (list.clone(), render.clone());
            Rc::new(move || {
                kit::clear(&list);
                let urls = settings::get().extra_epg_urls.clone();
                if urls.is_empty() {
                    list.append(&kit::text(
                        "No extra guide sources. Your playlists' own guides are always used.",
                        13.0,
                        "muted",
                        400,
                        true,
                    ));
                }
                for url in urls {
                    let row = kit::hbox(6);
                    let chip = kit::hbox(0);
                    chip.add_css_class("card");
                    chip.set_hexpand(true);
                    let t = kit::text(&url, 13.0, "", 400, false);
                    t.set_max_width_chars(1);
                    t.set_hexpand(true);
                    kit::margins(&t, 10, 7, 10, 7);
                    chip.append(&t);
                    row.append(&chip);
                    let (render, u) = (render.clone(), url.clone());
                    row.append(&kit::icon_button(
                        "madhyam-edit-delete-symbolic",
                        "Remove",
                        move || {
                            settings::update(|s| s.extra_epg_urls.retain(|x| x != &u));
                            if let Some(f) = render.borrow().clone() {
                                f();
                            }
                        },
                    ));
                    list.append(&row);
                }
            })
        };
        *render.borrow_mut() = Some(r.clone());
        r();
        let add_row = kit::hbox(8);
        add_row.set_margin_top(4);
        let input = gtk::Entry::new();
        input.set_hexpand(true);
        input.set_placeholder_text(Some(
            "https://example.org/guide.xml.gz  or  /home/me/epg/guide.xml",
        ));
        add_row.append(&input);
        let add = {
            let (input, r) = (input.clone(), r.clone());
            Rc::new(move || {
                let u = input.text().trim().to_string();
                if !u.is_empty() && !settings::get().extra_epg_urls.contains(&u) {
                    settings::update(|s| s.extra_epg_urls.push(u.clone()));
                    input.set_text("");
                    r();
                }
            })
        };
        let a2 = add.clone();
        input.connect_activate(move |_| a2());
        add_row.append(&btn("Add", false, move || add()));
        let sources = kit::vbox(6);
        sources.append(&list);
        sources.append(&add_row);
        sources.set_hexpand(true);
        self.row("Extra guide sources (XMLTV)", "Add XMLTV links (http/https, .xml or .xml.gz) or files on this computer. Useful when your provider's guide is missing channels.", &sources, true);

        let status = note("");
        status.set_visible(false);
        let (w, st) = (Rc::downgrade(app), status.clone());
        let refresh = btn("Refresh all guides now", true, move || {
            let Some(a) = w.upgrade() else { return };
            st.set_visible(true);
            st.set_text("Downloading guides…");
            let st = st.clone();
            spawn(async move {
                a.refresh_all_guides().await;
                st.set_text("Guides updated.");
            });
        });
        let (w, st) = (Rc::downgrade(app), status.clone());
        let clear = btn("Clear guide data", false, move || {
            let Some(a) = w.upgrade() else { return };
            let st = st.clone();
            spawn(async move {
                let repo = a.repo.clone();
                bgv(move || repo.clear_epg()).await;
                a.refresh_list_appearance();
                st.set_visible(true);
                st.set_text(
                    "Guide data cleared. Use “Refresh all guides now” to download it again.",
                );
            });
        });
        let actions = kit::vbox(0);
        actions.append(&buttons(&[&refresh, &clear]));
        actions.append(&status);
        self.add_row(&actions);
    }

    // ================================================================ data

    fn data(self: &Rc<Self>, app: &Rc<App>) {
        let name = crate::brand::NAME;
        self.header(
            "Backup & data",
            "Your favorites, watch progress and history.",
        );
        let status = note("");
        status.set_visible(false);
        let (w, st) = (Rc::downgrade(app), status.clone());
        let export = btn("Export…", false, move || {
            let Some(a) = w.upgrade() else { return };
            let st = st.clone();
            let dlg = gtk::FileDialog::builder()
                .title("Export backup")
                .initial_name(format!(
                    "{name}-backup-{}.json",
                    chrono::Local::now().format("%Y-%m-%d")
                ))
                .modal(true)
                .build();
            spawn(async move {
                let Ok(file) = dlg.save_future(Some(&a.window)).await else {
                    return;
                };
                let Some(path) = file.path() else { return };
                let repo = a.repo.clone();
                let p2 = path.clone();
                let r = bg(move || {
                    let text = repo.export_user_state()?;
                    std::fs::write(&p2, text)?;
                    Ok(())
                })
                .await;
                st.set_visible(true);
                st.set_text(&match r {
                    Ok(()) => format!("Saved to {}", path.display()),
                    Err(e) => format!("Could not export: {e}"),
                });
            });
        });
        let (w, st) = (Rc::downgrade(app), status.clone());
        let import = btn("Import…", false, move || {
            let Some(a) = w.upgrade() else { return };
            let st = st.clone();
            let dlg = gtk::FileDialog::builder()
                .title("Import backup")
                .modal(true)
                .build();
            spawn(async move {
                let Ok(file) = dlg.open_future(Some(&a.window)).await else {
                    return;
                };
                let Some(path) = file.path() else { return };
                let repo = a.repo.clone();
                let r = bg(move || {
                    let bytes = std::fs::read(&path)?;
                    repo.import_user_state(&bytes)
                })
                .await;
                st.set_visible(true);
                match r {
                    Ok(()) => {
                        a.reload_favorites().await;
                        st.set_text("Backup restored (merged with what you already had).");
                    }
                    Err(e) => st.set_text(&format!("Could not import: {e}")),
                }
            });
        });
        self.row("Backup", "Save favorites, resume positions and recently watched to a file, or restore them on another computer. Playlists and passwords are not included.", &buttons(&[&export, &import]), true);
        let w = Rc::downgrade(app);
        let folder = crate::paths::data_dir().to_path_buf();
        let open = btn("Open folder", false, move || {
            if let Some(a) = w.upgrade() {
                super::dialogs::open_folder(&a.window, &folder);
            }
        });
        let path_label = kit::text(
            &crate::paths::data_dir().display().to_string(),
            12.0,
            "muted",
            400,
            false,
        );
        path_label.set_selectable(true);
        let fb = kit::vbox(8);
        fb.append(&path_label);
        fb.append(&buttons(&[&open]));
        self.row(
            "Data folder",
            &format!("Where {name} keeps its catalog, guides, settings and logs."),
            &fb,
            true,
        );
        let w = Rc::downgrade(app);
        let reset = btn("Reset everything…", false, move || {
            let Some(a) = w.upgrade() else { return };
            spawn(async move {
                let ok = super::dialogs::confirm(&a.window, "This removes all playlists, favorites, watch progress, guides and settings from this computer.\n\nContinue?", "Reset").await;
                if ok {
                    a.reset_on_exit.set(true);
                    a.restart_app();
                }
            });
        });
        self.row(
            &format!("Reset {name}"),
            "Remove all playlists, favorites, progress, guides and settings, and start fresh.",
            &buttons(&[&reset]),
            true,
        );
        self.content.append(&status);
    }

    // ================================================================ about

    fn about(self: &Rc<Self>, app: &Rc<App>) {
        let b = crate::brand::NAME;
        self.header(
            "About",
            &format!(
                "{b} ({}) — a player for your own IPTV subscription.",
                crate::brand::NATIVE_NAME
            ),
        );
        let empty = || gtk::Box::new(gtk::Orientation::Horizontal, 0);
        self.row(
            "Version",
            &format!(
                "{b} {} for Linux · engine {} (Rust) · video by mpv",
                crate::brand::VERSION,
                Repo::new_version()
            ),
            &empty(),
            false,
        );
        let w = Rc::downgrade(app);
        self.row(
            "Source code and releases",
            "github.com/diwaskhadka/Madhyam",
            &buttons(&[&btn("Open on GitHub", false, move || {
                if let Some(a) = w.upgrade() {
                    super::dialogs::open_uri(
                        &a.window,
                        "https://github.com/diwaskhadka/Madhyam/releases",
                    );
                }
            })]),
            false,
        );
        self.row("Content", &format!("{b} ships with no channels of its own. It plays the Xtream Codes or M3U playlists you add from a provider you are entitled to use, and the optional Free TV list of public streams."), &empty(), false);
        self.row("Keyboard", "Ctrl+K search · F11 full screen · Space pause · PgUp/PgDn change channel · ←/→ seek 10 s · ↑/↓ volume · J/K audio delay · G/H subtitle delay · Esc back", &empty(), false);
    }
}

impl Repo {
    /// Engine version without a repository instance (About).
    pub fn new_version() -> String {
        crate::engine::Engine::version()
    }
}
