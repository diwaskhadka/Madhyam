//! Parental controls on screen: the PIN dialog, the checks before opening or playing something,
//! the open-lock button shown while unlocked, and zapping past locked channels.

use super::app::{App, Mode};
use super::{bg, bgv, dialogs, kit, sleep_ms, spawn};
use crate::parental::{self, Lock};
use crate::repo::Item;
use crate::settings;
use gtk::glib;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Ask for a 4-digit PIN in four boxes. With `verify` it is checked against the stored PIN: a
/// wrong one shakes and clears, and after 5 wrong tries entry waits 30 s. Returns the digits, or
/// None when the dialog was closed.
pub async fn ask_pin(
    owner: &impl IsA<gtk::Window>,
    heading: &str,
    detail: &str,
    verify: bool,
) -> Option<String> {
    let (w, root) = dialogs::make(owner, heading, 380);
    let t = kit::text(heading, 21.0, "", 600, false);
    root.append(&t);
    if !detail.is_empty() {
        let d = kit::text(detail, 13.0, "muted", 400, true);
        d.set_margin_top(4);
        root.append(&d);
    }
    let row = kit::hbox(10);
    row.set_halign(gtk::Align::Center);
    row.set_margin_top(20);
    let boxes: Rc<Vec<gtk::Entry>> = Rc::new(
        (0..4)
            .map(|_| {
                let e = gtk::Entry::new();
                e.add_css_class("pin-box");
                e.set_max_length(1);
                e.set_width_chars(1);
                e.set_max_width_chars(1);
                EditableExt::set_alignment(&e, 0.5);
                e.set_visibility(false);
                e.set_input_purpose(gtk::InputPurpose::Pin);
                row.append(&e);
                e
            })
            .collect(),
    );
    root.append(&row);
    let msg = kit::text("", 12.5, "error", 400, true);
    msg.set_halign(gtk::Align::Center);
    msg.set_margin_top(12);
    root.append(&msg);

    let result: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let done = Rc::new(Cell::new(false));
    let busy = Rc::new(Cell::new(false)); // ignore our own set_text calls

    let clear = {
        let (boxes, busy) = (boxes.clone(), busy.clone());
        Rc::new(move || {
            busy.set(true);
            for b in boxes.iter() {
                b.set_text("");
            }
            busy.set(false);
            boxes[0].grab_focus();
        })
    };
    // After 5 wrong tries: boxes off, a countdown, then back on.
    let wait = {
        let (boxes, msg, clear) = (boxes.clone(), msg.clone(), clear.clone());
        Rc::new(move || {
            for b in boxes.iter() {
                b.set_sensitive(false);
            }
            let (boxes, msg, clear) = (boxes.clone(), msg.clone(), clear.clone());
            spawn(async move {
                loop {
                    let left = parental::wait_left();
                    if left == 0 {
                        break;
                    }
                    msg.set_text(&format!("Too many wrong PINs. Try again in {left} s."));
                    sleep_ms(500).await;
                }
                msg.set_text("");
                for b in boxes.iter() {
                    b.set_sensitive(true);
                }
                clear();
            });
        })
    };
    let submit = {
        let (boxes, msg, result, w, row, clear, wait) = (
            boxes.clone(),
            msg.clone(),
            result.clone(),
            w.clone(),
            row.clone(),
            clear.clone(),
            wait.clone(),
        );
        Rc::new(move || {
            let pin: String = boxes.iter().map(|b| b.text().to_string()).collect();
            if pin.len() != 4 {
                return;
            }
            if verify && !parental::check_pin(&pin) {
                row.remove_css_class("shake");
                row.add_css_class("shake");
                let r = row.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                    r.remove_css_class("shake")
                });
                if parental::wrong_pin() {
                    wait();
                } else {
                    msg.set_text("Wrong PIN.");
                    clear();
                }
                return;
            }
            *result.borrow_mut() = Some(pin);
            w.close();
        })
    };
    for (i, b) in boxes.iter().enumerate() {
        let (boxes2, busy2, submit) = (boxes.clone(), busy.clone(), submit.clone());
        b.connect_changed(move |e| {
            if busy2.get() {
                return;
            }
            let t = e.text();
            let digits: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits != t.as_str() {
                busy2.set(true);
                e.set_text(&digits);
                busy2.set(false);
            }
            if digits.is_empty() {
                return;
            }
            match boxes2.get(i + 1) {
                Some(next) if next.text().is_empty() => {
                    next.grab_focus();
                }
                _ => submit(),
            }
        });
        // Backspace in an empty box goes back one.
        let keys = gtk::EventControllerKey::new();
        let (boxes2, busy) = (boxes.clone(), busy.clone());
        keys.connect_key_pressed(move |_, k, _, _| {
            if k == gtk::gdk::Key::BackSpace && i > 0 && boxes2[i].text().is_empty() {
                busy.set(true);
                boxes2[i - 1].set_text("");
                busy.set(false);
                boxes2[i - 1].grab_focus();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        b.add_controller(keys);
    }
    let d2 = done.clone();
    w.connect_close_request(move |_| {
        d2.set(true);
        glib::Propagation::Proceed
    });
    let ww = w.clone();
    let cancel = dialogs::button("Cancel", false, move || ww.close());
    let forgot = Rc::new(Cell::new(false));
    let footer = if verify {
        // Reachable even when Settings are locked: the reset needs RESET typed, not the PIN.
        let (ww, f2) = (w.clone(), forgot.clone());
        let b = gtk::Button::with_label("Forgot PIN?");
        b.add_css_class("link");
        b.connect_clicked(move |_| {
            f2.set(true);
            ww.close();
        });
        let f = dialogs::footer(&[&cancel]);
        f.prepend(&b);
        b.set_hexpand(true);
        b.set_halign(gtk::Align::Start);
        f.set_halign(gtk::Align::Fill);
        f
    } else {
        dialogs::footer(&[&cancel])
    };
    root.append(&footer);
    w.present();
    boxes[0].grab_focus();
    if verify && parental::wait_left() > 0 {
        wait();
    }
    while !done.get() {
        sleep_ms(50).await;
    }
    if forgot.get() {
        // Parental controls are off after a reset, so whatever was asked may go ahead.
        return confirm_reset(owner).await.then(String::new);
    }
    result.take()
}

/// "Reset parental controls": only after typing RESET. Removes the PIN and every lock.
pub async fn confirm_reset(owner: &impl IsA<gtk::Window>) -> bool {
    let (w, root) = dialogs::make(owner, "Reset parental controls", 420);
    root.append(&kit::text("Reset parental controls", 21.0, "", 600, false));
    let d = kit::text(
        "This removes the PIN and every lock. Type RESET to confirm.",
        13.0,
        "muted",
        400,
        true,
    );
    d.set_margin_top(4);
    root.append(&d);
    let e = gtk::Entry::new();
    e.set_placeholder_text(Some("RESET"));
    e.set_margin_top(14);
    root.append(&e);
    let ok = Rc::new(Cell::new(false));
    let done = Rc::new(Cell::new(false));
    let (ww, ok2) = (w.clone(), ok.clone());
    let reset = dialogs::button("Reset", true, move || {
        ok2.set(true);
        ww.close();
    });
    reset.set_sensitive(false);
    let r2 = reset.clone();
    e.connect_changed(move |e| r2.set_sensitive(e.text().trim() == "RESET"));
    let ww = w.clone();
    let cancel = dialogs::button("Cancel", false, move || ww.close());
    root.append(&dialogs::footer(&[&cancel, &reset]));
    let d2 = done.clone();
    w.connect_close_request(move |_| {
        d2.set(true);
        glib::Propagation::Proceed
    });
    w.present();
    e.grab_focus();
    while !done.get() {
        sleep_ms(50).await;
    }
    if ok.get() {
        parental::reset();
    }
    ok.get()
}

/// A new PIN, typed twice. None when cancelled or the two did not match (after saying so).
pub async fn ask_new_pin(owner: &impl IsA<gtk::Window>) -> Option<String> {
    let first = ask_pin(owner, "Set PIN", "Choose a 4-digit PIN.", false).await?;
    let second = ask_pin(owner, "Set PIN", "Type the same PIN again.", false).await?;
    if first != second {
        dialogs::confirm(
            owner,
            "The two PINs were different. Nothing was changed.",
            "OK",
        )
        .await;
        return None;
    }
    Some(second)
}

impl App {
    /// True when the action may go ahead: parental controls are off, the PIN was entered in
    /// the last 10 minutes, or it is entered now.
    pub async fn require_pin(self: &Rc<Self>, why: &str) -> bool {
        let s = settings::snapshot();
        if !s.parental_enabled || !parental::has_pin(&s) {
            return true;
        }
        if parental::unlocked() {
            parental::touch();
            return true;
        }
        if ask_pin(&self.window, "Enter PIN", why, true)
            .await
            .is_none()
        {
            return false;
        }
        parental::unlock();
        self.update_lock_button();
        true
    }

    /// Settings, playlists and Parental controls behind the PIN when "Lock Settings" is on.
    pub async fn settings_allowed(self: &Rc<Self>) -> bool {
        let s = settings::snapshot();
        if !(s.parental_enabled && s.parental_lock_settings) {
            return true;
        }
        self.require_pin("Settings are locked by parental controls.")
            .await
    }

    /// Why this item is locked right now (None when open or unlocked for the session).
    pub fn lock_of(&self, it: &Item) -> Option<Lock> {
        if parental::unlocked() {
            return None;
        }
        parental::item_lock(&settings::snapshot(), it)
    }

    /// Ask for the PIN when the item is locked by its category, an adult name or the age limit.
    pub async fn may_open(self: &Rc<Self>, it: &Item) -> bool {
        let s = settings::snapshot();
        if !s.parental_enabled || parental::unlocked() {
            return true;
        }
        if let Some(lock) = parental::item_lock(&s, it) {
            let why = match lock {
                Lock::Adult => format!("“{}” is locked as adult content.", it.display_name()),
                Lock::Category => format!("The category “{}” is locked.", it.group),
            };
            return self.require_pin(&why).await;
        }
        if s.parental_max_age > 0 && !it.is_live() {
            let age = self.age_of(it).await;
            if parental::age_blocked(&s, age) {
                let why = format!(
                    "“{}” is rated {}+, above the limit of {}.",
                    it.display_name(),
                    age.unwrap_or(0),
                    s.parental_max_age
                );
                return self.require_pin(&why).await;
            }
        }
        true
    }

    /// The provider's age rating for a film, series or episode (from the details).
    async fn age_of(self: &Rc<Self>, it: &Item) -> Option<i32> {
        let mut target = it.clone();
        if it.kind == "episode" {
            let sid = it.series_id.clone()?;
            let repo = self.repo.clone();
            target = bgv(move || repo.item(&sid)).await?;
        }
        let repo = self.repo.clone();
        bg(move || {
            if target.is_series() {
                repo.series_details(&target)
            } else {
                repo.movie_details(&target)
            }
        })
        .await
        .ok()
        .flatten()
        .and_then(|d| d.age)
    }

    /// Category names of a section that are locked right now (for queries over "All").
    pub async fn locked_groups(self: &Rc<Self>, source: i64, kind: &'static str) -> Vec<String> {
        let s = settings::snapshot();
        if !s.parental_enabled || parental::unlocked() {
            return Vec::new();
        }
        let repo = self.repo.clone();
        let groups = bg(move || repo.groups(source, kind, None))
            .await
            .unwrap_or_default();
        parental::locked_names(&s, source, kind, groups.iter().map(|g| g.name.as_str()))
    }

    /// The open-lock button in the top bar: shown while unlocked; clicking it locks again.
    pub fn update_lock_button(&self) {
        let show = settings::get().parental_enabled && parental::unlocked();
        if self.unlock_btn.is_visible() != show {
            self.unlock_btn.set_visible(show);
        }
    }

    pub fn lock_again(self: &Rc<Self>) {
        parental::lock_now();
        self.update_lock_button();
        let s = settings::snapshot();
        if self.mode.get() == Mode::Settings && s.parental_lock_settings {
            self.nav_button(Mode::Dashboard).set_active(true);
            return;
        }
        // Locked items leave the lists again; a locked one that is playing stops.
        if self.playing().is_some_and(|p| self.lock_of(&p).is_some()) {
            self.stop_playback();
        }
        self.refresh_lists();
        self.set_status("Locked again.");
    }

    /// Rebuild the lists after locks changed (Settings rebuilds its own page).
    pub fn refresh_lists(self: &Rc<Self>) {
        if self.mode.get() != Mode::Settings && !self.vod_playing.get() {
            let me = self.clone();
            spawn(async move { me.show_mode().await });
        }
    }

    /// Next / previous channel, skipping locked ones (as if they weren't there).
    pub async fn zap(self: &Rc<Self>, delta: i64) {
        let n = self.playlist.borrow().len() as i64;
        let cur = self.play_index.get();
        for step in 1..=n {
            let j = (((cur + delta * step) % n) + n) % n;
            let it = self.playlist.borrow()[j as usize].clone();
            if self.lock_of(&it).is_none() {
                self.play_at(j).await;
                return;
            }
        }
    }
}
