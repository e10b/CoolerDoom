//! Rust port of vendor/doomgeneric/dummy.c.
//!
//! Provides networking stub globals and sound stub function.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_uint;

// `boolean` in doomtype.h is `typedef unsigned int boolean;` — use c_uint.
#[no_mangle]
pub static mut net_client_connected: c_uint = 0; // false

#[no_mangle]
pub static mut drone: c_uint = 0; // false

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
    fn globals_store_unsigned_values() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            net_client_connected = c_uint::MAX;
            drone = c_uint::MAX;
            assert_eq!(net_client_connected, c_uint::MAX);
            assert_eq!(drone, c_uint::MAX);
            net_client_connected = 0;
            drone = 0;
        }
    }

    #[test]
    fn timidity_stub_does_not_panic() {
        I_InitTimidityConfig();
    }
}
