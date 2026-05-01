//! Mapping from winit physical key codes to Doom engine key codes.
//!
//! The Doom engine uses a small set of 8-bit key identifiers defined in
//! `doomkeys.h`.  Most printable characters map to their lower-case ASCII
//! value; special keys use values in the range `0x80–0xFF`.
//!
//! This module mirrors the mapping found in `doomgeneric_sdl.c` so that the
//! winit-based platform behaves identically to the SDL reference port.

// Some key constants are part of the public API and are included here for
// completeness even if they are not yet referenced in the platform code.
#![allow(dead_code)]

use winit::keyboard::KeyCode;

// Re-export the Doom key constants so callers don't need to know the
// numeric values.  These match the `#define KEY_*` constants in
// `vendor/doomgeneric/doomkeys.h`.

/// Arrow key – move right / strafe right (default).
pub const KEY_RIGHTARROW: u8 = 0xae;
/// Arrow key – move left / strafe left (default).
pub const KEY_LEFTARROW: u8 = 0xac;
/// Arrow key – move forward.
pub const KEY_UPARROW: u8 = 0xad;
/// Arrow key – move backward.
pub const KEY_DOWNARROW: u8 = 0xaf;
/// Strafe-left action key.
pub const KEY_STRAFE_L: u8 = 0xa0;
/// Strafe-right action key.
pub const KEY_STRAFE_R: u8 = 0xa1;
/// Use / open door action key.
pub const KEY_USE: u8 = 0xa2;
/// Fire weapon action key.
pub const KEY_FIRE: u8 = 0xa3;
/// Escape key.
pub const KEY_ESCAPE: u8 = 27;
/// Enter / confirm key.
pub const KEY_ENTER: u8 = 13;
/// Tab key.
pub const KEY_TAB: u8 = 9;
/// F1 function key.
pub const KEY_F1: u8 = 0x80 + 0x3b;
/// F2 function key.
pub const KEY_F2: u8 = 0x80 + 0x3c;
/// F3 function key.
pub const KEY_F3: u8 = 0x80 + 0x3d;
/// F4 function key.
pub const KEY_F4: u8 = 0x80 + 0x3e;
/// F5 function key.
pub const KEY_F5: u8 = 0x80 + 0x3f;
/// F6 function key.
pub const KEY_F6: u8 = 0x80 + 0x40;
/// F7 function key.
pub const KEY_F7: u8 = 0x80 + 0x41;
/// F8 function key.
pub const KEY_F8: u8 = 0x80 + 0x42;
/// F9 function key.
pub const KEY_F9: u8 = 0x80 + 0x43;
/// F10 function key.
pub const KEY_F10: u8 = 0x80 + 0x44;
/// F11 function key.
pub const KEY_F11: u8 = 0x80 + 0x57;
/// F12 function key.
pub const KEY_F12: u8 = 0x80 + 0x58;
/// Backspace key.
pub const KEY_BACKSPACE: u8 = 0x7f;
/// Pause key.
pub const KEY_PAUSE: u8 = 0xff;
/// Equals / plus key.
pub const KEY_EQUALS: u8 = b'=';
/// Minus / hyphen key.
pub const KEY_MINUS: u8 = b'-';
/// Right shift key.
pub const KEY_RSHIFT: u8 = 0x80 + 0x36;
/// Right control key (mapped to fire by default in doomgeneric).
pub const KEY_RCTRL: u8 = 0x80 + 0x1d;
/// Right alt key.
pub const KEY_RALT: u8 = 0x80 + 0x38;
/// Left alt key (same value as right alt in Doom).
pub const KEY_LALT: u8 = KEY_RALT;

/// Convert a winit [`KeyCode`] into the corresponding Doom key byte.
///
/// Returns `None` if the key has no Doom equivalent and should be ignored.
///
/// The mapping follows `doomgeneric_sdl.c`'s `convertToDoomKey` function,
/// extended with additional special keys supported by the winit API.
pub fn to_doom_key(code: KeyCode) -> Option<u8> {
    let key = match code {
        // --- Navigation ---
        KeyCode::Enter => KEY_ENTER,
        KeyCode::Escape => KEY_ESCAPE,
        KeyCode::ArrowLeft => KEY_LEFTARROW,
        KeyCode::ArrowRight => KEY_RIGHTARROW,
        KeyCode::ArrowUp => KEY_UPARROW,
        KeyCode::ArrowDown => KEY_DOWNARROW,

        // --- Action keys ---
        // Ctrl → fire
        KeyCode::ControlLeft | KeyCode::ControlRight => KEY_FIRE,
        // Space → use
        KeyCode::Space => KEY_USE,
        // Shift → run
        KeyCode::ShiftLeft | KeyCode::ShiftRight => KEY_RSHIFT,
        // Alt → strafe
        KeyCode::AltLeft | KeyCode::AltRight => KEY_LALT,

        // --- Function keys ---
        KeyCode::F1 => KEY_F1,
        KeyCode::F2 => KEY_F2,
        KeyCode::F3 => KEY_F3,
        KeyCode::F4 => KEY_F4,
        KeyCode::F5 => KEY_F5,
        KeyCode::F6 => KEY_F6,
        KeyCode::F7 => KEY_F7,
        KeyCode::F8 => KEY_F8,
        KeyCode::F9 => KEY_F9,
        KeyCode::F10 => KEY_F10,
        KeyCode::F11 => KEY_F11,
        KeyCode::F12 => KEY_F12,

        // --- Punctuation ---
        KeyCode::Equal => KEY_EQUALS,
        KeyCode::Minus => KEY_MINUS,
        KeyCode::Backspace => KEY_BACKSPACE,
        KeyCode::Tab => KEY_TAB,
        KeyCode::Pause => KEY_PAUSE,

        // --- Printable ASCII keys ---
        // Map winit physical key codes to lower-case ASCII.
        KeyCode::KeyA => b'a',
        KeyCode::KeyB => b'b',
        KeyCode::KeyC => b'c',
        KeyCode::KeyD => b'd',
        KeyCode::KeyE => b'e',
        KeyCode::KeyF => b'f',
        KeyCode::KeyG => b'g',
        KeyCode::KeyH => b'h',
        KeyCode::KeyI => b'i',
        KeyCode::KeyJ => b'j',
        KeyCode::KeyK => b'k',
        KeyCode::KeyL => b'l',
        KeyCode::KeyM => b'm',
        KeyCode::KeyN => b'n',
        KeyCode::KeyO => b'o',
        KeyCode::KeyP => b'p',
        KeyCode::KeyQ => b'q',
        KeyCode::KeyR => b'r',
        KeyCode::KeyS => b's',
        KeyCode::KeyT => b't',
        KeyCode::KeyU => b'u',
        KeyCode::KeyV => b'v',
        KeyCode::KeyW => b'w',
        KeyCode::KeyX => b'x',
        KeyCode::KeyY => b'y',
        KeyCode::KeyZ => b'z',
        KeyCode::Digit0 => b'0',
        KeyCode::Digit1 => b'1',
        KeyCode::Digit2 => b'2',
        KeyCode::Digit3 => b'3',
        KeyCode::Digit4 => b'4',
        KeyCode::Digit5 => b'5',
        KeyCode::Digit6 => b'6',
        KeyCode::Digit7 => b'7',
        KeyCode::Digit8 => b'8',
        KeyCode::Digit9 => b'9',

        // Ignore all other keys.
        _ => return None,
    };
    Some(key)
}
