//! The app's name as users see it, from branding/brand.ini (build.rs), and the version from
//! the repository VERSION file. The source code never spells the name out.

pub const NAME: &str = env!("MADHYAM_NAME");
pub const NATIVE_NAME: &str = env!("MADHYAM_NATIVE_NAME");
pub const VERSION: &str = env!("MADHYAM_VERSION");

/// Desktop application ID (desktop file, icon, D-Bus name). Fixed: never change it.
pub const APP_ID: &str = "dev.madhyam.Madhyam";

/// Both names, for places with room for them (About, the rail logo's tooltip).
pub fn full_name() -> String {
    if NATIVE_NAME.is_empty() {
        NAME.to_string()
    } else {
        format!("{NAME} · {NATIVE_NAME}")
    }
}

/// The app icon, rendered from branding/icon.svg (branding/build.py).
pub fn icon_texture() -> gtk::gdk::Texture {
    static ICON: &[u8] = include_bytes!("../../branding/png/icon-512.png");
    gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_static(ICON)).expect("embedded icon")
}
