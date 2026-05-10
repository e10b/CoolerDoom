//! Doom engine key code constants.
//!
//! These match the `#define KEY_*` constants in `vendor/doomgeneric/doomkeys.h`.
//! The `to_doom_key` mapping lives in the binary's `platform/keys.rs` because
//! it depends on winit.

pub const KEY_RIGHTARROW: u8 = 0xae;
pub const KEY_LEFTARROW: u8 = 0xac;
pub const KEY_UPARROW: u8 = 0xad;
pub const KEY_DOWNARROW: u8 = 0xaf;
pub const KEY_STRAFE_L: u8 = 0xa0;
pub const KEY_STRAFE_R: u8 = 0xa1;
pub const KEY_USE: u8 = 0xa2;
pub const KEY_FIRE: u8 = 0xa3;
pub const KEY_ESCAPE: u8 = 27;
pub const KEY_ENTER: u8 = 13;
pub const KEY_TAB: u8 = 9;
pub const KEY_F1: u8 = 0x80 + 0x3b;
pub const KEY_F2: u8 = 0x80 + 0x3c;
pub const KEY_F3: u8 = 0x80 + 0x3d;
pub const KEY_F4: u8 = 0x80 + 0x3e;
pub const KEY_F5: u8 = 0x80 + 0x3f;
pub const KEY_F6: u8 = 0x80 + 0x40;
pub const KEY_F7: u8 = 0x80 + 0x41;
pub const KEY_F8: u8 = 0x80 + 0x42;
pub const KEY_F9: u8 = 0x80 + 0x43;
pub const KEY_F10: u8 = 0x80 + 0x44;
pub const KEY_F11: u8 = 0x80 + 0x57;
pub const KEY_F12: u8 = 0x80 + 0x58;
pub const KEY_BACKSPACE: u8 = 0x7f;
pub const KEY_PAUSE: u8 = 0xff;
pub const KEY_EQUALS: u8 = b'=';
pub const KEY_MINUS: u8 = b'-';
pub const KEY_RSHIFT: u8 = 0x80 + 0x36;
pub const KEY_RCTRL: u8 = 0x80 + 0x1d;
pub const KEY_RALT: u8 = 0x80 + 0x38;
pub const KEY_LALT: u8 = KEY_RALT;
pub const KEY_HOME: u8 = 0x80 + 0x47;
pub const KEY_END: u8 = 0x80 + 0x4f;
pub const KEY_PGUP: u8 = 0x80 + 0x49;
pub const KEY_PGDN: u8 = 0x80 + 0x51;
pub const KEY_INS: u8 = 0x80 + 0x52;
pub const KEY_DEL: u8 = 0x80 + 0x53;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that key constants match the values in vendor/doomgeneric/doomkeys.h.
    #[test]
    fn directional_keys_match_header() {
        assert_eq!(KEY_RIGHTARROW, 0xae);
        assert_eq!(KEY_LEFTARROW, 0xac);
        assert_eq!(KEY_UPARROW, 0xad);
        assert_eq!(KEY_DOWNARROW, 0xaf);
    }

    #[test]
    fn action_keys_match_header() {
        assert_eq!(KEY_USE, 0xa2);
        assert_eq!(KEY_FIRE, 0xa3);
        assert_eq!(KEY_STRAFE_L, 0xa0);
        assert_eq!(KEY_STRAFE_R, 0xa1);
    }

    #[test]
    fn control_keys_match_header() {
        assert_eq!(KEY_ESCAPE, 27);
        assert_eq!(KEY_ENTER, 13);
        assert_eq!(KEY_TAB, 9);
        assert_eq!(KEY_BACKSPACE, 0x7f);
        assert_eq!(KEY_PAUSE, 0xff);
    }

    #[test]
    fn function_keys_match_header() {
        assert_eq!(KEY_F1, 0x80 + 0x3b);
        assert_eq!(KEY_F2, 0x80 + 0x3c);
        assert_eq!(KEY_F10, 0x80 + 0x44);
        assert_eq!(KEY_F11, 0x80 + 0x57);
        assert_eq!(KEY_F12, 0x80 + 0x58);
    }

    #[test]
    fn modifier_keys_match_header() {
        assert_eq!(KEY_RSHIFT, 0x80 + 0x36);
        assert_eq!(KEY_RCTRL, 0x80 + 0x1d);
        assert_eq!(KEY_RALT, 0x80 + 0x38);
        // LALT is aliased to RALT in the original header
        assert_eq!(KEY_LALT, KEY_RALT);
    }

    #[test]
    fn navigation_keys_match_header() {
        assert_eq!(KEY_HOME, 0x80 + 0x47);
        assert_eq!(KEY_END, 0x80 + 0x4f);
        assert_eq!(KEY_PGUP, 0x80 + 0x49);
        assert_eq!(KEY_PGDN, 0x80 + 0x51);
        assert_eq!(KEY_INS, 0x80 + 0x52);
        assert_eq!(KEY_DEL, 0x80 + 0x53);
    }

    #[test]
    fn equals_and_minus_match_ascii() {
        assert_eq!(KEY_EQUALS, b'=');
        assert_eq!(KEY_MINUS, b'-');
    }
}
