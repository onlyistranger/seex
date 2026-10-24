//! The SeEx palette and font plumbing.
//!
//! The colour functions carry the same values the egui front end used
//! (`src/main.rs`), only expressed as GPUI [`Rgba`] instead of `Color32`, so the
//! ported pages come out pixel-for-pixel the same.

use gpui_kit::*;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// egui drew at 24pt for a 32px row; GPUI measures in pixels, so keep the same
/// reduction the old UI applied to every font size it asked for. The result is
/// rounded to whole pixels: a fractional size rasterises with blurry stems.
pub const FONT_SCALE_FROM_CURRENT: f32 = 0.75;

static DARK_MODE: AtomicBool = AtomicBool::new(false);
static THEME_PRESET: AtomicU8 = AtomicU8::new(0);

/// Corner radii. Every rectangle in the window - controls, rows and cards -
/// uses one of these, in the rounded style Windows 11 and macOS share.
pub const RADIUS_SM: f32 = 6.0;
pub const RADIUS_MD: f32 = 8.0;
pub const RADIUS_LG: f32 = 10.0;

pub fn set_dark_mode(dark: bool) {
    DARK_MODE.store(dark, Ordering::Relaxed);
}

pub fn is_dark_mode() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

pub fn detect_dark_mode() -> bool {
    matches!(dark_light::detect(), Ok(dark_light::Mode::Dark))
}

pub fn normalize_theme_name(theme: &str) -> String {
    match theme.to_ascii_lowercase().as_str() {
        "graphite" | "forest" | "amber" | "rose" | "violet" | "teal" | "cobalt" | "sunset"
        | "mint" | "indigo" | "coral" => theme.to_ascii_lowercase(),
        _ => "ocean".to_string(),
    }
}

pub fn theme_index(theme: &str) -> u8 {
    match normalize_theme_name(theme).as_str() {
        "graphite" => 1,
        "forest" => 2,
        "amber" => 3,
        "rose" => 4,
        "violet" => 5,
        "teal" => 6,
        "cobalt" => 7,
        "sunset" => 8,
        "mint" => 9,
        "indigo" => 10,
        "coral" => 11,
        _ => 0,
    }
}

pub fn set_theme_index(index: u8) {
    THEME_PRESET.store(index, Ordering::Relaxed);
}

pub fn current_theme_index() -> u8 {
    THEME_PRESET.load(Ordering::Relaxed)
}

pub fn scaled_font(size: f32) -> f32 {
    (size * FONT_SCALE_FROM_CURRENT).round()
}

fn rgb8(r: u8, g: u8, b: u8) -> Rgba {
    rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
}

pub fn primary() -> Rgba {
    const DARK: [[u8; 3]; 12] = [
        [74, 174, 214],
        [165, 174, 190],
        [84, 193, 135],
        [235, 170, 78],
        [235, 120, 145],
        [180, 135, 235],
        [72, 190, 180],
        [100, 155, 235],
        [245, 135, 95],
        [105, 205, 160],
        [125, 145, 245],
        [245, 120, 105],
    ];
    const LIGHT: [[u8; 3]; 12] = [
        [33, 137, 182],
        [92, 102, 120],
        [38, 152, 98],
        [194, 119, 34],
        [198, 76, 105],
        [121, 76, 180],
        [18, 135, 130],
        [42, 94, 180],
        [215, 91, 52],
        [38, 150, 105],
        [82, 86, 190],
        [205, 78, 72],
    ];
    let colors = if is_dark_mode() { DARK } else { LIGHT };
    let color = colors[(current_theme_index().min(11)) as usize];
    rgb8(color[0], color[1], color[2])
}

pub fn theme_tint() -> Rgba {
    if is_dark_mode() {
        app_bg()
    } else {
        match current_theme_index() {
            1 => rgb8(247, 248, 250),
            2 => rgb8(246, 251, 248),
            3 => rgb8(255, 250, 243),
            4 => rgb8(253, 246, 248),
            5 => rgb8(249, 246, 253),
            6 => rgb8(243, 251, 250),
            7 => rgb8(244, 248, 253),
            8 => rgb8(255, 247, 243),
            9 => rgb8(244, 251, 247),
            10 => rgb8(246, 247, 253),
            11 => rgb8(253, 246, 244),
            _ => rgb8(246, 248, 250),
        }
    }
}

pub fn app_bg() -> Rgba {
    if is_dark_mode() {
        rgb8(24, 27, 31)
    } else {
        match current_theme_index() {
            1 => rgb8(246, 247, 249),
            2 => rgb8(244, 249, 246),
            3 => rgb8(252, 248, 242),
            4 => rgb8(252, 244, 247),
            5 => rgb8(247, 243, 252),
            6 => rgb8(241, 249, 248),
            7 => rgb8(242, 247, 252),
            8 => rgb8(253, 244, 240),
            9 => rgb8(241, 249, 244),
            10 => rgb8(243, 244, 251),
            11 => rgb8(252, 243, 241),
            _ => rgb8(246, 248, 250),
        }
    }
}

pub fn surface() -> Rgba {
    if is_dark_mode() {
        rgb8(32, 36, 42)
    } else {
        theme_tint()
    }
}

pub fn row_bg() -> Rgba {
    if is_dark_mode() {
        rgb8(38, 43, 50)
    } else {
        rgb8(250, 252, 253)
    }
}

pub fn border() -> Rgba {
    if is_dark_mode() {
        rgb8(55, 62, 72)
    } else {
        rgb8(224, 229, 235)
    }
}

pub fn subtle_border() -> Rgba {
    if is_dark_mode() {
        rgb8(47, 54, 63)
    } else {
        rgb8(235, 239, 244)
    }
}

pub fn success() -> Rgba {
    if is_dark_mode() {
        rgb8(84, 193, 135)
    } else {
        rgb8(38, 152, 98)
    }
}

pub fn danger() -> Rgba {
    if is_dark_mode() {
        rgb8(245, 109, 120)
    } else {
        rgb8(210, 65, 82)
    }
}

pub fn muted() -> Rgba {
    if is_dark_mode() {
        rgb8(166, 176, 188)
    } else {
        rgb8(99, 111, 125)
    }
}

pub fn text_color() -> Rgba {
    if is_dark_mode() {
        rgb8(235, 239, 244)
    } else {
        rgb8(27, 34, 43)
    }
}

pub fn field_bg() -> Rgba {
    if is_dark_mode() {
        rgb8(38, 43, 50)
    } else {
        rgb8(250, 252, 253)
    }
}

pub fn widget_hover_bg() -> Rgba {
    if is_dark_mode() {
        rgb8(48, 55, 64)
    } else {
        rgb8(239, 246, 250)
    }
}

pub fn top_bar_button_bg() -> Rgba {
    if is_dark_mode() {
        rgb8(36, 41, 48)
    } else {
        rgb8(246, 248, 250)
    }
}

pub fn top_bar_hover() -> Rgba {
    if is_dark_mode() {
        rgb8(48, 55, 64)
    } else {
        rgb8(235, 241, 246)
    }
}

pub fn chip_bg(color: Rgba) -> Rgba {
    let alpha = if is_dark_mode() {
        42.0 / 255.0
    } else {
        24.0 / 255.0
    };
    Rgba { a: alpha, ..color }
}

/// The filled part of a progress bar: strong enough to read as progress, light
/// enough that the message printed over it stays legible.
pub fn progress_fill() -> Rgba {
    let alpha = if is_dark_mode() { 0.55 } else { 0.40 };
    Rgba {
        a: alpha,
        ..primary()
    }
}

pub fn white() -> Rgba {
    rgb(0xffffff)
}

/// A `#RRGGBB` triple as the `Hsla` the colour picker works in.
pub fn hsla_of(color: [u8; 3]) -> Hsla {
    let packed = ((color[0] as u32) << 16) | ((color[1] as u32) << 8) | color[2] as u32;
    rgb(packed).into()
}

/// The reverse, dropping the alpha: the exporter and the config both store
/// three opaque bytes.
pub fn rgb_of(color: Hsla) -> [u8; 3] {
    let rgba: Rgba = color.into();
    let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
    [channel(rgba.r), channel(rgba.g), channel(rgba.b)]
}
