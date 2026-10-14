//! Minimal libc shim for doomgeneric running on wasm32-unknown-unknown.
//! Provides the core C types, memory allocation, string functions, and
//! an in-memory virtual filesystem for WAD loading and savegames.

#![allow(non_camel_case_types, non_snake_case, clippy::missing_safety_doc)]

use std::alloc::{alloc, alloc_zeroed, dealloc, Layout};
use std::collections::HashMap;
use std::sync::Mutex;

pub use core::ffi::c_void;

pub type c_char = i8;
pub type c_schar = i8;
pub type c_uchar = u8;
pub type c_short = i16;
pub type c_ushort = u16;
pub type c_int = i32;
pub type c_uint = u32;
pub type c_long = i32;
pub type c_ulong = u32;
pub type c_longlong = i64;
pub type c_ulonglong = u64;
pub type c_float = f32;
pub type c_double = f64;
pub type size_t = usize;
pub type ssize_t = isize;
pub type intptr_t = isize;
pub type uintptr_t = usize;

pub const SEEK_SET: c_int = 0;
pub const SEEK_CUR: c_int = 1;
pub const SEEK_END: c_int = 2;
pub const EISDIR: c_int = 21;
pub const ENOENT: c_int = 2;
pub const ENOMEM: c_int = 12;
pub const EINVAL: c_int = 22;

/// Opaque stand-in for libc's `FILE`.
pub enum FILE {}

/// In-memory file handle for the virtual filesystem.
pub struct FileHandle {
    pub name: String,
    pub data: Vec<u8>,
    pub pos: usize,
    pub is_write: bool,
    pub is_mem: bool,
    pub mem_ptr: *mut u8,
    pub mem_len: usize,
}

static VFS: Mutex<Option<HashMap<String, Vec<u8>>>> = Mutex::new(None);

/// Register an in-memory file (e.g. `doom1.wad`) in the virtual filesystem.
pub fn register_file(name: &str, data: Vec<u8>) {
    let mut vfs = VFS.lock().unwrap();
    if vfs.is_none() {
        *vfs = Some(HashMap::new());
    }
    vfs.as_mut().unwrap().insert(name.to_lowercase(), data);
}

/// Retrieve a copy of a file from the virtual filesystem.
pub fn get_file_data(name: &str) -> Option<Vec<u8>> {
    let vfs = VFS.lock().unwrap();
    vfs.as_ref().and_then(|m| m.get(&name.to_lowercase()).cloned())
}

#[no_mangle]
pub unsafe extern "C" fn fopen(path: *const c_char, mode: *const c_char) -> *mut FILE {
    if path.is_null() || mode.is_null() {
        return std::ptr::null_mut();
    }
    let path_str = std::ffi::CStr::from_ptr(path).to_string_lossy().into_owned();
    let mode_str = std::ffi::CStr::from_ptr(mode).to_string_lossy();

    // Match by basename (case-insensitive)
    let key = path_str
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&path_str)
        .to_lowercase();

    let is_write = mode_str.contains('w') || mode_str.contains('a');
    let data = if is_write {
        Vec::new()
    } else {
        let vfs = VFS.lock().unwrap();
        if let Some(map) = vfs.as_ref() {
            if let Some(d) = map.get(&key) {
                d.clone()
            } else {
                return std::ptr::null_mut();
            }
        } else {
            return std::ptr::null_mut();
        }
    };

    let handle = Box::new(FileHandle {
        name: key,
        data,
        pos: 0,
        is_write,
        is_mem: false,
        mem_ptr: std::ptr::null_mut(),
        mem_len: 0,
    });
    Box::into_raw(handle) as *mut FILE
}

#[no_mangle]
pub unsafe extern "C" fn fclose(stream: *mut FILE) -> c_int {
    if stream.is_null() {
        return 0;
    }
    let handle = Box::from_raw(stream as *mut FileHandle);
    if handle.is_write && !handle.is_mem {
        register_file(&handle.name, handle.data);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn fread(
    ptr: *mut c_void,
    size: usize,
    nmemb: usize,
    stream: *mut FILE,
) -> usize {
    if stream.is_null() || ptr.is_null() || size == 0 || nmemb == 0 {
        return 0;
    }
    let handle = &mut *(stream as *mut FileHandle);
    let to_read = size * nmemb;
    if handle.is_mem {
        if handle.pos >= handle.mem_len {
            return 0;
        }
        let available = handle.mem_len - handle.pos;
        let actual = available.min(to_read);
        std::ptr::copy_nonoverlapping(handle.mem_ptr.add(handle.pos), ptr as *mut u8, actual);
        handle.pos += actual;
        actual / size
    } else {
        if handle.pos >= handle.data.len() {
            return 0;
        }
        let available = handle.data.len() - handle.pos;
        let actual = available.min(to_read);
        std::ptr::copy_nonoverlapping(
            handle.data.as_ptr().add(handle.pos),
            ptr as *mut u8,
            actual,
        );
        handle.pos += actual;
        actual / size
    }
}

#[no_mangle]
pub unsafe extern "C" fn fwrite(
    ptr: *const c_void,
    size: usize,
    nmemb: usize,
    stream: *mut FILE,
) -> usize {
    if stream.is_null() || ptr.is_null() || size == 0 || nmemb == 0 {
        return 0;
    }
    let handle = &mut *(stream as *mut FileHandle);
    let to_write = size * nmemb;
    if handle.is_mem {
        if handle.pos >= handle.mem_len {
            return 0;
        }
        let available = handle.mem_len - handle.pos;
        let actual = available.min(to_write);
        std::ptr::copy_nonoverlapping(ptr as *const u8, handle.mem_ptr.add(handle.pos), actual);
        handle.pos += actual;
        actual / size
    } else {
        let needed = handle.pos + to_write;
        if needed > handle.data.len() {
            handle.data.resize(needed, 0);
        }
        std::ptr::copy_nonoverlapping(
            ptr as *const u8,
            handle.data.as_mut_ptr().add(handle.pos),
            to_write,
        );
        handle.pos += to_write;
        nmemb
    }
}

#[no_mangle]
pub unsafe extern "C" fn fseek(stream: *mut FILE, offset: c_long, whence: c_int) -> c_int {
    if stream.is_null() {
        return -1;
    }
    let handle = &mut *(stream as *mut FileHandle);
    let len = if handle.is_mem {
        handle.mem_len
    } else {
        handle.data.len()
    };
    let new_pos = match whence {
        SEEK_SET => offset as i64,
        SEEK_CUR => handle.pos as i64 + offset as i64,
        SEEK_END => len as i64 + offset as i64,
        _ => return -1,
    };
    if new_pos < 0 {
        return -1;
    }
    handle.pos = (new_pos as usize).min(len);
    0
}

#[no_mangle]
pub unsafe extern "C" fn ftell(stream: *mut FILE) -> c_long {
    if stream.is_null() {
        return -1;
    }
    let handle = &*(stream as *const FileHandle);
    handle.pos as c_long
}

#[no_mangle]
pub unsafe extern "C" fn fmemopen(
    buf: *mut c_void,
    size: usize,
    _mode: *const c_char,
) -> *mut FILE {
    let handle = Box::new(FileHandle {
        name: String::new(),
        data: Vec::new(),
        pos: 0,
        is_write: true,
        is_mem: true,
        mem_ptr: buf as *mut u8,
        mem_len: size,
    });
    Box::into_raw(handle) as *mut FILE
}

#[no_mangle]
pub unsafe extern "C" fn remove(path: *const c_char) -> c_int {
    if path.is_null() {
        return -1;
    }
    let s = std::ffi::CStr::from_ptr(path).to_string_lossy();
    let key = s.rsplit(['/', '\\']).next().unwrap_or(&s).to_lowercase();
    let mut vfs = VFS.lock().unwrap();
    if let Some(map) = vfs.as_mut() {
        map.remove(&key);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn rename(old: *const c_char, new: *const c_char) -> c_int {
    if old.is_null() || new.is_null() {
        return -1;
    }
    let old_s = std::ffi::CStr::from_ptr(old).to_string_lossy();
    let old_key = old_s
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&old_s)
        .to_lowercase();
    let new_s = std::ffi::CStr::from_ptr(new).to_string_lossy();
    let new_key = new_s
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&new_s)
        .to_lowercase();
    let mut vfs = VFS.lock().unwrap();
    if let Some(map) = vfs.as_mut() {
        if let Some(data) = map.remove(&old_key) {
            map.insert(new_key, data);
            return 0;
        }
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn fflush(_stream: *mut FILE) -> c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    if !s.is_null() {
        println!("{}", std::ffi::CStr::from_ptr(s).to_string_lossy());
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn putchar(c: c_int) -> c_int {
    print!("{}", c as u8 as char);
    c
}

#[repr(C, align(16))]
struct AllocHeader {
    size: usize,
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    let header_size = std::mem::size_of::<AllocHeader>();
    let total = size + header_size;
    let layout = Layout::from_size_align_unchecked(total, 16);
    let ptr = alloc(layout) as *mut AllocHeader;
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    (*ptr).size = size;
    ptr.add(1) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    let total_user = nmemb * size;
    let header_size = std::mem::size_of::<AllocHeader>();
    let total = total_user + header_size;
    let layout = Layout::from_size_align_unchecked(total, 16);
    let ptr = alloc_zeroed(layout) as *mut AllocHeader;
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    (*ptr).size = total_user;
    ptr.add(1) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let header = (ptr as *mut AllocHeader).sub(1);
    let total = (*header).size + std::mem::size_of::<AllocHeader>();
    let layout = Layout::from_size_align_unchecked(total, 16);
    dealloc(header as *mut u8, layout);
}

#[no_mangle]
pub unsafe extern "C" fn strdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return std::ptr::null_mut();
    }
    let len = strlen(s);
    let p = malloc(len + 1) as *mut c_char;
    if !p.is_null() {
        std::ptr::copy_nonoverlapping(s, p, len + 1);
    }
    p
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut p = s;
    while *p != 0 {
        p = p.add(1);
    }
    p.offset_from(s) as usize
}

#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
    let mut p1 = s1;
    let mut p2 = s2;
    while *p1 != 0 && *p1 == *p2 {
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    (*p1 as c_int) - (*p2 as c_int)
}

#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    if n == 0 {
        return 0;
    }
    let mut p1 = s1;
    let mut p2 = s2;
    for _ in 0..n {
        if *p1 == 0 || *p1 != *p2 {
            return (*p1 as c_int) - (*p2 as c_int);
        }
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int {
    let mut p1 = s1;
    let mut p2 = s2;
    while *p1 != 0 && (*p1 as u8).to_ascii_lowercase() == (*p2 as u8).to_ascii_lowercase() {
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    ((*p1 as u8).to_ascii_lowercase() as c_int) - ((*p2 as u8).to_ascii_lowercase() as c_int)
}

#[no_mangle]
pub unsafe extern "C" fn strncasecmp(s1: *const c_char, s2: *const c_char, n: usize) -> c_int {
    if n == 0 {
        return 0;
    }
    let mut p1 = s1;
    let mut p2 = s2;
    for _ in 0..n {
        let c1 = (*p1 as u8).to_ascii_lowercase();
        let c2 = (*p2 as u8).to_ascii_lowercase();
        if c1 == 0 || c1 != c2 {
            return (c1 as c_int) - (c2 as c_int);
        }
        p1 = p1.add(1);
        p2 = p2.add(1);
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut c_void, c: c_int, n: usize) -> *mut c_void {
    std::ptr::write_bytes(dest as *mut u8, c as u8, n);
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    std::ptr::copy_nonoverlapping(src as *const u8, dest as *mut u8, n);
    dest
}

#[no_mangle]
pub unsafe extern "C" fn atoi(s: *const c_char) -> c_int {
    if s.is_null() {
        return 0;
    }
    std::ffi::CStr::from_ptr(s)
        .to_str()
        .ok()
        .and_then(|str_val| str_val.trim().parse::<c_int>().ok())
        .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn atof(s: *const c_char) -> f64 {
    if s.is_null() {
        return 0.0;
    }
    std::ffi::CStr::from_ptr(s)
        .to_str()
        .ok()
        .and_then(|str_val| str_val.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
}

#[no_mangle]
pub unsafe extern "C" fn getenv(_name: *const c_char) -> *mut c_char {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn mkdir(_path: *const c_char, _mode: u32) -> c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char {
    let mut i = 0;
    while i < n && *src.add(i) != 0 {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
    while i < n {
        *dst.add(i) = 0;
        i += 1;
    }
    dst
}

#[no_mangle]
pub unsafe extern "C" fn strchr(s: *const c_char, c: c_int) -> *mut c_char {
    let mut p = s;
    let target = c as c_char;
    loop {
        if *p == target {
            return p as *mut c_char;
        }
        if *p == 0 {
            return std::ptr::null_mut();
        }
        p = p.add(1);
    }
}

#[no_mangle]
pub unsafe extern "C" fn strrchr(s: *const c_char, c: c_int) -> *mut c_char {
    let mut p = s;
    let target = c as c_char;
    let mut last = std::ptr::null_mut();
    loop {
        if *p == target {
            last = p as *mut c_char;
        }
        if *p == 0 {
            break;
        }
        p = p.add(1);
    }
    last
}

#[no_mangle]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    if *needle == 0 {
        return haystack as *mut c_char;
    }
    let mut h = haystack;
    while *h != 0 {
        let mut h_curr = h;
        let mut n = needle;
        while *h_curr != 0 && *n != 0 && *h_curr == *n {
            h_curr = h_curr.add(1);
            n = n.add(1);
        }
        if *n == 0 {
            return h as *mut c_char;
        }
        h = h.add(1);
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn strcat(dest: *mut c_char, src: *const c_char) -> *mut c_char {
    let len = strlen(dest);
    let mut i = 0;
    while *src.add(i) != 0 {
        *dest.add(len + i) = *src.add(i);
        i += 1;
    }
    *dest.add(len + i) = 0;
    dest
}

#[no_mangle]
pub unsafe extern "C" fn toupper(c: c_int) -> c_int {
    (c as u8).to_ascii_uppercase() as c_int
}

#[no_mangle]
pub unsafe extern "C" fn tolower(c: c_int) -> c_int {
    (c as u8).to_ascii_lowercase() as c_int
}

#[no_mangle]
pub unsafe extern "C" fn isspace(c: c_int) -> c_int {
    if (c as u8).is_ascii_whitespace() { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "C" fn isdigit(c: c_int) -> c_int {
    if (c as u8).is_ascii_digit() { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "C" fn isalpha(c: c_int) -> c_int {
    if (c as u8).is_ascii_alphabetic() { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "C" fn isalnum(c: c_int) -> c_int {
    if (c as u8).is_ascii_alphanumeric() { 1 } else { 0 }
}

#[no_mangle]
pub unsafe extern "C" fn abs(x: c_int) -> c_int {
    x.abs()
}

#[no_mangle]
pub unsafe extern "C" fn abort() -> ! {
    panic!("abort called")
}

#[no_mangle]
pub unsafe extern "C" fn exit(code: c_int) -> ! {
    panic!("exit({}) called", code)
}

