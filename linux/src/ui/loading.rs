//! "Adding playlist": the pop-up shown while a playlist loads. Its steps get a tick as each
//! finishes, the running one shows the size downloaded, the live / movies / series lines show
//! how many came in, and a bar moves along, so nobody waits at a blank window. Hide lets the
//! load go on in the background (the status line still shows it). Twin of LoadingWindow.cs.

use super::dialogs;
use super::kit;
use crate::repo::{LoadStatus, Step};
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const XTREAM: &[(Step, &str)] = &[
    (Step::SignIn, "Signing in"),
    (Step::Live, "Live channels"),
    (Step::Movies, "Movies"),
    (Step::Series, "Series"),
    (Step::Save, "Saving the catalog"),
    (Step::Guide, "Programme guide"),
    (Step::Pages, "Getting the pages ready"),
];

const M3U: &[(Step, &str)] = &[
    (Step::Download, "Downloading the playlist"),
    (Step::Read, "Reading the playlist"),
    (Step::Live, "Live channels"),
    (Step::Movies, "Movies"),
    (Step::Series, "Series"),
    (Step::Save, "Saving the catalog"),
    (Step::Guide, "Programme guide"),
    (Step::Pages, "Getting the pages ready"),
];

struct Line {
    step: Step,
    mark: gtk::Label,
    label: gtk::Label,
    detail: gtk::Label,
    done: Cell<bool>,
}

pub struct Loading {
    window: gtk::Window,
    title: gtk::Label,
    sub: gtk::Label,
    error: gtk::Label,
    bar: gtk::ProgressBar,
    button: gtk::Button,
    lines: Vec<Line>,
    current: Cell<Option<usize>>,
    step_started: Cell<Instant>,
    started: Instant,
    finished: Cell<bool>,
    /// Reports from the worker thread, taken on the main loop.
    queue: Arc<Mutex<Vec<LoadStatus>>>,
    on_button: RefCell<Option<Box<dyn Fn()>>>,
}

impl Loading {
    pub fn show(owner: &impl IsA<gtk::Window>, name: &str, xtream: bool) -> Rc<Loading> {
        let (window, root) = dialogs::make(owner, "Adding playlist", 460);
        window.set_modal(false);
        let title = kit::text(&format!("Adding “{name}”"), 19.0, "", 600, false);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        root.append(&title);
        let sub = kit::text(
            "This can take a minute for a large playlist.",
            13.0,
            "muted",
            400,
            true,
        );
        sub.set_margin_top(4);
        sub.set_margin_bottom(14);
        root.append(&sub);
        let mut lines = Vec::new();
        for (step, text) in if xtream { XTREAM } else { M3U } {
            let row = kit::hbox(10);
            row.set_margin_bottom(8);
            let mark = kit::text("○", 13.0, "muted", 400, false);
            mark.set_width_chars(2);
            let label = kit::text(text, 14.0, "muted", 400, false);
            label.set_hexpand(true);
            label.set_xalign(0.0);
            let detail = kit::text("", 12.5, "muted", 400, false);
            row.append(&mark);
            row.append(&label);
            row.append(&detail);
            root.append(&row);
            lines.push(Line {
                step: *step,
                mark,
                label,
                detail,
                done: Cell::new(false),
            });
        }
        let bar = kit::progress(0.0);
        bar.set_margin_top(8);
        root.append(&bar);
        let error = kit::text("", 13.0, "live-red", 400, true);
        error.set_margin_top(12);
        error.set_visible(false);
        root.append(&error);
        let me: Rc<Loading> = Rc::new_cyclic(|weak: &std::rc::Weak<Loading>| {
            let w = weak.clone();
            let button = dialogs::button("Hide", false, move || {
                if let Some(me) = w.upgrade() {
                    if let Some(f) = me.on_button.borrow().as_ref() {
                        f();
                    }
                }
            });
            button.set_tooltip_text(Some("Keep loading in the background"));
            root.append(&dialogs::footer(&[&button]));
            Loading {
                window: window.clone(),
                title,
                sub,
                error,
                bar,
                button,
                lines,
                current: Cell::new(None),
                step_started: Cell::new(Instant::now()),
                started: Instant::now(),
                finished: Cell::new(false),
                queue: Arc::new(Mutex::new(Vec::new())),
                on_button: RefCell::new(None),
            }
        });
        let ww = window.downgrade();
        *me.on_button.borrow_mut() = Some(Box::new(move || {
            if let Some(w) = ww.upgrade() {
                w.set_visible(false);
            }
        }));
        // Closing early (Esc, the title bar) only hides it while the load goes on.
        let weak = Rc::downgrade(&me);
        window.connect_close_request(move |w| {
            if weak.upgrade().is_some_and(|m| !m.finished.get()) {
                w.set_visible(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        // Take the worker's reports, and move the bar along (a step without a known size
        // creeps forward so it never looks stuck).
        let weak = Rc::downgrade(&me);
        glib::timeout_add_local(std::time::Duration::from_millis(120), move || {
            let Some(me) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let pending: Vec<LoadStatus> =
                std::mem::take(&mut *me.queue.lock().unwrap_or_else(|p| p.into_inner()));
            for s in pending {
                me.report(s);
            }
            if me.finished.get() {
                return glib::ControlFlow::Break;
            }
            if let Some(i) = me.current.get() {
                let secs = me.step_started.get().elapsed().as_secs_f64();
                let within = 0.9 * (1.0 - (-secs / 6.0).exp());
                let target = (i as f64 + within) / me.lines.len() as f64;
                me.bar.set_fraction(me.bar.fraction().max(target));
            }
            glib::ControlFlow::Continue
        });
        // On the first start Free TV loads before the main window is up: show the pop-up once
        // it is, or it can end up behind it (or nowhere).
        let owner = owner.as_ref().clone();
        if owner.is_mapped() {
            window.present();
        } else {
            let w = window.downgrade();
            let fired = Rc::new(Cell::new(false));
            owner.connect_map(move |_| {
                if fired.replace(true) {
                    return;
                }
                if let Some(w) = w.upgrade() {
                    glib::idle_add_local_once(move || w.present());
                }
            });
        }
        me.report(LoadStatus::new(me.lines[0].step, ""));
        me
    }

    /// Where worker threads put their reports (shown within a tenth of a second).
    pub fn sink(&self) -> Arc<Mutex<Vec<LoadStatus>>> {
        self.queue.clone()
    }

    pub fn report(&self, s: LoadStatus) {
        if self.finished.get() {
            return;
        }
        let Some(i) = self.lines.iter().position(|l| l.step == s.step) else {
            return;
        };
        let cur = self.current.get();
        if cur.is_none_or(|c| i > c) {
            for j in cur.unwrap_or(0)..i {
                self.mark(j, true);
            }
            self.current.set(Some(i));
            self.step_started.set(Instant::now());
            self.mark(i, false);
        }
        let line = &self.lines[i];
        if s.done {
            self.mark(i, true);
            line.detail.set_text(&s.detail);
            if s.detail == "None" {
                line.label.set_css_classes(&["muted"]);
            } else {
                line.detail.set_css_classes(&[]);
            }
        } else if Some(i) == self.current.get() {
            line.detail.set_text(&s.detail);
        }
    }

    fn mark(&self, i: usize, done: bool) {
        let line = &self.lines[i];
        if done && line.done.get() {
            return;
        }
        line.done.set(done);
        line.mark.set_text(if done { "✓" } else { "●" });
        line.mark.set_css_classes(&["accent"]);
        line.label.set_css_classes(&[]);
        line.label.set_attributes(None);
        if !done {
            let attrs = gtk::pango::AttrList::new();
            attrs.insert(gtk::pango::AttrInt::new_weight(
                gtk::pango::Weight::Semibold,
            ));
            line.label.set_attributes(Some(&attrs));
        }
    }

    /// All done: tick every line, say how long it took, and close after a moment (later
    /// while the pointer is over it).
    pub fn done(self: &Rc<Self>) {
        self.report(LoadStatus::new(Step::Pages, ""));
        for i in 0..self.lines.len() {
            self.mark(i, true);
        }
        self.finished.set(true);
        self.bar.set_fraction(1.0);
        let took = self.started.elapsed().as_secs();
        self.title.set_text(&if took >= 1 {
            format!("Ready in {took} s")
        } else {
            "Ready".into()
        });
        self.sub.set_text("Here's what the playlist has.");
        self.button.set_label("Done");
        self.button.set_tooltip_text(None);
        let ww = self.window.downgrade();
        *self.on_button.borrow_mut() = Some(Box::new(move || {
            if let Some(w) = ww.upgrade() {
                w.close();
            }
        }));
        let hover = gtk::EventControllerMotion::new();
        let inside = Rc::new(Cell::new(false));
        let (a, b) = (inside.clone(), inside.clone());
        hover.connect_enter(move |_, _, _| a.set(true));
        hover.connect_leave(move |_| b.set(false));
        self.window.add_controller(hover);
        let ww = self.window.downgrade();
        super::spawn(async move {
            super::sleep_ms(4000).await;
            while inside.get() {
                super::sleep_ms(500).await;
            }
            if let Some(w) = ww.upgrade() {
                w.close();
            }
        });
    }

    /// It failed: say why on the pop-up (shown again if hidden), with a Close button.
    pub fn failed(&self, message: &str) {
        self.finished.set(true);
        if let Some(i) = self.current.get() {
            let m = &self.lines[i].mark;
            m.set_text("✕");
            m.set_css_classes(&["live-red"]);
        }
        self.title.set_text("The playlist could not be added");
        self.sub.set_visible(false);
        self.error.set_text(message);
        self.error.set_visible(true);
        self.button.set_label("Close");
        self.button.set_tooltip_text(None);
        let ww = self.window.downgrade();
        *self.on_button.borrow_mut() = Some(Box::new(move || {
            if let Some(w) = ww.upgrade() {
                w.close();
            }
        }));
        self.window.present();
    }
}
