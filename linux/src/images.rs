//! Logos and posters: downloaded on worker threads, scaled to the size they are shown at,
//! kept in memory for the session and on disk in ~/.cache/Madhyam/images so the next start
//! shows them without the network.

use gtk::gdk;
use gtk::gdk_pixbuf::Pixbuf;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

thread_local! {
    static CACHE: RefCell<HashMap<(String, i32), gdk::Texture>> = RefCell::new(HashMap::new());
    static WAITING: RefCell<HashMap<(String, i32), Vec<Box<dyn FnOnce(Option<gdk::Texture>)>>>> = RefCell::new(HashMap::new());
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::paths::home().join(".cache"))
        .join("Madhyam")
        .join("images")
}

fn disk_name(url: &str) -> PathBuf {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    cache_dir().join(format!("{:016x}", h.finish()))
}

fn fetch(url: &str) -> Option<Vec<u8>> {
    let local = url.strip_prefix("file://").unwrap_or(url);
    if local.starts_with('/') {
        return std::fs::read(local).ok();
    }
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let disk = disk_name(url);
    if let Ok(bytes) = std::fs::read(&disk) {
        return Some(bytes);
    }
    let bytes = crate::http::get_quick(url, None).ok()?;
    if bytes.len() > 64 {
        let _ = std::fs::create_dir_all(cache_dir());
        let _ = std::fs::write(&disk, &bytes);
    }
    Some(bytes)
}

/// Decoded pixels: plain data, so it can cross from the worker to the main thread.
struct Pixels {
    width: i32,
    height: i32,
    stride: usize,
    alpha: bool,
    data: glib::Bytes,
}

/// Decode at (at most) `width` pixels wide; 0 keeps the original size.
fn decode(bytes: Vec<u8>, width: i32) -> Option<Pixels> {
    let gbytes = glib::Bytes::from_owned(bytes);
    let stream = gio::MemoryInputStream::from_bytes(&gbytes);
    let p = if width > 0 {
        Pixbuf::from_stream_at_scale(&stream, width, -1, true, gio::Cancellable::NONE)
    } else {
        Pixbuf::from_stream(&stream, gio::Cancellable::NONE)
    }
    .ok()?;
    Some(Pixels {
        width: p.width(),
        height: p.height(),
        stride: p.rowstride() as usize,
        alpha: p.has_alpha(),
        data: p.read_pixel_bytes(),
    })
}

fn texture(p: Pixels) -> gdk::Texture {
    let format = if p.alpha {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    gdk::MemoryTexture::new(p.width, p.height, format, &p.data, p.stride).upcast()
}

/// Load an image and call `done` on the main thread (None when it could not be loaded).
pub fn load(url: Option<&str>, width: i32, done: impl FnOnce(Option<gdk::Texture>) + 'static) {
    let Some(url) = url
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string)
    else {
        return done(None);
    };
    let key = (url.clone(), width);
    if let Some(t) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return done(Some(t));
    }
    let first = WAITING.with(|w| {
        let mut w = w.borrow_mut();
        let list = w.entry(key.clone()).or_default();
        list.push(Box::new(done));
        list.len() == 1
    });
    if !first {
        return;
    }
    glib::spawn_future_local(async move {
        let u = url.clone();
        let texture = gio::spawn_blocking(move || fetch(&u).and_then(|b| decode(b, width)))
            .await
            .ok()
            .flatten()
            .map(texture);
        if let Some(t) = &texture {
            CACHE.with(|c| {
                let mut c = c.borrow_mut();
                if c.len() > 1500 {
                    c.clear();
                }
                c.insert(key.clone(), t.clone());
            });
        }
        let waiting = WAITING
            .with(|w| w.borrow_mut().remove(&key))
            .unwrap_or_default();
        for cb in waiting {
            cb(texture.clone());
        }
    });
}

/// Fill a picture once its image arrives (nothing happens if the widget is gone by then).
pub fn set_picture(pic: &gtk::Picture, url: Option<&str>, width: i32) {
    let weak = pic.downgrade();
    load(url, width, move |t| {
        if let (Some(pic), Some(t)) = (weak.upgrade(), t) {
            pic.set_paintable(Some(&t));
            pic.set_visible(true);
        }
    });
}
