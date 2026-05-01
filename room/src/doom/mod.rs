//! Rust re-implementations of vendored doomgeneric modules.
//!
//! Each submodule replaces one `.c` file from `vendor/doomgeneric/`.
//! Functions are exported with `#[no_mangle] extern "C"` so the
//! remaining C code resolves them at final link time.

pub mod m_bbox;
pub mod m_fixed;
pub mod m_random;
