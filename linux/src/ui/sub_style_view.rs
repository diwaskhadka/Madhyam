//! Subtitle style controls: Settings › Playback › Subtitle style (with a live preview) and the
//! player's "Subtitle style…" window (changes show on the real subtitles at once).

use super::app::App;
use super::{dialogs, kit};
use crate::settings::{self, AppSettings};
use crate::sub_style::{self as st, BOX_COLORS, BOX_OPACITIES, EDGES, FONTS, POSITIONS, SIZES};
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Row {
    pub title: &'static str,
    pub desc: &'static str,
    pub widget: gtk::Widget,
    /// Control below the text (wide controls) rather than at the right.
    pub below: bool,
}

fn dropdown(labels: &[String], current: usize, on_pick: impl Fn(usize) + 'static) -> gtk::DropDown {
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let d = gtk::DropDown::from_strings(&refs);
    d.set_selected(current as u32);
    d.set_valign(gtk::Align::Center);
    d.connect_selected_notify(move |d| on_pick(d.selected() as usize));
    d
}

fn pct(v: i32) -> String {
    format!("{v}%")
}

/// One row per option, plus "Move up / down" and "Reset to defaults". `changed` runs after every
/// change (refresh a preview, apply to mpv); `reset` after Reset (rebuild the rows).
pub fn rows(changed: Rc<dyn Fn()>, reset: Rc<dyn Fn()>) -> Vec<Row> {
    let s = settings::snapshot();
    let mut out = Vec::new();
    let set = {
        let changed = changed.clone();
        move |f: Box<dyn FnOnce(&mut AppSettings)>| {
            settings::update(f);
            changed();
        }
    };
    let set = Rc::new(set);

    let sizes: Vec<String> = SIZES.iter().map(|p| format!("{p} pt")).collect();
    let cur = SIZES
        .iter()
        .position(|p| *p == s.sub_font_size_pt)
        .unwrap_or(5);
    let f = set.clone();
    out.push(Row {
        title: "Font size",
        desc: "In points at full-HD size; subtitles keep their size relative to the picture, in a window or full screen.",
        widget: dropdown(&sizes, cur, move |i| {
            let v = SIZES[i.min(SIZES.len() - 1)];
            f(Box::new(move |s| s.sub_font_size_pt = v));
        })
        .upcast(),
        below: false,
    });

    let names = |list: &[&str]| list.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let pick = |list: &'static [&'static str], cur: &str, apply: fn(&mut AppSettings, String)| {
        let f = set.clone();
        dropdown(
            &names(list),
            list.iter().position(|x| *x == cur).unwrap_or(0),
            move |i| {
                let v = list[i.min(list.len() - 1)].to_string();
                f(Box::new(move |s| apply(s, v)));
            },
        )
        .upcast::<gtk::Widget>()
    };
    let opacity = |list: &'static [i32], cur: i32, apply: fn(&mut AppSettings, i32)| {
        let f = set.clone();
        let opts: Vec<(String, String)> = list.iter().map(|v| (v.to_string(), pct(*v))).collect();
        let refs: Vec<(&str, &str)> = opts.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        kit::segments(&refs, &cur.to_string(), move |v| {
            let n: i32 = v.parse().unwrap_or(100);
            f(Box::new(move |s| apply(s, n)));
        })
        .upcast::<gtk::Widget>()
    };

    out.push(Row {
        title: "Font family",
        desc: "The closest installed font is used.",
        widget: pick(&FONTS, &s.sub_font, |s, v| s.sub_font = v),
        below: false,
    });
    out.push(Row {
        title: "Text colour",
        desc: "",
        widget: pick(&st::TEXT_COLORS, &s.sub_text_color, |s, v| {
            s.sub_text_color = v
        }),
        below: false,
    });
    out.push(Row {
        title: "Text opacity",
        desc: "",
        widget: opacity(&st::TEXT_OPACITIES, s.sub_text_opacity, |s, v| {
            s.sub_text_opacity = v
        }),
        below: false,
    });
    out.push(Row {
        title: "Background colour",
        desc: "The box behind each line of text.",
        widget: pick(&BOX_COLORS, &s.sub_back_color, |s, v| s.sub_back_color = v),
        below: false,
    });
    out.push(Row {
        title: "Background opacity",
        desc: "",
        widget: opacity(&BOX_OPACITIES, s.sub_back_opacity, |s, v| {
            s.sub_back_opacity = v
        }),
        below: false,
    });
    out.push(Row {
        title: "Window colour",
        desc: "A box around the whole caption area.",
        widget: pick(&BOX_COLORS, &s.sub_window_color, |s, v| {
            s.sub_window_color = v
        }),
        below: false,
    });
    out.push(Row {
        title: "Window opacity",
        desc: "",
        widget: opacity(&BOX_OPACITIES, s.sub_window_opacity, |s, v| {
            s.sub_window_opacity = v
        }),
        below: false,
    });
    {
        let f = set.clone();
        let opts: Vec<(&str, &str)> = EDGES.iter().map(|e| (*e, *e)).collect();
        out.push(Row {
            title: "Edge style",
            desc: "Shown when the background and window are off (a box replaces the edge).",
            widget: kit::segments(&opts, &s.sub_edge, move |v| {
                f(Box::new(move |s| s.sub_edge = v));
            })
            .upcast(),
            below: true,
        });
    }
    {
        let f = set.clone();
        let opts: Vec<(&str, &str)> = POSITIONS.iter().map(|e| (*e, *e)).collect();
        let pos = kit::hbox(12);
        let seg = kit::segments(&opts, &s.sub_position, move |v| {
            f(Box::new(move |s| {
                s.sub_position = v;
                s.sub_pos_offset = 0;
            }));
        });
        pos.append(&seg);
        let value = gtk::Label::new(None);
        value.set_width_chars(6);
        let show = {
            let value = value.clone();
            move || {
                value.set_text(&match settings::get().sub_pos_offset {
                    0 => "±0%".to_string(),
                    o if o > 0 => format!("↑ {o}%"),
                    o => format!("↓ {}%", -o),
                })
            }
        };
        show();
        let show = Rc::new(show);
        let nudge = |label: &str, d: i32| {
            let (f, show) = (set.clone(), show.clone());
            let b = gtk::Button::with_label(label);
            b.add_css_class("secondary");
            b.connect_clicked(move |_| {
                f(Box::new(move |s| {
                    s.sub_pos_offset = (s.sub_pos_offset + d).clamp(st::POS_MIN, st::POS_MAX);
                }));
                show();
            });
            b
        };
        pos.append(&nudge("Move down", -st::POS_STEP));
        pos.append(&value);
        pos.append(&nudge("Move up", st::POS_STEP));
        out.push(Row {
            title: "Position",
            desc: "Move up / down fine-tunes the position in 2% steps.",
            widget: pos.upcast(),
            below: true,
        });
    }
    let b = gtk::Button::with_label("Reset to defaults");
    b.add_css_class("secondary");
    b.connect_clicked(move |_| {
        st::reset();
        changed();
        reset();
    });
    out.push(Row {
        title: "Reset subtitle style",
        desc: "24 pt sans-serif, white on a half-transparent black background, drop shadow, at the bottom.",
        widget: b.upcast(),
        below: false,
    });
    out
}

thread_local! {
    static CSS: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// The preview's edge and window CSS, shared by every preview on screen.
fn load_css(frame_h: f64) {
    let css = st::preview_css(&settings::snapshot(), frame_h);
    CSS.with(|c| {
        let mut c = c.borrow_mut();
        let p = c.get_or_insert_with(|| {
            let p = gtk::CssProvider::new();
            if let Some(d) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &d,
                    &p,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2,
                );
            }
            p
        });
        p.load_from_string(&css);
    });
}

const PREVIEW_W: i32 = 480;
const PREVIEW_H: i32 = 270;

/// A 16:9 dark frame with a sample caption; the returned function redraws it.
pub fn preview() -> (gtk::Widget, Rc<dyn Fn()>) {
    let frame = gtk::Overlay::new();
    frame.add_css_class("sub-preview-frame");
    frame.set_size_request(PREVIEW_W, PREVIEW_H);
    frame.set_halign(gtk::Align::Start);
    frame.set_overflow(gtk::Overflow::Hidden);
    let bg = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bg.set_size_request(PREVIEW_W, PREVIEW_H);
    frame.set_child(Some(&bg));
    let window = kit::vbox(0);
    window.add_css_class("sub-preview-window");
    window.set_halign(gtk::Align::Center);
    window.set_valign(gtk::Align::End);
    let label = gtk::Label::new(None);
    label.add_css_class("sub-preview-text");
    label.set_justify(gtk::Justification::Center);
    window.append(&label);
    frame.add_overlay(&window);
    let redraw = {
        let (label, window) = (label.clone(), window.clone());
        Rc::new(move || {
            let s = settings::snapshot();
            let h = PREVIEW_H as f64;
            label.set_markup(&st::preview_markup(
                &s,
                "This is how subtitles will look.",
                h,
            ));
            load_css(h);
            // sub-pos is where the bottom of the text sits, in % of the height.
            let bottom = h * (1.0 - st::pos_percent(&s) / 100.0) + h * 0.03;
            window.set_margin_bottom(bottom.round() as i32);
        }) as Rc<dyn Fn()>
    };
    redraw();
    (frame.upcast(), redraw)
}

/// The player's "Subtitle style…" window: every change shows on the playing subtitles.
pub fn open_window(app: &Rc<App>) {
    let (w, root) = dialogs::make(&app.window, "Subtitle style", 640);
    w.set_modal(false);
    let content = kit::vbox(0);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_min_content_height(520);
    scroll.set_child(Some(&content));
    root.append(&kit::text("Subtitle style", 21.0, "", 600, false));
    root.append(&scroll);
    let weak = Rc::downgrade(app);
    let fill: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let f2 = fill.clone();
    let build = Rc::new(move || {
        kit::clear(&content);
        let w2 = weak.clone();
        let changed: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(m) = w2.upgrade().and_then(|a| a.mpv().cloned()) {
                st::apply(&m);
            }
        });
        let f3 = f2.clone();
        let reset: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(f) = f3.borrow().clone() {
                // Rebuild after the click handler returns.
                gtk::glib::idle_add_local_once(move || f());
            }
        });
        for r in rows(changed, reset) {
            let line = kit::line();
            kit::margins(&line, 0, 12, 0, 12);
            content.append(&line);
            content.append(&super::settings_view::row_widget(
                r.title, r.desc, &r.widget, r.below,
            ));
        }
    }) as Rc<dyn Fn()>;
    *fill.borrow_mut() = Some(build.clone());
    build();
    let ww = w.clone();
    root.append(&dialogs::footer(&[&dialogs::button(
        "Done",
        true,
        move || ww.close(),
    )]));
    w.present();
}
