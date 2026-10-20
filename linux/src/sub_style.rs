//! Subtitle style (like YouTube's caption options): the choices, how they map onto mpv's
//! sub-* options, and the markup for the preview in Settings. Same options and defaults as the
//! Windows and Android TV apps (spec 1.1.1).

use crate::mpv::Mpv;
use crate::settings::{self, AppSettings};
use gtk::pango;
use gtk::prelude::*;
use std::sync::OnceLock;

pub const SIZES: [i32; 14] = [12, 14, 16, 18, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72];
pub const FONTS: [&str; 5] = ["Sans-serif", "Serif", "Monospace", "Casual", "Small caps"];
pub const TEXT_COLORS: [&str; 8] = [
    "White", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Red", "Black",
];
pub const BOX_COLORS: [&str; 8] = [
    "Black", "White", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Red",
];
pub const TEXT_OPACITIES: [i32; 4] = [25, 50, 75, 100];
pub const BOX_OPACITIES: [i32; 5] = [0, 25, 50, 75, 100];
pub const EDGES: [&str; 5] = ["None", "Drop shadow", "Raised", "Depressed", "Outline"];
pub const POSITIONS: [&str; 4] = ["Bottom", "Lower third", "Middle", "Top"];
/// "Move up / down" step and limit, in % of the picture height.
pub const POS_STEP: i32 = 2;
pub const POS_MIN: i32 = -20;
pub const POS_MAX: i32 = 40;

/// Restore every subtitle-style setting to its default.
pub fn reset() {
    let d = AppSettings::default();
    settings::update(|s| {
        s.sub_font_size_pt = d.sub_font_size_pt;
        s.sub_font = d.sub_font;
        s.sub_text_color = d.sub_text_color;
        s.sub_text_opacity = d.sub_text_opacity;
        s.sub_back_color = d.sub_back_color;
        s.sub_back_opacity = d.sub_back_opacity;
        s.sub_window_color = d.sub_window_color;
        s.sub_window_opacity = d.sub_window_opacity;
        s.sub_edge = d.sub_edge;
        s.sub_position = d.sub_position;
        s.sub_pos_offset = d.sub_pos_offset;
    });
}

pub fn rgb(name: &str) -> (u8, u8, u8) {
    match name {
        "Yellow" => (0xFF, 0xFF, 0x00),
        "Green" => (0x00, 0xFF, 0x00),
        "Cyan" => (0x00, 0xFF, 0xFF),
        "Blue" => (0x00, 0x00, 0xFF),
        "Magenta" => (0xFF, 0x00, 0xFF),
        "Red" => (0xFF, 0x00, 0x00),
        "Black" => (0x00, 0x00, 0x00),
        _ => (0xFF, 0xFF, 0xFF),
    }
}

/// mpv colours are #AARRGGBB.
fn mpv_color(name: &str, opacity: i32) -> String {
    let (r, g, b) = rgb(name);
    let a = ((opacity.clamp(0, 100) * 255 + 50) / 100) as u8;
    format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

fn css_color(name: &str, opacity: i32) -> String {
    let (r, g, b) = rgb(name);
    format!(
        "rgba({r},{g},{b},{:.2})",
        opacity.clamp(0, 100) as f64 / 100.0
    )
}

/// Pixels on a 1080-line picture for typographic points (1 pt = 1/72 in at 96 dpi).
pub fn px_at_1080(pt: i32) -> f64 {
    pt as f64 * 96.0 / 72.0
}

/// mpv's sub-font-size is in pixels of a 720-line window and scales with the window, so the
/// subtitles keep their size relative to the picture, windowed or full screen.
fn mpv_font_size(pt: i32) -> f64 {
    px_at_1080(pt) * 720.0 / 1080.0
}

/// sub-pos: where the bottom of the subtitles sits, in % of the picture height.
pub fn pos_percent(s: &AppSettings) -> f64 {
    let base = match s.sub_position.as_str() {
        "Lower third" => 85.0,
        "Middle" => 58.0,
        "Top" => 16.0,
        _ => 100.0,
    };
    (base - s.sub_pos_offset as f64).clamp(0.0, 100.0)
}

/// The installed font family for a choice; fontconfig takes over when none matches.
pub fn family(choice: &str) -> String {
    static INSTALLED: OnceLock<Vec<String>> = OnceLock::new();
    let installed = INSTALLED.get_or_init(|| {
        pangocairo_families()
            .into_iter()
            .map(|f| f.to_lowercase())
            .collect()
    });
    let first = |names: &[&str], fallback: &str| {
        names
            .iter()
            .find(|n| installed.contains(&n.to_lowercase()))
            .map(|n| n.to_string())
            .unwrap_or_else(|| fallback.to_string())
    };
    match choice {
        "Serif" => "serif".into(),
        "Monospace" => "monospace".into(),
        "Casual" => first(
            &[
                "Comic Neue",
                "Comic Sans MS",
                "Comic Relief",
                "Purisa",
                "Chilanka",
            ],
            "sans-serif",
        ),
        "Small caps" => first(
            &[
                "Alegreya Sans SC",
                "Cormorant SC",
                "Linux Biolinum Capitals",
                "Fira Sans SC",
            ],
            "sans-serif",
        ),
        _ => "sans-serif".into(),
    }
}

fn pangocairo_families() -> Vec<String> {
    // GTK's own font map (fontconfig), the same fonts libass finds.
    gtk::Label::new(None)
        .pango_context()
        .font_map()
        .map(|m| {
            m.list_families()
                .iter()
                .map(|f| f.name().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// (outline size, shadow offset, shadow colour) for an edge style, in mpv's 720-line pixels.
fn edge(style: &str, text_opacity: i32) -> (f64, f64, String) {
    match style {
        "None" => (0.0, 0.0, mpv_color("Black", 0)),
        "Raised" => (0.0, 1.2, mpv_color("Black", text_opacity)),
        "Depressed" => (0.0, 1.2, mpv_color("White", text_opacity * 3 / 4)),
        "Outline" => (1.8, 0.0, mpv_color("Black", 0)),
        _ => (0.0, 2.0, mpv_color("Black", text_opacity * 3 / 4)), // Drop shadow
    }
}

/// Apply the user's style to mpv now (embedded subtitles, files and downloads alike).
pub fn apply(mpv: &Mpv) {
    let s = settings::snapshot();
    // Styled ASS subtitles follow the user's choice too.
    mpv.set("sub-ass-override", "force");
    mpv.set_f64("sub-font-size", mpv_font_size(s.sub_font_size_pt));
    mpv.set("sub-font", &family(&s.sub_font));
    mpv.set(
        "sub-color",
        &mpv_color(&s.sub_text_color, s.sub_text_opacity),
    );
    mpv.set_f64("sub-pos", pos_percent(&s));
    let (outline, shadow, shadow_color) = edge(&s.sub_edge, s.sub_text_opacity);
    if s.sub_window_opacity > 0 && mpv.try_set("sub-border-style", "background-box") {
        // mpv 0.38+: one box around the whole caption area.
        mpv.set(
            "sub-back-color",
            &mpv_color(&s.sub_window_color, s.sub_window_opacity),
        );
        mpv.set_f64("sub-outline-size", outline);
        mpv.set("sub-outline-color", &mpv_color("Black", s.sub_text_opacity));
        mpv.set_f64("sub-shadow-offset", 0.0);
    } else if s.sub_window_opacity > 0 || s.sub_back_opacity > 0 {
        // A box behind each line: mpv draws it with the outline colour, padded by the outline
        // size. The edge style cannot be drawn on top of it.
        let (color, opacity, pad) = if s.sub_window_opacity > 0 {
            (&s.sub_window_color, s.sub_window_opacity, 6.0)
        } else {
            (&s.sub_back_color, s.sub_back_opacity, 2.5)
        };
        mpv.set("sub-border-style", "opaque-box");
        mpv.set("sub-outline-color", &mpv_color(color, opacity));
        mpv.set_f64("sub-outline-size", pad);
        mpv.set_f64("sub-shadow-offset", 0.0);
    } else {
        mpv.set("sub-border-style", "outline-and-shadow");
        mpv.set_f64("sub-outline-size", outline);
        mpv.set("sub-outline-color", &mpv_color("Black", s.sub_text_opacity));
        mpv.set_f64("sub-shadow-offset", shadow);
        mpv.set("sub-back-color", &shadow_color);
    }
}

// ------------------------------------------------------------------ preview

/// Pango markup for the sample line, drawn in a frame `frame_h` pixels high.
pub fn preview_markup(s: &AppSettings, text: &str, frame_h: f64) -> String {
    let px = px_at_1080(s.sub_font_size_pt) * frame_h / 1080.0;
    let (r, g, b) = rgb(&s.sub_text_color);
    let mut attrs = format!(
        "font_family='{}' size='{}' foreground='#{r:02X}{g:02X}{b:02X}' alpha='{}%'",
        family(&s.sub_font),
        (px * pango::SCALE as f64).round() as i64,
        s.sub_text_opacity.max(1),
    );
    if s.sub_font == "Small caps" {
        attrs.push_str(" font_variant='smallcaps'");
    }
    if s.sub_back_opacity > 0 {
        let (r, g, b) = rgb(&s.sub_back_color);
        attrs.push_str(&format!(
            " background='#{r:02X}{g:02X}{b:02X}' bgalpha='{}%'",
            s.sub_back_opacity
        ));
    }
    format!(
        "<span {attrs}> {} </span>",
        gtk::glib::markup_escape_text(text)
    )
}

/// CSS for the preview's edge style and window box (classes .sub-preview-text / -window).
pub fn preview_css(s: &AppSettings, frame_h: f64) -> String {
    let k = frame_h / 720.0; // mpv's sizes are 720-line pixels
    let shadow = match s.sub_edge.as_str() {
        "None" => "none".to_string(),
        "Raised" => format!("{0:.1}px {0:.1}px 0 rgba(0,0,0,0.9)", (1.2 * k).max(1.0)),
        "Depressed" => format!(
            "{0:.1}px {0:.1}px 0 rgba(255,255,255,0.7)",
            (1.2 * k).max(1.0)
        ),
        "Outline" => {
            let o = (1.8 * k).max(1.0);
            format!(
                "{o:.1}px 0 0 black, -{o:.1}px 0 0 black, 0 {o:.1}px 0 black, 0 -{o:.1}px 0 black"
            )
        }
        _ => format!(
            "{0:.1}px {0:.1}px {0:.1}px rgba(0,0,0,0.8)",
            (2.0 * k).max(1.0)
        ),
    };
    format!(
        ".sub-preview-text {{ text-shadow: {shadow}; }}\n\
         .sub-preview-window {{ background-color: {}; border-radius: 4px; padding: 4px 8px; }}",
        css_color(&s.sub_window_color, s.sub_window_opacity)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_sizes() {
        assert_eq!(mpv_color("White", 100), "#FFFFFFFF");
        assert_eq!(mpv_color("Black", 50), "#80000000");
        assert_eq!(px_at_1080(24), 32.0);
        assert!((mpv_font_size(24) - 21.333).abs() < 0.01);
    }
}
