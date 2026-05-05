//! Centralized FFI declarations for unported C modules.
//!
//! Each `extern "C"` block is organized by the original `.c` source file.
//! These declarations allow Rust unit tests to call the original C functions
//! directly and read C global variables, establishing behavioral baselines
//! before porting each module.

#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::ffi::{c_char, c_int, c_short, c_uint, c_void};

// ---------------------------------------------------------------------------
// Opaque types — we only need pointers to these for many FFI signatures.
// ---------------------------------------------------------------------------

pub enum state_t {}
pub enum mobjinfo_t {}

// ---------------------------------------------------------------------------
// Minimal repr(C) types needed for p_maputl.c tests.
//
// These definitions match the layouts in p_telept.rs / p_sight.rs so that
// pointer casts between the two are safe.
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Clone, Copy)]
pub struct vertex_t {
    pub x: c_int,
    pub y: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct divline_t {
    pub x: c_int,
    pub y: c_int,
    pub dx: c_int,
    pub dy: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct subsector_t {
    pub sector: *mut c_void,
    pub numlines: i16,
    pub firstline: i16,
    _pad: [u8; 4],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct mapthing_t {
    pub x: i16,
    pub y: i16,
    pub angle: i16,
    pub r#type: i16,
    pub options: i16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct sector_t {
    pub floorheight: c_int,
    pub ceilingheight: c_int,
    pub floorpic: c_short,
    pub ceilingpic: c_short,
    pub lightlevel: c_short,
    pub special: c_short,
    pub tag: c_short,
    _pad0: [u8; 2],
    pub soundtraversed: c_int,
    pub soundtarget: *mut c_void,
    pub blockbox: [c_int; 4],
    pub soundorg: [u8; 40],
    pub validcount: c_int,
    pub thinglist: *mut c_void,
    pub specialdata: *mut c_void,
    pub linecount: c_int,
    pub lines: *mut *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct line_t {
    pub v1: *mut vertex_t,
    pub v2: *mut vertex_t,
    pub dx: c_int,
    pub dy: c_int,
    pub flags: c_short,
    pub special: c_short,
    pub tag: c_short,
    pub sidenum: [c_short; 2],
    pub bbox: [c_int; 4],
    pub slopetype: c_int,
    pub frontsector: *mut c_void,
    pub backsector: *mut c_void,
    pub validcount: c_int,
    pub specialdata: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct intercept_t {
    pub frac: c_int,
    pub isaline: c_int, // boolean
    pub d: intercept_t_d,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union intercept_t_d {
    pub thing: *mut c_void,
    pub line: *mut line_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct mobj_t {
    pub thinker_prev: *mut c_void,
    pub thinker_next: *mut c_void,
    pub thinker_fn: *mut c_void,
    pub x: c_int,
    pub y: c_int,
    pub z: c_int,
    _pad0: [u8; 4],
    pub snext: *mut c_void,
    pub sprev: *mut c_void,
    pub angle: c_uint,
    pub sprite: c_int,
    pub frame: c_int,
    _pad1: [u8; 4],
    pub bnext: *mut c_void,
    pub bprev: *mut c_void,
    pub subsector: *mut c_void,
    pub floorz: c_int,
    pub ceilingz: c_int,
    pub radius: c_int,
    pub height: c_int,
    pub momx: c_int,
    pub momy: c_int,
    pub momz: c_int,
    pub validcount: c_int,
    pub type_: c_int,
    _pad2: [u8; 4],
    pub info: *mut mobjinfo_t,
    pub tics: c_int,
    _pad3: [u8; 4],
    pub state: *mut state_t,
    pub flags: c_int,
    pub health: c_int,
    pub movedir: c_int,
    pub movecount: c_int,
    pub target: *mut c_void,
    pub reactiontime: c_int,
    pub threshold: c_int,
    pub player: *mut c_void,
    pub lastlook: c_int,
    pub spawnpoint: mapthing_t,
    _pad4: [u8; 2],
    pub tracer: *mut c_void,
}

// ---------------------------------------------------------------------------
// p_maputl.c
// ---------------------------------------------------------------------------

pub const MAXINTERCEPTS_ORIGINAL: usize = 128;
pub const MAXINTERCEPTS: usize = MAXINTERCEPTS_ORIGINAL + 61;

extern "C" {
    pub static mut opentop: c_int;
    pub static mut openbottom: c_int;
    pub static mut openrange: c_int;
    pub static mut lowfloor: c_int;

    pub static mut intercepts: [intercept_t; MAXINTERCEPTS];
    pub static mut intercept_p: *mut intercept_t;

    pub static mut trace: divline_t;

    pub fn P_AproxDistance(dx: c_int, dy: c_int) -> c_int;
    pub fn P_PointOnLineSide(x: c_int, y: c_int, line: *mut line_t) -> c_int;
    pub fn P_PointOnDivlineSide(x: c_int, y: c_int, line: *mut divline_t) -> c_int;
    pub fn P_MakeDivline(li: *mut line_t, dl: *mut divline_t);
    pub fn P_InterceptVector(v2: *mut divline_t, v1: *mut divline_t) -> c_int;
    pub fn P_BoxOnLineSide(tmbox: *mut c_int, ld: *mut line_t) -> c_int;
    pub fn P_LineOpening(linedef: *mut line_t);
    pub fn P_UnsetThingPosition(thing: *mut mobj_t);
    pub fn P_SetThingPosition(thing: *mut mobj_t);
    pub fn P_PathTraverse(
        x1: c_int,
        y1: c_int,
        x2: c_int,
        y2: c_int,
        flags: c_int,
        trav: Option<unsafe extern "C" fn(*mut intercept_t) -> c_uint>,
    ) -> c_uint;
    pub fn P_TraverseIntercepts(
        func: Option<unsafe extern "C" fn(*mut intercept_t) -> c_uint>,
        maxfrac: c_int,
    ) -> c_uint;
    pub fn PIT_AddLineIntercepts(ld: *mut line_t) -> c_uint;
    pub fn PIT_AddThingIntercepts(thing: *mut mobj_t) -> c_uint;
}

// ---------------------------------------------------------------------------
// p_inter.c
// ---------------------------------------------------------------------------

extern "C" {
    pub static mut maxammo: [c_int; 4];
    pub static mut clipammo: [c_int; 4];

    pub fn P_GiveAmmo(player: *mut c_void, ammo: c_int, num: c_int) -> c_uint;
    pub fn P_GiveWeapon(player: *mut c_void, weapon: c_int, dropped: c_uint) -> c_uint;
    pub fn P_GiveBody(player: *mut c_void, num: c_int) -> c_uint;
    pub fn P_GiveArmor(player: *mut c_void, armortype: c_int) -> c_uint;
    pub fn P_GivePower(player: *mut c_void, power: c_int) -> c_uint;
    pub fn P_TouchSpecialThing(special: *mut mobj_t, toucher: *mut mobj_t);
    pub fn P_KillMobj(source: *mut mobj_t, target: *mut mobj_t);
    pub fn P_DamageMobj(
        target: *mut mobj_t,
        inflictor: *mut mobj_t,
        source: *mut mobj_t,
        damage: c_int,
    );
}

// ---------------------------------------------------------------------------
// p_spec.c
// ---------------------------------------------------------------------------

extern "C" {
    pub static mut leveltime: c_int;
}

// ---------------------------------------------------------------------------
// p_mobj.c
// ---------------------------------------------------------------------------

extern "C" {
    pub static mut itemrespawnque: [u8; 128 * 10]; // mapthing_t[ITEMQUESIZE]
    pub static mut itemrespawntime: [c_int; 128];
    pub static mut iquehead: c_int;
    pub static mut iquetail: c_int;

    pub fn P_SetMobjState(mobj: *mut mobj_t, state: c_int) -> c_uint;
    pub fn P_ExplodeMissile(mo: *mut mobj_t);
    pub fn P_XYMovement(mo: *mut mobj_t);
    pub fn P_ZMovement(mo: *mut mobj_t);
    pub fn P_NightmareRespawn(mobj: *mut mobj_t);
    pub fn P_MobjThinker(mobj: *mut mobj_t);
    pub fn P_SpawnMobj(x: c_int, y: c_int, z: c_int, type_: c_int) -> *mut mobj_t;
    pub fn P_RemoveMobj(mobj: *mut mobj_t);
    pub fn P_RespawnSpecials();
    pub fn P_SpawnPuff(x: c_int, y: c_int, z: c_int);
    pub fn P_SpawnBlood(x: c_int, y: c_int, z: c_int, damage: c_int);
    pub fn P_CheckMissileSpawn(th: *mut mobj_t);
    pub fn P_SubstNullMobj(mobj: *mut mobj_t) -> *mut mobj_t;
    pub fn P_SpawnMissile(source: *mut mobj_t, dest: *mut mobj_t, type_: c_int) -> *mut mobj_t;
    pub fn P_SpawnPlayerMissile(source: *mut mobj_t, type_: c_int);
}

// ---------------------------------------------------------------------------
// r_draw.c
// ---------------------------------------------------------------------------

extern "C" {
    pub static mut viewwidth: c_int;
    pub static mut viewheight: c_int;
    pub static mut viewwindowx: c_int;
    pub static mut viewwindowy: c_int;
    pub static mut dc_x: c_int;
    pub static mut dc_yl: c_int;
    pub static mut dc_yh: c_int;
    pub static mut dc_iscale: c_int;
    pub static mut dc_texturemid: c_int;
    pub static mut fuzzpos: c_int;
    pub static mut ds_y: c_int;
    pub static mut ds_x1: c_int;
    pub static mut ds_x2: c_int;
    pub static mut ds_xfrac: c_int;
    pub static mut ds_yfrac: c_int;
    pub static mut ds_xstep: c_int;
    pub static mut ds_ystep: c_int;

    pub fn R_InitBuffer(width: c_int, height: c_int);
    pub fn R_InitTranslationTables();
    pub fn R_FillBackScreen();
    pub fn R_VideoErase(ofs: c_uint, count: c_int);
    pub fn R_DrawViewBorder();
}

// ---------------------------------------------------------------------------
// r_data.c
// ---------------------------------------------------------------------------

extern "C" {
    pub static mut firstflat: c_int;
    pub static mut lastflat: c_int;
    pub static mut numflats: c_int;
    pub static mut firstspritelump: c_int;
    pub static mut lastspritelump: c_int;
    pub static mut numspritelumps: c_int;
    pub static mut numtextures: c_int;

    pub fn R_GetColumn(tex: c_int, col: c_int) -> *mut u8;
    pub fn R_GenerateComposite(texnum: c_int);
    pub fn R_GenerateLookup(texnum: c_int);
    pub fn R_InitTextures();
    pub fn R_InitFlats();
    pub fn R_InitSpriteLumps();
    pub fn R_InitColormaps();
    pub fn R_InitData();
    pub fn R_FlatNumForName(name: *mut c_char) -> c_int;
    pub fn R_CheckTextureNumForName(name: *mut c_char) -> c_int;
    pub fn R_TextureNumForName(name: *mut c_char) -> c_int;
    pub fn R_PrecacheLevel();
}

// ---------------------------------------------------------------------------
// Constants from C headers
// ---------------------------------------------------------------------------

pub const FRACBITS: u32 = 16;
pub const FRACUNIT: c_int = 1 << FRACBITS;

pub const MAPBLOCKUNITS: c_int = 128;
pub const MAPBLOCKSIZE: c_int = MAPBLOCKUNITS * FRACUNIT;
pub const MAPBLOCKSHIFT: c_int = FRACBITS as c_int + 7;
pub const MAPBMASK: c_int = MAPBLOCKSIZE - 1;
pub const MAPBTOFRAC: c_int = MAPBLOCKSHIFT - FRACBITS as c_int;

pub const FINEANGLES: usize = 8192;
pub const FINEMASK: c_int = FINEANGLES as c_int - 1;
pub const ANGLETOFINESHIFT: c_int = 19;

pub const ANG45: c_uint = 1 << 29;
pub const ANG90: c_uint = 1 << 30;
pub const ANG180: c_uint = 1 << 31;

pub const ITEMQUESIZE: usize = 128;

pub const PT_ADDLINES: c_int = 1;
pub const PT_ADDTHINGS: c_int = 2;
pub const PT_EARLYOUT: c_int = 4;

pub const BOXTOP: usize = 0;
pub const BOXBOTTOM: usize = 1;
pub const BOXLEFT: usize = 2;
pub const BOXRIGHT: usize = 3;

pub const ST_HORIZONTAL: c_int = 0;
pub const ST_VERTICAL: c_int = 1;
pub const ST_POSITIVE: c_int = 2;
pub const ST_NEGATIVE: c_int = 3;
