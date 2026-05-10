//! Rust port of vendor/doomgeneric/dummy.c.
//!
//! Provides networking stub globals and sound stub function.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_int;

// `boolean` in doomtype.h is `typedef unsigned int boolean;` — use c_int.
#[no_mangle]
pub static mut net_client_connected: c_int = 0; // false

#[no_mangle]
pub static mut drone: c_int = 0; // false

#[no_mangle]
pub extern "C" fn I_InitTimidityConfig() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn globals_default_to_zero() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert_eq!(net_client_connected, 0);
            assert_eq!(drone, 0);
        }
    }

    #[test]
    fn timidity_stub_does_not_panic() {
        I_InitTimidityConfig();
    }
}
