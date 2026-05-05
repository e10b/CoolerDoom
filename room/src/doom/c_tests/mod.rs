//! Unit tests that call original C functions directly.
//!
//! Each submodule tests a specific unported C module, establishing
//! behavioral baselines before the module is ported to Rust.

mod harness;
mod lookup_tables;
mod p_maputl_c;
mod struct_layouts;
mod wrapping_arithmetic;
