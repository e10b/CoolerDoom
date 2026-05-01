#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_int};

use libc::printf;

extern "C" {
    fn M_CheckParmWithArgs(check: *mut c_char, num_args: c_int) -> c_int;
    static mut myargc: c_int;
    static mut myargv: *mut *mut c_char;
    fn D_TryFindWADByName(name: *mut c_char) -> *mut c_char;
    fn W_AddFile(filename: *mut c_char);
}

type boolean = c_int;

#[no_mangle]
pub extern "C" fn W_ParseCommandLine() -> boolean {
    let mut modifiedgame: boolean = 0;

    unsafe {
        let p = M_CheckParmWithArgs(b"-file\0".as_ptr() as *mut c_char, 1);
        if p != 0 {
            let mut idx = p + 1;
            modifiedgame = 1;
            while idx < myargc && **myargv.offset(idx as isize) != b'-' as c_char {
                let filename = D_TryFindWADByName(*myargv.offset(idx as isize));
                printf(
                    b" adding %s\n\0".as_ptr() as *const c_char,
                    filename,
                );
                W_AddFile(filename);
                idx += 1;
            }
        }
    }

    modifiedgame
}
