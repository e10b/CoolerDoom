//! Rust port of vendor/doomgeneric/d_mode.c.
//!
//! Provides game mode/mission validation functions and string constants.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::{c_char, c_int};

// ── GameMission_t enum values (d_mode.h:28-40) ──────────────────────

pub const none: c_int = 9;
pub const doom: c_int = 0;
pub const doom2: c_int = 1;
pub const pack_tnt: c_int = 2;
pub const pack_plut: c_int = 3;
pub const pack_chex: c_int = 4;
pub const pack_hacx: c_int = 5;
pub const heretic: c_int = 6;
pub const hexen: c_int = 7;
pub const strife: c_int = 8;

// ── GameMode_t enum values (d_mode.h:47-53) ─────────────────────────

pub const shareware: c_int = 0;
pub const registered: c_int = 1;
pub const commercial: c_int = 2;
pub const retail: c_int = 3;
pub const indetermined: c_int = 4;

// ── GameVersion_t enum values (d_mode.h:57-75) ──────────────────────

pub const exe_doom_1_2: c_int = 0;
pub const exe_doom_1_666: c_int = 1;
pub const exe_doom_1_7: c_int = 2;
pub const exe_doom_1_8: c_int = 3;
pub const exe_doom_1_9: c_int = 4;
pub const exe_hacx: c_int = 5;
pub const exe_ultimate: c_int = 6;
pub const exe_final: c_int = 7;
pub const exe_final2: c_int = 8;
pub const exe_chex: c_int = 9;
pub const exe_heretic_1_3: c_int = 10;
pub const exe_hexen_1_1: c_int = 11;
pub const exe_strife_1_2: c_int = 12;
pub const exe_strife_1_31: c_int = 13;

// ── valid_modes table (d_mode.c:26-46) ──────────────────────────────

struct ValidMode {
    mission: c_int,
    mode: c_int,
    episode: c_int,
    map: c_int,
}

static VALID_MODES: [ValidMode; 13] = [
    ValidMode {
        mission: pack_chex,
        mode: shareware,
        episode: 1,
        map: 5,
    },
    ValidMode {
        mission: doom,
        mode: shareware,
        episode: 1,
        map: 9,
    },
    ValidMode {
        mission: doom,
        mode: registered,
        episode: 3,
        map: 9,
    },
    ValidMode {
        mission: doom,
        mode: retail,
        episode: 4,
        map: 9,
    },
    ValidMode {
        mission: doom2,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: pack_tnt,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: pack_plut,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: pack_hacx,
        mode: commercial,
        episode: 1,
        map: 32,
    },
    ValidMode {
        mission: heretic,
        mode: shareware,
        episode: 1,
        map: 9,
    },
    ValidMode {
        mission: heretic,
        mode: registered,
        episode: 3,
        map: 9,
    },
    ValidMode {
        mission: heretic,
        mode: retail,
        episode: 5,
        map: 9,
    },
    ValidMode {
        mission: hexen,
        mode: commercial,
        episode: 1,
        map: 60,
    },
    ValidMode {
        mission: strife,
        mode: commercial,
        episode: 1,
        map: 34,
    },
];

// ── valid_versions table (d_mode.c:119-133) ─────────────────────────

struct ValidVersion {
    mission: c_int,
    version: c_int,
}

static VALID_VERSIONS: [ValidVersion; 10] = [
    ValidVersion {
        mission: doom,
        version: exe_doom_1_9,
    },
    ValidVersion {
        mission: doom,
        version: exe_hacx,
    },
    ValidVersion {
        mission: doom,
        version: exe_ultimate,
    },
    ValidVersion {
        mission: doom,
        version: exe_final,
    },
    ValidVersion {
        mission: doom,
        version: exe_final2,
    },
    ValidVersion {
        mission: doom,
        version: exe_chex,
    },
    ValidVersion {
        mission: heretic,
        version: exe_heretic_1_3,
    },
    ValidVersion {
        mission: hexen,
        version: exe_hexen_1_1,
    },
    ValidVersion {
        mission: strife,
        version: exe_strife_1_2,
    },
    ValidVersion {
        mission: strife,
        version: exe_strife_1_31,
    },
];

// ── D_ValidGameMode (d_mode.c:50-63) ────────────────────────────────

#[no_mangle]
pub extern "C" fn D_ValidGameMode(mission: c_int, mode: c_int) -> c_int {
    for vm in &VALID_MODES {
        if vm.mission == mission && vm.mode == mode {
            return 1; // true
        }
    }
    0 // false
}

// ── D_ValidEpisodeMap (d_mode.c:65-99) ──────────────────────────────

#[no_mangle]
pub extern "C" fn D_ValidEpisodeMap(
    mission: c_int,
    mode: c_int,
    episode: c_int,
    map: c_int,
) -> c_int {
    // Hacks for Heretic secret episodes
    if mission == heretic {
        if mode == retail && episode == 6 {
            return if (1..=3).contains(&map) { 1 } else { 0 };
        } else if mode == registered && episode == 4 {
            return if map == 1 { 1 } else { 0 };
        }
    }

    for vm in &VALID_MODES {
        if mission == vm.mission && mode == vm.mode {
            return if episode >= 1 && episode <= vm.episode && map >= 1 && map <= vm.map {
                1
            } else {
                0
            };
        }
    }

    0 // Unknown mode/mission combination
}

// ── D_GetNumEpisodes (d_mode.c:103-115) ─────────────────────────────

#[no_mangle]
pub extern "C" fn D_GetNumEpisodes(mission: c_int, mode: c_int) -> c_int {
    let mut episode = 1;
    while D_ValidEpisodeMap(mission, mode, episode, 1) != 0 {
        episode += 1;
    }
    episode - 1
}

// ── D_ValidGameVersion (d_mode.c:135-157) ───────────────────────────

#[no_mangle]
pub extern "C" fn D_ValidGameVersion(mission: c_int, version: c_int) -> c_int {
    let mission = if mission == doom2
        || mission == pack_plut
        || mission == pack_tnt
        || mission == pack_hacx
        || mission == pack_chex
    {
        doom
    } else {
        mission
    };

    for vv in &VALID_VERSIONS {
        if vv.mission == mission && vv.version == version {
            return 1; // true
        }
    }

    0 // false
}

// ── D_IsEpisodeMap (d_mode.c:161-180) ───────────────────────────────

#[no_mangle]
pub extern "C" fn D_IsEpisodeMap(mission: c_int) -> c_int {
    match mission {
        doom | heretic | pack_chex => 1, // true
        _ => 0,                          // false
    }
}

// ── D_GameMissionString (d_mode.c:182-208) ──────────────────────────

macro_rules! cstr {
    ($s:literal) => {
        concat!($s, "\0").as_ptr() as *mut c_char
    };
}

#[no_mangle]
pub extern "C" fn D_GameMissionString(mission: c_int) -> *mut c_char {
    match mission {
        doom => cstr!("doom"),
        doom2 => cstr!("doom2"),
        pack_tnt => cstr!("tnt"),
        pack_plut => cstr!("plutonia"),
        pack_hacx => cstr!("hacx"),
        pack_chex => cstr!("chex"),
        heretic => cstr!("heretic"),
        hexen => cstr!("hexen"),
        strife => cstr!("strife"),
        _ => cstr!("none"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn valid_game_mode_doom_shareware() {
        assert_eq!(D_ValidGameMode(doom, shareware), 1);
    }

    #[test]
    fn valid_game_mode_doom2_shareware_invalid() {
        assert_eq!(D_ValidGameMode(doom2, shareware), 0);
    }

    #[test]
    fn get_num_episodes_doom_retail() {
        assert_eq!(D_GetNumEpisodes(doom, retail), 4);
    }

    #[test]
    fn is_episode_map_doom2_false() {
        assert_eq!(D_IsEpisodeMap(doom2), 0);
    }

    #[test]
    fn is_episode_map_doom_true() {
        assert_eq!(D_IsEpisodeMap(doom), 1);
    }

    #[test]
    fn game_mission_string_heretic() {
        let ptr = D_GameMissionString(heretic);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "heretic");
    }

    #[test]
    fn game_mission_string_unknown() {
        let ptr = D_GameMissionString(42);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "none");
    }

    #[test]
    fn valid_game_version_doom_final2() {
        assert_eq!(D_ValidGameVersion(doom, exe_final2), 1);
    }

    #[test]
    fn valid_game_version_doom2_mapped_to_doom() {
        // doom2 variants check against doom versions
        assert_eq!(D_ValidGameVersion(doom2, exe_final2), 1);
    }

    #[test]
    fn valid_episode_map_doom_retail_ep4_map9() {
        assert_eq!(D_ValidEpisodeMap(doom, retail, 4, 9), 1);
    }

    #[test]
    fn valid_episode_map_doom_retail_ep5_map1_invalid() {
        assert_eq!(D_ValidEpisodeMap(doom, retail, 5, 1), 0);
    }
}
