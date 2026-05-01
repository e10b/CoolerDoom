//! Build script for `doomgeneric-sys`.
//!
//! Compiles the vendored doomgeneric C source files into a static library
//! using the [`cc`] crate. The compiled library provides the core Doom
//! engine along with the doomgeneric platform abstraction layer, but
//! **without** the platform-specific display/input implementation
//! (DG_Init, DG_DrawFrame, etc.), which must be provided by the linking
//! binary (i.e. the `room` crate).
//!
//! ## Build flags
//!
//! The C code is compiled with the following preprocessor definitions,
//! matching the original doomgeneric Linux/X11 build:
//!
//! | Flag | Purpose |
//! |------|---------|
//! | `NORMALUNIX` | Enable POSIX / Unix code paths |
//! | `LINUX` | Linux-specific system calls |
//! | `_DEFAULT_SOURCE` | Expose POSIX extensions from glibc |
//!
//! The `FEATURE_MULTIPLAYER` flag is intentionally **not** defined, which
//! causes all networking code to be compiled out via `#ifdef` guards.
//!
//! Sound (`FEATURE_SOUND`) is also **not** defined, which is consistent
//! with the doomgeneric approach of leaving sound as optional.

use std::path::PathBuf;

fn main() {
    let vendor = PathBuf::from("../vendor/doomgeneric");

    // Emit cargo rerun-if-changed directives so the build is incremental.
    println!("cargo:rerun-if-changed=../vendor/doomgeneric");

    // The complete set of C source files to compile, matching the
    // doomgeneric Makefile (minus the platform-specific files).
    // Platform files (doomgeneric_xlib.c, doomgeneric_sdl.c, etc.) are
    // excluded because the `room` crate provides its own implementation
    // of the DG_* functions via Rust.
    let sources: &[&str] = &[
        // Stub / dummy implementations (networking, etc.)
        // dummy  — ported to Rust (room/src/doom/dummy.rs)
        // Automap
        "am_map.c",
        // Doom definitions & state
        // doomdef  — removed, no symbols
        // doomstat — ported to Rust (room/src/doom/doomstat.rs)
        // String tables
        // dstrings — ported to Rust (room/src/doom/dstrings.rs)
        // Events
        "d_event.c",
        // Items
        "d_items.c",
        // IWAD loading
        "d_iwad.c",
        // Main game loop
        "d_loop.c",
        "d_main.c",
        // Game mode detection
        // d_mode   — ported to Rust (room/src/doom/d_mode.rs)
        // Networking stub
        "d_net.c",
        // Finale / end screens
        "f_finale.c",
        // Screen wipe effect
        "f_wipe.c",
        // Core game logic
        "g_game.c",
        // HUD text library
        "hu_lib.c",
        "hu_stuff.c",
        // Thing info tables
        "info.c",
        // CD music stub
        // i_cdmus  — ported to Rust (room/src/doom/i_cdmus.rs)
        // ENDOOM screen
        // i_endoom — ported to Rust (room/src/doom/i_endoom.rs)
        // Joystick stub
        // i_joystick — ported to Rust (room/src/doom/i_joystick.rs)
        // Screen scaling
        "i_scale.c",
        // Sound stub
        "i_sound.c",
        // System functions (error handling, etc.)
        "i_system.c",
        // Timer
        "i_timer.c",
        // Miscellaneous I/O
        // memio — ported to Rust (room/src/doom/memio.rs)
        // Command-line argument parsing
        "m_argv.c",
        // Bounding box — ported to Rust (room/src/doom/m_bbox.rs)
        // Cheat codes
        // m_cheat  — ported to Rust (room/src/doom/m_cheat.rs)
        // Configuration file
        "m_config.c",
        // Control bindings
        "m_controls.c",
        // Fixed-point math — ported to Rust (room/src/doom/m_fixed.rs)
        // Menus
        "m_menu.c",
        // Miscellaneous utilities
        "m_misc.c",
        // Random number generator — ported to Rust (room/src/doom/m_random.rs)
        // Ceiling actions
        "p_ceilng.c",
        // Door actions
        "p_doors.c",
        // AI / enemy logic
        "p_enemy.c",
        // Floor actions
        "p_floor.c",
        // Player interactions
        "p_inter.c",
        // Lighting effects
        "p_lights.c",
        // Map collisions
        "p_map.c",
        // Map utility functions
        "p_maputl.c",
        // Map objects (things)
        "p_mobj.c",
        // Moving platforms
        "p_plats.c",
        // Player sprite logic
        "p_pspr.c",
        // Save games
        "p_saveg.c",
        // Map loading
        "p_setup.c",
        // Line-of-sight checks
        "p_sight.c",
        // Special actions
        "p_spec.c",
        // Switch actions
        "p_switch.c",
        // Teleporter
        "p_telept.c",
        // Thinker / object tick
        "p_tick.c",
        // Player movement
        "p_user.c",
        // Binary space partitioner traversal
        "r_bsp.c",
        // Texture / flat data
        "r_data.c",
        // Column / span drawing
        "r_draw.c",
        // Renderer main
        "r_main.c",
        // Visplane rendering
        "r_plane.c",
        // Segment rendering
        "r_segs.c",
        // Sky rendering
        "r_sky.c",
        // Sprite rendering
        "r_things.c",
        // SHA-1 hash (for WAD checksums)
        // sha1 — ported to Rust (room/src/doom/sha1.rs)
        // Sound data tables
        // sounds — ported to Rust (room/src/doom/sounds.rs)
        // Intermission stats
        "statdump.c",
        // Status bar library
        "st_lib.c",
        // Status bar
        "st_stuff.c",
        // Sound subsystem (no-op when FEATURE_SOUND is not defined)
        "s_sound.c",
        // Trigonometry tables
        // tables — ported to Rust (room/src/doom/tables.rs)
        // Video / screen buffer management
        "v_video.c",
        // Intermission / victory screens
        "wi_stuff.c",
        // WAD checksum
        // w_checksum — ported to Rust (room/src/doom/w_checksum.rs)
        // WAD file abstraction
        "w_file.c",
        "w_file_stdc.c",
        // WAD main loader
        "w_main.c",
        // WAD directory
        "w_wad.c",
        // Zone memory allocator
        "z_zone.c",
        // Input handling (calls DG_GetKey)
        "i_input.c",
        // Video output (calls DG_DrawFrame, DG_Init)
        "i_video.c",
        // doomgeneric glue (allocates DG_ScreenBuffer, calls DG_Init)
        "doomgeneric.c",
    ];

    let mut build = cc::Build::new();

    build
        // Use the vendor directory for include resolution.
        .include(&vendor)
        // Match the original doomgeneric Linux build flags.
        .define("NORMALUNIX", None)
        .define("LINUX", None)
        .define("_DEFAULT_SOURCE", None)
        // Do NOT define FEATURE_MULTIPLAYER – networking code is compiled
        // out via #ifdef guards throughout the source.
        // Do NOT define FEATURE_SOUND – sound is out of scope.
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-implicit-fallthrough")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-maybe-uninitialized");

    for src in sources {
        build.file(vendor.join(src));
    }

    build.compile("doomgeneric");

    // Link against libm for math functions used by the engine.
    println!("cargo:rustc-link-lib=m");
}
