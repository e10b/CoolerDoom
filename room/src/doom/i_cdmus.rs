#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_int;

#[no_mangle]
pub static mut cd_Error: c_int = 0;

#[no_mangle]
pub extern "C" fn I_CDMusInit() -> c_int {
    unsafe {
        cd_Error = 0;
    }
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusPrintStartup() {}

#[no_mangle]
pub extern "C" fn I_CDMusPlay(_track: c_int) -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusStop() -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusResume() -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusSetVolume(_volume: c_int) -> c_int {
    unsafe {
        cd_Error = 0;
    }
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusFirstTrack() -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusLastTrack() -> c_int {
    0
}

#[no_mangle]
pub extern "C" fn I_CDMusTrackLength(_track_num: c_int) -> c_int {
    0
}
