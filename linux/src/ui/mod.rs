//! The window and everything in it. `App` (app.rs) is the MainWindow of the Windows version;
//! its methods are split by area like the C# partial class: live.rs, vod.rs, pages.rs,
//! player.rs, and settings_view.rs / dialogs.rs for the rest.

pub mod app;
pub mod dialogs;
pub mod kit;
pub mod live;
pub mod pages;
pub mod player;
pub mod ratio;
pub mod settings_view;
pub mod vod;

use crate::http::{AppError, Res};
use gtk::gio;
use std::future::Future;

/// Run blocking work (engine calls, downloads) on a worker thread.
pub async fn bg<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    match gio::spawn_blocking(f).await {
        Ok(r) => r,
        Err(_) => {
            crate::log("worker thread panicked");
            Err(AppError::Other(
                "Something went wrong (see app.log).".into(),
            ))
        }
    }
}

/// Like `bg` for work that cannot fail.
pub async fn bgv<T: Send + Default + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    gio::spawn_blocking(f).await.unwrap_or_default()
}

pub fn spawn(f: impl Future<Output = ()> + 'static) {
    gtk::glib::spawn_future_local(f);
}

/// Wait without blocking the main loop.
pub async fn sleep_ms(ms: u64) {
    gtk::glib::timeout_future(std::time::Duration::from_millis(ms)).await;
}
