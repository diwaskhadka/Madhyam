//! Madhyam for Linux: GTK 4 window, madhyam-core engine (linked in), video by libmpv.
//!
//! Data folder: $XDG_DATA_HOME/Madhyam (normally ~/.local/share/Madhyam), same files as the
//! Windows app: settings.json, catalog.jsonl, epg/, app.log, error.log.

// Signal handlers and pop-up builders take boxed closures; naming every one would not help.
#![allow(clippy::type_complexity)]

mod brand;
mod engine;
mod freetv;
mod http;
mod images;
mod mpv;
mod paths;
mod ratings;
mod repo;
mod secrets;
mod settings;
mod subtitles;
mod theme;
mod ui;

use gtk::prelude::*;
use gtk::{gio, glib};
use std::io::Write;

/// Append a line to app.log in the data folder (never fails).
pub fn log(msg: &str) {
    let path = paths::data_dir().join("app.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            f,
            "{} {msg}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
    }
}

/// Crashes go to error.log (CI's smoke test fails when it exists), kept small.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let path = paths::data_dir().join("error.log");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 2_000_000) {
            let _ = std::fs::rename(&path, path.with_extension("log.old"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(
                f,
                "{} {info}\n{}\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                std::backtrace::Backtrace::force_capture()
            );
        }
        default(info);
    }));
}

fn main() -> glib::ExitCode {
    if std::env::args().any(|a| a == "--version") {
        println!(
            "{} {} (engine {})",
            brand::NAME,
            brand::VERSION,
            engine::Engine::version()
        );
        return glib::ExitCode::SUCCESS;
    }
    paths::ensure_data_dir();
    install_panic_hook();
    settings::load();
    glib::set_application_name(brand::NAME);
    glib::set_prgname(Some(brand::APP_ID));
    // Our own command-line options (--screenshot, --mode…) are not GTK's: don't let GApplication parse them.
    let app = gtk::Application::builder()
        .application_id(brand::APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    gio::resources_register_include!("icons.gresource").expect("bundled icons");
    app.connect_activate(|app| {
        if let Some(display) = gtk::gdk::Display::default() {
            // Madhyam's own icons (linux/icons), found before any system theme.
            gtk::IconTheme::for_display(&display).add_resource_path("/dev/madhyam/Madhyam/icons");
        }
        // Icons installed next to the binary (AppImage, a build tree) before the system theme.
        if let (Ok(exe), Some(display)) = (std::env::current_exe(), gtk::gdk::Display::default()) {
            if let Some(share) = exe
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("share").join("icons"))
            {
                gtk::IconTheme::for_display(&display).add_search_path(share);
            }
        }
        let window = ui::app::App::new(app);
        window.start();
        // Keep the App alive as long as its window.
        let keep = std::cell::RefCell::new(Some(window.clone()));
        window.window.connect_destroy(move |_| {
            keep.borrow_mut().take();
        });
    });
    app.run_with_args::<&str>(&[])
}
