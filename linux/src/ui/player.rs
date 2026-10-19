//! Player chrome over the video: click to pause, double-click for full screen, controls that
//! hide after a few seconds and come back on mouse movement, pop-ups for audio / subtitles /
//! speed, audio and subtitle delay, subtitle files and online subtitle search.
//! Twin of MainWindow.Player.cs.

use super::app::App;
use super::kit::{self, MenuRow};
use super::spawn;
use crate::mpv::Event;
use crate::repo::Item;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use regex::Regex;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::LazyLock;

pub struct Controls {
    pub osd: gtk::Label,
    pub big_play: gtk::Box,
    pub big_play_icon: gtk::Image,
    pub bar: gtk::Box,
    pub seek_row: gtk::Overlay,
    pub seek: gtk::Scale,
    pub seek_tip: gtk::Label,
    pub time_row: gtk::Box,
    pub time_pos: gtk::Label,
    pub time_len: gtk::Label,
    pub play_pause: gtk::Button,
    pub back10: gtk::Button,
    pub fwd10: gtk::Button,
    pub next_ep: gtk::Button,
    pub bar_live: gtk::Label,
    pub bar_title: gtk::Label,
    pub mute: gtk::Button,
    pub volume: gtk::Scale,
    pub audio: gtk::Button,
    pub subs: gtk::Button,
    pub speed: gtk::Button,
    pub full: gtk::Button,
    pub area: RefCell<Option<gtk::GLArea>>,
    pub muted: Cell<bool>,
    pub audio_delay: Cell<i64>,
    pub sub_delay: Cell<i64>,
    pub bar_timer: RefCell<Option<glib::SourceId>>,
    pub osd_timer: RefCell<Option<glib::SourceId>>,
}

fn player_button(icon: &str, tooltip: &str) -> gtk::Button {
    let b = gtk::Button::new();
    b.set_child(Some(&kit::icon(icon, 18)));
    b.add_css_class("player-button");
    b.set_tooltip_text(Some(tooltip));
    b.set_valign(gtk::Align::Center);
    b.set_focus_on_click(false);
    b
}

/// The overlay that holds the video and its controls (the GLArea is added once mpv exists).
pub fn build_controls() -> (gtk::Overlay, Controls) {
    let video = gtk::Overlay::new();
    video.set_hexpand(true);
    video.set_vexpand(true);
    video.set_visible(false);

    let osd = gtk::Label::new(None);
    osd.add_css_class("osd");
    osd.set_halign(gtk::Align::Center);
    osd.set_valign(gtk::Align::Start);
    osd.set_margin_top(22);
    osd.set_visible(false);
    osd.set_can_target(false);
    video.add_overlay(&osd);

    let big_play = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    big_play.add_css_class("big-play");
    big_play.set_halign(gtk::Align::Center);
    big_play.set_valign(gtk::Align::Center);
    let big_play_icon = kit::icon("madhyam-media-playback-start-symbolic", 30);
    big_play_icon.set_halign(gtk::Align::Center);
    big_play.set_homogeneous(true);
    big_play.append(&big_play_icon);
    big_play.set_visible(false);
    big_play.set_can_target(false);
    video.add_overlay(&big_play);

    let bar = kit::vbox(0);
    bar.add_css_class("player-bar");
    bar.set_valign(gtk::Align::End);
    let seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    seek.set_draw_value(false);
    seek.add_css_class("seek");
    seek.set_hexpand(true);
    let seek_row = gtk::Overlay::new();
    seek_row.set_child(Some(&seek));
    let seek_tip = gtk::Label::new(None);
    seek_tip.add_css_class("seek-tip");
    seek_tip.set_halign(gtk::Align::Start);
    seek_tip.set_valign(gtk::Align::Start);
    seek_tip.set_visible(false);
    seek_tip.set_can_target(false);
    seek_row.add_overlay(&seek_tip);
    kit::margins(&seek_row, 4, 0, 4, 0);
    bar.append(&seek_row);
    let time_row = kit::hbox(0);
    kit::margins(&time_row, 6, 0, 6, 2);
    let time_pos = gtk::Label::new(None);
    let time_len = gtk::Label::new(None);
    time_pos.set_hexpand(true);
    time_pos.set_xalign(0.0);
    time_row.append(&time_pos);
    time_row.append(&time_len);
    bar.append(&time_row);
    let buttons = kit::hbox(2);
    let play_pause = player_button(
        "madhyam-media-playback-pause-symbolic",
        "Play / pause (Space, or click the picture)",
    );
    let back10 = player_button("madhyam-media-seek-backward-symbolic", "Back 10 s (←)");
    let fwd10 = player_button("madhyam-media-seek-forward-symbolic", "Forward 10 s (→)");
    let next_ep = player_button("madhyam-media-skip-forward-symbolic", "Next episode");
    next_ep.set_visible(false);
    let bar_live = gtk::Label::new(Some("● LIVE"));
    bar_live.add_css_class("live-badge");
    bar_live.set_valign(gtk::Align::Center);
    bar_live.set_margin_start(10);
    bar_live.set_visible(false);
    let bar_title = gtk::Label::new(None);
    bar_title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    bar_title.set_margin_start(12);
    bar_title.set_max_width_chars(48);
    for w in [
        play_pause.upcast_ref::<gtk::Widget>(),
        back10.upcast_ref(),
        fwd10.upcast_ref(),
        next_ep.upcast_ref(),
        bar_live.upcast_ref(),
        bar_title.upcast_ref(),
    ] {
        buttons.append(w);
    }
    buttons.append(&kit::spacer());
    let mute = player_button("madhyam-audio-volume-high-symbolic", "Mute (M)");
    let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    volume.set_value(100.0);
    volume.set_draw_value(false);
    volume.add_css_class("seek");
    volume.set_size_request(90, -1);
    volume.set_valign(gtk::Align::Center);
    volume.set_margin_end(8);
    volume.set_tooltip_text(Some("Volume (↑ / ↓)"));
    let audio = player_button("madhyam-audio-speakers-symbolic", "Audio track and delay");
    let subs = player_button("madhyam-media-view-subtitles-symbolic", "Subtitles");
    let speed = gtk::Button::with_label("1×");
    speed.add_css_class("player-button");
    speed.set_tooltip_text(Some("Playback speed"));
    speed.set_valign(gtk::Align::Center);
    let full = player_button(
        "madhyam-view-fullscreen-symbolic",
        "Full screen (F11 or double-click)",
    );
    for w in [
        mute.upcast_ref::<gtk::Widget>(),
        volume.upcast_ref(),
        audio.upcast_ref(),
        subs.upcast_ref(),
        speed.upcast_ref(),
        full.upcast_ref(),
    ] {
        buttons.append(w);
    }
    bar.append(&buttons);
    video.add_overlay(&bar);

    let c = Controls {
        osd,
        big_play,
        big_play_icon,
        bar,
        seek_row,
        seek,
        seek_tip,
        time_row,
        time_pos,
        time_len,
        play_pause,
        back10,
        fwd10,
        next_ep,
        bar_live,
        bar_title,
        mute,
        volume,
        audio,
        subs,
        speed,
        full,
        area: RefCell::new(None),
        muted: Cell::new(false),
        audio_delay: Cell::new(0),
        sub_delay: Cell::new(0),
        bar_timer: RefCell::new(None),
        osd_timer: RefCell::new(None),
    };
    (video, c)
}

impl Controls {
    /// Put mpv's drawing area under the controls.
    pub fn attach_area(&self, app: &Rc<App>) {
        let Some(mpv) = app.mpv() else { return };
        let area = crate::mpv::video_area(mpv);
        app.video.set_child(Some(&area));
        *self.area.borrow_mut() = Some(area);
    }

    pub fn set_fullscreen_icon(&self, on: bool) {
        self.full.set_child(Some(&kit::icon(
            if on {
                "madhyam-view-restore-symbolic"
            } else {
                "madhyam-view-fullscreen-symbolic"
            },
            18,
        )));
    }

    pub fn wire(&self, app: &Rc<App>) {
        let w = Rc::downgrade(app);
        macro_rules! on {
            ($b:expr, $f:expr) => {{
                let w = w.clone();
                $b.connect_clicked(move |_| {
                    if let Some(a) = w.upgrade() {
                        #[allow(clippy::redundant_closure_call)]
                        ($f)(a);
                    }
                });
            }};
        }
        on!(self.play_pause, |a: Rc<App>| a.toggle_pause(false));
        on!(self.back10, |a: Rc<App>| a.seek_by(-10_000));
        on!(self.fwd10, |a: Rc<App>| a.seek_by(10_000));
        on!(self.next_ep, |a: Rc<App>| {
            let i = a.play_index.get();
            if (i as usize) + 1 < a.playlist.borrow().len() {
                spawn(async move { a.play_at(i + 1).await });
            }
        });
        on!(self.mute, |a: Rc<App>| a.toggle_mute());
        on!(self.audio, |a: Rc<App>| a.audio_menu());
        on!(self.subs, |a: Rc<App>| a.subtitles_menu());
        on!(self.speed, |a: Rc<App>| a.speed_menu());
        on!(self.full, |a: Rc<App>| a.toggle_fullscreen(false));
        {
            let w = w.clone();
            self.seek.connect_change_value(move |_, _, v| {
                if let Some(a) = w.upgrade() {
                    if let (Some(m), Some(_)) = (a.mpv(), a.playing().filter(|p| !p.is_live())) {
                        let len = m.length_ms();
                        if len > 0 {
                            m.seek_ms((v.clamp(0.0, 1.0) * len as f64) as i64);
                        }
                    }
                    a.show_bar();
                }
                glib::Propagation::Proceed
            });
        }
        {
            // Time preview above the seek bar where the mouse is.
            let motion = gtk::EventControllerMotion::new();
            let w2 = w.clone();
            motion.connect_motion(move |_, x, _| {
                let Some(a) = w2.upgrade() else { return };
                let c = &a.controls;
                let len = a.mpv().map(|m| m.length_ms()).unwrap_or(0);
                let width = c.seek.width() as f64;
                if len <= 0 || width <= 0.0 || !x.is_finite() {
                    return;
                }
                let ratio = (x / width).clamp(0.0, 1.0);
                c.seek_tip.set_text(&fmt((ratio * len as f64) as i64));
                c.seek_tip.set_margin_start((x - 24.0).max(0.0) as i32);
                c.seek_tip.set_visible(true);
            });
            let w3 = w.clone();
            motion.connect_leave(move |_| {
                if let Some(a) = w3.upgrade() {
                    a.controls.seek_tip.set_visible(false);
                }
            });
            self.seek.add_controller(motion);
        }
        {
            let w = w.clone();
            self.volume.connect_value_changed(move |s| {
                let Some(a) = w.upgrade() else { return };
                if let Some(m) = a.mpv() {
                    m.set_f64("volume", s.value());
                    if a.controls.muted.get() && s.value() > 0.0 {
                        a.controls.muted.set(false);
                        m.set_flag("mute", false);
                    }
                }
                a.update_mute_icons();
            });
        }
        // The video: click = pause, double-click = full screen, motion shows the controls.
        let click = gtk::GestureClick::new();
        {
            let w = w.clone();
            click.connect_pressed(move |_, n, _, _| {
                let Some(a) = w.upgrade() else { return };
                if n == 2 {
                    a.toggle_pause(true); // undo the first click's pause
                    a.toggle_fullscreen(false);
                } else if n == 1 {
                    a.toggle_pause(false);
                }
            });
        }
        if let Some(area) = self.area.borrow().as_ref() {
            area.add_controller(click);
        }
        let motion = gtk::EventControllerMotion::new();
        {
            let w = w.clone();
            motion.connect_motion(move |_, _, _| {
                if let Some(a) = w.upgrade() {
                    a.show_bar();
                }
            });
        }
        app.video.add_controller(motion);
        // The wheel over the playing film scrolls the page (as on Windows).
        let wheel = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        {
            let w = w.clone();
            wheel.connect_scroll(move |_, _, dy| {
                let Some(a) = w.upgrade() else {
                    return glib::Propagation::Proceed;
                };
                if a.fullscreen.get() || !a.vod_playing.get() {
                    return glib::Propagation::Proceed;
                }
                let adj = a.vod_scroll.vadjustment();
                adj.set_value(adj.value() + dy * 60.0);
                glib::Propagation::Stop
            });
        }
        app.video.add_controller(wheel);
    }
}

pub fn fmt(ms: i64) -> String {
    let s = ms.max(0) / 1000;
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h >= 1 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

/// VLC-style language codes from the settings → what mpv's alang/slang match.
pub fn mpv_lang(code: &str) -> String {
    match code {
        "eng" => "eng,en".into(),
        "fre" => "fre,fra,fr".into(),
        "spa" => "spa,es".into(),
        "ger" => "ger,deu,de".into(),
        "ara" => "ara,ar".into(),
        "hin" => "hin,hi".into(),
        "por" => "por,pt".into(),
        "tur" => "tur,tr".into(),
        other => other.to_string(),
    }
}

static SE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bS\d{1,2}\s*E\d{1,3}\b").unwrap());
static LANG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[A-Z]{2,4}\s*[-|:]\s*").unwrap());

/// "S03E01 · Title" without the series name providers repeat in every episode title.
pub fn episode_label(series_name: &str, ep: &Item) -> String {
    let mut name = ep.name.clone();
    for cut in [
        series_name.to_string(),
        crate::ratings::clean_title(series_name),
    ] {
        if cut.chars().count() > 2 {
            let lower = name.to_lowercase();
            if let Some(i) = lower.find(&cut.to_lowercase()) {
                name.replace_range(i..i + cut.len(), "");
            }
        }
    }
    let name = SE_RE.replace_all(&name, "").to_string();
    let name = LANG_RE.replace(&name, "").to_string();
    let name = name
        .trim_matches(|c: char| " -·|:.".contains(c))
        .to_string();
    if name.chars().count() > 1 {
        format!("{} · {name}", ep.se())
    } else {
        ep.se()
    }
}

fn delay_text(ms: i64) -> String {
    if ms == 0 {
        "0 ms".into()
    } else if ms > 0 {
        format!("+{ms} ms")
    } else {
        format!("−{} ms", -ms)
    }
}

impl App {
    pub fn on_player_event(self: &Rc<Self>, e: Event) {
        let c = &self.controls;
        match e {
            Event::Playing => {
                c.play_pause.set_child(Some(&kit::icon(
                    "madhyam-media-playback-pause-symbolic",
                    18,
                )));
                if let Some(m) = self.mpv() {
                    m.set_flag("mute", c.muted.get()); // a new audio output starts unmuted
                }
            }
            Event::Paused(p) => {
                c.play_pause.set_child(Some(&kit::icon(
                    if p {
                        "madhyam-media-playback-start-symbolic"
                    } else {
                        "madhyam-media-playback-pause-symbolic"
                    },
                    18,
                )));
            }
            Event::EndReached => {
                let me = self.clone();
                spawn(async move { me.on_end_reached().await });
            }
            Event::Error => {
                if self.playing().is_some() {
                    self.set_status("Playback failed: the stream could not be opened. Try another channel or refresh the playlist.");
                }
            }
        }
    }

    pub fn show_bar(self: &Rc<Self>) {
        let c = &self.controls;
        c.bar.set_opacity(1.0);
        c.bar.set_can_target(true);
        self.video.set_cursor(None);
        if let Some(id) = c.bar_timer.borrow_mut().take() {
            id.remove();
        }
        let w = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(2800), move || {
            let Some(a) = w.upgrade() else { return };
            a.controls.bar_timer.borrow_mut().take();
            let hovering_bar = a
                .controls
                .bar
                .state_flags()
                .contains(gtk::StateFlags::PRELIGHT);
            let paused = a.mpv().is_none_or(|m| m.paused());
            if hovering_bar || kit::open_popover().is_some() || a.playing().is_none() || paused {
                return;
            }
            a.hide_bar();
        });
        *c.bar_timer.borrow_mut() = Some(id);
    }

    fn hide_bar(&self) {
        let c = &self.controls;
        c.bar.set_opacity(0.0);
        c.bar.set_can_target(false);
        c.seek_tip.set_visible(false);
        self.video.set_cursor_from_name(Some("none"));
    }

    /// Short message over the video ("Subtitle delay +100 ms").
    pub fn show_osd(self: &Rc<Self>, text: &str) {
        let c = &self.controls;
        c.osd.set_text(text);
        c.osd.set_visible(true);
        self.restart_osd_timer();
    }

    fn restart_osd_timer(self: &Rc<Self>) {
        let c = &self.controls;
        if let Some(id) = c.osd_timer.borrow_mut().take() {
            id.remove();
        }
        let w = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(std::time::Duration::from_millis(1600), move || {
            if let Some(a) = w.upgrade() {
                a.controls.osd_timer.borrow_mut().take();
                a.controls.osd.set_visible(false);
                a.controls.big_play.set_visible(false);
            }
        });
        *c.osd_timer.borrow_mut() = Some(id);
    }

    /// Called when a new stream starts.
    pub fn on_new_media(self: &Rc<Self>, it: &Item) {
        let c = &self.controls;
        let vod = !it.is_live();
        c.seek_row.set_visible(vod);
        c.time_row.set_visible(vod);
        c.back10.set_visible(vod);
        c.fwd10.set_visible(vod);
        c.speed.set_visible(vod);
        c.speed.set_label("1×");
        c.bar_title.set_text(&if it.is_live() {
            it.display_name()
        } else {
            String::new()
        });
        c.bar_live.set_visible(it.is_live());
        c.subs.set_visible(vod); // subtitles are a film / episode thing
        c.seek.set_value(0.0);
        c.time_pos.set_text("0:00");
        c.time_len.set_text("");
        if it.is_live() {
            self.live_card_title.set_text(&it.display_name());
            self.live_card_sub.set_text(&match self.now_of(&it.id) {
                Some(p) => format!("Now: {}", p.title),
                None => it.group.clone(),
            });
        } else {
            self.set_vod_card_titles(it);
        }
        c.audio_delay.set(0);
        c.sub_delay.set(0);
        if let Some(m) = self.mpv() {
            m.set_f64("audio-delay", 0.0);
            m.set_f64("sub-delay", 0.0);
            m.set_flag("mute", c.muted.get());
        }
        self.show_bar();
    }

    fn set_vod_card_titles(&self, it: &Item) {
        self.vod_card_label.set_text("NOW PLAYING");
        let series = self.details.borrow().clone().filter(|d| d.is_series());
        match (it.kind.as_str(), series) {
            ("episode", Some(s)) => {
                self.vod_card_title.set_text(&s.name);
                self.vod_card_sub.set_text(&episode_label(&s.name, it));
            }
            _ => {
                self.vod_card_title.set_text(&it.name);
                let parts: Vec<String> = [it.year.clone(), it.genre.clone()]
                    .into_iter()
                    .flatten()
                    .filter(|x| !x.is_empty())
                    .collect();
                self.vod_card_sub.set_text(&parts.join("  ·  "));
            }
        }
    }

    pub fn update_seek_bar(&self) {
        if !self.vod_playing.get() {
            return;
        }
        let Some(m) = self.mpv() else { return };
        let (t, len) = (m.time_ms(), m.length_ms());
        if len > 0 {
            let c = &self.controls;
            c.seek.set_value((t as f64 / len as f64).clamp(0.0, 1.0));
            c.time_pos.set_text(&fmt(t));
            c.time_len.set_text(&fmt(len));
        }
    }

    // ================================================================ transport

    pub fn toggle_pause(self: &Rc<Self>, silent: bool) {
        if self.playing().is_none() {
            return;
        }
        let Some(m) = self.mpv() else { return };
        let will_pause = !m.paused();
        m.set_pause(will_pause);
        if silent {
            return;
        }
        let c = &self.controls;
        c.big_play_icon.set_icon_name(Some(if will_pause {
            "madhyam-media-playback-pause-symbolic"
        } else {
            "madhyam-media-playback-start-symbolic"
        }));
        c.big_play.set_visible(true);
        self.restart_osd_timer();
        self.show_bar();
    }

    pub fn seek_by(self: &Rc<Self>, ms: i64) {
        if self.playing().is_none_or(|p| p.is_live()) {
            return;
        }
        let Some(m) = self.mpv() else { return };
        let len = m.length_ms();
        if len <= 0 {
            return;
        }
        let t = (m.time_ms() + ms).clamp(0, len);
        m.seek_ms(t);
        self.show_osd(&if ms > 0 {
            format!("+{} s   {}", ms / 1000, fmt(t))
        } else {
            format!("−{} s   {}", -ms / 1000, fmt(t))
        });
        self.show_bar();
    }

    /// Our own flag is the truth: reading mute back right after setting it can be stale.
    pub fn toggle_mute(self: &Rc<Self>) {
        let c = &self.controls;
        c.muted.set(!c.muted.get());
        if let Some(m) = self.mpv() {
            m.set_flag("mute", c.muted.get());
        }
        self.update_mute_icons();
        self.show_osd(&if c.muted.get() {
            "Muted".to_string()
        } else {
            format!("Volume {}%", c.volume.value() as i32)
        });
    }

    fn update_mute_icons(&self) {
        let c = &self.controls;
        let name = if c.muted.get() || c.volume.value() == 0.0 {
            "madhyam-audio-volume-muted-symbolic"
        } else {
            "madhyam-audio-volume-high-symbolic"
        };
        c.mute.set_child(Some(&kit::icon(name, 18)));
        self.live_mute.set_child(Some(&kit::icon(name, 16)));
    }

    pub fn change_volume(self: &Rc<Self>, delta: i32) {
        let c = &self.controls;
        c.volume
            .set_value((c.volume.value() + delta as f64).clamp(0.0, 100.0));
        self.show_osd(&format!("Volume {}%", c.volume.value() as i32));
        self.show_bar();
    }

    pub fn change_audio_delay(self: &Rc<Self>, ms: i64) {
        let c = &self.controls;
        c.audio_delay
            .set(if ms == 0 { 0 } else { c.audio_delay.get() + ms });
        if let Some(m) = self.mpv() {
            m.set_f64("audio-delay", c.audio_delay.get() as f64 / 1000.0);
        }
        self.show_osd(&format!("Audio delay {}", delay_text(c.audio_delay.get())));
    }

    pub fn change_sub_delay(self: &Rc<Self>, ms: i64) {
        let c = &self.controls;
        c.sub_delay
            .set(if ms == 0 { 0 } else { c.sub_delay.get() + ms });
        if let Some(m) = self.mpv() {
            m.set_f64("sub-delay", c.sub_delay.get() as f64 / 1000.0);
        }
        self.show_osd(&format!("Subtitle delay {}", delay_text(c.sub_delay.get())));
    }

    // ================================================================ pop-ups

    fn audio_menu(self: &Rc<Self>) {
        let Some(m) = self.mpv().cloned() else { return };
        let mut rows: Vec<MenuRow> = m
            .tracks("audio")
            .into_iter()
            .map(|(id, label, sel)| {
                let m = m.clone();
                MenuRow::new("madhyam-audio-speakers-symbolic", &label, sel, move || {
                    m.set("aid", &id.to_string())
                })
            })
            .collect();
        if rows.is_empty() {
            rows.push(MenuRow::new(
                "madhyam-audio-speakers-symbolic",
                "Default audio",
                true,
                || {},
            ));
        }
        let sep = kit::line();
        kit::margins(&sep, 6, 5, 6, 5);
        let (a1, a2, a3, a4) = (self.clone(), self.clone(), self.clone(), self.clone());
        let step = kit::stepper(
            "madhyam-media-playlist-repeat-symbolic",
            "Audio delay  (J / K)",
            Rc::new(move || delay_text(a1.controls.audio_delay.get())),
            Rc::new(move || a2.change_audio_delay(-50)),
            Rc::new(move || a3.change_audio_delay(50)),
            Rc::new(move || a4.change_audio_delay(0)),
        );
        let pop = kit::pop_menu(
            &self.controls.audio,
            kit::Menu {
                title: Some("Audio"),
                rows,
                extra: vec![sep.upcast(), step.upcast()],
                width: 300,
                rows_max_height: 230,
                above: true,
                ..Default::default()
            },
        );
        self.track_popover(&pop);
    }

    fn subtitles_menu(self: &Rc<Self>) {
        let Some(m) = self.mpv().cloned() else { return };
        let tracks = m.tracks("sub");
        let any_selected = tracks.iter().any(|t| t.2);
        let m2 = m.clone();
        let mut rows = vec![MenuRow::new(
            "madhyam-window-close-symbolic",
            "Off",
            !any_selected,
            move || m2.set("sid", "no"),
        )];
        rows.extend(tracks.into_iter().map(|(id, label, sel)| {
            let m = m.clone();
            MenuRow::new(
                "madhyam-media-view-subtitles-symbolic",
                &label,
                sel,
                move || m.set("sid", &id.to_string()),
            )
        }));
        let (me1, me2) = (self.clone(), self.clone());
        let fixed = vec![
            MenuRow::sep(),
            MenuRow::new(
                "madhyam-document-open-symbolic",
                "Load subtitle file…",
                false,
                move || me1.load_subtitle_file(),
            ),
            MenuRow::new(
                "madhyam-system-search-symbolic",
                "Search subtitles online…",
                false,
                move || me2.search_subtitles_online(),
            )
            .sub("OpenSubtitles.com"),
        ];
        let sep = kit::line();
        kit::margins(&sep, 6, 5, 6, 5);
        let (a1, a2, a3, a4) = (self.clone(), self.clone(), self.clone(), self.clone());
        let step = kit::stepper(
            "madhyam-appointment-soon-symbolic",
            "Subtitle delay  (G / H)",
            Rc::new(move || delay_text(a1.controls.sub_delay.get())),
            Rc::new(move || a2.change_sub_delay(-50)),
            Rc::new(move || a3.change_sub_delay(50)),
            Rc::new(move || a4.change_sub_delay(0)),
        );
        let pop = kit::pop_menu(
            &self.controls.subs,
            kit::Menu {
                title: Some("Subtitles"),
                rows,
                fixed,
                extra: vec![sep.upcast(), step.upcast()],
                width: 330,
                rows_max_height: 230,
                above: true,
                ..Default::default()
            },
        );
        self.track_popover(&pop);
    }

    fn speed_menu(self: &Rc<Self>) {
        let Some(m) = self.mpv().cloned() else { return };
        let cur = m.get_f64("speed").unwrap_or(1.0);
        let rows = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0]
            .into_iter()
            .map(|s| {
                let (m, me) = (m.clone(), self.clone());
                let label = if s == 1.0 {
                    "Normal (1×)".to_string()
                } else {
                    format!("{s}×")
                };
                let icon = if s < 1.0 {
                    "madhyam-media-seek-backward-symbolic"
                } else if s == 1.0 {
                    "madhyam-media-playback-start-symbolic"
                } else {
                    "madhyam-media-seek-forward-symbolic"
                };
                MenuRow::new(icon, &label, (s - cur).abs() < 0.01, move || {
                    m.set_f64("speed", s);
                    me.controls.speed.set_label(&format!("{s}×"));
                    me.show_osd(&format!("Speed {s}×"));
                })
            })
            .collect();
        let pop = kit::pop_menu(
            &self.controls.speed,
            kit::Menu {
                title: Some("Playback speed"),
                rows,
                width: 220,
                above: true,
                ..Default::default()
            },
        );
        self.track_popover(&pop);
    }

    fn track_popover(self: &Rc<Self>, pop: &gtk::Popover) {
        let w = Rc::downgrade(self);
        pop.connect_closed(move |_| {
            if let Some(a) = w.upgrade() {
                a.show_bar();
            }
        });
    }

    // ================================================================ subtitle files

    fn load_subtitle_file(self: &Rc<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Subtitles"));
        for p in ["*.srt", "*.ass", "*.ssa", "*.vtt", "*.sub", "*.idx"] {
            filter.add_pattern(p);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dlg = gtk::FileDialog::builder()
            .title("Load subtitle file")
            .filters(&filters)
            .modal(true)
            .build();
        let me = self.clone();
        spawn(async move {
            if let Ok(file) = dlg.open_future(Some(&me.window)).await {
                if let Some(path) = file.path() {
                    me.use_subtitle(&path.to_string_lossy());
                }
            }
        });
    }

    pub fn use_subtitle(self: &Rc<Self>, path: &str) {
        if self.playing().is_none() {
            return;
        }
        let ok = self
            .mpv()
            .is_some_and(|m| m.command(&["sub-add", path, "select"]));
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.show_osd(&if ok {
            format!("Subtitles: {name}")
        } else {
            "These subtitles could not be loaded.".into()
        });
    }

    fn search_subtitles_online(self: &Rc<Self>) {
        let Some(it) = self.playing() else { return };
        let query = if it.kind == "episode" {
            self.details
                .borrow()
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or(it.name.clone())
        } else {
            it.name.clone()
        };
        let (season, episode) = if it.kind == "episode" {
            (
                it.season.as_deref().and_then(|s| s.parse().ok()),
                it.episode.as_deref().and_then(|e| e.parse().ok()),
            )
        } else {
            (None, None)
        };
        let me = self.clone();
        super::dialogs::subtitle_search(
            &self.window,
            &crate::ratings::clean_title(&query),
            season,
            episode,
            move |path| me.use_subtitle(&path),
        );
    }
}
