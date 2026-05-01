#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};
use std::ptr;

// Pull in the p_lights, p_telept, and p_sight anchors so all #[no_mangle] functions
// survive link-time dead-code elimination (they are only called from C).
use super::p_lights::P_Lights_Link_Anchor;
use super::p_telept::P_Telept_Link_Anchor;
use super::p_sight::P_Sight_Link_Anchor;

// Constants matching DOOMGENERIC_RESX * DOOMGENERIC_RESY from doomgeneric.h
const DOOMGENERIC_RESX: usize = 640;
const DOOMGENERIC_RESY: usize = 400;

#[no_mangle]
pub static mut DG_ScreenBuffer: *mut u32 = ptr::null_mut();

extern "C" {
    fn M_FindResponseFile();
    fn DG_Init();
    fn D_DoomMain();
}

extern "C" {
    static mut myargc: c_int;
    static mut myargv: *mut *mut c_char;
}

#[no_mangle]
pub unsafe extern "C" fn doomgeneric_Create(argc: c_int, argv: *mut *mut c_char) {
    // Anchor p_lights, p_telept, and p_sight symbols so they survive LTO (called only from C).
    P_Lights_Link_Anchor();
    P_Telept_Link_Anchor();
    P_Sight_Link_Anchor();

    myargc = argc;
    myargv = argv;

    M_FindResponseFile();

    let total_pixels = DOOMGENERIC_RESX * DOOMGENERIC_RESY;
    let mut buffer = vec![0u32; total_pixels];
    DG_ScreenBuffer = buffer.as_mut_ptr();
    std::mem::forget(buffer);

    DG_Init();
    D_DoomMain();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dg_screenbuffer_is_null_init() {
        unsafe {
            assert!(DG_ScreenBuffer.is_null());
        }
    }
}
