#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int, c_void};
use std::os::raw::c_uint;

//
// Keyboard controls
//

#[no_mangle]
pub static mut key_right: c_int = 205; // KEY_RIGHTARROW
#[no_mangle]
pub static mut key_left: c_int = 203; // KEY_LEFTARROW
#[no_mangle]
pub static mut key_up: c_int = 200; // KEY_UPARROW
#[no_mangle]
pub static mut key_down: c_int = 208; // KEY_DOWNARROW
#[no_mangle]
pub static mut key_strafeleft: c_int = b',' as c_int;
#[no_mangle]
pub static mut key_straferight: c_int = b'.' as c_int;
#[no_mangle]
pub static mut key_fire: c_int = b'/' as c_int;
#[no_mangle]
pub static mut key_use: c_int = b' ' as c_int;
#[no_mangle]
pub static mut key_strafe: c_int = 164; // KEY_RALT
#[no_mangle]
pub static mut key_speed: c_int = 161; // KEY_RSHIFT

//
// Heretic keyboard controls
//

#[no_mangle]
pub static mut key_flyup: c_int = 201; // KEY_PGUP
#[no_mangle]
pub static mut key_flydown: c_int = 210; // KEY_INS
#[no_mangle]
pub static mut key_flycenter: c_int = 199; // KEY_HOME

#[no_mangle]
pub static mut key_lookup: c_int = 209; // KEY_PGDN
#[no_mangle]
pub static mut key_lookdown: c_int = 207; // KEY_DEL
#[no_mangle]
pub static mut key_lookcenter: c_int = 206; // KEY_END

#[no_mangle]
pub static mut key_invleft: c_int = b'[' as c_int;
#[no_mangle]
pub static mut key_invright: c_int = b']' as c_int;
#[no_mangle]
pub static mut key_useartifact: c_int = b'\r' as c_int; // KEY_ENTER

//
// Hexen key controls
//

#[no_mangle]
pub static mut key_jump: c_int = b'/' as c_int;

#[no_mangle]
pub static mut key_arti_all: c_int = 14; // KEY_BACKSPACE
#[no_mangle]
pub static mut key_arti_health: c_int = b'\\' as c_int;
#[no_mangle]
pub static mut key_arti_poisonbag: c_int = b'0' as c_int;
#[no_mangle]
pub static mut key_arti_blastradius: c_int = b'9' as c_int;
#[no_mangle]
pub static mut key_arti_teleport: c_int = b'8' as c_int;
#[no_mangle]
pub static mut key_arti_teleportother: c_int = b'7' as c_int;
#[no_mangle]
pub static mut key_arti_egg: c_int = b'6' as c_int;
#[no_mangle]
pub static mut key_arti_invulnerability: c_int = b'5' as c_int;

//
// Strife key controls
//

#[no_mangle]
pub static mut key_usehealth: c_int = b'h' as c_int;
#[no_mangle]
pub static mut key_invquery: c_int = b'q' as c_int;
#[no_mangle]
pub static mut key_mission: c_int = b'w' as c_int;
#[no_mangle]
pub static mut key_invpop: c_int = b'z' as c_int;
#[no_mangle]
pub static mut key_invkey: c_int = b'k' as c_int;
#[no_mangle]
pub static mut key_invhome: c_int = 199; // KEY_HOME
#[no_mangle]
pub static mut key_invend: c_int = 206; // KEY_END
#[no_mangle]
pub static mut key_invuse: c_int = b'\r' as c_int; // KEY_ENTER
#[no_mangle]
pub static mut key_invdrop: c_int = 14; // KEY_BACKSPACE

//
// Mouse controls
//

#[no_mangle]
pub static mut mousebfire: c_int = 0;
#[no_mangle]
pub static mut mousebstrafe: c_int = 1;
#[no_mangle]
pub static mut mousebforward: c_int = 2;

#[no_mangle]
pub static mut mousebjump: c_int = -1;

#[no_mangle]
pub static mut mousebstrafeleft: c_int = -1;
#[no_mangle]
pub static mut mousebstraferight: c_int = -1;
#[no_mangle]
pub static mut mousebbackward: c_int = -1;
#[no_mangle]
pub static mut mousebuse: c_int = -1;

#[no_mangle]
pub static mut mousebprevweapon: c_int = -1;
#[no_mangle]
pub static mut mousebnextweapon: c_int = -1;

#[no_mangle]
pub static mut key_message_refresh: c_int = b'\r' as c_int; // KEY_ENTER
#[no_mangle]
pub static mut key_pause: c_int = 119; // KEY_PAUSE
#[no_mangle]
pub static mut key_demo_quit: c_int = b'q' as c_int;
#[no_mangle]
pub static mut key_spy: c_int = 123; // KEY_F12

// Multiplayer chat keys:

#[no_mangle]
pub static mut key_multi_msg: c_int = b't' as c_int;
#[no_mangle]
pub static mut key_multi_msgplayer: [c_int; 8] = [0; 8];

// Weapon selection keys:

#[no_mangle]
pub static mut key_weapon1: c_int = b'1' as c_int;
#[no_mangle]
pub static mut key_weapon2: c_int = b'2' as c_int;
#[no_mangle]
pub static mut key_weapon3: c_int = b'3' as c_int;
#[no_mangle]
pub static mut key_weapon4: c_int = b'4' as c_int;
#[no_mangle]
pub static mut key_weapon5: c_int = b'5' as c_int;
#[no_mangle]
pub static mut key_weapon6: c_int = b'6' as c_int;
#[no_mangle]
pub static mut key_weapon7: c_int = b'7' as c_int;
#[no_mangle]
pub static mut key_weapon8: c_int = b'8' as c_int;
#[no_mangle]
pub static mut key_prevweapon: c_int = 0;
#[no_mangle]
pub static mut key_nextweapon: c_int = 0;

// Map control keys:

#[no_mangle]
pub static mut key_map_north: c_int = 200; // KEY_UPARROW
#[no_mangle]
pub static mut key_map_south: c_int = 208; // KEY_DOWNARROW
#[no_mangle]
pub static mut key_map_east: c_int = 205; // KEY_RIGHTARROW
#[no_mangle]
pub static mut key_map_west: c_int = 203; // KEY_LEFTARROW
#[no_mangle]
pub static mut key_map_zoomin: c_int = b'=' as c_int;
#[no_mangle]
pub static mut key_map_zoomout: c_int = b'-' as c_int;
#[no_mangle]
pub static mut key_map_toggle: c_int = 15; // KEY_TAB
#[no_mangle]
pub static mut key_map_maxzoom: c_int = b'0' as c_int;
#[no_mangle]
pub static mut key_map_follow: c_int = b'f' as c_int;
#[no_mangle]
pub static mut key_map_grid: c_int = b'g' as c_int;
#[no_mangle]
pub static mut key_map_mark: c_int = b'm' as c_int;
#[no_mangle]
pub static mut key_map_clearmark: c_int = b'c' as c_int;

// menu keys:

#[no_mangle]
pub static mut key_menu_activate: c_int = 27; // KEY_ESCAPE
#[no_mangle]
pub static mut key_menu_up: c_int = 200; // KEY_UPARROW
#[no_mangle]
pub static mut key_menu_down: c_int = 208; // KEY_DOWNARROW
#[no_mangle]
pub static mut key_menu_left: c_int = 203; // KEY_LEFTARROW
#[no_mangle]
pub static mut key_menu_right: c_int = 205; // KEY_RIGHTARROW
#[no_mangle]
pub static mut key_menu_back: c_int = 14; // KEY_BACKSPACE
#[no_mangle]
pub static mut key_menu_forward: c_int = b'\r' as c_int; // KEY_ENTER
#[no_mangle]
pub static mut key_menu_confirm: c_int = b'y' as c_int;
#[no_mangle]
pub static mut key_menu_abort: c_int = b'n' as c_int;

#[no_mangle]
pub static mut key_menu_help: c_int = 112; // KEY_F1
#[no_mangle]
pub static mut key_menu_save: c_int = 113; // KEY_F2
#[no_mangle]
pub static mut key_menu_load: c_int = 114; // KEY_F3
#[no_mangle]
pub static mut key_menu_volume: c_int = 115; // KEY_F4
#[no_mangle]
pub static mut key_menu_detail: c_int = 116; // KEY_F5
#[no_mangle]
pub static mut key_menu_qsave: c_int = 117; // KEY_F6
#[no_mangle]
pub static mut key_menu_endgame: c_int = 118; // KEY_F7
#[no_mangle]
pub static mut key_menu_messages: c_int = 119; // KEY_F8
#[no_mangle]
pub static mut key_menu_qload: c_int = 120; // KEY_F9
#[no_mangle]
pub static mut key_menu_quit: c_int = 121; // KEY_F10
#[no_mangle]
pub static mut key_menu_gamma: c_int = 122; // KEY_F11

#[no_mangle]
pub static mut key_menu_incscreen: c_int = b'=' as c_int; // KEY_EQUALS
#[no_mangle]
pub static mut key_menu_decscreen: c_int = 165; // KEY_MINUS
#[no_mangle]
pub static mut key_menu_screenshot: c_int = 0;

//
// Joystick controls
//

#[no_mangle]
pub static mut joybfire: c_int = 0;
#[no_mangle]
pub static mut joybstrafe: c_int = 1;
#[no_mangle]
pub static mut joybuse: c_int = 3;
#[no_mangle]
pub static mut joybspeed: c_int = 2;

#[no_mangle]
pub static mut joybstrafeleft: c_int = -1;
#[no_mangle]
pub static mut joybstraferight: c_int = -1;

#[no_mangle]
pub static mut joybjump: c_int = -1;

#[no_mangle]
pub static mut joybprevweapon: c_int = -1;
#[no_mangle]
pub static mut joybnextweapon: c_int = -1;

#[no_mangle]
pub static mut joybmenu: c_int = -1;

// Control whether if a mouse button is double clicked, it acts like
// "use" has been pressed

#[no_mangle]
pub static mut dclick_use: c_int = 1;

extern "C" {
    fn M_BindVariable(name: *mut c_char, variable: *mut c_void);
    fn M_snprintf(buf: *mut c_char, buf_len: usize, fmt: *const c_char, ...) -> c_int;
}

//
// Bind all of the common controls used by Doom and all other games.
//

#[no_mangle]
pub extern "C" fn M_BindBaseControls() {
    unsafe {
        M_BindVariable(
            cstr(b"key_right\0"),
            &mut key_right as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_left\0"),
            &mut key_left as *mut c_int as *mut c_void,
        );
        M_BindVariable(cstr(b"key_up\0"), &mut key_up as *mut c_int as *mut c_void);
        M_BindVariable(
            cstr(b"key_down\0"),
            &mut key_down as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_strafeleft\0"),
            &mut key_strafeleft as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_straferight\0"),
            &mut key_straferight as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_fire\0"),
            &mut key_fire as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_use\0"),
            &mut key_use as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_strafe\0"),
            &mut key_strafe as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_speed\0"),
            &mut key_speed as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"mouseb_fire\0"),
            &mut mousebfire as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_strafe\0"),
            &mut mousebstrafe as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_forward\0"),
            &mut mousebforward as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"joyb_fire\0"),
            &mut joybfire as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_strafe\0"),
            &mut joybstrafe as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_use\0"),
            &mut joybuse as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_speed\0"),
            &mut joybspeed as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"joyb_menu_activate\0"),
            &mut joybmenu as *mut c_int as *mut c_void,
        );

        // Extra controls that are not in the Vanilla versions:

        M_BindVariable(
            cstr(b"joyb_strafeleft\0"),
            &mut joybstrafeleft as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_straferight\0"),
            &mut joybstraferight as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_strafeleft\0"),
            &mut mousebstrafeleft as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_straferight\0"),
            &mut mousebstraferight as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_use\0"),
            &mut mousebuse as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_backward\0"),
            &mut mousebbackward as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"dclick_use\0"),
            &mut dclick_use as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_pause\0"),
            &mut key_pause as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_message_refresh\0"),
            &mut key_message_refresh as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindHereticControls() {
    unsafe {
        M_BindVariable(
            cstr(b"key_flyup\0"),
            &mut key_flyup as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_flydown\0"),
            &mut key_flydown as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_flycenter\0"),
            &mut key_flycenter as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"key_lookup\0"),
            &mut key_lookup as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_lookdown\0"),
            &mut key_lookdown as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_lookcenter\0"),
            &mut key_lookcenter as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"key_invleft\0"),
            &mut key_invleft as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invright\0"),
            &mut key_invright as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_useartifact\0"),
            &mut key_useartifact as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindHexenControls() {
    unsafe {
        M_BindVariable(
            cstr(b"key_jump\0"),
            &mut key_jump as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_jump\0"),
            &mut mousebjump as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_jump\0"),
            &mut joybjump as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"key_arti_all\0"),
            &mut key_arti_all as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_health\0"),
            &mut key_arti_health as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_poisonbag\0"),
            &mut key_arti_poisonbag as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_blastradius\0"),
            &mut key_arti_blastradius as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_teleport\0"),
            &mut key_arti_teleport as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_teleportother\0"),
            &mut key_arti_teleportother as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_egg\0"),
            &mut key_arti_egg as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_arti_invulnerability\0"),
            &mut key_arti_invulnerability as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindStrifeControls() {
    unsafe {
        // These are shared with all games, but have different defaults:
        key_message_refresh = b'/' as c_int;

        // These keys are shared with Heretic/Hexen but have different defaults:
        key_jump = b'a' as c_int;
        key_lookup = 201; // KEY_PGUP
        key_lookdown = 209; // KEY_PGDN
        key_invleft = 210; // KEY_INS
        key_invright = 207; // KEY_DEL

        M_BindVariable(
            cstr(b"key_jump\0"),
            &mut key_jump as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_lookUp\0"),
            &mut key_lookup as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_lookDown\0"),
            &mut key_lookdown as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invLeft\0"),
            &mut key_invleft as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invRight\0"),
            &mut key_invright as *mut c_int as *mut c_void,
        );

        // Custom Strife-only Keys:
        M_BindVariable(
            cstr(b"key_useHealth\0"),
            &mut key_usehealth as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invquery\0"),
            &mut key_invquery as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_mission\0"),
            &mut key_mission as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invPop\0"),
            &mut key_invpop as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invKey\0"),
            &mut key_invkey as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invHome\0"),
            &mut key_invhome as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invEnd\0"),
            &mut key_invend as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invUse\0"),
            &mut key_invuse as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_invDrop\0"),
            &mut key_invdrop as *mut c_int as *mut c_void,
        );

        // Strife also supports jump on mouse and joystick, and in the exact same
        // manner as Hexen!
        M_BindVariable(
            cstr(b"mouseb_jump\0"),
            &mut mousebjump as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_jump\0"),
            &mut joybjump as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindWeaponControls() {
    unsafe {
        M_BindVariable(
            cstr(b"key_weapon1\0"),
            &mut key_weapon1 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon2\0"),
            &mut key_weapon2 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon3\0"),
            &mut key_weapon3 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon4\0"),
            &mut key_weapon4 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon5\0"),
            &mut key_weapon5 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon6\0"),
            &mut key_weapon6 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon7\0"),
            &mut key_weapon7 as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_weapon8\0"),
            &mut key_weapon8 as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"key_prevweapon\0"),
            &mut key_prevweapon as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_nextweapon\0"),
            &mut key_nextweapon as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"joyb_prevweapon\0"),
            &mut joybprevweapon as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"joyb_nextweapon\0"),
            &mut joybnextweapon as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"mouseb_prevweapon\0"),
            &mut mousebprevweapon as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"mouseb_nextweapon\0"),
            &mut mousebnextweapon as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindMapControls() {
    unsafe {
        M_BindVariable(
            cstr(b"key_map_north\0"),
            &mut key_map_north as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_south\0"),
            &mut key_map_south as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_east\0"),
            &mut key_map_east as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_west\0"),
            &mut key_map_west as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_zoomin\0"),
            &mut key_map_zoomin as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_zoomout\0"),
            &mut key_map_zoomout as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_toggle\0"),
            &mut key_map_toggle as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_maxzoom\0"),
            &mut key_map_maxzoom as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_follow\0"),
            &mut key_map_follow as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_grid\0"),
            &mut key_map_grid as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_mark\0"),
            &mut key_map_mark as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_map_clearmark\0"),
            &mut key_map_clearmark as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindMenuControls() {
    unsafe {
        M_BindVariable(
            cstr(b"key_menu_activate\0"),
            &mut key_menu_activate as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_up\0"),
            &mut key_menu_up as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_down\0"),
            &mut key_menu_down as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_left\0"),
            &mut key_menu_left as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_right\0"),
            &mut key_menu_right as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_back\0"),
            &mut key_menu_back as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_forward\0"),
            &mut key_menu_forward as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_confirm\0"),
            &mut key_menu_confirm as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_abort\0"),
            &mut key_menu_abort as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"key_menu_help\0"),
            &mut key_menu_help as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_save\0"),
            &mut key_menu_save as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_load\0"),
            &mut key_menu_load as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_volume\0"),
            &mut key_menu_volume as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_detail\0"),
            &mut key_menu_detail as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_qsave\0"),
            &mut key_menu_qsave as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_endgame\0"),
            &mut key_menu_endgame as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_messages\0"),
            &mut key_menu_messages as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_qload\0"),
            &mut key_menu_qload as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_quit\0"),
            &mut key_menu_quit as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_gamma\0"),
            &mut key_menu_gamma as *mut c_int as *mut c_void,
        );

        M_BindVariable(
            cstr(b"key_menu_incscreen\0"),
            &mut key_menu_incscreen as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_decscreen\0"),
            &mut key_menu_decscreen as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_menu_screenshot\0"),
            &mut key_menu_screenshot as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_demo_quit\0"),
            &mut key_demo_quit as *mut c_int as *mut c_void,
        );
        M_BindVariable(
            cstr(b"key_spy\0"),
            &mut key_spy as *mut c_int as *mut c_void,
        );
    }
}

#[no_mangle]
pub extern "C" fn M_BindChatControls(num_players: c_uint) {
    unsafe {
        let mut name: [c_char; 32] = [0; 32];
        let mut i: c_uint = 0;

        M_BindVariable(
            cstr(b"key_multi_msg\0"),
            &mut key_multi_msg as *mut c_int as *mut c_void,
        );

        while i < num_players {
            M_snprintf(
                name.as_mut_ptr(),
                name.len(),
                cstr(b"key_multi_msgplayer%i\0"),
                i + 1,
            );
            M_BindVariable(
                name.as_ptr() as *mut c_char,
                &mut key_multi_msgplayer[i as usize] as *mut c_int as *mut c_void,
            );
            i += 1;
        }
    }
}

//
// Apply custom patches to the default values depending on the
// platform we are running on.
//

#[no_mangle]
pub extern "C" fn M_ApplyPlatformDefaults() {
    // no-op. Add your platform-specific patches here.
}

const fn cstr(bytes: &[u8]) -> *mut c_char {
    bytes.as_ptr() as *mut c_char
}
