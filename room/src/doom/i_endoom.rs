#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_char;

#[no_mangle]
pub extern "C" fn I_Endoom(_endoom_data: *mut c_char) {}
