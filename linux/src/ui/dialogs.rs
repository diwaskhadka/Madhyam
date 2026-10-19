//! The small dialogs: add / edit a playlist, playlist and account info, manage categories,
//! the custom-theme colour picker and the online subtitle search. Twin of Dialogs.cs,
//! AddSourceWindow, ColorPickerWindow and SubtitleSearchWindow on Windows.

use super::kit;
use super::{bg, spawn};
use crate::repo::{Group, Source, group_digits};
use crate::settings;
use crate::subtitles::{Client, SubtitleHit};
use crate::theme;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// A modal window in the app's style; Esc closes it.
pub fn make(owner: &impl IsA<gtk::Window>, title: &str, width: i32) -> (gtk::Window, gtk::Box) {
    let w = gtk::Window::builder()
        .title(title)
        .transient_for(owner)
        .modal(true)
        .resizable(false)
        .default_width(width)
        .build();
    w.add_css_class("main-window");
    let root = kit::vbox(0);
    root.add_css_class("dialog-root");
    w.set_child(Some(&root));
    let keys = gtk::EventControllerKey::new();
    let ww = w.downgrade();
    keys.connect_key_pressed(move |_, k, _, _| {
        if k == gtk::gdk::Key::Escape {
            if let Some(w) = ww.upgrade() {
                w.close();
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    w.add_controller(keys);
    (w, root)
}

pub fn button(label: &str, primary: bool, f: impl Fn() + 'static) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class(if primary { "primary" } else { "secondary" });
    b.set_size_request(90, -1);
    b.connect_clicked(move |_| f());
    b
}

pub fn footer(buttons: &[&gtk::Button]) -> gtk::Box {
    let p = kit::hbox(10);
    p.set_halign(gtk::Align::End);
    p.set_margin_top(18);
    for b in buttons {
        p.append(*b);
    }
    p
}

pub fn field_label(t: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(t));
    l.add_css_class("field-label");
    l.set_xalign(0.0);
    l
}

fn title(t: &str) -> gtk::Label {
    kit::text(t, 21.0, "", 600, false)
}

/// Yes / no question; true when the user picked `yes`.
pub async fn confirm(owner: &impl IsA<gtk::Window>, message: &str, yes: &str) -> bool {
    let d = gtk::AlertDialog::builder()
        .message(crate::brand::NAME)
        .detail(message)
        .buttons(["Cancel", yes])
        .cancel_button(0)
        .default_button(1)
        .modal(true)
        .build();
    matches!(d.choose_future(Some(owner)).await, Ok(1))
}

/// Open a web page in the default browser.
pub fn open_uri(owner: &impl IsA<gtk::Window>, uri: &str) {
    let l = gtk::UriLauncher::new(uri);
    l.launch(Some(owner), gio::Cancellable::NONE, |r| {
        if let Err(e) = r {
            crate::log(&format!("could not open a link: {e}"));
        }
    });
}

/// Show a folder in the file manager.
pub fn open_folder(owner: &impl IsA<gtk::Window>, path: &std::path::Path) {
    let l = gtk::FileLauncher::new(Some(&gio::File::for_path(path)));
    l.launch(Some(owner), gio::Cancellable::NONE, |r| {
        if let Err(e) = r {
            crate::log(&format!("could not open the folder: {e}"));
        }
    });
}

// ------------------------------------------------------------------ info

/// Two-column read-only information (Playlist info, Account info).
pub fn info_window(owner: &impl IsA<gtk::Window>, heading: &str, rows: Vec<(String, String)>) {
    let (w, root) = make(owner, heading, 520);
    root.append(&title(heading));
    let card = gtk::Grid::new();
    card.add_css_class("panel");
    card.set_column_spacing(16);
    card.set_margin_top(14);
    for (i, (label, value)) in rows.iter().enumerate() {
        let l = kit::text(label, 13.0, "muted", 400, false);
        kit::margins(&l, 16, 8, 0, 8);
        l.set_size_request(150, -1);
        card.attach(&l, 0, i as i32, 1, 1);
        // Selectable, so a server address can be copied.
        let v = gtk::Label::new(Some(if value.is_empty() { "—" } else { value }));
        v.set_selectable(true);
        v.set_wrap(true);
        v.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        v.set_xalign(0.0);
        v.set_hexpand(true);
        v.set_max_width_chars(40);
        kit::margins(&v, 0, 8, 16, 8);
        card.attach(&v, 1, i as i32, 1, 1);
    }
    root.append(&card);
    let ww = w.clone();
    let close = button("Close", true, move || ww.close());
    root.append(&footer(&[&close]));
    // Otherwise the first selectable value takes focus and shows up selected.
    GtkWindowExt::set_focus(&w, Some(&close));
    w.present();
}

// ------------------------------------------------------------------ add / edit playlist

pub struct NewSource {
    pub xtream: bool,
    pub name: String,
    pub server: String,
    pub username: String,
    pub password: String,
    pub url: String,
}

pub fn add_source(owner: &gtk::ApplicationWindow, done: impl Fn(NewSource) + 'static) {
    let (w, root) = make(owner, "Add a playlist", 540);
    let t = kit::text("Add a playlist", 22.0, "", 600, false);
    t.set_margin_bottom(8);
    root.append(&t);
    let intro = kit::text(
        &format!(
            "{} plays your own Xtream Codes account or M3U playlist. It ships with no channels.",
            crate::brand::NAME
        ),
        13.0,
        "muted",
        400,
        true,
    );
    intro.set_margin_bottom(12);
    root.append(&intro);
    let mode = kit::hbox(20);
    mode.set_margin_bottom(12);
    let xt = gtk::CheckButton::with_label("Xtream Codes login");
    let m3u = gtk::CheckButton::with_label("M3U playlist URL");
    m3u.set_group(Some(&xt));
    xt.set_active(true);
    mode.append(&xt);
    mode.append(&m3u);
    root.append(&mode);
    root.append(&field_label("Name (optional)"));
    let name = gtk::Entry::new();
    root.append(&name);
    let xbox = kit::vbox(0);
    xbox.append(&field_label("Server, e.g. http://example.com:8080"));
    let server = gtk::Entry::new();
    xbox.append(&server);
    xbox.append(&field_label("Username"));
    let user = gtk::Entry::new();
    xbox.append(&user);
    xbox.append(&field_label("Password"));
    let pass = gtk::PasswordEntry::new();
    pass.set_show_peek_icon(true);
    xbox.append(&pass);
    root.append(&xbox);
    let mbox = kit::vbox(0);
    mbox.append(&field_label("Playlist URL (http://… .m3u) or a file"));
    let row = kit::hbox(8);
    let url = gtk::Entry::new();
    url.set_hexpand(true);
    row.append(&url);
    let browse = gtk::Button::with_label("Browse…");
    browse.add_css_class("secondary");
    row.append(&browse);
    mbox.append(&row);
    mbox.set_visible(false);
    root.append(&mbox);
    let error = kit::text("", 12.5, "error", 400, true);
    error.set_margin_top(8);
    root.append(&error);
    {
        let (xbox, mbox, xt2, error) = (xbox.clone(), mbox.clone(), xt.clone(), error.clone());
        xt.connect_toggled(move |_| {
            xbox.set_visible(xt2.is_active());
            mbox.set_visible(!xt2.is_active());
            // The message was about the other kind's fields.
            error.set_text("");
        });
    }
    {
        let (w2, url2) = (w.clone(), url.clone());
        browse.connect_clicked(move |_| {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Playlists"));
            filter.add_pattern("*.m3u");
            filter.add_pattern("*.m3u8");
            let filters = gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let dlg = gtk::FileDialog::builder()
                .title("Choose a playlist")
                .filters(&filters)
                .modal(true)
                .build();
            let (w3, url3) = (w2.clone(), url2.clone());
            spawn(async move {
                if let Ok(f) = dlg.open_future(Some(&w3)).await {
                    if let Some(p) = f.path() {
                        url3.set_text(&p.to_string_lossy());
                    }
                }
            });
        });
    }
    let done = Rc::new(done);
    let ok = {
        let (w, xt, name, server, user, pass, url, error) = (
            w.clone(),
            xt.clone(),
            name.clone(),
            server.clone(),
            user.clone(),
            pass.clone(),
            url.clone(),
            error.clone(),
        );
        move || {
            let xtream = xt.is_active();
            let msg = match () {
                _ if xtream && server.text().trim().is_empty() => "Enter the server address.",
                _ if xtream && user.text().trim().is_empty() => "Enter your username.",
                _ if !xtream && url.text().trim().is_empty() => {
                    "Enter a playlist URL or choose a file."
                }
                _ => "",
            };
            error.set_text(msg);
            if !msg.is_empty() {
                return;
            }
            let n = name.text().trim().to_string();
            done(NewSource {
                xtream,
                name: if n.is_empty() {
                    (if xtream { "Provider" } else { "Playlist" }).into()
                } else {
                    n
                },
                server: server.text().trim().to_string(),
                username: user.text().trim().to_string(),
                password: pass.text().to_string(),
                url: url.text().trim().to_string(),
            });
            w.close();
        }
    };
    let ok = Rc::new(ok);
    for e in [&name, &server, &user, &url] {
        let ok = ok.clone();
        e.connect_activate(move |_| ok());
    }
    let okp = ok.clone();
    pass.connect_activate(move |_| okp());
    let ww = w.clone();
    let add = button("Add playlist", true, move || ok());
    root.append(&footer(&[
        &button("Cancel", false, move || ww.close()),
        &add,
    ]));
    w.present();
    server.grab_focus();
}

pub struct EditResult {
    pub name: String,
    pub location: Option<String>,
    pub server: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub user_agent: String,
}

pub fn edit_source(
    owner: &gtk::ApplicationWindow,
    s: &Source,
    user_agent: &str,
    password: &str,
    done: impl Fn(EditResult) + 'static,
) {
    let (w, root) = make(owner, "Edit playlist", 540);
    root.append(&title("Edit playlist"));
    let kind = kit::text(s.kind_label(), 13.0, "muted", 400, false);
    kit::margins(&kind, 0, 4, 0, 4);
    root.append(&kind);
    root.append(&field_label("Title"));
    let name = gtk::Entry::new();
    name.set_text(&s.name);
    root.append(&name);
    let (mut server, mut user, mut pass, mut location) = (None, None, None, None);
    if s.kind == "xtream" {
        root.append(&field_label("Server URL"));
        let e = gtk::Entry::new();
        e.set_text(&s.location);
        root.append(&e);
        server = Some(e);
        root.append(&field_label("Username"));
        let e = gtk::Entry::new();
        e.set_text(&s.username);
        root.append(&e);
        user = Some(e);
        root.append(&field_label("Password"));
        let p = gtk::PasswordEntry::new();
        p.set_show_peek_icon(true);
        p.set_text(password);
        root.append(&p);
        pass = Some(p);
    } else {
        root.append(&field_label("Playlist URL or file"));
        let e = gtk::Entry::new();
        e.set_text(&s.location);
        root.append(&e);
        location = Some(e);
    }
    root.append(&field_label(
        "User agent (optional; some providers require a specific one)",
    ));
    let ua = gtk::Entry::new();
    ua.set_text(user_agent);
    root.append(&ua);
    let error = kit::text("", 12.5, "error", 400, true);
    error.set_margin_top(10);
    root.append(&error);
    let note = kit::text(
        "Saving re-downloads the playlist. Favorites and progress are kept.",
        12.5,
        "muted",
        400,
        true,
    );
    note.set_margin_top(6);
    root.append(&note);
    let old_name = s.name.clone();
    let (w2, err2) = (w.clone(), error.clone());
    let save = button("Save", true, move || {
        if server.as_ref().is_some_and(|e| e.text().trim().is_empty()) {
            return err2.set_text("Enter the server address.");
        }
        if user.as_ref().is_some_and(|e| e.text().trim().is_empty()) {
            return err2.set_text("Enter the username.");
        }
        if location
            .as_ref()
            .is_some_and(|e| e.text().trim().is_empty())
        {
            return err2.set_text("Enter a playlist URL or file.");
        }
        let n = name.text().trim().to_string();
        done(EditResult {
            name: if n.is_empty() { old_name.clone() } else { n },
            location: location.as_ref().map(|e| e.text().trim().to_string()),
            server: server.as_ref().map(|e| e.text().trim().to_string()),
            username: user.as_ref().map(|e| e.text().trim().to_string()),
            password: pass.as_ref().map(|e| e.text().to_string()),
            user_agent: ua.text().trim().to_string(),
        });
        w2.close();
    });
    let ww = w.clone();
    root.append(&footer(&[
        &button("Cancel", false, move || ww.close()),
        &save,
    ]));
    w.set_default_widget(Some(&save));
    w.present();
}

// ------------------------------------------------------------------ manage categories

/// Show / hide categories. `done` gets the new list of hidden names.
pub fn manage_categories(
    owner: &gtk::ApplicationWindow,
    groups: Vec<Group>,
    hidden: Vec<String>,
    noun: &str,
    done: impl Fn(Vec<String>) + 'static,
) {
    let (w, root) = make(owner, &format!("Manage {noun}"), 560);
    root.append(&title(&format!("Manage {noun}")));
    let sub = kit::text(
        &format!(
            "Unticked {noun} are hidden from the list and from “All items”{}",
            if noun == "categories" {
                " and from search."
            } else {
                "."
            }
        ),
        13.0,
        "muted",
        400,
        true,
    );
    kit::margins(&sub, 0, 4, 0, 12);
    root.append(&sub);
    let filter = gtk::SearchEntry::new();
    filter.set_placeholder_text(Some(&format!("Search {noun}")));
    root.append(&filter);
    let checks: Vec<(String, gtk::CheckButton)> = groups
        .iter()
        .map(|g| {
            let c =
                gtk::CheckButton::with_label(&format!("{}   ({})", g.name, group_digits(g.count)));
            c.set_active(!hidden.contains(&g.name));
            kit::margins(&c, 2, 3, 2, 3);
            (g.name.clone(), c)
        })
        .collect();
    let checks = Rc::new(checks);
    let count = kit::text("", 12.5, "muted", 400, false);
    let recount = {
        let (checks, count) = (checks.clone(), count.clone());
        Rc::new(move || {
            count.set_text(&format!(
                "{} of {} shown",
                group_digits(checks.iter().filter(|c| c.1.is_active()).count() as i64),
                group_digits(checks.len() as i64)
            ))
        })
    };
    recount();
    let list = kit::vbox(0);
    for (_, c) in checks.iter() {
        let r = recount.clone();
        c.connect_toggled(move |_| r());
        list.append(c);
    }
    {
        let checks = checks.clone();
        filter.connect_search_changed(move |f| {
            let q = f.text().trim().to_lowercase();
            for (n, c) in checks.iter() {
                c.set_visible(q.is_empty() || n.to_lowercase().contains(&q));
            }
        });
    }
    let tools = kit::hbox(8);
    kit::margins(&tools, 0, 12, 0, 8);
    count.set_hexpand(true);
    tools.append(&count);
    for (label, on) in [("Select all", true), ("Deselect all", false)] {
        let b = gtk::Button::with_label(label);
        b.add_css_class("chip-solid");
        let checks = checks.clone();
        b.connect_clicked(move |_| {
            for (_, c) in checks.iter().filter(|c| c.1.is_visible()) {
                c.set_active(on);
            }
        });
        tools.append(&b);
    }
    root.append(&tools);
    let sw = gtk::ScrolledWindow::new();
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sw.set_child(Some(&list));
    sw.set_size_request(-1, 380);
    sw.add_css_class("panel");
    list.set_margin_start(12);
    list.set_margin_top(6);
    root.append(&sw);
    let ww = w.clone();
    let cancel = button("Cancel", false, move || ww.close());
    let (w2, checks2) = (w.clone(), checks.clone());
    let save = button("Save", true, move || {
        done(
            checks2
                .iter()
                .filter(|c| !c.1.is_active())
                .map(|c| c.0.clone())
                .collect(),
        );
        w2.close();
    });
    root.append(&footer(&[&cancel, &save]));
    w.present();
    filter.grab_focus();
}

// ------------------------------------------------------------------ colour picker

/// Settings › Visual theme › Custom: one colour (presets, hue/saturation/lightness or hex)
/// and a dark or light base. `done` gets (hex, light).
pub fn color_picker(
    owner: &gtk::ApplicationWindow,
    color: &str,
    light: bool,
    done: impl Fn(String, bool) + 'static,
) {
    let (w, root) = make(owner, "Custom theme colour", 460);
    let current = Rc::new(Cell::new(
        theme::parse(color).unwrap_or_else(|| theme::parse(theme::CRIMSON).unwrap()),
    ));
    let is_light = Rc::new(Cell::new(light));
    let updating = Rc::new(Cell::new(false));

    // Preview: a mini window in the palette this colour produces (its own stylesheet).
    let preview_css = gtk::CssProvider::new();
    let preview = kit::hbox(10);
    preview.add_css_class("pv-bg");
    preview.set_size_request(-1, 92);
    let rail = kit::vbox(0);
    rail.add_css_class("pv-rail");
    rail.set_size_request(34, -1);
    preview.append(&rail);
    let card = kit::vbox(0);
    card.add_css_class("pv-card");
    card.set_hexpand(true);
    let pt = kit::text("Live TV", 15.0, "pv-fg", 600, false);
    let pm = kit::text("Now: Evening News", 12.0, "pv-muted", 400, false);
    let chip = gtk::Label::new(Some("Watch"));
    chip.add_css_class("pv-chip");
    chip.set_halign(gtk::Align::Start);
    chip.set_margin_top(8);
    card.append(&pt);
    card.append(&pm);
    card.append(&chip);
    preview.append(&card);
    for wdg in [
        preview.upcast_ref::<gtk::Widget>(),
        rail.upcast_ref(),
        card.upcast_ref(),
        pt.upcast_ref(),
        pm.upcast_ref(),
        chip.upcast_ref(),
    ] {
        #[allow(deprecated)]
        wdg.style_context()
            .add_provider(&preview_css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    }
    root.append(&preview);
    let name = kit::text("", 14.0, "", 600, false);
    kit::margins(&name, 2, 8, 0, 0);
    root.append(&name);

    root.append(&field_label("Presets"));
    let swatches = gtk::FlowBox::new();
    swatches.set_selection_mode(gtk::SelectionMode::None);
    swatches.set_max_children_per_line(12);
    root.append(&swatches);
    let slider = |label: &str, max: f64| {
        root.append(&field_label(label));
        let s = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, max, 1.0);
        s.set_draw_value(false);
        s.add_css_class("theme");
        root.append(&s);
        s
    };
    let hue = slider("Hue", 359.0);
    let sat = slider("Saturation", 100.0);
    let lig = slider("Lightness", 100.0);
    root.append(&field_label("Hex code"));
    let hex = gtk::Entry::new();
    hex.set_max_width_chars(10);
    hex.set_halign(gtk::Align::Start);
    root.append(&hex);
    root.append(&field_label("Base"));
    let refresh: Rc<RefCell<Option<Rc<dyn Fn(bool, bool)>>>> = Rc::new(RefCell::new(None));
    let base = {
        let (il, r) = (is_light.clone(), refresh.clone());
        kit::segments(
            &[("dark", "☾  Dark"), ("light", "☀  Light")],
            if light { "light" } else { "dark" },
            move |v| {
                il.set(v == "light");
                if let Some(f) = r.borrow().clone() {
                    f(false, false);
                }
            },
        )
    };
    root.append(&base);

    let f: Rc<dyn Fn(bool, bool)> = {
        let (current, is_light, updating) = (current.clone(), is_light.clone(), updating.clone());
        let (hue, sat, lig, hex, name, preview_css) = (
            hue.clone(),
            sat.clone(),
            lig.clone(),
            hex.clone(),
            name.clone(),
            preview_css.clone(),
        );
        Rc::new(move |from_sliders: bool, from_hex: bool| {
            if updating.get() {
                return;
            }
            updating.set(true);
            if from_sliders {
                current.set(theme::from_hsl(
                    hue.value(),
                    sat.value() / 100.0,
                    lig.value() / 100.0,
                ));
            }
            let (h, s, l) = theme::to_hsl(current.get());
            if !from_sliders {
                hue.set_value(h);
                sat.set_value(s * 100.0);
                lig.set_value(l * 100.0);
            }
            let code = theme::hex(current.get());
            if !from_hex {
                hex.set_text(&code);
            }
            let n = theme::name_of(&code);
            name.set_text(&if n.starts_with('#') {
                n
            } else {
                format!("{n} · {code}")
            });
            let p = theme::custom(&code, is_light.get());
            let c = |k: &str| theme::css_color(&p[k]);
            preview_css.load_from_string(&format!(
                ".pv-bg {{ background-color: {}; border-radius: 12px; padding: 12px; }}
                 .pv-rail {{ background-color: {}; border-radius: 8px; }}
                 .pv-card {{ background-color: {}; border: 1.5px solid {}; border-radius: 8px; padding: 8px 12px; }}
                 .pv-fg {{ color: {}; }} .pv-muted {{ color: {}; }}
                 .pv-chip {{ background-color: {}; color: {}; border-radius: 10px; padding: 3px 10px; font-weight: 600; }}",
                c("Bg"), c("Rail"), c("Card"), c("Accent"), c("Fg"), c("Muted"), c("Accent"), c(if is_light.get() { "PrimaryFg" } else { "Bg" })
            ));
            updating.set(false);
        })
    };
    *refresh.borrow_mut() = Some(f.clone());
    for (pname, phex) in theme::PRESETS {
        let sw = gtk::Button::new();
        sw.add_css_class("bare");
        let dot = gtk::Label::new(None);
        dot.set_markup(&format!(
            "<span foreground='{phex}' size='xx-large'>●</span>"
        ));
        sw.set_child(Some(&dot));
        sw.set_tooltip_text(Some(pname));
        let (cur, f) = (current.clone(), f.clone());
        let c = theme::parse(phex).unwrap();
        sw.connect_clicked(move |_| {
            cur.set(c);
            f(false, false);
        });
        swatches.insert(&sw, -1);
    }
    for s in [&hue, &sat, &lig] {
        let f = f.clone();
        s.connect_value_changed(move |_| f(true, false));
    }
    {
        let (cur, f, upd) = (current.clone(), f.clone(), updating.clone());
        hex.connect_changed(move |e| {
            let t = e.text();
            if upd.get() || t.trim().trim_start_matches('#').len() != 6 {
                return;
            }
            if let Some(c) = theme::parse(&t) {
                cur.set(c);
                f(false, true);
            }
        });
    }
    f(false, false);
    let ww = w.clone();
    let cancel = button("Cancel", false, move || ww.close());
    let (w2, cur, il) = (w.clone(), current.clone(), is_light.clone());
    let use_it = button("Use this theme", true, move || {
        done(theme::hex(cur.get()), il.get());
        w2.close();
    });
    root.append(&footer(&[&cancel, &use_it]));
    w.present();
}

// ------------------------------------------------------------------ subtitle search

/// VLsub-like search: title, season/episode, languages → list → download and use.
pub fn subtitle_search(
    owner: &gtk::ApplicationWindow,
    query: &str,
    season: Option<i32>,
    episode: Option<i32>,
    done: impl Fn(String) + 'static,
) {
    if settings::get().os_api_key.trim().is_empty() {
        let owner = owner.clone();
        spawn(async move {
            let open = confirm(
                &owner,
                "Online subtitle search uses OpenSubtitles.com, which needs a free API key.\n\n1. Create an account at opensubtitles.com\n2. Open “API consumers” in your profile and create a key\n3. Paste it in Settings › Playback › Online subtitles\n\nOpen the OpenSubtitles website now?",
                "Open website",
            )
            .await;
            if open {
                open_uri(&owner, "https://www.opensubtitles.com/en/consumers");
            }
        });
        return;
    }
    let (w, root) = make(owner, "Search subtitles", 680);
    root.append(&title("Search subtitles"));
    let sub = kit::text(
        "From OpenSubtitles.com. Pick one to download it and turn it on.",
        13.0,
        "muted",
        400,
        false,
    );
    kit::margins(&sub, 0, 4, 0, 10);
    root.append(&sub);
    let form = kit::hbox(8);
    let field = |label: &str, e: &gtk::Entry, width: i32, expand: bool| {
        let b = kit::vbox(0);
        b.append(&field_label(label));
        e.set_size_request(width, -1);
        b.append(e);
        b.set_hexpand(expand);
        b
    };
    let t = gtk::Entry::new();
    t.set_text(query);
    let se = gtk::Entry::new();
    se.set_text(&season.map(|s| s.to_string()).unwrap_or_default());
    let ep = gtk::Entry::new();
    ep.set_text(&episode.map(|e| e.to_string()).unwrap_or_default());
    let langs = gtk::Entry::new();
    langs.set_text(&settings::get().os_languages);
    langs.set_tooltip_text(Some("Comma-separated codes: en, fr, es, de, ar, hi, pt…"));
    form.append(&field("Title", &t, 200, true));
    form.append(&field("Season", &se, 60, false));
    form.append(&field("Episode", &ep, 60, false));
    form.append(&field("Languages", &langs, 100, false));
    let search_btn = button("Search", true, || {});
    search_btn.set_valign(gtk::Align::End);
    form.append(&search_btn);
    root.append(&form);
    let status = kit::text("", 12.5, "muted", 400, true);
    kit::margins(&status, 0, 12, 0, 6);
    root.append(&status);
    let list = gtk::ListBox::new();
    list.add_css_class("plain");
    list.set_activate_on_single_click(false);
    let sw = gtk::ScrolledWindow::new();
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sw.set_child(Some(&list));
    sw.set_size_request(-1, 330);
    sw.add_css_class("panel");
    root.append(&sw);
    let hits: Rc<RefCell<Vec<SubtitleHit>>> = Rc::new(RefCell::new(Vec::new()));
    let client = Arc::new(Mutex::new(Client::new()));
    let use_btn = button("Download and use", true, || {});
    use_btn.set_sensitive(false);
    let ww = w.clone();
    root.append(&footer(&[
        &button("Close", false, move || ww.close()),
        &use_btn,
    ]));

    let do_search: Rc<dyn Fn()> = {
        let (t, se, ep, langs, status, list, hits, client, use_btn) = (
            t.clone(),
            se.clone(),
            ep.clone(),
            langs.clone(),
            status.clone(),
            list.clone(),
            hits.clone(),
            client.clone(),
            use_btn.clone(),
        );
        Rc::new(move || {
            settings::update(|s| s.os_languages = langs.text().trim().to_string());
            status.set_text("Searching…");
            list.remove_all();
            use_btn.set_sensitive(false);
            let (q, l, s, e) = (
                t.text().trim().to_string(),
                langs.text().to_string(),
                se.text().trim().parse::<i32>().ok(),
                ep.text().trim().parse::<i32>().ok(),
            );
            let (client, status, list, hits) =
                (client.clone(), status.clone(), list.clone(), hits.clone());
            spawn(async move {
                let r = bg(move || {
                    client
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .search(&q, &l, s, e)
                })
                .await;
                match r {
                    Ok(mut found) => {
                        found.sort_by_key(|h| std::cmp::Reverse(h.downloads));
                        status.set_text(&if found.is_empty() {
                            "No subtitles found. Try a shorter title or another language.".into()
                        } else {
                            format!("{} results, most downloaded first.", found.len())
                        });
                        for h in &found {
                            let g = kit::hbox(10);
                            kit::margins(&g, 8, 6, 8, 6);
                            let lang = kit::badge(&h.language.to_uppercase(), "accent");
                            lang.set_valign(gtk::Align::Center);
                            lang.set_size_request(36, -1);
                            g.append(&lang);
                            let tx = kit::vbox(0);
                            tx.set_hexpand(true);
                            let rel = kit::text(
                                if h.release.is_empty() {
                                    &h.file_name
                                } else {
                                    &h.release
                                },
                                13.0,
                                "",
                                600,
                                false,
                            );
                            rel.set_max_width_chars(1);
                            tx.append(&rel);
                            let ti = kit::text(
                                &format!(
                                    "{}{}",
                                    h.title,
                                    if h.hearing_impaired {
                                        "  ·  hearing impaired"
                                    } else {
                                        ""
                                    }
                                ),
                                11.5,
                                "muted",
                                400,
                                false,
                            );
                            ti.set_max_width_chars(1);
                            tx.append(&ti);
                            g.append(&tx);
                            g.append(&kit::text(
                                &format!("↓ {}", group_digits(h.downloads)),
                                12.0,
                                "muted",
                                400,
                                false,
                            ));
                            list.append(&g);
                        }
                        *hits.borrow_mut() = found;
                    }
                    Err(e) => status.set_text(&e.to_string()),
                }
            });
        })
    };
    let done = Rc::new(done);
    let do_use: Rc<dyn Fn()> = {
        let (list, hits, client, status, w, done) = (
            list.clone(),
            hits.clone(),
            client.clone(),
            status.clone(),
            w.clone(),
            done.clone(),
        );
        Rc::new(move || {
            let Some(row) = list.selected_row() else {
                return;
            };
            let Some(h) = hits.borrow().get(row.index() as usize).cloned() else {
                return;
            };
            status.set_text("Downloading…");
            let (client, status, w, done) =
                (client.clone(), status.clone(), w.clone(), done.clone());
            spawn(async move {
                match bg(move || {
                    client
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .download(&h)
                })
                .await
                {
                    Ok((path, remaining)) => {
                        status.set_text(&format!("Downloaded. Downloads left today: {remaining}."));
                        done(path);
                        w.close();
                    }
                    Err(e) => status.set_text(&e.to_string()),
                }
            });
        })
    };
    {
        let f = do_search.clone();
        search_btn.connect_clicked(move |_| f());
        let f = do_search.clone();
        t.connect_activate(move |_| f());
    }
    {
        let u = use_btn.clone();
        list.connect_row_selected(move |_, r| u.set_sensitive(r.is_some()));
        let f = do_use.clone();
        list.connect_row_activated(move |_, _| f());
        let f = do_use.clone();
        use_btn.connect_clicked(move |_| f());
    }
    w.present();
    if !query.is_empty() {
        do_search();
    }
}
