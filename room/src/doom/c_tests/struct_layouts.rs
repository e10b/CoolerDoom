//! Verify every critical struct field offset with `offset_of!`.
//!
//! These tests use `#[repr(C)]` mirrors of the C structs.  If a field
//! offset does not match the C compiler output, the test fails,
//! catching data-type-size and alignment mismatches immediately.

#![allow(non_snake_case)]

use std::mem::{offset_of, size_of};

use crate::doom::c_ffi::{divline_t, intercept_t, line_t, mobj_t, sector_t, vertex_t};

// ---------------------------------------------------------------------------
// vertex_t
// ---------------------------------------------------------------------------

#[test]
fn vertex_t_size() {
    assert_eq!(size_of::<vertex_t>(), 8);
}

#[test]
fn vertex_t_offsets() {
    assert_eq!(offset_of!(vertex_t, x), 0);
    assert_eq!(offset_of!(vertex_t, y), 4);
}

// ---------------------------------------------------------------------------
// divline_t
// ---------------------------------------------------------------------------

#[test]
fn divline_t_size() {
    assert_eq!(size_of::<divline_t>(), 16);
}

#[test]
fn divline_t_offsets() {
    assert_eq!(offset_of!(divline_t, x), 0);
    assert_eq!(offset_of!(divline_t, y), 4);
    assert_eq!(offset_of!(divline_t, dx), 8);
    assert_eq!(offset_of!(divline_t, dy), 12);
}

// ---------------------------------------------------------------------------
// line_t
// ---------------------------------------------------------------------------

#[test]
fn line_t_size() {
    assert_eq!(size_of::<line_t>(), 88);
}

#[test]
fn line_t_offsets() {
    assert_eq!(offset_of!(line_t, v1), 0);
    assert_eq!(offset_of!(line_t, v2), 8);
    assert_eq!(offset_of!(line_t, dx), 16);
    assert_eq!(offset_of!(line_t, dy), 20);
    assert_eq!(offset_of!(line_t, flags), 24);
    assert_eq!(offset_of!(line_t, special), 26);
    assert_eq!(offset_of!(line_t, tag), 28);
    assert_eq!(offset_of!(line_t, sidenum), 30);
    assert_eq!(offset_of!(line_t, bbox), 36);
    assert_eq!(offset_of!(line_t, slopetype), 52);
    assert_eq!(offset_of!(line_t, frontsector), 56);
    assert_eq!(offset_of!(line_t, backsector), 64);
    assert_eq!(offset_of!(line_t, validcount), 72);
    assert_eq!(offset_of!(line_t, specialdata), 80);
}

// ---------------------------------------------------------------------------
// sector_t
// ---------------------------------------------------------------------------

#[test]
fn sector_t_size() {
    assert_eq!(size_of::<sector_t>(), 128);
}

#[test]
fn sector_t_offsets() {
    assert_eq!(offset_of!(sector_t, floorheight), 0);
    assert_eq!(offset_of!(sector_t, ceilingheight), 4);
    assert_eq!(offset_of!(sector_t, floorpic), 8);
    assert_eq!(offset_of!(sector_t, ceilingpic), 10);
    assert_eq!(offset_of!(sector_t, lightlevel), 12);
    assert_eq!(offset_of!(sector_t, special), 14);
    assert_eq!(offset_of!(sector_t, tag), 16);
    assert_eq!(offset_of!(sector_t, soundtraversed), 20);
    assert_eq!(offset_of!(sector_t, soundtarget), 24);
    assert_eq!(offset_of!(sector_t, blockbox), 32);
    assert_eq!(offset_of!(sector_t, validcount), 88);
    assert_eq!(offset_of!(sector_t, thinglist), 96);
    assert_eq!(offset_of!(sector_t, specialdata), 104);
    assert_eq!(offset_of!(sector_t, linecount), 112);
    assert_eq!(offset_of!(sector_t, lines), 120);
}

// ---------------------------------------------------------------------------
// intercept_t
// ---------------------------------------------------------------------------

#[test]
fn intercept_t_size() {
    assert_eq!(size_of::<intercept_t>(), 16);
}

#[test]
fn intercept_t_offsets() {
    assert_eq!(offset_of!(intercept_t, frac), 0);
    assert_eq!(offset_of!(intercept_t, isaline), 4);
    // union d starts at offset 8
}

// ---------------------------------------------------------------------------
// mobj_t
// ---------------------------------------------------------------------------

#[test]
fn mobj_t_size() {
    assert_eq!(size_of::<mobj_t>(), 224);
}

#[test]
fn mobj_t_offsets() {
    assert_eq!(offset_of!(mobj_t, x), 24);
    assert_eq!(offset_of!(mobj_t, y), 28);
    assert_eq!(offset_of!(mobj_t, z), 32);
    assert_eq!(offset_of!(mobj_t, angle), 56);
    assert_eq!(offset_of!(mobj_t, radius), 104);
    assert_eq!(offset_of!(mobj_t, height), 108);
    assert_eq!(offset_of!(mobj_t, type_), 128);
    assert_eq!(offset_of!(mobj_t, state), 152);
    assert_eq!(offset_of!(mobj_t, flags), 160);
    assert_eq!(offset_of!(mobj_t, health), 164);
    assert_eq!(offset_of!(mobj_t, reactiontime), 184);
    assert_eq!(offset_of!(mobj_t, player), 192);
    assert_eq!(offset_of!(mobj_t, tracer), 216);
}
