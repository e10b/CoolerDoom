//! Rust port of vendor/doomgeneric/m_fixed.c.
//!
//! Provides 16.16 fixed-point multiplication and division with the exact
//! same overflow / saturation semantics as the original C code.

#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::c_int;

/// `fixed_t` — matches `typedef int fixed_t;` in m_fixed.h.
pub type fixed_t = c_int;

const FRACBITS: u32 = 16;

/// Exported to C as `FixedMul`. Consumed by ~20 vendored .c files.
#[no_mangle]
pub extern "C" fn FixedMul(a: fixed_t, b: fixed_t) -> fixed_t {
    ((a as i64 * b as i64) >> FRACBITS) as fixed_t
}

/// Exported to C as `FixedDiv`. Matches the C overflow/saturation path.
///
/// C's `abs(INT_MIN)` is UB; Rust's `wrapping_abs()` returns `INT_MIN`,
/// which still triggers the saturation branch — same effective behaviour
/// but without UB or panics.
#[no_mangle]
pub extern "C" fn FixedDiv(a: fixed_t, b: fixed_t) -> fixed_t {
    if (a.wrapping_abs() >> 14) >= b.wrapping_abs() {
        if (a ^ b) < 0 {
            i32::MIN
        } else {
            i32::MAX
        }
    } else {
        (((a as i64) << 16) / b as i64) as fixed_t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_identity() {
        assert_eq!(FixedMul(1 << 16, 1 << 16), 1 << 16);
    }

    #[test]
    fn mul_half_times_two() {
        assert_eq!(FixedMul(1 << 15, 2 << 16), 1 << 16);
    }

    #[test]
    fn mul_negative() {
        assert_eq!(FixedMul(-(1 << 16), 1 << 16), -(1 << 16));
    }

    #[test]
    fn div_one() {
        assert_eq!(FixedDiv(3 << 16, 1 << 16), 3 << 16);
    }

    #[test]
    fn div_saturates_pos() {
        assert_eq!(FixedDiv(i32::MAX, 1), i32::MAX);
    }

    #[test]
    fn div_saturates_neg() {
        assert_eq!(FixedDiv(i32::MAX, -1), i32::MIN);
    }

    #[test]
    fn div_min_no_panic() {
        let _ = FixedDiv(i32::MIN, 1 << 16);
    }
}
