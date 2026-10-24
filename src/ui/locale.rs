//! Language selection.
//!
//! The switch lives on the Font page now instead of the top bar, so the choice
//! is kept in one global that any page or component can read; the strings
//! themselves stay next to their use as `t("English", "中文")` pairs, which
//! keeps a hover label single-language instead of the old hand-written
//! bilingual blobs.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(lang: Lang) {
    LANG.store(
        match lang {
            Lang::En => 0,
            Lang::Zh => 1,
        },
        Ordering::Relaxed,
    );
}

pub fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::Zh
    } else {
        Lang::En
    }
}

/// Picks the string for the active language.
pub fn t(en: &'static str, zh: &'static str) -> &'static str {
    match lang() {
        Lang::En => en,
        Lang::Zh => zh,
    }
}
