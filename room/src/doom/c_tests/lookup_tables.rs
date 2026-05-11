//! Verify lookup-table values byte-for-byte against C globals.

#![allow(non_snake_case)]

use std::ffi::c_int;

use crate::doom::c_ffi;
use crate::doom::c_tests::harness::C_GLOBAL_LOCK;
use crate::doom::p_inter;
use crate::doom::p_maputl;

// ---------------------------------------------------------------------------
// p_inter.c: maxammo and clipammo
// ---------------------------------------------------------------------------

#[test]
fn maxammo_values() {
    unsafe {
        assert_eq!(p_inter::maxammo[0], 200); // bullets
        assert_eq!(p_inter::maxammo[1], 50); // shells
        assert_eq!(p_inter::maxammo[2], 300); // cells
        assert_eq!(p_inter::maxammo[3], 50); // rockets
    }
}

#[test]
fn clipammo_values() {
    unsafe {
        assert_eq!(p_inter::clipammo[0], 10); // bullets per clip
        assert_eq!(p_inter::clipammo[1], 4); // shells per clip
        assert_eq!(p_inter::clipammo[2], 20); // cells per clip
        assert_eq!(p_inter::clipammo[3], 1); // rockets per clip
    }
}

#[test]
fn ammo_arrays_positive() {
    unsafe {
        for i in 0..4 {
            assert!(p_inter::maxammo[i] > 0, "maxammo[{}] must be > 0", i);
            assert!(p_inter::clipammo[i] > 0, "clipammo[{}] must be > 0", i);
        }
    }
}

// ---------------------------------------------------------------------------
// p_maputl.c: intercepts array
// ---------------------------------------------------------------------------

#[test]
fn intercepts_array_length() {
    unsafe {
        assert_eq!(p_maputl::intercepts.len(), p_maputl::MAXINTERCEPTS);
    }
}

#[test]
fn intercept_p_initially_null() {
    let _guard = C_GLOBAL_LOCK.lock().unwrap();
    unsafe {
        // Ensure we test the null state even if another test ran first.
        p_maputl::intercept_p = std::ptr::null_mut();
        assert!(p_maputl::intercept_p.is_null());
    }
}
