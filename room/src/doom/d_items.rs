#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::c_int;
use std::mem::size_of;

// ---------------------------------------------------------------------------
// Ammo type enum values (from doomdef.h)
// ---------------------------------------------------------------------------

const am_clip: c_int = 0;
const am_shell: c_int = 1;
const am_cell: c_int = 2;
const am_misl: c_int = 3;
// NUMAMMO = 4 (not used directly)
const am_noammo: c_int = 5;

// ---------------------------------------------------------------------------
// State index constants (from info.h statenum_t enum)
// ---------------------------------------------------------------------------

const S_NULL: c_int = 0;
const S_PUNCH: c_int = 2;
const S_PUNCHDOWN: c_int = 3;
const S_PUNCHUP: c_int = 4;
const S_PUNCH1: c_int = 5;
const S_PISTOL: c_int = 10;
const S_PISTOLDOWN: c_int = 11;
const S_PISTOLUP: c_int = 12;
const S_PISTOL1: c_int = 13;
const S_PISTOLFLASH: c_int = 17;
const S_SGUN: c_int = 18;
const S_SGUNDOWN: c_int = 19;
const S_SGUNUP: c_int = 20;
const S_SGUN1: c_int = 21;
const S_SGUNFLASH1: c_int = 30;
const S_DSGUN: c_int = 32;
const S_DSGUNDOWN: c_int = 33;
const S_DSGUNUP: c_int = 34;
const S_DSGUN1: c_int = 35;
const S_DSGUNFLASH1: c_int = 46;
const S_CHAIN: c_int = 48;
const S_CHAINDOWN: c_int = 49;
const S_CHAINUP: c_int = 50;
const S_CHAIN1: c_int = 51;
const S_CHAINFLASH1: c_int = 54;
const S_MISSILE: c_int = 56;
const S_MISSILEDOWN: c_int = 57;
const S_MISSILEUP: c_int = 58;
const S_MISSILE1: c_int = 59;
const S_MISSILEFLASH1: c_int = 62;
const S_SAW: c_int = 66;
const S_SAWDOWN: c_int = 68;
const S_SAWUP: c_int = 69;
const S_SAW1: c_int = 70;
const S_PLASMA: c_int = 73;
const S_PLASMADOWN: c_int = 74;
const S_PLASMAUP: c_int = 75;
const S_PLASMA1: c_int = 76;
const S_PLASMAFLASH1: c_int = 78;
const S_BFG: c_int = 80;
const S_BFGDOWN: c_int = 81;
const S_BFGUP: c_int = 82;
const S_BFG1: c_int = 83;
const S_BFGFLASH1: c_int = 87;

// ---------------------------------------------------------------------------
// Weapon info table
// ---------------------------------------------------------------------------

/// Weapon info: sprite frames, ammunition use.
/// Matches `weaponinfo_t` from `d_items.h`.
#[repr(C)]
pub struct weaponinfo_t {
    pub ammo: c_int,
    pub upstate: c_int,
    pub downstate: c_int,
    pub readystate: c_int,
    pub atkstate: c_int,
    pub flashstate: c_int,
}

const NUMWEAPONS: usize = 9;

/// Weapon animation state table.
///
/// Order: wp_fist, wp_pistol, wp_shotgun, wp_chaingun, wp_missile,
///        wp_plasma, wp_bfg, wp_chainsaw, wp_supershotgun.
#[no_mangle]
pub static weaponinfo: [weaponinfo_t; NUMWEAPONS] = [
    // fist
    weaponinfo_t {
        ammo: am_noammo,
        upstate: S_PUNCHUP,
        downstate: S_PUNCHDOWN,
        readystate: S_PUNCH,
        atkstate: S_PUNCH1,
        flashstate: S_NULL,
    },
    // pistol
    weaponinfo_t {
        ammo: am_clip,
        upstate: S_PISTOLUP,
        downstate: S_PISTOLDOWN,
        readystate: S_PISTOL,
        atkstate: S_PISTOL1,
        flashstate: S_PISTOLFLASH,
    },
    // shotgun
    weaponinfo_t {
        ammo: am_shell,
        upstate: S_SGUNUP,
        downstate: S_SGUNDOWN,
        readystate: S_SGUN,
        atkstate: S_SGUN1,
        flashstate: S_SGUNFLASH1,
    },
    // chaingun
    weaponinfo_t {
        ammo: am_clip,
        upstate: S_CHAINUP,
        downstate: S_CHAINDOWN,
        readystate: S_CHAIN,
        atkstate: S_CHAIN1,
        flashstate: S_CHAINFLASH1,
    },
    // missile launcher
    weaponinfo_t {
        ammo: am_misl,
        upstate: S_MISSILEUP,
        downstate: S_MISSILEDOWN,
        readystate: S_MISSILE,
        atkstate: S_MISSILE1,
        flashstate: S_MISSILEFLASH1,
    },
    // plasma rifle
    weaponinfo_t {
        ammo: am_cell,
        upstate: S_PLASMAUP,
        downstate: S_PLASMADOWN,
        readystate: S_PLASMA,
        atkstate: S_PLASMA1,
        flashstate: S_PLASMAFLASH1,
    },
    // bfg 9000
    weaponinfo_t {
        ammo: am_cell,
        upstate: S_BFGUP,
        downstate: S_BFGDOWN,
        readystate: S_BFG,
        atkstate: S_BFG1,
        flashstate: S_BFGFLASH1,
    },
    // chainsaw
    weaponinfo_t {
        ammo: am_noammo,
        upstate: S_SAWUP,
        downstate: S_SAWDOWN,
        readystate: S_SAW,
        atkstate: S_SAW1,
        flashstate: S_NULL,
    },
    // super shotgun
    weaponinfo_t {
        ammo: am_shell,
        upstate: S_DSGUNUP,
        downstate: S_DSGUNDOWN,
        readystate: S_DSGUN,
        atkstate: S_DSGUN1,
        flashstate: S_DSGUNFLASH1,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weaponinfo_length() {
        assert_eq!(weaponinfo.len(), 9);
    }

    #[test]
    fn test_fist_entry() {
        assert_eq!(weaponinfo[0].ammo, am_noammo);
        assert_eq!(weaponinfo[0].readystate, S_PUNCH);
    }

    #[test]
    fn test_super_shotgun_entry() {
        assert_eq!(weaponinfo[8].ammo, am_shell);
        assert_eq!(weaponinfo[8].flashstate, S_DSGUNFLASH1);
    }

    #[test]
    fn test_weaponinfo_t_size() {
        assert_eq!(size_of::<weaponinfo_t>(), 24);
    }
}
