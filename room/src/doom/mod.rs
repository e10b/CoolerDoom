//! Rust re-implementations of vendored doomgeneric modules.
//!
//! Each submodule replaced one `.c` file from `vendor/doomgeneric/`.
//! Functions are exported with `#[no_mangle] extern "C"` so the
//! remaining C code resolves them at final link time.

pub mod d_event;
pub mod d_mode;
pub mod doomstat;
pub mod dstrings;
pub mod dummy;
pub mod i_cdmus;
pub mod i_endoom;
pub mod i_joystick;
pub mod i_sound;
pub mod i_timer;
pub mod info;
pub mod m_argv;
pub mod m_bbox;
pub mod m_cheat;
pub mod m_controls;
pub mod m_fixed;
pub mod m_misc;
pub mod m_random;
pub mod memio;
pub mod s_sound;
pub mod sha1;
pub mod sounds;
pub mod statdump;
pub mod tables;
pub mod w_checksum;
pub mod w_file;
pub mod w_main;
