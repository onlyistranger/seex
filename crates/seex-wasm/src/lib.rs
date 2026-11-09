use std::slice;

#[path = "../../../src/step_logo.rs"]
pub mod step_logo;

use step_logo::{CleanMode, LogoOutcome, strip_logo_with_mode};

/// Allocate memory buffer in WASM linear memory
#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Deallocate memory buffer previously allocated by `alloc`
#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    if !ptr.is_null() && size > 0 {
        unsafe {
            let _ = Vec::from_raw_parts(ptr, size, size);
        }
    }
}

/// Clean STEP 3D model in WebAssembly
///
/// Parameters:
/// - `data_ptr`: pointer to raw STEP file bytes
/// - `data_len`: length of raw STEP file bytes
/// - `mode`: 0 = LogoOnly (EasyEDA/LCEDA only), 1 = AllMarkings (with Pin 1 protection)
/// - `out_meta`: pointer to array of 5 x u32:
///     [0] = result byte length
///     [1] = logos/markings removed
///     [2] = pieces removed
///     [3] = faces removed
///     [4] = outcome code (0: Removed, 1: NotFound, 2: Kept, 3: Unreadable)
///
/// Returns:
/// Pointer to cleaned bytes buffer (must be freed with `dealloc`), or null if failed.
#[unsafe(no_mangle)]
pub extern "C" fn clean_step(
    data_ptr: *const u8,
    data_len: usize,
    mode: u32,
    out_meta: *mut u32,
) -> *mut u8 {
    if data_ptr.is_null() || data_len == 0 || out_meta.is_null() {
        return std::ptr::null_mut();
    }

    let input_bytes = unsafe { slice::from_raw_parts(data_ptr, data_len) };
    let clean_mode = match mode {
        1 => CleanMode::AllMarkings,
        _ => CleanMode::LogoOnly,
    };

    let (cleaned_opt, outcome) = strip_logo_with_mode(input_bytes, clean_mode);

    let (logos, pieces, faces, outcome_code) = match &outcome {
        LogoOutcome::Removed(rem) => (rem.logos as u32, rem.pieces as u32, rem.faces as u32, 0u32),
        LogoOutcome::NotFound => (0, 0, 0, 1u32),
        LogoOutcome::Kept => (0, 0, 0, 2u32),
        LogoOutcome::Unreadable => (0, 0, 0, 3u32),
    };

    let result_vec = cleaned_opt.unwrap_or_else(|| input_bytes.to_vec());
    let result_len = result_vec.len();

    unsafe {
        let meta = slice::from_raw_parts_mut(out_meta, 5);
        meta[0] = result_len as u32;
        meta[1] = logos;
        meta[2] = pieces;
        meta[3] = faces;
        meta[4] = outcome_code;
    }

    let mut buf = result_vec.into_boxed_slice();
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Restore standard footprint name from LCSC/EasyEDA title
/// E.g. "SOT-223_C6186" -> "SOT-223", "C0603_L1_6-W0_8" -> "C0603_L1.6-W0.8"
#[unsafe(no_mangle)]
pub extern "C" fn restore_footprint_name(
    title_ptr: *const u8,
    title_len: usize,
    replace_dot: bool,
    out_len: *mut usize,
) -> *mut u8 {
    if title_ptr.is_null() || title_len == 0 || out_len.is_null() {
        return std::ptr::null_mut();
    }

    let slice_bytes = unsafe { slice::from_raw_parts(title_ptr, title_len) };
    let Ok(s) = std::str::from_utf8(slice_bytes) else {
        return std::ptr::null_mut();
    };

    let cleaned = clean_footprint_name(s, replace_dot);
    let bytes = cleaned.into_bytes();
    unsafe {
        *out_len = bytes.len();
    }

    let mut buf = bytes.into_boxed_slice();
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Restore standard 3D STEP model name
#[unsafe(no_mangle)]
pub extern "C" fn restore_step_model_name(
    title_ptr: *const u8,
    title_len: usize,
    replace_dot: bool,
    out_len: *mut usize,
) -> *mut u8 {
    if title_ptr.is_null() || title_len == 0 || out_len.is_null() {
        return std::ptr::null_mut();
    }

    let slice_bytes = unsafe { slice::from_raw_parts(title_ptr, title_len) };
    let Ok(s) = std::str::from_utf8(slice_bytes) else {
        return std::ptr::null_mut();
    };

    let cleaned = clean_step_name(s, replace_dot);
    let bytes = cleaned.into_bytes();
    unsafe {
        *out_len = bytes.len();
    }

    let mut buf = bytes.into_boxed_slice();
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Get SeEx WebAssembly Engine Version
#[unsafe(no_mangle)]
pub extern "C" fn get_version(out_len: *mut usize) -> *const u8 {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    if !out_len.is_null() {
        unsafe {
            *out_len = VERSION.len();
        }
    }
    VERSION.as_ptr()
}

// ---------------- Helper algorithms ----------------

fn clean_footprint_name(title: &str, replace_dot: bool) -> String {
    let trimmed = title.trim();
    let base = strip_c_suffix(trimmed);
    if replace_dot {
        restore_dot_in_numbers(&base)
    } else {
        base
    }
}

fn clean_step_name(title: &str, replace_dot: bool) -> String {
    let mut s = title.trim().to_string();
    if s.to_ascii_lowercase().ends_with(".step") {
        s.truncate(s.len() - 5);
    }
    let base = strip_c_suffix(&s);
    let restored = if replace_dot {
        restore_dot_in_numbers(&base)
    } else {
        base
    };
    format!("{restored}.step")
}

fn strip_c_suffix(s: &str) -> String {
    let re = regex::Regex::new(r"(?i)_c\d+$").unwrap();
    re.replace(s, "").to_string()
}

fn restore_dot_in_numbers(s: &str) -> String {
    let re = regex::Regex::new(r"(\d)_(\d)").unwrap();
    re.replace_all(s, "${1}.${2}").to_string()
}
