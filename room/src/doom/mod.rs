//! Rust re-implementations of vendored doomgeneric modules.
//!
//! Each submodule replaced one `.c` file from `vendor/doomgeneric/`.
//! Functions are exported with `#[no_mangle] extern "C"` so the
//! remaining C code resolves them at final link time.

pub mod c_ffi;

pub mod d_event;
pub mod d_items;
pub mod d_mode;
pub mod d_net;
pub mod d_player;
pub mod doomgeneric;
pub mod doomkeys;
pub mod doomstat;
pub mod dstrings;
pub mod dummy;
pub mod f_wipe;
pub mod hu_lib;
pub mod hu_stuff;
pub mod i_cdmus;
pub mod i_endoom;
pub mod i_input;
pub mod i_joystick;
pub mod i_sound;
pub mod i_system;
pub mod i_timer;
pub mod i_video;
pub mod info;
pub mod m_argv;
pub mod m_bbox;
pub mod m_cheat;
pub mod m_config;
pub mod m_controls;
pub mod m_fixed;
pub mod m_menu;
pub mod m_misc;
pub mod m_random;
pub mod memio;
pub mod p_ceilng;
pub mod p_floor;
pub mod p_lights;
pub mod p_plats;
pub mod p_sight;
pub mod p_telept;
pub mod p_tick;
pub mod p_user;
pub mod r_bsp;
pub mod r_main;
pub mod r_plane;
pub mod r_sky;
pub mod s_sound;
pub mod sha1;
pub mod sounds;
pub mod st_lib;
pub mod statdump;
pub mod tables;
pub mod v_video;
pub mod w_checksum;
pub mod w_file;
pub mod w_main;
pub mod w_wad;
pub mod z_zone;

#[cfg(test)]
mod c_tests;
