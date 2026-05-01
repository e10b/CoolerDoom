#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types)]

use std::ffi::{c_int, c_void};

#[cfg(not(test))]
extern "C" {
    fn Z_Malloc(size: c_int, tag: c_int, user: *mut c_void) -> *mut c_void;
    fn Z_Free(ptr: *mut c_void);
}

#[cfg(not(test))]
const PU_STATIC: c_int = 1;

#[repr(C)]
pub enum memfile_mode_t {
    MODE_READ = 0,
    MODE_WRITE = 1,
}

#[repr(C)]
pub enum mem_rel_t {
    MEM_SEEK_SET = 0,
    MEM_SEEK_CUR = 1,
    MEM_SEEK_END = 2,
}

#[repr(C)]
pub struct _MEMFILE {
    pub buf: *mut u8,
    pub buflen: usize,
    pub alloced: usize,
    pub position: u32,
    pub mode: memfile_mode_t,
}

#[cfg(test)]
fn mem_malloc(size: usize) -> *mut c_void {
    let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
    unsafe { std::alloc::alloc(layout) as *mut c_void }
}

#[cfg(test)]
fn mem_free(ptr: *mut c_void) {
    if !ptr.is_null() {
        let layout = std::alloc::Layout::from_size_align(1, 1).unwrap();
        unsafe { std::alloc::dealloc(ptr as *mut u8, layout) };
    }
}

#[cfg(not(test))]
fn mem_malloc(size: usize) -> *mut c_void {
    unsafe { Z_Malloc(size as c_int, PU_STATIC, std::ptr::null_mut()) }
}

#[cfg(not(test))]
fn mem_free(ptr: *mut c_void) {
    unsafe { Z_Free(ptr) }
}

#[cfg(test)]
fn mem_malloc_zero(size: usize) -> *mut c_void {
    let layout = std::alloc::Layout::from_size_align(size, 1).unwrap();
    let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
    ptr as *mut c_void
}

#[cfg(not(test))]
fn mem_malloc_zero(size: usize) -> *mut c_void {
    let ptr = mem_malloc(size);
    if !ptr.is_null() {
        unsafe { std::ptr::write_bytes(ptr as *mut u8, 0, size) };
    }
    ptr
}

#[no_mangle]
pub extern "C" fn mem_fopen_read(buf: *mut c_void, buflen: usize) -> *mut _MEMFILE {
    let file: *mut _MEMFILE = mem_malloc(std::mem::size_of::<_MEMFILE>()) as *mut _MEMFILE;
    unsafe {
        (*file).buf = buf as *mut u8;
        (*file).buflen = buflen;
        (*file).alloced = 0;
        (*file).position = 0;
        (*file).mode = memfile_mode_t::MODE_READ;
    }
    file
}

#[no_mangle]
pub extern "C" fn mem_fread(
    buf: *mut c_void,
    size: usize,
    nmemb: usize,
    stream: *mut _MEMFILE,
) -> usize {
    let stream = unsafe { &mut *stream };

    if !matches!(stream.mode, memfile_mode_t::MODE_READ) {
        return 0;
    }

    let mut items = nmemb;

    if items * size > stream.buflen - stream.position as usize {
        items = (stream.buflen - stream.position as usize) / size;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(
            stream.buf.add(stream.position as usize),
            buf as *mut u8,
            items * size,
        );
    }

    stream.position += (items * size) as u32;

    items
}

#[no_mangle]
pub extern "C" fn mem_fopen_write() -> *mut _MEMFILE {
    let file: *mut _MEMFILE = mem_malloc(std::mem::size_of::<_MEMFILE>()) as *mut _MEMFILE;
    let initial_alloc = 1024usize;
    let buf = mem_malloc(initial_alloc) as *mut u8;

    unsafe {
        (*file).alloced = initial_alloc;
        (*file).buf = buf;
        (*file).buflen = 0;
        (*file).position = 0;
        (*file).mode = memfile_mode_t::MODE_WRITE;
    }
    file
}

#[no_mangle]
pub extern "C" fn mem_fwrite(
    ptr: *const c_void,
    size: usize,
    nmemb: usize,
    stream: *mut _MEMFILE,
) -> usize {
    let stream = unsafe { &mut *stream };

    if !matches!(stream.mode, memfile_mode_t::MODE_WRITE) {
        return 0;
    }

    let bytes = size * nmemb;

    while bytes > stream.alloced - stream.position as usize {
        let new_alloc = stream.alloced * 2;
        let newbuf = mem_malloc(new_alloc) as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(stream.buf, newbuf, stream.alloced);
        }
        mem_free(stream.buf as *mut c_void);
        stream.buf = newbuf;
        stream.alloced = new_alloc;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(
            ptr as *const u8,
            stream.buf.add(stream.position as usize),
            bytes,
        );
    }
    stream.position += bytes as u32;

    if stream.position as usize > stream.buflen {
        stream.buflen = stream.position as usize;
    }

    nmemb
}

#[no_mangle]
pub extern "C" fn mem_get_buf(stream: *mut _MEMFILE, buf: *mut *mut c_void, buflen: *mut usize) {
    let stream = unsafe { &*stream };
    unsafe {
        *buf = stream.buf as *mut c_void;
        *buflen = stream.buflen;
    }
}

#[no_mangle]
pub extern "C" fn mem_fclose(stream: *mut _MEMFILE) {
    let stream = unsafe { &*stream };

    if matches!(stream.mode, memfile_mode_t::MODE_WRITE) {
        mem_free(stream.buf as *mut c_void);
    }

    mem_free(stream as *const _MEMFILE as *mut c_void);
}

#[no_mangle]
pub extern "C" fn mem_ftell(stream: *mut _MEMFILE) -> c_int {
    let stream = unsafe { &*stream };
    stream.position as c_int
}

#[no_mangle]
pub extern "C" fn mem_fseek(stream: *mut _MEMFILE, position: c_int, whence: mem_rel_t) -> c_int {
    let stream = unsafe { &mut *stream };

    let newpos = match whence {
        mem_rel_t::MEM_SEEK_SET => position as u32,
        mem_rel_t::MEM_SEEK_CUR => stream.position.wrapping_add(position as u32),
        mem_rel_t::MEM_SEEK_END => (stream.buflen as c_int + position) as u32,
        _ => return -1,
    };

    if (newpos as usize) < stream.buflen {
        stream.position = newpos;
        0
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_small_write() {
        let write_file = mem_fopen_write();
        let data: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

        mem_fwrite(data.as_ptr() as *const c_void, 1, 4, write_file);

        let mut buf_ptr: *mut c_void = std::ptr::null_mut();
        let mut buflen: usize = 0;
        mem_get_buf(write_file, &mut buf_ptr, &mut buflen);

        assert_eq!(buflen, 4);

        let read_file = mem_fopen_read(buf_ptr, buflen);
        let mut read_buf = [0u8; 4];

        let items_read = mem_fread(read_buf.as_mut_ptr() as *mut c_void, 1, 4, read_file);

        assert_eq!(items_read, 4);
        assert_eq!(read_buf, data);

        mem_fclose(read_file);
        mem_fclose(write_file);
    }

    #[test]
    fn seek_set_cur_end() {
        let write_file = mem_fopen_write();
        let data: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

        mem_fwrite(data.as_ptr() as *const c_void, 1, 8, write_file);

        let mut buf_ptr: *mut c_void = std::ptr::null_mut();
        let mut buflen: usize = 0;
        mem_get_buf(write_file, &mut buf_ptr, &mut buflen);

        let read_file = mem_fopen_read(buf_ptr, buflen);

        let mut buf = [0u8; 2];

        assert_eq!(mem_fseek(read_file, 4, mem_rel_t::MEM_SEEK_SET), 0);
        assert_eq!(mem_ftell(read_file), 4);
        assert_eq!(
            mem_fread(buf.as_mut_ptr() as *mut c_void, 1, 2, read_file),
            2
        );
        assert_eq!(buf, [5, 6]);

        assert_eq!(mem_fseek(read_file, -3, mem_rel_t::MEM_SEEK_CUR), 0);
        assert_eq!(mem_ftell(read_file), 3);
        assert_eq!(
            mem_fread(buf.as_mut_ptr() as *mut c_void, 1, 2, read_file),
            2
        );
        assert_eq!(buf, [4, 5]);

        assert_eq!(mem_fseek(read_file, -2, mem_rel_t::MEM_SEEK_END), 0);
        assert_eq!(mem_ftell(read_file), 6);
        assert_eq!(
            mem_fread(buf.as_mut_ptr() as *mut c_void, 1, 2, read_file),
            2
        );
        assert_eq!(buf, [7, 8]);

        mem_fclose(read_file);
        mem_fclose(write_file);
    }
}
