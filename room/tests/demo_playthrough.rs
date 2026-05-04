//! Synthetic Demo Playthrough Test
//!
//! Drives the Doom engine headlessly for ~30 s of virtual time,
//! captures snapshots of player / RNG / game state at checkpoint tics,
//! and compares them against a frozen baseline.
//!
//! Usage:
//!   cargo test --test demo_playthrough        — compare vs BASELINE
//!   BLESS=1 cargo test --test demo_playthrough — print new BASELINE to stdout

#![allow(non_snake_case, non_upper_case_globals)]

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_uint, CString};
use std::mem::offset_of;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// headless:: TLS re-exports
// ---------------------------------------------------------------------------

thread_local! {
    static VIRTUAL_MS: Cell<u32> = const { Cell::new(1000) };
    static FRAMES: Cell<u64> = const { Cell::new(0) };
}

const TICK_MS: u32 = 1000 / 35;

fn virtual_ms() -> u32 {
    VIRTUAL_MS.with(|v| {
        let old = v.get();
        v.set(old + TICK_MS);
        old
    })
}

fn note_frame() {
    FRAMES.with(|f| f.set(f.get() + 1));
}

fn frame_count() -> u64 {
    FRAMES.with(|f| f.get())
}

// ---------------------------------------------------------------------------
// DG_* stubs (C-callable, replaces room/src/platform/mod.rs)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn DG_Init() {}

#[no_mangle]
pub extern "C" fn DG_DrawFrame() {
    note_frame();
}

#[no_mangle]
pub extern "C" fn DG_SleepMs(_ms: u32) {}

#[no_mangle]
pub extern "C" fn DG_GetTicksMs() -> u32 {
    virtual_ms()
}

#[no_mangle]
pub extern "C" fn DG_GetKey(_pressed: *mut i32, _doom_key: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub extern "C" fn DG_SetWindowTitle(_title: *const c_char) {}

// ---------------------------------------------------------------------------
// C global declarations
// ---------------------------------------------------------------------------

extern "C" {
    static gametic: c_int;
    static gamestate: c_int;
    static mut prndindex: c_int;
    static mut singletics: c_uint;
}

// ---------------------------------------------------------------------------
// MobjPrefix – repr-C mirror of the first fields of mobj_s
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct MobjPrefix {
    thinker_prev: *mut (),
    thinker_next: *mut (),
    thinker_fn: *mut (),
    x: i32,
    y: i32,
    z: i32,
    snext: *mut (),
    sprev: *mut (),
    angle: u32,
}

#[test]
fn mobj_prefix_offsets() {
    assert_eq!(offset_of!(MobjPrefix, x), 24, "thinker size mismatch");
    assert_eq!(offset_of!(MobjPrefix, y), 28);
    assert_eq!(offset_of!(MobjPrefix, z), 32);
    assert_eq!(offset_of!(MobjPrefix, angle), 56, "angle offset mismatch");
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Snapshot {
    tic: usize,
    virtual_ms: u32,
    frames: u64,
    gametic: c_int,
    gamestate: c_int,
    rndindex: c_int,
    prndindex: c_int,
    health: c_int,
    armorpoints: c_int,
    killcount: c_int,
    itemcount: c_int,
    secretcount: c_int,
    readyweapon: c_int,
    ammo: [c_int; 4],
    mo_x: i32,
    mo_y: i32,
    mo_z: i32,
    mo_angle: u32,
}

// ---------------------------------------------------------------------------
// BASELINE (paste BLESS output here, then re-run without BLESS)
// ---------------------------------------------------------------------------

const BASELINE: &[Snapshot] = &[
    Snapshot {
        tic: 35,
        virtual_ms: 2064,
        frames: 36,
        gametic: 37,
        gamestate: 3,
        rndindex: 0,
        prndindex: 0,
        health: 0,
        armorpoints: 0,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 0,
        ammo: [0, 0, 0, 0],
        mo_x: 0,
        mo_y: 0,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 70,
        virtual_ms: 3072,
        frames: 71,
        gametic: 72,
        gamestate: 3,
        rndindex: 0,
        prndindex: 0,
        health: 0,
        armorpoints: 0,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 0,
        ammo: [0, 0, 0, 0],
        mo_x: 0,
        mo_y: 0,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 140,
        virtual_ms: 5060,
        frames: 141,
        gametic: 142,
        gamestate: 3,
        rndindex: 0,
        prndindex: 0,
        health: 0,
        armorpoints: 0,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 0,
        ammo: [0, 0, 0, 0],
        mo_x: 0,
        mo_y: 0,
        mo_z: 0,
        mo_angle: 0,
    },
    Snapshot {
        tic: 280,
        virtual_ms: 10240,
        frames: 321,
        gametic: 282,
        gamestate: 0,
        rndindex: 175,
        prndindex: 56,
        health: 98,
        armorpoints: 99,
        killcount: 0,
        itemcount: 0,
        secretcount: 0,
        readyweapon: 1,
        ammo: [48, 0, 0, 0],
        mo_x: -19551832,
        mo_y: -9170049,
        mo_z: 0,
        mo_angle: 352321536,
    },
    Snapshot {
        tic: 560,
        virtual_ms: 18108,
        frames: 601,
        gametic: 562,
        gamestate: 0,
        rndindex: 199,
        prndindex: 230,
        health: 92,
        armorpoints: 99,
        killcount: 4,
        itemcount: 3,
        secretcount: 0,
        readyweapon: 1,
        ammo: [44, 0, 0, 0],
        mo_x: 20134750,
        mo_y: 8949751,
        mo_z: 0,
        mo_angle: 4278190080,
    },
    Snapshot {
        tic: 1050,
        virtual_ms: 31856,
        frames: 1091,
        gametic: 1052,
        gamestate: 0,
        rndindex: 177,
        prndindex: 53,
        health: 83,
        armorpoints: 94,
        killcount: 9,
        itemcount: 4,
        secretcount: 0,
        readyweapon: 2,
        ammo: [39, 10, 0, 0],
        mo_x: 33547877,
        mo_y: 35079301,
        mo_z: 3670016,
        mo_angle: 234881024,
    },
];

// ---------------------------------------------------------------------------
// Checkpoints
// ---------------------------------------------------------------------------

const CHECKPOINTS: &[usize] = &[35, 70, 140, 280, 560, 1050];
const TOTAL_TICS: usize = 1050;

fn is_checkpoint(tic: usize) -> bool {
    CHECKPOINTS.contains(&tic)
}

// ---------------------------------------------------------------------------
// Snapshot capture
// ---------------------------------------------------------------------------

fn capture_snapshot(tic: usize) -> Snapshot {
    use room::doom::d_player::{consoleplayer, players, MAXPLAYERS};
    use room::doom::m_random::rndindex;

    let cp = unsafe { consoleplayer };
    let pidx = if cp >= 0 && (cp as usize) < MAXPLAYERS {
        cp as usize
    } else {
        0
    };
    let p = unsafe { &players[pidx] };
    let mo_ptr = p.mo as *const MobjPrefix;
    let (mo_x, mo_y, mo_z, mo_angle) = if mo_ptr.is_null() {
        (0, 0, 0, 0)
    } else {
        let mo = unsafe { &*mo_ptr };
        (mo.x, mo.y, mo.z, mo.angle)
    };

    Snapshot {
        tic,
        virtual_ms: virtual_ms(),
        frames: frame_count(),
        gametic: unsafe { gametic },
        gamestate: unsafe { gamestate },
        rndindex: unsafe { rndindex },
        prndindex: unsafe { prndindex },
        health: p.health,
        armorpoints: p.armorpoints,
        killcount: p.killcount,
        itemcount: p.itemcount,
        secretcount: p.secretcount,
        readyweapon: p.readyweapon,
        ammo: p.ammo,
        mo_x,
        mo_y,
        mo_z,
        mo_angle,
    }
}

// ---------------------------------------------------------------------------
// WAD discovery
// ---------------------------------------------------------------------------

fn find_wad() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let wad = manifest
        .join("../doom1.wad")
        .canonicalize()
        .unwrap_or_else(|_| manifest.parent().unwrap_or(&manifest).join("doom1.wad"));
    wad
}

// ---------------------------------------------------------------------------
// The one-and-only test
// ---------------------------------------------------------------------------

#[test]
fn demo_playthrough() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .try_init();
    let wad_path = find_wad();
    if !wad_path.exists() {
        panic!(
            "doom1.wad not found at {}; the test requires the shareware IWAD",
            wad_path.display()
        );
    }

    let wad_str = wad_path.to_str().expect("WAD path is not valid UTF-8");

    let argv: Vec<CString> = vec![
        CString::new("room").unwrap(),
        CString::new("-iwad").unwrap(),
        CString::new(wad_str).unwrap(),
        CString::new("-nomusic").unwrap(),
        CString::new("-nosound").unwrap(),
        CString::new("-nomouse").unwrap(),
        CString::new("-nojoy").unwrap(),
        CString::new("-nograbmouse").unwrap(),
    ];

    let mut c_argv: Vec<*mut c_char> = argv.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    c_argv.push(std::ptr::null_mut());

    let argc = (c_argv.len() - 1) as c_int;

    // Enable singletics mode: one game tic per TryRunTics() call,
    // bypassing the real-time waiting loop.
    unsafe {
        singletics = 1;
    }

    // Initialize the Doom engine.
    unsafe {
        doomgeneric_sys::doomgeneric_Create(argc, c_argv.as_mut_ptr());
    }

    let mut snapshots: Vec<Snapshot> = Vec::new();

    // Drive the engine for TOTAL_TICS ticks.
    for tic in 0..TOTAL_TICS {
        unsafe {
            doomgeneric_sys::doomgeneric_Tick();
        }

        if is_checkpoint(tic + 1) {
            snapshots.push(capture_snapshot(tic + 1));
        }
    }

    // BLESS mode or compare mode.
    let bless = std::env::var("BLESS").is_ok();

    if bless {
        println!("/* BLESS output — paste into BASELINE */");
        println!("const BASELINE: &[Snapshot] = &[");
        for snap in &snapshots {
            println!("    Snapshot {{");
            println!("        tic: {},", snap.tic);
            println!("        virtual_ms: {},", snap.virtual_ms);
            println!("        frames: {},", snap.frames);
            println!("        gametic: {},", snap.gametic);
            println!("        gamestate: {},", snap.gamestate);
            println!("        rndindex: {},", snap.rndindex);
            println!("        prndindex: {},", snap.prndindex);
            println!("        health: {},", snap.health);
            println!("        armorpoints: {},", snap.armorpoints);
            println!("        killcount: {},", snap.killcount);
            println!("        itemcount: {},", snap.itemcount);
            println!("        secretcount: {},", snap.secretcount);
            println!("        readyweapon: {},", snap.readyweapon);
            println!("        ammo: {:?},", snap.ammo);
            println!("        mo_x: {},", snap.mo_x);
            println!("        mo_y: {},", snap.mo_y);
            println!("        mo_z: {},", snap.mo_z);
            println!("        mo_angle: {},", snap.mo_angle);
            println!("    }},");
        }
        println!("];");
        panic!("BLESS mode: paste the output above into BASELINE, then re-run without BLESS=1");
    } else {
        assert_eq!(
            snapshots.len(),
            BASELINE.len(),
            "snapshot count mismatch: got {}, expected {}",
            snapshots.len(),
            BASELINE.len()
        );
        for (i, (got, expected)) in snapshots.iter().zip(BASELINE.iter()).enumerate() {
            assert_eq!(got.tic, expected.tic, "checkpoint {}: tic mismatch", i);
            assert_eq!(
                got.virtual_ms, expected.virtual_ms,
                "checkpoint {}: virtual_ms mismatch",
                i
            );
            assert_eq!(
                got.gametic, expected.gametic,
                "checkpoint {}: gametic mismatch",
                i
            );
            assert_eq!(
                got.gamestate, expected.gamestate,
                "checkpoint {}: gamestate mismatch",
                i
            );
            assert_eq!(
                got.rndindex, expected.rndindex,
                "checkpoint {}: rndindex mismatch",
                i
            );
            assert_eq!(
                got.prndindex, expected.prndindex,
                "checkpoint {}: prndindex mismatch",
                i
            );
            assert_eq!(
                got.health, expected.health,
                "checkpoint {}: health mismatch",
                i
            );
            assert_eq!(
                got.armorpoints, expected.armorpoints,
                "checkpoint {}: armorpoints mismatch",
                i
            );
            assert_eq!(
                got.killcount, expected.killcount,
                "checkpoint {}: killcount mismatch",
                i
            );
            assert_eq!(
                got.itemcount, expected.itemcount,
                "checkpoint {}: itemcount mismatch",
                i
            );
            assert_eq!(
                got.secretcount, expected.secretcount,
                "checkpoint {}: secretcount mismatch",
                i
            );
            assert_eq!(
                got.readyweapon, expected.readyweapon,
                "checkpoint {}: readyweapon mismatch",
                i
            );
            assert_eq!(got.ammo, expected.ammo, "checkpoint {}: ammo mismatch", i);
            assert_eq!(got.mo_x, expected.mo_x, "checkpoint {}: mo_x mismatch", i);
            assert_eq!(got.mo_y, expected.mo_y, "checkpoint {}: mo_y mismatch", i);
            assert_eq!(got.mo_z, expected.mo_z, "checkpoint {}: mo_z mismatch", i);
            assert_eq!(
                got.mo_angle, expected.mo_angle,
                "checkpoint {}: mo_angle mismatch",
                i
            );
        }
    }
}
