//! Passwords (Xtream accounts, OpenSubtitles login) go to the desktop keyring through the
//! freedesktop Secret Service (GNOME Keyring, KWallet, KeePassXC…), never into the catalog
//! file. Without a keyring they fall back to secrets.json in the data folder, readable by this
//! user only. On Windows the same job is done by DPAPI.

use crate::paths;
use gtk::gio;
use gtk::glib::{self, prelude::*, variant::ObjectPath};
use std::collections::HashMap;
use std::sync::Mutex;

const APP: &str = "dev.madhyam.Madhyam";
const SERVICE: &str = "org.freedesktop.secrets";
const SERVICE_PATH: &str = "/org/freedesktop/secrets";
const DEFAULT_COLLECTION: &str = "/org/freedesktop/secrets/aliases/default";

static LOCK: Mutex<()> = Mutex::new(());

struct Keyring {
    bus: gio::DBusConnection,
    session: ObjectPath,
}

impl Keyring {
    fn open() -> Option<Keyring> {
        let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
        let reply = call(
            &bus,
            SERVICE_PATH,
            "org.freedesktop.Secret.Service",
            "OpenSession",
            Some(&("plain", "".to_variant()).to_variant()),
        )?;
        let session = reply.child_value(1).get::<ObjectPath>()?;
        Some(Keyring { bus, session })
    }

    fn attributes(key: &str) -> HashMap<String, String> {
        HashMap::from([
            ("application".to_string(), APP.to_string()),
            ("key".to_string(), key.to_string()),
        ])
    }

    fn find(&self, key: &str) -> Option<Vec<ObjectPath>> {
        let reply = call(
            &self.bus,
            SERVICE_PATH,
            "org.freedesktop.Secret.Service",
            "SearchItems",
            Some(&(Self::attributes(key),).to_variant()),
        )?;
        let unlocked = reply.child_value(0).get::<Vec<ObjectPath>>()?;
        let locked = reply.child_value(1).get::<Vec<ObjectPath>>()?;
        if unlocked.is_empty() && !locked.is_empty() {
            // Unlock without a prompt works when the keyring is open for the session.
            let r = call(
                &self.bus,
                SERVICE_PATH,
                "org.freedesktop.Secret.Service",
                "Unlock",
                Some(&(locked,).to_variant()),
            )?;
            return r.child_value(0).get::<Vec<ObjectPath>>();
        }
        Some(unlocked)
    }

    fn lookup(&self, key: &str) -> Option<Option<String>> {
        let items = self.find(key)?;
        let Some(first) = items.into_iter().next() else {
            return Some(None);
        };
        let reply = call(
            &self.bus,
            SERVICE_PATH,
            "org.freedesktop.Secret.Service",
            "GetSecrets",
            Some(&(vec![first], self.session.clone()).to_variant()),
        )?;
        let map = reply.child_value(0);
        if map.n_children() == 0 {
            return Some(None);
        }
        // a{o(oayays)}: entry → (path, (session, params, value, content_type))
        let secret = map.child_value(0).child_value(1);
        let value = secret.child_value(2).get::<Vec<u8>>()?;
        Some(Some(String::from_utf8_lossy(&value).into_owned()))
    }

    fn store(&self, key: &str, value: &str) -> Option<()> {
        let mut props: HashMap<String, glib::Variant> = HashMap::new();
        props.insert(
            "org.freedesktop.Secret.Item.Label".into(),
            format!("{} ({key})", crate::brand::NAME).to_variant(),
        );
        props.insert(
            "org.freedesktop.Secret.Item.Attributes".into(),
            Self::attributes(key).to_variant(),
        );
        let secret = (
            self.session.clone(),
            Vec::<u8>::new(),
            value.as_bytes().to_vec(),
            "text/plain".to_string(),
        );
        let reply = call(
            &self.bus,
            DEFAULT_COLLECTION,
            "org.freedesktop.Secret.Collection",
            "CreateItem",
            Some(&(props, secret, true).to_variant()),
        )?;
        // A prompt (locked keyring) means nothing was stored yet: fall back to the file.
        let prompt = reply.child_value(1).get::<ObjectPath>()?;
        (prompt.as_str() == "/").then_some(())
    }

    fn delete(&self, key: &str) -> Option<()> {
        for item in self.find(key)? {
            call(
                &self.bus,
                item.as_str(),
                "org.freedesktop.Secret.Item",
                "Delete",
                None,
            )?;
        }
        Some(())
    }
}

fn call(
    bus: &gio::DBusConnection,
    path: &str,
    iface: &str,
    method: &str,
    args: Option<&glib::Variant>,
) -> Option<glib::Variant> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static NO_KEYRING_LOGGED: AtomicBool = AtomicBool::new(false);
    bus.call_sync(
        Some(SERVICE),
        path,
        iface,
        method,
        args,
        None,
        gio::DBusCallFlags::NONE,
        5000,
        gio::Cancellable::NONE,
    )
    .map_err(|e| {
        // No keyring at all is normal on some desktops: say it once, then use the file.
        let missing = method == "OpenSession";
        if !missing || !NO_KEYRING_LOGGED.swap(true, Ordering::Relaxed) {
            crate::log(&format!(
                "keyring {method}: {e}{}",
                if missing {
                    " (passwords go to secrets.json)"
                } else {
                    ""
                }
            ));
        }
    })
    .ok()
}

// ------------------------------------------------------------------ file fallback

fn file_path() -> std::path::PathBuf {
    paths::data_dir().join("secrets.json")
}

fn read_file() -> HashMap<String, String> {
    std::fs::read_to_string(file_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_file(map: &HashMap<String, String>) {
    if map.is_empty() {
        let _ = std::fs::remove_file(file_path());
        return;
    }
    if let Ok(text) = serde_json::to_string(map) {
        if let Err(e) = paths::write_atomic(&file_path(), text.as_bytes()) {
            crate::log(&format!("secrets not saved: {e}"));
        }
    }
}

// ------------------------------------------------------------------ API (blocking)

/// The stored secret, or None.
pub fn lookup(key: &str) -> Option<String> {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let keyring = Keyring::open();
    if let Some(found) = keyring.as_ref().and_then(|k| k.lookup(key)) {
        if found.is_some() {
            return found;
        }
    }
    let mut file = read_file();
    let value = file.remove(key)?;
    // Saved to the file while no keyring was reachable: move it to the keyring now that one is.
    if keyring.is_some_and(|k| k.store(key, &value).is_some()) {
        write_file(&file);
    }
    Some(value)
}

/// Store a secret; returns true when it went to the keyring (false: the private file).
pub fn store(key: &str, value: &str) -> bool {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut file = read_file();
    if Keyring::open().and_then(|k| k.store(key, value)).is_some() {
        if file.remove(key).is_some() {
            write_file(&file);
        }
        return true;
    }
    file.insert(key.to_string(), value.to_string());
    write_file(&file);
    false
}

pub fn delete(key: &str) {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(k) = Keyring::open() {
        k.delete(key);
    }
    let mut file = read_file();
    if file.remove(key).is_some() {
        write_file(&file);
    }
}
