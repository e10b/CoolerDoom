//! Rust port of vendor/doomgeneric/hu_lib.c.
//!
//! Heads-up text and input code: text lines, scrolling text, and input widgets.

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint, c_void};

use crate::doom::v_video::patch_t;

// ── Constants ─────────────────────────────────────────────────────────

const HU_MAXLINES: usize = 4;
const HU_MAXLINELENGTH: usize = 80;
const SCREENWIDTH: c_int = 320;

// ── Widget structs (from hu_lib.h) ────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
pub struct hu_textline_t {
    pub x: c_int,
    pub y: c_int,
    pub f: *mut *mut patch_t,
    pub sc: c_int,
    pub l: [c_char; HU_MAXLINELENGTH + 1],
    pub len: c_int,
    pub needsupdate: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct hu_stext_t {
    pub l: [hu_textline_t; HU_MAXLINES],
    pub h: c_int,
    pub cl: c_int,
    pub on: *mut c_int,
    pub laston: c_int,
    _pad: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct hu_itext_t {
    pub l: hu_textline_t,
    pub lm: c_int,
    _pad0: [u8; 4],
    pub on: *mut c_int,
    pub laston: c_int,
    _pad1: [u8; 4],
}

// ── Layout checks ─────────────────────────────────────────────────────

#[cfg(target_pointer_width = "64")]
mod layout_checks {
    use super::*;
    const _: () = assert!(std::mem::size_of::<hu_textline_t>() == 112);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, x) == 0);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, y) == 4);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, f) == 8);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, sc) == 16);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, l) == 20);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, len) == 104);
    const _: () = assert!(std::mem::offset_of!(hu_textline_t, needsupdate) == 108);

    const _: () = assert!(std::mem::size_of::<hu_stext_t>() == 472);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, l) == 0);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, h) == 448);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, cl) == 452);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, on) == 456);
    const _: () = assert!(std::mem::offset_of!(hu_stext_t, laston) == 464);

    const _: () = assert!(std::mem::size_of::<hu_itext_t>() == 136);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, l) == 0);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, lm) == 112);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, on) == 120);
    const _: () = assert!(std::mem::offset_of!(hu_itext_t, laston) == 128);
}

// ── External declarations ─────────────────────────────────────────────

extern "C" {
    fn V_DrawPatchDirect(x: c_int, y: c_int, patch: *mut patch_t);
    fn R_VideoErase(ofs: c_uint, count: c_int);
    static mut automapactive: c_int;
    static mut viewwindowx: c_int;
    static mut viewwindowy: c_int;
    static mut viewwidth: c_int;
    static mut viewheight: c_int;
}

/// Byte-swap for little-endian (SHORT macro from i_swap.h).
#[inline(always)]
fn short_swap(v: i16) -> i16 {
    v
}

// ── textline code ─────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn HUlib_init() {}

#[no_mangle]
pub extern "C" fn HUlib_clearTextLine(t: *mut hu_textline_t) {
    unsafe {
        (*t).len = 0;
        (*t).l[0] = 0;
        (*t).needsupdate = 1;
    }
}

#[no_mangle]
pub extern "C" fn HUlib_initTextLine(
    t: *mut hu_textline_t,
    x: c_int,
    y: c_int,
    f: *mut *mut patch_t,
    sc: c_int,
) {
    unsafe {
        (*t).x = x;
        (*t).y = y;
        (*t).f = f;
        (*t).sc = sc;
        HUlib_clearTextLine(t);
    }
}

#[no_mangle]
pub extern "C" fn HUlib_addCharToTextLine(t: *mut hu_textline_t, ch: c_char) -> c_int {
    unsafe {
        if (*t).len == HU_MAXLINELENGTH as c_int {
            0
        } else {
            (*t).l[(*t).len as usize] = ch;
            (*t).len += 1;
            (*t).l[(*t).len as usize] = 0;
            (*t).needsupdate = 4;
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_delCharFromTextLine(t: *mut hu_textline_t) -> c_int {
    unsafe {
        if (*t).len == 0 {
            0
        } else {
            (*t).len -= 1;
            (*t).l[(*t).len as usize] = 0;
            (*t).needsupdate = 4;
            1
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_drawTextLine(l: *mut hu_textline_t, drawcursor: c_int) {
    unsafe {
        let mut x = (*l).x;
        let f = (*l).f;
        let sc = (*l).sc;

        for i in 0..(*l).len as usize {
            let c = ((*l).l[i] as u8).to_ascii_uppercase();
            if c != b' ' && c >= sc as u8 && c <= b'_' {
                let patch = *f.add((c as c_int - sc) as usize);
                let w = short_swap((*patch).width) as c_int;
                if x + w > SCREENWIDTH {
                    break;
                }
                V_DrawPatchDirect(x, (*l).y, patch);
                x += w;
            } else {
                x += 4;
                if x >= SCREENWIDTH {
                    break;
                }
            }
        }

        if drawcursor != 0 {
            let cursor_patch = *f.add((b'_' as c_int - sc) as usize);
            let cursor_w = short_swap((*cursor_patch).width) as c_int;
            if x + cursor_w <= SCREENWIDTH {
                V_DrawPatchDirect(x, (*l).y, cursor_patch);
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_eraseTextLine(l: *mut hu_textline_t) {
    unsafe {
        if automapactive == 0 && viewwindowx != 0 && (*l).needsupdate != 0 {
            let lh = short_swap((**(*l).f).height) as c_int + 1;
            for y in (*l).y..(*l).y + lh {
                let yoffset = y * SCREENWIDTH;
                if y < viewwindowy || y >= viewwindowy + viewheight {
                    R_VideoErase(yoffset as c_uint, SCREENWIDTH);
                } else {
                    R_VideoErase(yoffset as c_uint, viewwindowx);
                    R_VideoErase((yoffset + viewwindowx + viewwidth) as c_uint, viewwindowx);
                }
            }
        }

        if (*l).needsupdate != 0 {
            (*l).needsupdate -= 1;
        }
    }
}

// ── Scrolling Text window widget routines ─────────────────────────────

#[no_mangle]
pub extern "C" fn HUlib_initSText(
    s: *mut hu_stext_t,
    x: c_int,
    y: c_int,
    h: c_int,
    font: *mut *mut patch_t,
    startchar: c_int,
    on: *mut c_int,
) {
    unsafe {
        (*s).h = h;
        (*s).on = on;
        (*s).laston = 1;
        (*s).cl = 0;
        let font_h = short_swap((**font).height) as c_int + 1;
        for i in 0..h as usize {
            HUlib_initTextLine(
                &mut (*s).l[i],
                x,
                y - (i as c_int) * font_h,
                font,
                startchar,
            );
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_addLineToSText(s: *mut hu_stext_t) {
    unsafe {
        (*s).cl += 1;
        if (*s).cl == (*s).h {
            (*s).cl = 0;
        }
        HUlib_clearTextLine(&mut (*s).l[(*s).cl as usize]);

        for i in 0..(*s).h as usize {
            (*s).l[i].needsupdate = 4;
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_addMessageToSText(
    s: *mut hu_stext_t,
    prefix: *mut c_char,
    msg: *mut c_char,
) {
    unsafe {
        HUlib_addLineToSText(s);
        if !prefix.is_null() {
            let mut p = prefix;
            while *p != 0 {
                HUlib_addCharToTextLine(&mut (*s).l[(*s).cl as usize], *p);
                p = p.add(1);
            }
        }
        let mut m = msg;
        while *m != 0 {
            HUlib_addCharToTextLine(&mut (*s).l[(*s).cl as usize], *m);
            m = m.add(1);
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_drawSText(s: *mut hu_stext_t) {
    unsafe {
        if *(*s).on == 0 {
            return;
        }
        for i in 0..(*s).h as usize {
            let mut idx = (*s).cl as isize - i as isize;
            if idx < 0 {
                idx += (*s).h as isize;
            }
            HUlib_drawTextLine(&mut (*s).l[idx as usize], 0);
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_eraseSText(s: *mut hu_stext_t) {
    unsafe {
        for i in 0..(*s).h as usize {
            if (*s).laston != 0 && *(*s).on == 0 {
                (*s).l[i].needsupdate = 4;
            }
            HUlib_eraseTextLine(&mut (*s).l[i]);
        }
        (*s).laston = if *(*s).on != 0 { 1 } else { 0 };
    }
}

// ── Input Text Line widget routines ───────────────────────────────────

#[no_mangle]
pub extern "C" fn HUlib_initIText(
    it: *mut hu_itext_t,
    x: c_int,
    y: c_int,
    font: *mut *mut patch_t,
    startchar: c_int,
    on: *mut c_int,
) {
    unsafe {
        (*it).lm = 0;
        (*it).on = on;
        (*it).laston = 1;
        HUlib_initTextLine(&mut (*it).l, x, y, font, startchar);
    }
}

#[no_mangle]
pub extern "C" fn HUlib_delCharFromIText(it: *mut hu_itext_t) {
    unsafe {
        if (*it).l.len != (*it).lm {
            HUlib_delCharFromTextLine(&mut (*it).l);
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_eraseLineFromIText(it: *mut hu_itext_t) {
    unsafe {
        while (*it).lm != (*it).l.len {
            HUlib_delCharFromTextLine(&mut (*it).l);
        }
    }
}

#[no_mangle]
pub extern "C" fn HUlib_resetIText(it: *mut hu_itext_t) {
    unsafe {
        (*it).lm = 0;
        HUlib_clearTextLine(&mut (*it).l);
    }
}

#[no_mangle]
pub extern "C" fn HUlib_addPrefixToIText(it: *mut hu_itext_t, str: *mut c_char) {
    unsafe {
        let mut p = str;
        while *p != 0 {
            HUlib_addCharToTextLine(&mut (*it).l, *p);
            p = p.add(1);
        }
        (*it).lm = (*it).l.len;
    }
}

#[no_mangle]
pub extern "C" fn HUlib_keyInIText(it: *mut hu_itext_t, ch: u8) -> c_int {
    unsafe {
        let ch = ch.to_ascii_uppercase();
        if ch >= b' ' && ch <= b'_' {
            HUlib_addCharToTextLine(&mut (*it).l, ch as c_char);
        } else if ch == crate::doom::doomkeys::KEY_BACKSPACE {
            HUlib_delCharFromIText(it);
        } else if ch != crate::doom::doomkeys::KEY_ENTER {
            return 0;
        }
        1
    }
}

#[no_mangle]
pub extern "C" fn HUlib_drawIText(it: *mut hu_itext_t) {
    unsafe {
        if *(*it).on == 0 {
            return;
        }
        HUlib_drawTextLine(&mut (*it).l, 1);
    }
}

#[no_mangle]
pub extern "C" fn HUlib_eraseIText(it: *mut hu_itext_t) {
    unsafe {
        if (*it).laston != 0 && *(*it).on == 0 {
            (*it).l.needsupdate = 4;
        }
        HUlib_eraseTextLine(&mut (*it).l);
        (*it).laston = if *(*it).on != 0 { 1 } else { 0 };
    }
}

// ── Layout assertions ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    const HU_TEXTLINE_T_SIZEOF: usize = 112;
    const HU_STEXT_T_SIZEOF: usize = 472;
    const HU_ITEXT_T_SIZEOF: usize = 136;

    #[test]
    fn hu_textline_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<hu_textline_t>(), HU_TEXTLINE_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(hu_textline_t, x), 0);
        assert_eq!(std::mem::offset_of!(hu_textline_t, y), 4);
        assert_eq!(std::mem::offset_of!(hu_textline_t, f), 8);
        assert_eq!(std::mem::offset_of!(hu_textline_t, sc), 16);
        assert_eq!(std::mem::offset_of!(hu_textline_t, l), 20);
        assert_eq!(std::mem::offset_of!(hu_textline_t, len), 104);
        assert_eq!(std::mem::offset_of!(hu_textline_t, needsupdate), 108);
    }

    #[test]
    fn hu_stext_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<hu_stext_t>(), HU_STEXT_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(hu_stext_t, l), 0);
        assert_eq!(std::mem::offset_of!(hu_stext_t, h), 448);
        assert_eq!(std::mem::offset_of!(hu_stext_t, cl), 452);
        assert_eq!(std::mem::offset_of!(hu_stext_t, on), 456);
        assert_eq!(std::mem::offset_of!(hu_stext_t, laston), 464);
    }

    #[test]
    fn hu_itext_t_layout_matches_c() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<hu_itext_t>(), HU_ITEXT_T_SIZEOF);
        assert_eq!(std::mem::offset_of!(hu_itext_t, l), 0);
        assert_eq!(std::mem::offset_of!(hu_itext_t, lm), 112);
        assert_eq!(std::mem::offset_of!(hu_itext_t, on), 120);
        assert_eq!(std::mem::offset_of!(hu_itext_t, laston), 128);
    }
}
