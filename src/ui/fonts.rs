//! Font selection, in the shape CAPS uses: two slots, a search box, and a
//! fallback chain for Han characters.
//!
//! GPUI can list what the system has installed (`cx.text_system().all_font_names()`),
//! which is why SeEx moved off egui; the lists here are ordered so the current
//! choice sits first, then the faces that actually suit the script, then
//! everything else alphabetically — nothing is hidden.

use gpui_kit::*;
use std::sync::Mutex;

static ACTIVE: Mutex<Option<Font>> = Mutex::new(None);

/// Remembers the face the window is drawn in, so pieces GPUI paints outside the
/// view tree — the kit's tooltips — can use the same one.
pub fn remember(font: Font) {
    if let Ok(mut active) = ACTIVE.lock() {
        *active = Some(font);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FontSlot {
    Latin,
    Han,
}

/// Faces worth reaching for first in each slot.
const LATIN_CANDIDATES: [&str; 10] = [
    "Segoe UI Variable Text",
    "Segoe UI Variable Display",
    "Segoe UI",
    "Inter",
    "Noto Sans",
    "Roboto",
    "Arial",
    "Helvetica Neue",
    "Source Sans 3",
    "Calibri",
];

const HAN_CANDIDATES: [&str; 8] = [
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Noto Sans SC",
    "Source Han Sans SC",
    "MiSans",
    "PingFang SC",
    "SimHei",
    "SimSun",
];

/// Consulted only for characters the Latin family cannot draw.
pub const HAN_FALLBACKS: [&str; 6] = [
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Noto Sans SC",
    "Source Han Sans SC",
    "SimSun",
    "SimHei",
];

/// The full list for one slot, ordered and de-duplicated.
pub fn choices(installed: &[String], slot: FontSlot, current: Option<&str>) -> Vec<String> {
    let recommended: &[&str] = match slot {
        FontSlot::Latin => &LATIN_CANDIDATES,
        FontSlot::Han => &HAN_CANDIDATES,
    };

    let mut ordered: Vec<String> = Vec::with_capacity(installed.len());
    let push = |name: &str, ordered: &mut Vec<String>| {
        let present = installed.iter().any(|font| font == name);
        let already = ordered.iter().any(|existing| existing == name);
        if present && !already {
            ordered.push(name.to_string());
        }
    };

    if let Some(current) = current {
        push(current, &mut ordered);
    }
    for name in recommended {
        push(name, &mut ordered);
    }

    let mut rest: Vec<String> = installed
        .iter()
        .filter(|name| !ordered.iter().any(|existing| existing == *name))
        .cloned()
        .collect();
    rest.sort_by_key(|name| name.to_lowercase());
    ordered.append(&mut rest);
    ordered
}

/// The chosen Han face followed by the stand-ins to try after it.
pub fn han_chain(picked: Option<&str>) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    if let Some(picked) = picked {
        chain.push(picked.to_string());
    }
    for name in HAN_FALLBACKS {
        if Some(name) != picked && !chain.iter().any(|existing| existing == name) {
            chain.push(name.to_string());
        }
    }
    chain
}

/// The text style the whole window is drawn with.
pub fn ui_font(family: SharedString, fallbacks: FontFallbacks) -> Font {
    Font {
        family,
        fallbacks: Some(fallbacks),
        ..Default::default()
    }
}

/// How many rows the picker shows at once.
pub fn matches(choices: &[String], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    choices
        .iter()
        .enumerate()
        .filter(|(_, name)| query.is_empty() || name.to_lowercase().contains(&query))
        .map(|(index, _)| index)
        .collect()
}
