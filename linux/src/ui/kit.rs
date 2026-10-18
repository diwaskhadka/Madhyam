//! Small builders for the code-built pages, the twin of UiKit.cs: text, buttons, chips,
//! poster and channel cards, rails, segmented choices and pop-up menus. Colours come from the
//! theme stylesheet (theme.rs), so every theme works.

use crate::images;
use crate::repo::{Item, Programme};
use gtk::glib;
use gtk::pango;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

// ------------------------------------------------------------------ text and icons

/// A label: `px` is the WPF font size (device-independent pixels), `class` a colour class
/// ("muted", "accent"…) or "" for the normal text colour; `weight` 400/600/700.
pub fn text(t: &str, px: f64, class: &str, weight: u16, wrap: bool) -> gtk::Label {
    let l = gtk::Label::new(Some(t));
    l.set_xalign(0.0);
    let attrs = pango::AttrList::new();
    attrs.insert(pango::AttrSize::new_size_absolute(
        (px * pango::SCALE as f64) as i32,
    ));
    if weight != 400 {
        attrs.insert(pango::AttrInt::new_weight(match weight {
            w if w >= 800 => pango::Weight::Heavy,
            w if w >= 700 => pango::Weight::Bold,
            w if w >= 600 => pango::Weight::Semibold,
            _ => pango::Weight::Normal,
        }));
    }
    l.set_attributes(Some(&attrs));
    if wrap {
        l.set_wrap(true);
        l.set_wrap_mode(pango::WrapMode::WordChar);
    } else {
        l.set_ellipsize(pango::EllipsizeMode::End);
    }
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}

/// Wrapped text limited to `lines` lines.
pub fn lines(t: &str, px: f64, class: &str, n: i32) -> gtk::Label {
    let l = text(t, px, class, 400, true);
    l.set_lines(n);
    l.set_ellipsize(pango::EllipsizeMode::End);
    l
}

pub fn icon(name: &str, px: i32) -> gtk::Image {
    let i = gtk::Image::from_icon_name(name);
    i.set_pixel_size(px);
    i
}

pub fn hbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Horizontal, spacing)
}

pub fn vbox(spacing: i32) -> gtk::Box {
    gtk::Box::new(gtk::Orientation::Vertical, spacing)
}

pub fn margins(w: &impl IsA<gtk::Widget>, l: i32, t: i32, r: i32, b: i32) {
    w.set_margin_start(l);
    w.set_margin_top(t);
    w.set_margin_end(r);
    w.set_margin_bottom(b);
}

pub fn spacer() -> gtk::Box {
    let b = hbox(0);
    b.set_hexpand(true);
    b
}

pub fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

pub fn line() -> gtk::Box {
    let b = hbox(0);
    b.add_css_class("separator-line");
    b
}

// ------------------------------------------------------------------ buttons

pub fn icon_button(name: &str, tooltip: &str, f: impl Fn() + 'static) -> gtk::Button {
    let b = gtk::Button::new();
    b.set_child(Some(&icon(name, 16)));
    b.add_css_class("icon-button");
    b.set_tooltip_text(Some(tooltip));
    b.set_valign(gtk::Align::Center);
    b.connect_clicked(move |_| f());
    b
}

/// Filled (primary) or tonal (secondary) button with an optional leading icon.
pub fn primary(
    label: &str,
    icon_name: Option<&str>,
    f: impl Fn() + 'static,
    secondary: bool,
) -> gtk::Button {
    let b = gtk::Button::new();
    let row = hbox(10);
    if let Some(i) = icon_name {
        row.append(&icon(i, 16));
    }
    row.append(&gtk::Label::new(Some(label)));
    b.set_child(Some(&row));
    b.add_css_class(if secondary { "secondary" } else { "primary" });
    b.connect_clicked(move |_| f());
    b
}

/// A button that is just its content (cards draw their own hover border).
pub fn bare(child: &impl IsA<gtk::Widget>, f: impl Fn() + 'static) -> gtk::Button {
    let b = gtk::Button::new();
    b.set_child(Some(child));
    b.add_css_class("bare");
    b.set_cursor_from_name(Some("pointer"));
    b.set_focus_on_click(false);
    // Cards keep their own width: an expanding label inside must not stretch them.
    b.set_hexpand(false);
    b.connect_clicked(move |_| f());
    b
}

pub fn chip(t: &str, class: &str) -> gtk::Box {
    let b = hbox(0);
    b.add_css_class("chip");
    b.append(&text(t, 13.0, class, 400, false));
    b
}

pub fn badge(t: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(t));
    l.add_css_class("badge");
    if !class.is_empty() {
        l.add_css_class(class);
    }
    l
}

pub fn count_pill(n: i64) -> gtk::Label {
    let l = gtk::Label::new(Some(&crate::repo::group_digits(n)));
    l.add_css_class("count-pill");
    l.set_valign(gtk::Align::Center);
    l
}

pub fn progress(fraction: f64) -> gtk::ProgressBar {
    let p = gtk::ProgressBar::new();
    p.add_css_class("thin");
    p.set_fraction(fraction.clamp(0.0, 1.0));
    p.set_valign(gtk::Align::Center);
    p
}

// ------------------------------------------------------------------ cards

/// Rounded picture with a placeholder icon underneath (shown until the image loads).
pub fn picture_frame(
    url: Option<&str>,
    w: i32,
    h: i32,
    placeholder: &str,
    contain: bool,
    class: &str,
) -> (gtk::Overlay, gtk::Overlay) {
    let frame = gtk::Overlay::new();
    frame.add_css_class(class);
    frame.set_size_request(w, h);
    frame.set_overflow(gtk::Overflow::Hidden);
    let ph = icon(placeholder, 30);
    ph.add_css_class("muted");
    ph.set_halign(gtk::Align::Center);
    ph.set_valign(gtk::Align::Center);
    frame.set_child(Some(&ph));
    let pic = gtk::Picture::new();
    pic.set_content_fit(if contain {
        gtk::ContentFit::Contain
    } else {
        gtk::ContentFit::Cover
    });
    pic.set_can_shrink(true);
    pic.set_visible(false);
    if contain {
        margins(&pic, 18, 18, 18, 18);
    }
    frame.add_overlay(&pic);
    images::set_picture(&pic, url, (w as f64 * 1.5) as i32);
    let outer = gtk::Overlay::new();
    outer.set_child(Some(&frame));
    (outer, frame)
}

pub const POSTER_W: i32 = 150;
pub const POSTER_H: i32 = 222;

/// Poster card: picture, optional type badge, rating, progress, title, subtitle, optional ✕.
pub fn poster(
    it: &Item,
    open: Rc<dyn Fn(Item)>,
    subtitle: Option<String>,
    type_badge: bool,
    remove: Option<Rc<dyn Fn(Item)>>,
    w: i32,
    h: i32,
) -> gtk::Widget {
    let placeholder = if it.is_live() {
        "tv-symbolic"
    } else {
        "video-x-generic-symbolic"
    };
    let (img, frame) = picture_frame(
        it.logo.as_deref(),
        w,
        h,
        placeholder,
        it.is_live(),
        "poster-frame",
    );
    if type_badge {
        let b = badge(
            it.type_badge(),
            if it.is_live() {
                "live-red"
            } else if it.is_series() {
                "rating"
            } else {
                "accent"
            },
        );
        b.set_halign(gtk::Align::Start);
        b.set_valign(gtk::Align::Start);
        margins(&b, 7, 7, 7, 7);
        img.add_overlay(&b);
    }
    if it.has_rating() && !it.is_live() {
        let b = badge(
            &format!("★ {}", it.rating.as_deref().unwrap_or("")),
            "rating",
        );
        b.set_halign(gtk::Align::End);
        b.set_valign(gtk::Align::Start);
        margins(&b, 7, 7, if remove.is_some() { 36 } else { 7 }, 7);
        img.add_overlay(&b);
    }
    if it.duration > 0.0 && it.watched_ratio() > 0.0 {
        let p = progress(it.watched_ratio());
        p.set_valign(gtk::Align::End);
        margins(&p, 8, 0, 8, 8);
        img.add_overlay(&p);
    }
    if let Some(rm) = remove {
        let it2 = it.clone();
        let x = icon_button("window-close-symbolic", "Remove", move || rm(it2.clone()));
        x.add_css_class("small");
        x.add_css_class("badge");
        x.set_halign(gtk::Align::End);
        x.set_valign(gtk::Align::Start);
        margins(&x, 5, 5, 5, 5);
        img.add_overlay(&x);
    }
    let _ = frame;
    let stack = vbox(0);
    stack.set_size_request(w, -1);
    stack.append(&img);
    let title = text(&it.display_name(), 13.0, "", 600, false);
    title.set_max_width_chars(1);
    margins(&title, 2, 8, 2, 0);
    stack.append(&title);
    if let Some(sub) = subtitle.filter(|s| !s.is_empty()) {
        let l = text(&sub, 11.5, "muted", 400, false);
        l.set_max_width_chars(1);
        margins(&l, 2, 2, 2, 0);
        stack.append(&l);
    }
    let it2 = it.clone();
    let card = bare(&stack, move || open(it2.clone()));
    card.set_tooltip_text(Some(&it.name));
    card.set_valign(gtk::Align::Start);
    margins(&card, 0, 0, 16, 18);
    card.upcast()
}

/// Wide live-channel card: logo, name, what is on now with progress.
pub fn live_card(
    it: &Item,
    now: Option<&Programme>,
    open: Rc<dyn Fn(Item)>,
    remove: Option<Rc<dyn Fn(Item)>>,
) -> gtk::Widget {
    let g = hbox(10);
    g.set_size_request(280, -1);
    let logo = logo_box(it, 48);
    g.append(&logo);
    let info = vbox(0);
    info.set_hexpand(true);
    info.set_valign(gtk::Align::Center);
    let name = text(&it.display_name(), 13.5, "", 600, false);
    name.set_max_width_chars(1);
    info.append(&name);
    let nowt = text(
        now.map(|p| p.title.as_str())
            .unwrap_or("No programme information"),
        12.0,
        "muted",
        400,
        false,
    );
    nowt.set_max_width_chars(1);
    info.append(&nowt);
    if let Some(p) = now {
        let row = hbox(6);
        margins(&row, 0, 4, 0, 0);
        row.append(&text(
            &crate::repo::clock(p.start),
            10.5,
            "muted",
            400,
            false,
        ));
        let bar = progress(p.progress(crate::repo::now_unix()));
        bar.set_hexpand(true);
        row.append(&bar);
        row.append(&text(
            &crate::repo::clock(p.stop),
            10.5,
            "muted",
            400,
            false,
        ));
        info.append(&row);
    }
    g.append(&info);
    if let Some(rm) = remove {
        let it2 = it.clone();
        let x = icon_button("window-close-symbolic", "Remove", move || rm(it2.clone()));
        x.add_css_class("small");
        g.append(&x);
    }
    let border = hbox(0);
    border.add_css_class("live-card");
    border.append(&g);
    let it2 = it.clone();
    let card = bare(&border, move || open(it2.clone()));
    card.set_tooltip_text(Some(&it.name));
    margins(&card, 0, 0, 12, 12);
    card.set_valign(gtk::Align::Start);
    card.upcast()
}

/// Channel logo on a chip, initials until the picture loads.
pub fn logo_box(it: &Item, size: i32) -> gtk::Overlay {
    let o = gtk::Overlay::new();
    o.add_css_class("logo-box");
    o.set_size_request(size, size);
    o.set_valign(gtk::Align::Center);
    o.set_overflow(gtk::Overflow::Hidden);
    let init = text(&it.initials(), 13.0, "muted", 400, false);
    init.set_halign(gtk::Align::Center);
    init.set_xalign(0.5);
    o.set_child(Some(&init));
    let pic = gtk::Picture::new();
    pic.set_content_fit(gtk::ContentFit::Contain);
    pic.set_can_shrink(true);
    pic.set_visible(false);
    margins(&pic, 3, 3, 3, 3);
    o.add_overlay(&pic);
    images::set_picture(&pic, it.logo.as_deref(), size * 2);
    o
}

// ------------------------------------------------------------------ layout

/// Section heading with a count pill and an optional "See all ›" link.
pub fn section_header(
    title: &str,
    count: Option<i64>,
    see_all: Option<Rc<dyn Fn()>>,
    see_all_label: &str,
) -> gtk::Box {
    let g = hbox(10);
    margins(&g, 0, 26, 0, 12);
    let t = text(title, 19.0, "", 600, false);
    g.append(&t);
    if let Some(c) = count {
        g.append(&count_pill(c));
    }
    g.append(&spacer());
    if let Some(f) = see_all {
        let b = gtk::Button::with_label(&format!("{see_all_label}  ›"));
        b.add_css_class("link");
        b.connect_clicked(move |_| f());
        g.append(&b);
    }
    g
}

/// The nearest scrolling page above a widget.
pub fn page_scroller(w: &gtk::Widget) -> Option<gtk::ScrolledWindow> {
    let mut p = w.parent();
    while let Some(x) = p {
        if let Ok(sw) = x.clone().downcast::<gtk::ScrolledWindow>() {
            if sw.vscrollbar_policy() != gtk::PolicyType::Never {
                return Some(sw);
            }
        }
        p = x.parent();
    }
    None
}

/// Horizontal rail of cards. The wheel scrolls the page; Shift+wheel scrolls the rail.
pub fn rail(cards: Vec<gtk::Widget>) -> gtk::ScrolledWindow {
    let panel = hbox(0);
    for c in cards {
        panel.append(&c);
    }
    let sw = gtk::ScrolledWindow::new();
    sw.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    sw.set_child(Some(&panel));
    sw.set_propagate_natural_height(true);
    let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
    scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = sw.downgrade();
    scroll.connect_scroll(move |c, dx, dy| {
        let Some(sw) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        let shift = c
            .current_event_state()
            .contains(gtk::gdk::ModifierType::SHIFT_MASK);
        if shift || dx.abs() > dy.abs() {
            let adj = sw.hadjustment();
            let d = if shift { dy } else { dx };
            adj.set_value(adj.value() + d * 60.0);
            return glib::Propagation::Stop;
        }
        if let Some(page) = page_scroller(sw.upcast_ref()) {
            let adj = page.vadjustment();
            adj.set_value(adj.value() + dy * 60.0);
        }
        glib::Propagation::Stop
    });
    sw.add_controller(scroll);
    sw
}

/// Cards that wrap onto new rows.
pub fn wrap(cards: Vec<gtk::Widget>) -> gtk::FlowBox {
    let f = gtk::FlowBox::new();
    f.set_selection_mode(gtk::SelectionMode::None);
    f.set_homogeneous(false);
    f.set_max_children_per_line(100);
    f.set_valign(gtk::Align::Start);
    f.set_activate_on_single_click(false);
    for c in cards {
        f.insert(&c, -1);
    }
    f
}

pub fn empty(icon_name: &str, title: &str, body: &str) -> gtk::Box {
    let s = vbox(0);
    s.set_halign(gtk::Align::Center);
    margins(&s, 0, 90, 0, 0);
    let i = icon(icon_name, 44);
    i.add_css_class("accent");
    s.append(&i);
    let t = text(title, 18.0, "accent", 600, false);
    t.set_halign(gtk::Align::Center);
    margins(&t, 0, 14, 0, 4);
    s.append(&t);
    let d = text(body, 13.0, "muted", 400, true);
    d.set_justify(gtk::Justification::Center);
    d.set_xalign(0.5);
    d.set_max_width_chars(52);
    d.set_halign(gtk::Align::Center);
    s.append(&d);
    s
}

/// Rounded on/off filter chip with a check mark (advanced search filters).
pub fn filter(label: &str, on: bool, changed: impl Fn(bool) + 'static) -> gtk::Button {
    let row = hbox(8);
    let tick = icon("object-select-symbolic", 12);
    row.append(&tick);
    row.append(&gtk::Label::new(Some(label)));
    let b = gtk::Button::new();
    b.set_child(Some(&row));
    b.add_css_class("filter-chip");
    b.set_cursor_from_name(Some("pointer"));
    margins(&b, 0, 0, 10, 8);
    let state = Rc::new(Cell::new(on));
    let paint = {
        let (b, tick, state) = (b.clone(), tick.clone(), state.clone());
        move || {
            if state.get() {
                b.add_css_class("on")
            } else {
                b.remove_css_class("on")
            }
            tick.set_visible(state.get());
        }
    };
    paint();
    b.connect_clicked(move |_| {
        state.set(!state.get());
        paint();
        changed(state.get());
    });
    b
}

/// Segmented choice (All | Live | Movies | Series).
pub fn segments(
    options: &[(&str, &str)],
    current: &str,
    changed: impl Fn(String) + 'static,
) -> gtk::Box {
    let p = hbox(2);
    p.add_css_class("segments");
    p.set_halign(gtk::Align::Start);
    p.set_valign(gtk::Align::Center);
    let changed = Rc::new(changed);
    let mut first: Option<gtk::ToggleButton> = None;
    for (value, label) in options {
        let t = gtk::ToggleButton::with_label(label);
        t.add_css_class("segment");
        if let Some(f) = &first {
            t.set_group(Some(f));
        } else {
            first = Some(t.clone());
        }
        t.set_active(*value == current);
        let (v, ch) = (value.to_string(), changed.clone());
        t.connect_toggled(move |t| {
            if t.is_active() {
                ch(v.clone());
            }
        });
        p.append(&t);
    }
    p
}

/// Blurred backdrop picture with a gradient towards the page colour.
pub fn backdrop(url: Option<&str>, opacity: f64) -> gtk::Overlay {
    let o = gtk::Overlay::new();
    o.set_overflow(gtk::Overflow::Hidden);
    o.set_can_target(false);
    let pic = gtk::Picture::new();
    pic.set_content_fit(gtk::ContentFit::Cover);
    pic.set_can_shrink(true);
    pic.set_opacity(opacity);
    pic.add_css_class("backdrop-pic");
    pic.set_hexpand(true);
    pic.set_vexpand(true);
    o.set_child(Some(&pic));
    images::set_picture(&pic, url, 800);
    let shade = hbox(0);
    shade.add_css_class("shade");
    shade.set_hexpand(true);
    shade.set_vexpand(true);
    o.add_overlay(&shade);
    o
}

// ------------------------------------------------------------------ pop-up menus

/// One row of a pop-up menu. Label "-" draws a separator.
#[derive(Clone)]
pub struct MenuRow {
    pub icon: String,
    pub label: String,
    pub checked: bool,
    pub run: Rc<dyn Fn()>,
    pub sub: Option<String>,
    pub danger: bool,
}

impl MenuRow {
    pub fn new(icon: &str, label: &str, checked: bool, run: impl Fn() + 'static) -> MenuRow {
        MenuRow {
            icon: icon.into(),
            label: label.into(),
            checked,
            run: Rc::new(run),
            sub: None,
            danger: false,
        }
    }
    pub fn sep() -> MenuRow {
        MenuRow::new("", "-", false, || {})
    }
    pub fn sub(mut self, s: &str) -> MenuRow {
        self.sub = Some(s.into());
        self
    }
    pub fn danger(mut self) -> MenuRow {
        self.danger = true;
        self
    }
}

thread_local! {
    static OPEN: RefCell<Option<gtk::Popover>> = const { RefCell::new(None) };
}

/// The menu currently open (Esc closes it before anything else).
pub fn open_popover() -> Option<gtk::Popover> {
    OPEN.with(|o| o.borrow().clone()).filter(|p| p.is_visible())
}

pub fn close_popover() {
    if let Some(p) = open_popover() {
        p.popdown();
    }
}

fn menu_row_widget(r: &MenuRow, pop: &gtk::Popover) -> gtk::Widget {
    if r.label == "-" {
        let l = line();
        margins(&l, 6, 5, 6, 5);
        return l.upcast();
    }
    let g = hbox(0);
    let ic = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    ic.set_size_request(28, -1);
    if !r.icon.is_empty() {
        ic.append(&icon(&r.icon, 15));
    }
    g.append(&ic);
    let t = vbox(0);
    t.set_hexpand(true);
    t.append(&text(
        &r.label,
        13.5,
        "",
        if r.checked { 600 } else { 400 },
        false,
    ));
    if let Some(s) = &r.sub {
        t.append(&text(s, 11.5, "muted", 400, false));
    }
    g.append(&t);
    if r.checked {
        let check = icon("object-select-symbolic", 13);
        check.set_margin_start(10);
        g.append(&check);
    }
    let b = gtk::Button::new();
    b.set_child(Some(&g));
    b.add_css_class("menu-row");
    if r.checked {
        b.add_css_class("checked");
    }
    if r.danger {
        b.add_css_class("danger");
    }
    let (run, pop) = (r.run.clone(), pop.clone());
    b.connect_clicked(move |_| {
        pop.popdown();
        let run = run.clone();
        // Run after the menu has closed (the action may open another one).
        glib::idle_add_local_once(move || run());
    });
    b.upcast()
}

pub struct Menu<'a> {
    pub title: Option<&'a str>,
    pub rows: Vec<MenuRow>,
    pub fixed: Vec<MenuRow>,
    pub prefix: Vec<gtk::Widget>,
    pub extra: Vec<gtk::Widget>,
    pub width: i32,
    pub rows_max_height: i32,
    pub above: bool,
}

impl Default for Menu<'_> {
    fn default() -> Self {
        Menu {
            title: None,
            rows: Vec::new(),
            fixed: Vec::new(),
            prefix: Vec::new(),
            extra: Vec::new(),
            width: 250,
            rows_max_height: 420,
            above: false,
        }
    }
}

/// Show a pop-up menu under (or above) `anchor`.
pub fn pop_menu(anchor: &impl IsA<gtk::Widget>, m: Menu) -> gtk::Popover {
    close_popover();
    let pop = gtk::Popover::new();
    pop.set_has_arrow(false);
    pop.set_position(if m.above {
        gtk::PositionType::Top
    } else {
        gtk::PositionType::Bottom
    });
    let outer = vbox(0);
    outer.set_size_request(m.width, -1);
    if let Some(t) = m.title {
        let h = text(&t.to_uppercase(), 10.5, "muted", 700, false);
        h.add_css_class("menu-header");
        outer.append(&h);
    }
    for p in &m.prefix {
        outer.append(p);
    }
    let list = vbox(0);
    for r in &m.rows {
        list.append(&menu_row_widget(r, &pop));
    }
    let sw = gtk::ScrolledWindow::new();
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sw.set_propagate_natural_height(true);
    sw.set_max_content_height(m.rows_max_height);
    sw.set_child(Some(&list));
    if !m.rows.is_empty() {
        outer.append(&sw);
    }
    for r in &m.fixed {
        outer.append(&menu_row_widget(r, &pop));
    }
    for e in &m.extra {
        outer.append(e);
    }
    pop.set_child(Some(&outer));
    pop.set_parent(anchor);
    pop.connect_closed(|p| {
        let p = p.clone();
        glib::idle_add_local_once(move || {
            if p.parent().is_some() {
                p.unparent();
            }
        });
    });
    OPEN.with(|o| *o.borrow_mut() = Some(pop.clone()));
    pop.popup();
    pop
}

/// Simple menu with icons under a button.
pub fn menu(anchor: &impl IsA<gtk::Widget>, title: &str, rows: Vec<MenuRow>) -> gtk::Popover {
    pop_menu(
        anchor,
        Menu {
            title: Some(title),
            rows,
            ..Default::default()
        },
    )
}

/// "Label   [−]  value  [+]" row for pop-ups (audio / subtitle delay).
pub fn stepper(
    icon_name: &str,
    label: &str,
    value: Rc<dyn Fn() -> String>,
    minus: Rc<dyn Fn()>,
    plus: Rc<dyn Fn()>,
    reset: Rc<dyn Fn()>,
) -> gtk::Box {
    let g = hbox(0);
    margins(&g, 10, 6, 6, 6);
    let ic = hbox(0);
    ic.set_size_request(28, -1);
    ic.append(&icon(icon_name, 15));
    g.append(&ic);
    let t = text(label, 13.5, "", 400, false);
    t.set_hexpand(true);
    g.append(&t);
    let val = gtk::Button::with_label(&value());
    val.add_css_class("link");
    val.set_tooltip_text(Some("Click to reset"));
    val.set_size_request(64, -1);
    let refresh = {
        let (val, value) = (val.clone(), value.clone());
        Rc::new(move || val.set_label(&value()))
    };
    let r2 = refresh.clone();
    let m = icon_button("list-remove-symbolic", "Earlier", move || {
        minus();
        r2();
    });
    m.add_css_class("small");
    let r3 = refresh.clone();
    let p = icon_button("list-add-symbolic", "Later", move || {
        plus();
        r3();
    });
    p.add_css_class("small");
    val.connect_clicked(move |_| {
        reset();
        refresh();
    });
    g.append(&m);
    g.append(&val);
    g.append(&p);
    g
}

/// A ScrolledWindow for a vertical page.
pub fn page_scroll(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let sw = gtk::ScrolledWindow::new();
    sw.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    sw.set_child(Some(child));
    sw.set_vexpand(true);
    sw.set_hexpand(true);
    sw
}
