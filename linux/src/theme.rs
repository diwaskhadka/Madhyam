//! Settings › Visual theme: Navy & Crimson (dark), Light, System and Custom. The palettes are
//! the Windows app's (ThemeColors.cs / AppSettings.cs) key for key; `css` turns one into the
//! application stylesheet.

use crate::settings::AppSettings;
use std::collections::BTreeMap;

pub const CRIMSON: &str = "#DC143C";
pub const ROYAL_BLUE: &str = "#003893";

/// Picker presets with their names (same list on Windows and Android TV).
pub const PRESETS: [(&str, &str); 12] = [
    ("Crimson", CRIMSON),
    ("Royal blue", ROYAL_BLUE),
    ("Vermilion", "#E4572E"),
    ("Saffron", "#F2A541"),
    ("Green", "#2BA84A"),
    ("Teal", "#12A4A4"),
    ("Sky blue", "#2F6FEB"),
    ("Violet", "#6C4AD8"),
    ("Purple", "#B43FB4"),
    ("Rose", "#E0457B"),
    ("Slate", "#5E6B7D"),
    ("Brown", "#8B5E3C"),
];

pub type Palette = BTreeMap<&'static str, String>;

fn palette(pairs: &[(&'static str, &str)]) -> Palette {
    pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
}

pub fn navy_crimson() -> Palette {
    palette(&[
        ("Bg", "#0B1024"),
        ("Rail", "#080C1C"),
        ("Panel", "#10183A"),
        ("Card", "#162048"),
        ("CardHover", "#1C2856"),
        ("CardSelected", "#2A2352"),
        ("Line", "#26315E"),
        ("Accent", "#F2506B"),
        ("Fg", "#ECEEF7"),
        ("Muted", "#97A0C3"),
        ("LiveRed", "#FF6B81"),
        ("Rating", "#7EE2A8"),
        ("Chip", "#1E2958"),
        ("RailActive", "#33224C"),
        ("CatSelected", "#252452"),
        ("Secondary", "#222C5C"),
        ("Thumb", "#3A4680"),
        ("PrimaryBg", "#F2F4FA"),
        ("PrimaryFg", "#0B1024"),
        ("Badge", "#D90B1024"),
        ("Star", "#FFD166"),
        ("Shade", "#330B1024"),
        ("ShadeEnd", "#F20B1024"),
    ])
}

pub fn light() -> Palette {
    palette(&[
        ("Bg", "#F4F5F9"),
        ("Rail", "#E9EBF2"),
        ("Panel", "#FFFFFF"),
        ("Card", "#FFFFFF"),
        ("CardHover", "#ECEFF6"),
        ("CardSelected", "#DFE8FF"),
        ("Line", "#D6DAE5"),
        ("Accent", "#2F6FEB"),
        ("Fg", "#1A1D29"),
        ("Muted", "#5E6479"),
        ("LiveRed", "#D93838"),
        ("Rating", "#13874B"),
        ("Chip", "#E7EAF2"),
        ("RailActive", "#DCE5FF"),
        ("CatSelected", "#E3EAFF"),
        ("Secondary", "#E3E6EF"),
        ("Thumb", "#C3C8D6"),
        ("PrimaryBg", "#1A1D29"),
        ("PrimaryFg", "#FFFFFF"),
        ("Badge", "#E6FFFFFF"),
        ("Star", "#E0A100"),
        ("Shade", "#00F4F5F9"),
        ("ShadeEnd", "#F2F4F5F9"),
    ])
}

// ------------------------------------------------------------------ colour maths

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub fn parse(hex: &str) -> Option<Rgb> {
    let h = hex.trim().trim_start_matches('#');
    let h = if h.len() == 8 { &h[2..] } else { h }; // #AARRGGBB → RGB
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

fn hex_a(c: Rgb, alpha: u8) -> String {
    if alpha == 255 {
        hex(c)
    } else {
        format!("#{alpha:02X}{:02X}{:02X}{:02X}", c.0, c.1, c.2)
    }
}

/// "Crimson" for a preset, otherwise the hex code.
pub fn name_of(hex_code: &str) -> String {
    PRESETS
        .iter()
        .find(|(_, h)| h.eq_ignore_ascii_case(hex_code))
        .map(|(n, _)| n.to_string())
        .unwrap_or_else(|| hex_code.to_uppercase())
}

pub fn to_hsl(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
    let h = ((h % 360.0) + 360.0) % 360.0 / 360.0;
    let channel = |t: f64| {
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let t = (t + 1.0) % 1.0;
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let b = |v: f64| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    if s == 0.0 {
        Rgb(b(l), b(l), b(l))
    } else {
        Rgb(
            b(channel(h + 1.0 / 3.0)),
            b(channel(h)),
            b(channel(h - 1.0 / 3.0)),
        )
    }
}

fn blend(base: Rgb, over: Rgb, amount: f64) -> Rgb {
    let m = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * amount).round() as u8;
    Rgb(m(base.0, over.0), m(base.1, over.1), m(base.2, over.2))
}

/// A whole palette from one colour (same formulas as ThemeColors.Custom on Windows).
pub fn custom(accent_hex: &str, light: bool) -> Palette {
    let accent = parse(accent_hex).unwrap_or_else(|| parse(CRIMSON).unwrap());
    let (h, s, l) = to_hsl(accent);
    let tint = s.min(0.55);
    let hsl = |sat: f64, lig: f64| hex(from_hsl(h, sat, lig));
    let hsla = |sat: f64, lig: f64, a: u8| hex_a(from_hsl(h, sat, lig), a);
    let mix = |over: &str, amount: f64| hex(blend(parse(over).unwrap(), accent, amount));
    let mut p = Palette::new();
    if !light {
        let bg = hsl(tint * 0.8, 0.07);
        let rail = hsl(tint * 0.8, 0.05);
        let card = hsl(tint * 0.7, 0.14);
        p.insert("Bg", bg.clone());
        p.insert("Rail", rail.clone());
        p.insert("Panel", hsl(tint * 0.75, 0.10));
        p.insert("Card", card.clone());
        p.insert("CardHover", hsl(tint * 0.7, 0.18));
        p.insert("CardSelected", mix(&card, 0.22));
        p.insert("Line", hsl(tint * 0.55, 0.24));
        p.insert("Accent", hex(from_hsl(h, s.max(0.55), l.max(0.64))));
        p.insert("Fg", hsl(tint * 0.25, 0.94));
        p.insert("Muted", hsl(tint * 0.3, 0.68));
        p.insert("LiveRed", "#FF6B81".into());
        p.insert("Rating", "#7EE2A8".into());
        p.insert("Chip", hsl(tint * 0.65, 0.18));
        p.insert("RailActive", mix(&rail, 0.30));
        p.insert("CatSelected", mix(&card, 0.16));
        p.insert("Secondary", hsl(tint * 0.6, 0.20));
        p.insert("Thumb", hsl(tint * 0.5, 0.36));
        p.insert("PrimaryBg", "#F2F4FA".into());
        p.insert("PrimaryFg", bg);
        p.insert("Badge", hsla(tint * 0.8, 0.07, 0xD9));
        p.insert("Star", "#FFD166".into());
        p.insert("Shade", hsla(tint * 0.8, 0.07, 0x33));
        p.insert("ShadeEnd", hsla(tint * 0.8, 0.07, 0xF2));
    } else {
        let lbg = hsl(tint * 0.5, 0.965);
        p.insert("Bg", lbg);
        p.insert("Rail", hsl(tint * 0.5, 0.93));
        p.insert("Panel", "#FFFFFF".into());
        p.insert("Card", "#FFFFFF".into());
        p.insert("CardHover", hsl(tint * 0.5, 0.94));
        p.insert("CardSelected", mix("#FFFFFF", 0.16));
        p.insert("Line", hsl(tint * 0.35, 0.86));
        p.insert("Accent", hex(from_hsl(h, s.max(0.6), l.min(0.42))));
        p.insert("Fg", hsl(tint * 0.4, 0.13));
        p.insert("Muted", hsl(tint * 0.25, 0.42));
        p.insert("LiveRed", "#D93838".into());
        p.insert("Rating", "#13874B".into());
        p.insert("Chip", hsl(tint * 0.45, 0.92));
        p.insert("RailActive", mix(&hsl(tint * 0.5, 0.93), 0.18));
        p.insert("CatSelected", mix("#FFFFFF", 0.12));
        p.insert("Secondary", hsl(tint * 0.4, 0.91));
        p.insert("Thumb", hsl(tint * 0.3, 0.80));
        p.insert("PrimaryBg", hsl(tint * 0.4, 0.13));
        p.insert("PrimaryFg", "#FFFFFF".into());
        p.insert("Badge", "#E6FFFFFF".into());
        p.insert("Star", "#E0A100".into());
        p.insert("Shade", hsla(tint * 0.5, 0.965, 0x00));
        p.insert("ShadeEnd", hsla(tint * 0.5, 0.965, 0xF2));
    }
    p
}

/// The palette the current settings ask for.
pub fn current(s: &AppSettings) -> Palette {
    if s.theme == "Custom" {
        custom(&s.custom_color, s.custom_light)
    } else if s.is_light() {
        light()
    } else {
        navy_crimson()
    }
}

/// "#AARRGGBB" / "#RRGGBB" → CSS rgba().
pub fn css_color(hex_code: &str) -> String {
    let h = hex_code.trim().trim_start_matches('#');
    let (a, rgb) = if h.len() == 8 {
        (u8::from_str_radix(&h[..2], 16).unwrap_or(255), &h[2..])
    } else {
        (255, h)
    };
    let c = parse(rgb).unwrap_or(Rgb(255, 0, 255));
    format!("rgba({},{},{},{:.3})", c.0, c.1, c.2, a as f64 / 255.0)
}

/// Does the desktop prefer a dark style? Reads the freedesktop settings portal
/// (org.freedesktop.appearance color-scheme), then GTK's own setting.
pub fn system_prefers_dark() -> bool {
    use gtk::gio;
    use gtk::glib::{self, prelude::*};
    let portal = || -> Option<u32> {
        let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
        let reply = bus
            .call_sync(
                Some("org.freedesktop.portal.Desktop"),
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings",
                "Read",
                Some(&("org.freedesktop.appearance", "color-scheme").to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                500,
                gio::Cancellable::NONE,
            )
            .ok()?;
        // (v) holding v holding u (Read wraps the value twice).
        let mut v: glib::Variant = reply.child_value(0);
        while let Some(inner) = v.as_variant() {
            v = inner;
        }
        v.get::<u32>()
    };
    match portal() {
        Some(1) => return true,
        Some(2) => return false,
        _ => {}
    }
    if let Some(s) = gtk::Settings::default() {
        if s.is_gtk_application_prefer_dark_theme() {
            return true;
        }
        if let Some(name) = s.gtk_theme_name() {
            return name.to_lowercase().contains("dark");
        }
    }
    false
}

/// The application stylesheet for a palette. Class names follow the Windows styles
/// (RailButton, Card, Chip, Segment, PrimaryButton…), in kebab case.
pub fn css(p: &Palette, cover: &str) -> String {
    let mut out = String::new();
    for (k, v) in p {
        out.push_str(&format!("@define-color {} {};\n", k, css_color(v)));
    }
    let (pw, ph) = match cover {
        "Small" => (136, 202),
        "Large" => (220, 328),
        _ => (176, 262),
    };
    out.push_str(&format!(
        ".grid-poster {{ min-width: {pw}px; min-height: {ph}px; }}\n"
    ));
    out.push_str(STYLE);
    out
}

const STYLE: &str = r#"
window, window.background, .main-window { background-color: @Bg; color: @Fg; }
* { outline-color: alpha(@Accent, 0.6); }
label { color: inherit; }
.muted { color: @Muted; }
.accent { color: @Accent; }
.live-red { color: @LiveRed; }
.rating { color: @Rating; }
.star-on { color: @Star; }
.bold { font-weight: 700; }
.semibold { font-weight: 600; }
.caps { font-size: 0.78em; font-weight: 700; letter-spacing: 0.04em; }

/* ---------- rail */
.rail { background-color: @Rail; padding: 14px 0 10px 0; }
.rail-button { min-width: 44px; min-height: 44px; border-radius: 12px; padding: 0; margin: 3px 10px;
  background: transparent; color: @Muted; border: none; box-shadow: none; }
.rail-button:hover { background-color: @CardHover; color: @Fg; }
.rail-button:checked { background-color: @RailActive; color: @Accent; }
.rail-button image { -gtk-icon-size: 20px; }
.rail-sep { background-color: @Line; min-height: 1px; margin: 8px 18px; }
.rail-mark { background-color: @Accent; min-width: 3px; border-radius: 2px; }

/* ---------- top bar */
.topbar-box { background-color: @Card; border-radius: 12px; border: 1px solid transparent; }
.topbar-box.focused { border-color: @Accent; }
.chip-icon { background-color: @Chip; border-radius: 8px; min-width: 30px; min-height: 30px; }
.kbd { background-color: @Chip; color: @Muted; border-radius: 6px; padding: 1px 7px; font-size: 0.8em; }
.status { color: @Accent; font-size: 0.92em; }
entry.bare, entry.bare:focus-within { background: transparent; border: none; box-shadow: none; outline: none; color: @Fg; padding: 0; min-height: 0; }
entry.bare text placeholder { color: @Muted; }

/* ---------- generic surfaces */
.panel { background-color: @Panel; border-radius: 12px; }
.panel-lg { background-color: @Panel; border-radius: 16px; }
.card { background-color: @Card; border-radius: 12px; }
.card-hover:hover { background-color: @CardHover; }
.chip { background-color: @Card; border: 1px solid @Line; border-radius: 6px; padding: 4px 10px; }
.count-pill { background-color: @Chip; color: @Muted; border-radius: 9px; padding: 1px 7px; font-size: 0.8em; }
.badge { background-color: @Badge; border-radius: 5px; padding: 1px 6px; font-size: 0.72em; font-weight: 700; }
.separator-line { background-color: @Line; min-height: 1px; min-width: 1px; }
.empty-title { color: @Accent; font-size: 1.3em; font-weight: 600; }

/* ---------- buttons */
button { background-image: none; box-shadow: none; text-shadow: none; -gtk-icon-shadow: none; color: @Fg; }
.badge { color: @Fg; }
.badge.rating { color: @Rating; } .badge.accent { color: @Accent; } .badge.live-red { color: @LiveRed; }
button.icon-button { background: transparent; border: none; color: @Fg; border-radius: 10px; min-width: 34px; min-height: 34px; padding: 0; }
button.icon-button:hover { background-color: @CardHover; }
button.icon-button:disabled { color: alpha(@Muted, 0.5); }
button.icon-button.small { min-width: 28px; min-height: 28px; }
button.icon-button.star { color: @Muted; }
button.icon-button.star.on { color: @Star; }
button.primary { background-color: @PrimaryBg; color: @PrimaryFg; border: none; border-radius: 10px; padding: 9px 20px; font-weight: 600; }
button.primary:hover { background-color: alpha(@PrimaryBg, 0.88); }
button.secondary { background-color: @Secondary; color: @Fg; border: none; border-radius: 10px; padding: 9px 20px; }
button.secondary:hover { background-color: @CardHover; }
button.primary:disabled, button.secondary:disabled { opacity: 0.55; }
button.chip-button { background-color: transparent; color: @Muted; border: 1px solid transparent; border-radius: 8px; padding: 5px 12px; min-height: 0; }
button.chip-button:hover { background-color: @CardHover; color: @Fg; }
button.chip-button.on { color: @Accent; font-weight: 700; }
button.chip-solid { background-color: @Chip; color: @Fg; border: none; border-radius: 8px; padding: 6px 14px; min-height: 0; }
button.chip-solid:hover { background-color: @CardHover; }
button.pill { background-color: @Secondary; color: @Fg; border-radius: 16px; padding: 5px 14px; border: none; }
button.pill:hover { background-color: @CardHover; }
button.bare, button.bare:hover, button.bare:active { background: transparent; border: none; padding: 0; margin: 0; min-height: 0; min-width: 0; border-radius: 12px; }
button.link { color: @Muted; background: transparent; border: none; padding: 2px 4px; }
button.link:hover { color: @Accent; }

/* segmented choices */
.segments { background-color: @Card; border-radius: 10px; padding: 3px; }
button.segment { background: transparent; color: @Muted; border: none; border-radius: 8px; padding: 6px 14px; min-height: 0; }
button.segment:hover { color: @Fg; }
button.segment:checked { background-color: @CardSelected; color: @Accent; font-weight: 600; }

/* filter chips */
.filter-chip { background-color: @Card; border: 1px solid @Line; border-radius: 17px; padding: 6px 15px; color: @Fg; }
.filter-chip.on { background-color: @CardSelected; border-color: @Accent; color: @Accent; }

/* ---------- lists */
list, listview, list.plain, .plain-list { background: transparent; color: @Fg; }
list.plain > row, .category-row { background: transparent; border-radius: 10px; padding: 0; margin: 1px 0; border: 1px solid transparent; }
list.plain > row:hover { background-color: @CardHover; }
list.plain > row:selected, list.plain > row:selected:hover { background-color: @CatSelected; color: @Accent; border-color: @Accent; }
list.channels > row { border-radius: 12px; margin: 2px 0; }
list.channels > row:selected { background-color: @CardSelected; color: @Fg; border-color: @Accent; }
.logo-box { background-color: @Chip; border-radius: 8px; }
.list-header { background-color: @Panel; border-radius: 10px; padding: 6px 6px 6px 12px; }

/* progress */
progressbar.thin trough { background-color: @Line; min-height: 4px; min-width: 0; padding: 0; border-radius: 2px; border: none; }
progressbar.thin progress { background-color: @Accent; min-height: 4px; min-width: 0; margin: 0; border-radius: 2px; border: none; }
progressbar.thin { min-height: 4px; min-width: 0; }

/* ---------- cards */
.poster-frame { background-color: @Card; border-radius: 12px; border: 2px solid transparent; }
.poster-frame.hover, button.bare:hover .poster-frame { border-color: @Accent; }
.poster-frame picture { border-radius: 10px; }
.live-card { background-color: @Card; border-radius: 12px; padding: 10px; border: 1.5px solid transparent; }
button.bare:hover .live-card, button.bare:hover .source-card { border-color: @Accent; }
.source-card { background-color: @Card; border-radius: 12px; padding: 14px; border: 1.5px solid transparent; }
.source-card.active { border-color: @Accent; }
.section-title { font-size: 1.36em; font-weight: 600; }
.page-title { font-size: 1.86em; font-weight: 600; }
.hero { background-color: @Panel; border-radius: 18px; }
.hero-title { font-size: 2.1em; font-weight: 600; }
.details-title { font-size: 2.3em; font-weight: 600; }
.episode-frame { background-color: @Card; border-radius: 10px; border: 2px solid transparent; }
button.bare:hover .episode-frame, .episode-frame.playing { border-color: @Accent; }
.watched-dot { border-radius: 14px; border: 1.6px solid white; background-color: rgba(0,0,0,0.7); color: white; min-width: 26px; min-height: 26px; }
.watched-dot.on { background-color: @Accent; border-color: @Accent; }
.play-hint { background-color: @Accent; border-radius: 22px; min-width: 44px; min-height: 44px; color: white; }
.backdrop-pic { filter: blur(30px); }
.shade { background-image: linear-gradient(to bottom, @Shade, @ShadeEnd); }
.yt-play { background-color: rgba(229,45,39,0.9); border-radius: 12px; }
.welcome-icon { background-color: @RailActive; border-radius: 28px; }
.imdb { background-color: #F5C518; color: black; font-weight: 900; border-radius: 4px; padding: 0 5px; font-size: 0.8em; }

/* ---------- player */
.player-card { background-color: @Panel; border: 1px solid @Line; border-radius: 18px; box-shadow: 0 10px 34px rgba(0,0,0,0.36); }
.player-card.fullscreen { border-radius: 0; border: none; box-shadow: none; background-color: black; padding: 0; }
.video-frame { background-color: black; border-radius: 12px; }
.video-frame.fullscreen { border-radius: 0; }
.player-bar { background-image: linear-gradient(to bottom, rgba(0,0,0,0), rgba(0,0,0,0.85)); padding: 40px 16px 8px 16px; color: white; }
.player-bar label { color: rgba(255,255,255,0.88); }
button.player-button { background: transparent; color: white; border: none; border-radius: 10px; min-width: 38px; min-height: 38px; padding: 0; }
button.player-button:hover { background-color: rgba(255,255,255,0.14); }
.osd { background-color: rgba(0,0,0,0.8); color: white; border-radius: 10px; padding: 8px 16px; }
.big-play { background-color: rgba(0,0,0,0.6); border-radius: 38px; min-width: 76px; min-height: 76px; color: white; }
.live-badge { background-color: rgba(34,197,94,0.15); color: #22C55E; border-radius: 9px; padding: 2px 8px; font-size: 0.74em; font-weight: 700; }
.on-now { color: @LiveRed; font-size: 0.72em; font-weight: 700; }
scale.seek trough { background-color: rgba(255,255,255,0.25); min-height: 4px; border-radius: 2px; }
scale.seek highlight { background-color: @Accent; min-height: 4px; border-radius: 2px; }
scale.seek slider { background-color: white; min-width: 12px; min-height: 12px; border-radius: 6px; margin: -4px; box-shadow: none; border: none; }
scale.theme trough { background-color: @Line; min-height: 4px; border-radius: 2px; }
scale.theme highlight { background-color: @Accent; min-height: 4px; border-radius: 2px; }
scale.theme slider { background-color: @Fg; min-width: 14px; min-height: 14px; border-radius: 7px; margin: -5px; border: none; box-shadow: none; }
.seek-tip { background-color: rgba(0,0,0,0.9); color: white; border-radius: 6px; padding: 2px 7px; font-size: 0.85em; }

/* timeline */
.tl-block { background-color: @Card; border: 1.5px solid @Line; border-radius: 8px; padding: 6px 6px 6px 8px; }
.tl-block.live { background-color: @CardSelected; border-color: @Accent; }
.tl-block.past { opacity: 0.55; }
.tl-now { background-color: @LiveRed; }
.tl-now-tag { background-color: @LiveRed; color: white; border-radius: 4px; padding: 0 4px; font-size: 0.72em; font-weight: 700; }
.tl-tick { background-color: @Line; }
.guide-row { background-color: @Card; border-radius: 10px; padding: 10px 12px; }
.guide-row.past { opacity: 0.5; }
.live-outline { border: 1px solid @LiveRed; color: @LiveRed; border-radius: 6px; padding: 0 7px; font-size: 0.7em; font-weight: 700; }

/* ---------- popovers / menus / dialogs */
popover > contents, popover.menu > contents { background-color: @Panel; color: @Fg; border: 1px solid @Line; border-radius: 14px; padding: 6px; box-shadow: 0 4px 22px rgba(0,0,0,0.4); }
popover > arrow { background-color: @Panel; border-color: @Line; }
.menu-header { color: @Muted; font-size: 0.76em; font-weight: 700; margin: 6px 10px; }
button.menu-row { background: transparent; border: none; border-radius: 8px; padding: 7px 10px; color: @Fg; min-height: 0; }
button.menu-row:hover { background-color: @CardHover; }
button.menu-row.checked { background-color: @CardSelected; color: @Accent; font-weight: 600; }
button.menu-row.danger { color: @LiveRed; }
.dialog-root { background-color: @Bg; color: @Fg; padding: 24px; }
.dialog-title { font-size: 1.5em; font-weight: 600; }
.field-label { color: @Muted; font-size: 0.9em; margin-top: 10px; margin-bottom: 3px; }
.error { color: @LiveRed; }
.note { color: @Accent; }

/* entries, check boxes, switches */
entry, spinbutton, passwordentry { background-color: @Card; color: @Fg; border: 1px solid @Line; border-radius: 8px; min-height: 34px; padding: 0 10px; box-shadow: none; }
entry:focus-within, passwordentry:focus-within { border-color: @Accent; box-shadow: none; outline: none; }
entry text, passwordentry text { color: @Fg; }
entry text selection, label selection { background-color: alpha(@Accent, 0.4); }
switch { background-color: @Line; border: none; border-radius: 14px; min-width: 44px; min-height: 24px; }
switch:checked { background-color: @Accent; }
switch slider { background-color: white; border-radius: 12px; min-width: 20px; min-height: 20px; border: none; box-shadow: none; }
checkbutton check, checkbutton radio { background-color: @Card; border: 1px solid @Line; color: @PrimaryFg; }
checkbutton check:checked, checkbutton radio:checked { background-color: @Accent; border-color: @Accent; color: white; }
checkbutton { color: @Fg; }
tooltip, tooltip.background { background-color: @Panel; color: @Fg; border: 1px solid @Line; border-radius: 8px; }
tooltip label { color: @Fg; }

/* scrollbars */
scrollbar { background: transparent; border: none; }
scrollbar slider { background-color: @Thumb; border-radius: 6px; min-width: 6px; min-height: 6px; border: none; }
scrollbar slider:hover { background-color: @Muted; }
scrollbar trough { background: transparent; }
scrolledwindow undershoot, scrolledwindow overshoot { background: none; }

/* settings */
.settings-panel { background-color: @Panel; border-radius: 14px; }
.settings-title { font-size: 1.7em; font-weight: 600; }
.settings-sub { font-size: 1.2em; font-weight: 600; }
.row-title { font-weight: 600; }
.row-desc { color: @Muted; font-size: 0.9em; }
.in-use { background-color: @RailActive; color: @Accent; border-radius: 6px; padding: 1px 7px; font-size: 0.7em; font-weight: 700; }
.swatch { border-radius: 16px; min-width: 30px; min-height: 30px; border: 1px solid @Line; }
.swatch-sm { border-radius: 7px; min-width: 14px; min-height: 14px; }
"#;
