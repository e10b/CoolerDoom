#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_int;

const MAXEVENTS: usize = 64;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct event_t {
    pub type_: c_int,
    pub data1: c_int,
    pub data2: c_int,
    pub data3: c_int,
    pub data4: c_int,
}

const DEFAULT_EVENT: event_t = event_t {
    type_: 0,
    data1: 0,
    data2: 0,
    data3: 0,
    data4: 0,
};

static mut EVENTS: [event_t; MAXEVENTS] = [DEFAULT_EVENT; MAXEVENTS];
static mut EVENT_HEAD: usize = 0;
static mut EVENT_TAIL: usize = 0;

#[no_mangle]
pub extern "C" fn D_PostEvent(ev: *const event_t) {
    unsafe {
        EVENTS[EVENT_HEAD] = *ev;
        EVENT_HEAD = (EVENT_HEAD + 1) % MAXEVENTS;
    }
}

#[no_mangle]
pub extern "C" fn D_PopEvent() -> *mut event_t {
    unsafe {
        if EVENT_TAIL == EVENT_HEAD {
            return std::ptr::null_mut();
        }
        let result = &mut EVENTS[EVENT_TAIL];
        EVENT_TAIL = (EVENT_TAIL + 1) % MAXEVENTS;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe fn reset_queue() {
        EVENTS = [DEFAULT_EVENT; MAXEVENTS];
        EVENT_HEAD = 0;
        EVENT_TAIL = 0;
    }

    #[test]
    fn post_then_pop_returns_same_values() {
        unsafe {
            reset_queue();
            let ev = event_t {
                type_: 1,
                data1: 42,
                data2: 99,
                data3: 0,
                data4: 0,
            };
            D_PostEvent(&ev);
            let result = D_PopEvent();
            assert!(!result.is_null());
            let result = &*result;
            assert_eq!(result.type_, 1);
            assert_eq!(result.data1, 42);
            assert_eq!(result.data2, 99);
            assert_eq!(D_PopEvent(), std::ptr::null_mut());
        }
    }

    #[test]
    fn pop_on_empty_returns_null() {
        unsafe {
            reset_queue();
            assert!(D_PopEvent().is_null());
        }
    }

    #[test]
    fn wrap_around_at_maxevents() {
        unsafe {
            reset_queue();
            let max = MAXEVENTS - 1;
            for i in 0..max {
                let ev = event_t {
                    type_: i as c_int,
                    data1: i as c_int,
                    data2: 0,
                    data3: 0,
                    data4: 0,
                };
                D_PostEvent(&ev);
            }
            assert_eq!(EVENT_HEAD, max);
            for i in 0..max {
                let result = D_PopEvent();
                assert!(!result.is_null());
                assert_eq!((*result).type_, i as c_int);
                assert_eq!((*result).data1, i as c_int);
            }
            assert!(D_PopEvent().is_null());
        }
    }
}
