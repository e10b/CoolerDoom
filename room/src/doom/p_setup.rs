//! Rust port of vendor/doomgeneric/p_setup.c.
//!
//! Level/map loading and initialization. Reads all BSP and geometry lumps
//! from the WAD into runtime data structures (vertexes, lines, sectors, etc.).

#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_char, c_int, c_short, c_ushort, c_uint, c_void};
use std::ptr;

use crate::doom::c_ffi::{
    line_t, node_t, sector_t, seg_t, side_t, subsector_t, vertex_t,
};
use crate::doom::d_mode;
use crate::doom::d_player::{consoleplayer, players, MAXPLAYERS};
use crate::doom::info::sprnames;
use crate::doom::m_bbox::{M_AddToBox, M_ClearBox};
use crate::doom::p_tick::{leveltime, P_InitThinkers};

// ---------------------------------------------------------------------------
// Byte-order helper
// ---------------------------------------------------------------------------

/// Convert a little-endian `i16` from a WAD file to host byte order.
/// On little-endian hosts this is a no-op cast; on big-endian it swaps.
#[inline]
fn SHORT(x: i16) -> i16 {
    i16::from_le(x)
}

// ---------------------------------------------------------------------------
// Zone-memory tags
// ---------------------------------------------------------------------------

const PU_STATIC: c_int = 1;
const PU_LEVEL: c_int = 5;
const PU_PURGELEVEL: c_int = 7;

// ---------------------------------------------------------------------------
// WAD lump-order indices (must match doomdata.h)
// ---------------------------------------------------------------------------

const ML_LABEL: c_int = 0;
const ML_THINGS: c_int = 1;
const ML_LINEDEFS: c_int = 2;
const ML_SIDEDEFS: c_int = 3;
const ML_VERTEXES: c_int = 4;
const ML_SEGS: c_int = 5;
const ML_SSECTORS: c_int = 6;
const ML_NODES: c_int = 7;
const ML_SECTORS: c_int = 8;
const ML_REJECT: c_int = 9;
const ML_BLOCKMAP: c_int = 10;

// ---------------------------------------------------------------------------
// LineDef flags
// ---------------------------------------------------------------------------

const ML_TWOSIDED: i16 = 4;

// ---------------------------------------------------------------------------
// Slope types
// ---------------------------------------------------------------------------

const ST_HORIZONTAL: c_int = 0;
const ST_VERTICAL: c_int = 1;
const ST_POSITIVE: c_int = 2;
const ST_NEGATIVE: c_int = 3;

// ---------------------------------------------------------------------------
// Bounding-box indices
// ---------------------------------------------------------------------------

const BOXTOP: usize = 0;
const BOXBOTTOM: usize = 1;
const BOXLEFT: usize = 2;
const BOXRIGHT: usize = 3;

// ---------------------------------------------------------------------------
// Misc constants
// ---------------------------------------------------------------------------

const FRACBITS: u32 = 16;
const FRACUNIT: c_int = 1 << FRACBITS;
const MAPBLOCKSHIFT: c_int = FRACBITS as c_int + 7;
const MAXRADIUS: c_int = 32 * FRACUNIT;

/// Maximum number of deathmatch start positions in a level.
pub const MAX_DEATHMATCH_STARTS: usize = 10;

// ---------------------------------------------------------------------------
// Packed WAD structs (exact on-disk layout)
// ---------------------------------------------------------------------------

#[repr(C, packed)]
struct mapvertex_t {
    x: i16,
    y: i16,
}

#[repr(C, packed)]
struct mapsidedef_t {
    textureoffset: i16,
    rowoffset: i16,
    toptexture: [u8; 8],
    bottomtexture: [u8; 8],
    midtexture: [u8; 8],
    sector: i16,
}

#[repr(C, packed)]
struct maplinedef_t {
    v1: i16,
    v2: i16,
    flags: i16,
    special: i16,
    tag: i16,
    sidenum: [i16; 2],
}

#[repr(C, packed)]
struct mapsector_t {
    floorheight: i16,
    ceilingheight: i16,
    floorpic: [u8; 8],
    ceilingpic: [u8; 8],
    lightlevel: i16,
    special: i16,
    tag: i16,
}

#[repr(C, packed)]
struct mapsubsector_t {
    numsegs: i16,
    firstseg: i16,
}

#[repr(C, packed)]
struct mapseg_t {
    v1: i16,
    v2: i16,
    angle: i16,
    linedef: i16,
    side: i16,
    offset: i16,
}

#[repr(C, packed)]
struct mapnode_t {
    x: i16,
    y: i16,
    dx: i16,
    dy: i16,
    bbox: [[i16; 4]; 2],
    children: [u16; 2],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct mapthing_t {
    x: i16,
    y: i16,
    angle: i16,
    r#type: i16,
    options: i16,
}

// ---------------------------------------------------------------------------
// MAP-related lookup tables
// ---------------------------------------------------------------------------

#[no_mangle]
pub static mut numvertexes: c_int = 0;
#[no_mangle]
pub static mut vertexes: *mut vertex_t = ptr::null_mut();

#[no_mangle]
pub static mut numsegs: c_int = 0;
#[no_mangle]
pub static mut segs: *mut seg_t = ptr::null_mut();

#[no_mangle]
pub static mut numsectors: c_int = 0;
#[no_mangle]
pub static mut sectors: *mut sector_t = ptr::null_mut();

#[no_mangle]
pub static mut numsubsectors: c_int = 0;
#[no_mangle]
pub static mut subsectors: *mut subsector_t = ptr::null_mut();

#[no_mangle]
pub static mut numnodes: c_int = 0;
#[no_mangle]
pub static mut nodes: *mut node_t = ptr::null_mut();

#[no_mangle]
pub static mut numlines: c_int = 0;
#[no_mangle]
pub static mut lines: *mut line_t = ptr::null_mut();

#[no_mangle]
pub static mut numsides: c_int = 0;
#[no_mangle]
pub static mut sides: *mut side_t = ptr::null_mut();

static mut totallines: c_int = 0;

// ---------------------------------------------------------------------------
// BLOCKMAP
// ---------------------------------------------------------------------------

#[no_mangle]
pub static mut bmapwidth: c_int = 0;
#[no_mangle]
pub static mut bmapheight: c_int = 0;
#[no_mangle]
pub static mut blockmap: *mut c_short = ptr::null_mut();
#[no_mangle]
pub static mut blockmaplump: *mut c_short = ptr::null_mut();
#[no_mangle]
pub static mut bmaporgx: c_int = 0;
#[no_mangle]
pub static mut bmaporgy: c_int = 0;
#[no_mangle]
pub static mut blocklinks: *mut *mut c_void = ptr::null_mut();

// ---------------------------------------------------------------------------
// REJECT
// ---------------------------------------------------------------------------

#[no_mangle]
pub static mut rejectmatrix: *mut u8 = ptr::null_mut();

// ---------------------------------------------------------------------------
// Starting spots
// ---------------------------------------------------------------------------

#[no_mangle]
pub static mut deathmatchstarts: [mapthing_t; MAX_DEATHMATCH_STARTS] =
    [mapthing_t { x: 0, y: 0, angle: 0, r#type: 0, options: 0 }; MAX_DEATHMATCH_STARTS];

#[no_mangle]
pub static mut deathmatch_p: *mut mapthing_t = ptr::null_mut();

#[no_mangle]
pub static mut playerstarts: [mapthing_t; MAXPLAYERS] =
    [mapthing_t { x: 0, y: 0, angle: 0, r#type: 0, options: 0 }; MAXPLAYERS];

// ---------------------------------------------------------------------------
// Extern declarations for still-unported C modules
// ---------------------------------------------------------------------------

extern "C" {
    fn W_LumpLength(lump: c_int) -> c_int;
    fn W_CacheLumpNum(lumpnum: c_int, tag: c_int) -> *mut c_void;
    fn W_ReleaseLumpNum(lumpnum: c_int);
    fn W_ReadLump(lump: c_int, dest: *mut c_void);
    fn W_GetNumForName(name: *mut c_char) -> c_int;

    fn Z_Malloc(size: c_int, tag: c_int, user: *mut c_void) -> *mut c_void;
    fn Z_FreeTags(lowtag: c_int, hightag: c_int);

    fn R_FlatNumForName(name: *mut c_char) -> c_int;
    fn R_TextureNumForName(name: *mut c_char) -> c_int;
    fn R_PrecacheLevel();

    fn P_SpawnMapThing(mthing: *mut mapthing_t);
    fn S_Start();
    fn G_DeathMatchSpawnPlayer(playernum: c_int);
    fn P_SpawnSpecials();
    fn P_InitSwitchList();
    fn P_InitPicAnims();
    fn R_InitSprites(sprnames: *mut *mut c_char);

    fn I_GetMemoryValue(offset: c_uint, dest: *mut c_void, size: c_int);
    fn M_CheckParm(check: *const c_char) -> c_int;

    fn FixedDiv(a: c_int, b: c_int) -> c_int;

    static mut gamemode: c_int;
    static mut deathmatch: c_int;
    static mut playeringame: [c_int; MAXPLAYERS];

    static mut totalkills: c_int;
    static mut totalitems: c_int;
    static mut totalsecret: c_int;

    static mut precache: c_int;
    static mut bodyqueslot: c_int;
    static mut iquehead: c_int;
    static mut iquetail: c_int;

    static mut wminfo: wbstartstruct_t;
}

// ---------------------------------------------------------------------------
// wbstartstruct_t — only the fields we touch from p_setup.c
// ---------------------------------------------------------------------------

#[repr(C)]
struct wbstartstruct_t {
    epsd: c_int,
    didsecret: c_int,
    last: c_int,
    next: c_int,
    maxkills: c_int,
    maxitems: c_int,
    maxsecret: c_int,
    maxfrags: c_int,
    partime: c_int,
    pnum: c_int,
    // plyr omitted — not accessed from p_setup.c
}

// ---------------------------------------------------------------------------
// GetSectorAtNullAddress
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn GetSectorAtNullAddress() -> *mut sector_t {
    static mut NULL_SECTOR_IS_INITIALIZED: bool = false;
    static mut NULL_SECTOR: sector_t = unsafe { std::mem::zeroed() };

    unsafe {
        if !NULL_SECTOR_IS_INITIALIZED {
            NULL_SECTOR = std::mem::zeroed();
            I_GetMemoryValue(0, &mut NULL_SECTOR.floorheight as *mut c_int as *mut c_void, 4);
            I_GetMemoryValue(4, &mut NULL_SECTOR.ceilingheight as *mut c_int as *mut c_void, 4);
            NULL_SECTOR_IS_INITIALIZED = true;
        }
        &mut NULL_SECTOR
    }
}

// ---------------------------------------------------------------------------
// P_LoadVertexes
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadVertexes(lump: c_int) {
    unsafe {
        numvertexes = W_LumpLength(lump) / std::mem::size_of::<mapvertex_t>() as c_int;
        vertexes = Z_Malloc(
            numvertexes * std::mem::size_of::<vertex_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut vertex_t;

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ml = data as *mut mapvertex_t;
        let mut li = vertexes;

        for _ in 0..numvertexes {
            (*li).x = (SHORT((*ml).x) as c_int) << FRACBITS;
            (*li).y = (SHORT((*ml).y) as c_int) << FRACBITS;
            li = li.add(1);
            ml = ml.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadSegs
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadSegs(lump: c_int) {
    unsafe {
        numsegs = W_LumpLength(lump) / std::mem::size_of::<mapseg_t>() as c_int;
        segs = Z_Malloc(
            numsegs * std::mem::size_of::<seg_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut seg_t;
        ptr::write_bytes(segs, 0, (numsegs * std::mem::size_of::<seg_t>() as c_int) as usize);

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ml = data as *mut mapseg_t;
        let mut li = segs;

        for _ in 0..numsegs {
            (*li).v1 = vertexes.offset(SHORT((*ml).v1) as isize);
            (*li).v2 = vertexes.offset(SHORT((*ml).v2) as isize);
            (*li).angle = ((SHORT((*ml).angle) as c_int) << 16) as c_uint;
            (*li).offset = (SHORT((*ml).offset) as c_int) << 16;
            let linedef = SHORT((*ml).linedef) as c_int;
            let ldef = lines.offset(linedef as isize);
            (*li).linedef = ldef;
            let side = SHORT((*ml).side) as c_int;
            (*li).sidedef = sides.offset((*ldef).sidenum[side as usize] as isize);
            (*li).frontsector = (*sides.offset((*ldef).sidenum[side as usize] as isize)).sector;

            if (*ldef).flags & ML_TWOSIDED != 0 {
                let sidenum = (*ldef).sidenum[side as usize ^ 1];
                if sidenum < 0 || sidenum as c_int >= numsides {
                    (*li).backsector = GetSectorAtNullAddress();
                } else {
                    (*li).backsector = sides.offset(sidenum as isize).as_ref().unwrap().sector;
                }
            } else {
                (*li).backsector = ptr::null_mut();
            }

            li = li.add(1);
            ml = ml.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadSubsectors
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadSubsectors(lump: c_int) {
    unsafe {
        numsubsectors = W_LumpLength(lump) / std::mem::size_of::<mapsubsector_t>() as c_int;
        subsectors = Z_Malloc(
            numsubsectors * std::mem::size_of::<subsector_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut subsector_t;

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ms = data as *mut mapsubsector_t;
        ptr::write_bytes(subsectors, 0, (numsubsectors * std::mem::size_of::<subsector_t>() as c_int) as usize);
        let mut ss = subsectors;

        for _ in 0..numsubsectors {
            (*ss).numlines = SHORT((*ms).numsegs);
            (*ss).firstline = SHORT((*ms).firstseg);
            ss = ss.add(1);
            ms = ms.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadSectors
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadSectors(lump: c_int) {
    unsafe {
        numsectors = W_LumpLength(lump) / std::mem::size_of::<mapsector_t>() as c_int;
        sectors = Z_Malloc(
            numsectors * std::mem::size_of::<sector_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut sector_t;
        ptr::write_bytes(sectors, 0, (numsectors * std::mem::size_of::<sector_t>() as c_int) as usize);

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut ms = data as *mut mapsector_t;
        let mut ss = sectors;

        for _ in 0..numsectors {
            (*ss).floorheight = (SHORT((*ms).floorheight) as c_int) << FRACBITS;
            (*ss).ceilingheight = (SHORT((*ms).ceilingheight) as c_int) << FRACBITS;
            (*ss).floorpic = R_FlatNumForName((*ms).floorpic.as_ptr() as *mut c_char) as c_short;
            (*ss).ceilingpic = R_FlatNumForName((*ms).ceilingpic.as_ptr() as *mut c_char) as c_short;
            (*ss).lightlevel = SHORT((*ms).lightlevel);
            (*ss).special = SHORT((*ms).special);
            (*ss).tag = SHORT((*ms).tag);
            (*ss).thinglist = ptr::null_mut();
            ss = ss.add(1);
            ms = ms.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadNodes
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadNodes(lump: c_int) {
    unsafe {
        numnodes = W_LumpLength(lump) / std::mem::size_of::<mapnode_t>() as c_int;
        nodes = Z_Malloc(
            numnodes * std::mem::size_of::<node_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut node_t;

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut mn = data as *mut mapnode_t;
        let mut no = nodes;

        for _ in 0..numnodes {
            (*no).x = (SHORT((*mn).x) as c_int) << FRACBITS;
            (*no).y = (SHORT((*mn).y) as c_int) << FRACBITS;
            (*no).dx = (SHORT((*mn).dx) as c_int) << FRACBITS;
            (*no).dy = (SHORT((*mn).dy) as c_int) << FRACBITS;
            for j in 0..2usize {
                (*no).children[j] = SHORT((*mn).children[j] as i16) as c_ushort;
                for k in 0..4usize {
                    (*no).bbox[j][k] = (SHORT((*mn).bbox[j][k]) as c_int) << FRACBITS;
                }
            }
            no = no.add(1);
            mn = mn.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadThings
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadThings(lump: c_int) {
    unsafe {
        let data = W_CacheLumpNum(lump, PU_STATIC);
        let numthings = W_LumpLength(lump) / std::mem::size_of::<mapthing_t>() as c_int;
        let mut mt = data as *mut mapthing_t;

        for _ in 0..numthings {
            let mut spawn = true;

            if gamemode != d_mode::commercial {
                let thing_type = SHORT((*mt).r#type);
                match thing_type {
                    68 | 64 | 88 | 89 | 69 | 67 | 71 | 65 | 66 | 84 => {
                        spawn = false;
                    }
                    _ => {}
                }
            }

            if !spawn {
                mt = mt.add(1);
                continue;
            }

            let mut spawnthing = mapthing_t {
                x: SHORT((*mt).x),
                y: SHORT((*mt).y),
                angle: SHORT((*mt).angle),
                r#type: SHORT((*mt).r#type),
                options: SHORT((*mt).options),
            };
            P_SpawnMapThing(&mut spawnthing);
            mt = mt.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadLineDefs
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadLineDefs(lump: c_int) {
    unsafe {
        numlines = W_LumpLength(lump) / std::mem::size_of::<maplinedef_t>() as c_int;
        lines = Z_Malloc(
            numlines * std::mem::size_of::<line_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut line_t;
        ptr::write_bytes(lines, 0, (numlines * std::mem::size_of::<line_t>() as c_int) as usize);

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut mld = data as *mut maplinedef_t;
        let mut ld = lines;

        for _ in 0..numlines {
            (*ld).flags = SHORT((*mld).flags);
            (*ld).special = SHORT((*mld).special);
            (*ld).tag = SHORT((*mld).tag);
            let v1 = vertexes.offset(SHORT((*mld).v1) as isize);
            let v2 = vertexes.offset(SHORT((*mld).v2) as isize);
            (*ld).v1 = v1;
            (*ld).v2 = v2;
            (*ld).dx = (*v2).x - (*v1).x;
            (*ld).dy = (*v2).y - (*v1).y;

            if (*ld).dx == 0 {
                (*ld).slopetype = ST_VERTICAL;
            } else if (*ld).dy == 0 {
                (*ld).slopetype = ST_HORIZONTAL;
            } else {
                if FixedDiv((*ld).dy, (*ld).dx) > 0 {
                    (*ld).slopetype = ST_POSITIVE;
                } else {
                    (*ld).slopetype = ST_NEGATIVE;
                }
            }

            if (*v1).x < (*v2).x {
                (*ld).bbox[BOXLEFT] = (*v1).x;
                (*ld).bbox[BOXRIGHT] = (*v2).x;
            } else {
                (*ld).bbox[BOXLEFT] = (*v2).x;
                (*ld).bbox[BOXRIGHT] = (*v1).x;
            }

            if (*v1).y < (*v2).y {
                (*ld).bbox[BOXBOTTOM] = (*v1).y;
                (*ld).bbox[BOXTOP] = (*v2).y;
            } else {
                (*ld).bbox[BOXBOTTOM] = (*v2).y;
                (*ld).bbox[BOXTOP] = (*v1).y;
            }

            (*ld).sidenum[0] = SHORT((*mld).sidenum[0]);
            (*ld).sidenum[1] = SHORT((*mld).sidenum[1]);

            if (*ld).sidenum[0] != -1 {
                (*ld).frontsector = sides.offset((*ld).sidenum[0] as isize).as_ref().unwrap().sector as *mut c_void;
            } else {
                (*ld).frontsector = ptr::null_mut();
            }

            if (*ld).sidenum[1] != -1 {
                (*ld).backsector = sides.offset((*ld).sidenum[1] as isize).as_ref().unwrap().sector as *mut c_void;
            } else {
                (*ld).backsector = ptr::null_mut();
            }

            ld = ld.add(1);
            mld = mld.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadSideDefs
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadSideDefs(lump: c_int) {
    unsafe {
        numsides = W_LumpLength(lump) / std::mem::size_of::<mapsidedef_t>() as c_int;
        sides = Z_Malloc(
            numsides * std::mem::size_of::<side_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut side_t;
        ptr::write_bytes(sides, 0, (numsides * std::mem::size_of::<side_t>() as c_int) as usize);

        let data = W_CacheLumpNum(lump, PU_STATIC);
        let mut msd = data as *mut mapsidedef_t;
        let mut sd = sides;

        for _ in 0..numsides {
            (*sd).textureoffset = (SHORT((*msd).textureoffset) as c_int) << FRACBITS;
            (*sd).rowoffset = (SHORT((*msd).rowoffset) as c_int) << FRACBITS;
            (*sd).toptexture = R_TextureNumForName((*msd).toptexture.as_ptr() as *mut c_char) as c_short;
            (*sd).bottomtexture = R_TextureNumForName((*msd).bottomtexture.as_ptr() as *mut c_char) as c_short;
            (*sd).midtexture = R_TextureNumForName((*msd).midtexture.as_ptr() as *mut c_char) as c_short;
            (*sd).sector = sectors.offset(SHORT((*msd).sector) as isize);
            sd = sd.add(1);
            msd = msd.add(1);
        }

        W_ReleaseLumpNum(lump);
    }
}

// ---------------------------------------------------------------------------
// P_LoadBlockMap
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_LoadBlockMap(lump: c_int) {
    unsafe {
        let lumplen = W_LumpLength(lump);
        let count = lumplen / 2;

        blockmaplump = Z_Malloc(lumplen, PU_LEVEL, ptr::null_mut()) as *mut c_short;
        W_ReadLump(lump, blockmaplump as *mut c_void);
        blockmap = blockmaplump.add(4);

        // Swap all short integers to native byte ordering.
        for i in 0..count {
            *blockmaplump.add(i as usize) = SHORT(*blockmaplump.add(i as usize));
        }

        bmaporgx = (*blockmaplump.add(0) as c_int) << FRACBITS;
        bmaporgy = (*blockmaplump.add(1) as c_int) << FRACBITS;
        bmapwidth = *blockmaplump.add(2) as c_int;
        bmapheight = *blockmaplump.add(3) as c_int;

        let bcount = std::mem::size_of::<*mut c_void>() * bmapwidth as usize * bmapheight as usize;
        blocklinks = Z_Malloc(bcount as c_int, PU_LEVEL, ptr::null_mut()) as *mut *mut c_void;
        libc::memset(blocklinks as *mut c_void, 0, bcount);
    }
}

// ---------------------------------------------------------------------------
// P_GroupLines
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_GroupLines() {
    unsafe {
        // Look up sector number for each subsector.
        let mut ss = subsectors;
        for _ in 0..numsubsectors {
            let seg = segs.offset((*ss).firstline as isize);
            (*ss).sector = (*sides.offset((*seg).linedef.as_ref().unwrap().sidenum[0] as isize))
                .sector as *mut c_void;
            ss = ss.add(1);
        }

        // Count number of lines in each sector.
        let mut li = lines;
        totallines = 0;
        for _ in 0..numlines {
            totallines += 1;
            let frontsec = (*li).frontsector as *mut sector_t;
            if !frontsec.is_null() {
                (*frontsec).linecount += 1;
            }
            let backsec = (*li).backsector as *mut sector_t;
            if !backsec.is_null() && backsec != frontsec {
                (*backsec).linecount += 1;
                totallines += 1;
            }
            li = li.add(1);
        }

        // Build line tables for each sector.
        let linebuffer = Z_Malloc(
            totallines * std::mem::size_of::<*mut line_t>() as c_int,
            PU_LEVEL,
            ptr::null_mut(),
        ) as *mut *mut line_t;

        for i in 0..numsectors as usize {
            let sec = sectors.add(i);
            (*sec).lines = linebuffer.offset((*sec).linecount as isize) as *mut *mut c_void;
            (*sec).linecount = 0;
        }

        // Assign lines to sectors.
        for i in 0..numlines as usize {
            li = lines.add(i);
            if !(*li).frontsector.is_null() {
                let sector = (*li).frontsector as *mut sector_t;
                (*sector).lines.offset((*sector).linecount as isize).write(li as *mut c_void);
                (*sector).linecount += 1;
            }
            if !(*li).backsector.is_null() && (*li).frontsector != (*li).backsector {
                let sector = (*li).backsector as *mut sector_t;
                (*sector).lines.offset((*sector).linecount as isize).write(li as *mut c_void);
                (*sector).linecount += 1;
            }
        }

        // Generate bounding boxes for sectors.
        let mut sector = sectors;
        for _ in 0..numsectors {
            let mut bbox: [c_int; 4] = [0; 4];
            M_ClearBox(bbox.as_mut_ptr());

            for j in 0..(*sector).linecount {
                let line = *(*sector).lines.offset(j as isize) as *mut line_t;
                M_AddToBox(bbox.as_mut_ptr(), (*(*line).v1).x, (*(*line).v1).y);
                M_AddToBox(bbox.as_mut_ptr(), (*(*line).v2).x, (*(*line).v2).y);
            }

            // Set the degenmobj_t to the middle of the bounding box.
            let soundorg_x = (bbox[BOXRIGHT] + bbox[BOXLEFT]) / 2;
            let soundorg_y = (bbox[BOXTOP] + bbox[BOXBOTTOM]) / 2;
            // sector->soundorg is a 40-byte degenmobj_t; first two fields are x,y.
            let soundorg_ptr = (*sector).soundorg.as_mut_ptr() as *mut c_int;
            *soundorg_ptr = soundorg_x;
            *soundorg_ptr.add(1) = soundorg_y;

            // Adjust bounding box to map blocks.
            let mut block = (bbox[BOXTOP] - bmaporgy + MAXRADIUS) >> MAPBLOCKSHIFT;
            block = if block >= bmapheight { bmapheight - 1 } else { block };
            (*sector).blockbox[BOXTOP] = block;

            block = (bbox[BOXBOTTOM] - bmaporgy - MAXRADIUS) >> MAPBLOCKSHIFT;
            block = if block < 0 { 0 } else { block };
            (*sector).blockbox[BOXBOTTOM] = block;

            block = (bbox[BOXRIGHT] - bmaporgx + MAXRADIUS) >> MAPBLOCKSHIFT;
            block = if block >= bmapwidth { bmapwidth - 1 } else { block };
            (*sector).blockbox[BOXRIGHT] = block;

            block = (bbox[BOXLEFT] - bmaporgx - MAXRADIUS) >> MAPBLOCKSHIFT;
            block = if block < 0 { 0 } else { block };
            (*sector).blockbox[BOXLEFT] = block;

            sector = sector.add(1);
        }
    }
}

// ---------------------------------------------------------------------------
// PadRejectArray
// ---------------------------------------------------------------------------

unsafe fn PadRejectArray(array: *mut u8, len: usize) {
    let rejectpad: [u32; 4] = [
        ((totallines * 4 + 3) & !3) as u32 + 24,
        0,
        50,
        0x1d4a11,
    ];

    let mut dest = array;
    for i in 0..len.min(std::mem::size_of_val(&rejectpad)) {
        let byte_num = i % 4;
        *dest = ((rejectpad[i / 4] >> (byte_num * 8)) & 0xff) as u8;
        dest = dest.add(1);
    }

    if len > std::mem::size_of_val(&rejectpad) {
        eprintln!(
            "PadRejectArray: REJECT lump too short to pad! ({} > {})",
            len,
            std::mem::size_of_val(&rejectpad)
        );

        let padvalue = if M_CheckParm(c"-reject_pad_with_ff".as_ptr()) != 0 {
            0xff
        } else {
            0xf00
        };

        ptr::write_bytes(array.add(std::mem::size_of_val(&rejectpad)), padvalue as u8, len - std::mem::size_of_val(&rejectpad));
    }
}

// ---------------------------------------------------------------------------
// P_LoadReject
// ---------------------------------------------------------------------------

unsafe fn P_LoadReject(lumpnum: c_int) {
    let minlength = (numsectors * numsectors + 7) / 8;
    let lumplen = W_LumpLength(lumpnum);

    if lumplen >= minlength {
        rejectmatrix = W_CacheLumpNum(lumpnum, PU_LEVEL) as *mut u8;
    } else {
        rejectmatrix = Z_Malloc(minlength, PU_LEVEL, &mut rejectmatrix as *mut *mut u8 as *mut c_void) as *mut u8;
        W_ReadLump(lumpnum, rejectmatrix as *mut c_void);
        PadRejectArray(rejectmatrix.add(lumplen as usize), (minlength - lumplen) as usize);
    }
}

// ---------------------------------------------------------------------------
// P_SetupLevel
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_SetupLevel(episode: c_int, map: c_int, _playermask: c_int, _skill: c_int) {
    unsafe {
        totalkills = 0;
        totalitems = 0;
        totalsecret = 0;
        wminfo.maxfrags = 0;
        wminfo.partime = 180;

        for i in 0..MAXPLAYERS {
            players[i].killcount = 0;
            players[i].secretcount = 0;
            players[i].itemcount = 0;
        }

        players[consoleplayer as usize].viewz = 1;

        S_Start();
        Z_FreeTags(PU_LEVEL, PU_PURGELEVEL - 1);

        P_InitThinkers();

        // Find map name.
        let mut lumpname = [0i8; 9];
        if gamemode == d_mode::commercial {
            if map < 10 {
                let s = format!("map0{}", map);
                ptr::copy_nonoverlapping(s.as_ptr(), lumpname.as_mut_ptr() as *mut u8, s.len());
            } else {
                let s = format!("map{}", map);
                ptr::copy_nonoverlapping(s.as_ptr(), lumpname.as_mut_ptr() as *mut u8, s.len());
            }
        } else {
            lumpname[0] = b'E' as i8;
            lumpname[1] = (b'0' + episode as u8) as i8;
            lumpname[2] = b'M' as i8;
            lumpname[3] = (b'0' + map as u8) as i8;
            lumpname[4] = 0;
        }

        let lumpnum = W_GetNumForName(lumpname.as_mut_ptr());

        leveltime = 0;

        // Note: most of this ordering is important.
        P_LoadBlockMap(lumpnum + ML_BLOCKMAP);
        P_LoadVertexes(lumpnum + ML_VERTEXES);
        P_LoadSectors(lumpnum + ML_SECTORS);
        P_LoadSideDefs(lumpnum + ML_SIDEDEFS);

        P_LoadLineDefs(lumpnum + ML_LINEDEFS);
        P_LoadSubsectors(lumpnum + ML_SSECTORS);
        P_LoadNodes(lumpnum + ML_NODES);
        P_LoadSegs(lumpnum + ML_SEGS);

        P_GroupLines();
        P_LoadReject(lumpnum + ML_REJECT);

        bodyqueslot = 0;
        deathmatch_p = deathmatchstarts.as_mut_ptr();
        P_LoadThings(lumpnum + ML_THINGS);

        if deathmatch != 0 {
            for i in 0..MAXPLAYERS {
                if playeringame[i] != 0 {
                    players[i].mo = ptr::null_mut();
                    G_DeathMatchSpawnPlayer(i as c_int);
                }
            }
        }

        iquehead = 0;
        iquetail = 0;

        P_SpawnSpecials();

        if precache != 0 {
            R_PrecacheLevel();
        }
    }
}

// ---------------------------------------------------------------------------
// P_Init
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn P_Init() {
    unsafe {
        P_InitSwitchList();
        P_InitPicAnims();
        R_InitSprites(sprnames.as_mut_ptr());
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn max_deathmatch_starts_is_10() {
        let _g = LOCK.lock().unwrap();
        assert_eq!(MAX_DEATHMATCH_STARTS, 10);
    }

    #[test]
    fn setup_globals_default_zero() {
        let _g = LOCK.lock().unwrap();
        unsafe {
            assert_eq!(numvertexes, 0);
            assert_eq!(numsegs, 0);
            assert_eq!(numsectors, 0);
            assert_eq!(numsubsectors, 0);
            assert_eq!(numnodes, 0);
            assert_eq!(numlines, 0);
            assert_eq!(numsides, 0);
            assert_eq!(bmapwidth, 0);
            assert_eq!(bmapheight, 0);
            assert_eq!(bmaporgx, 0);
            assert_eq!(bmaporgy, 0);
        }
    }

    #[test]
    fn setup_globals_are_c_int_width() {
        use std::ffi::c_int;
        const _: () = assert!(std::mem::size_of::<c_int>() == 4);
        unsafe {
            let _: c_int = numvertexes;
            let _: c_int = numsegs;
            let _: c_int = numsectors;
            let _: c_int = numsubsectors;
            let _: c_int = numnodes;
            let _: c_int = numlines;
            let _: c_int = numsides;
            let _: c_int = bmapwidth;
            let _: c_int = bmapheight;
            let _: c_int = bmaporgx;
            let _: c_int = bmaporgy;
        }
    }
}
