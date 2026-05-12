# Porting Progress — Missing Conversions

This document lists every C module that has not yet been ported to Rust,
ordered by size (lines of code).  Use it to plan incremental porting work.

> A module is considered "ported" when the `.c` file is removed from
> `doomgeneric-sys/build.rs` and a Rust replacement exists in the `room`
> crate.  The checklist in [README.md](README.md#porting-progress) is the
> source of truth.

A transpiled reference Rust code is available in `c2rust-intermediate/`. Use it to verify assumptions in addition to the original C code. The transpiled code is not part of the build.

The ported code must be validated against the unit tests, as well as the `demo_playthrough` integration test.

## Summary

| Metric | Value |
|--------|------:|
| Remaining C modules | 3 |
| Total remaining LoC | ~6,039 |
| Already ported LoC | ~49,665 (est.) |
| Port completeness | ~89.2% (by line count) |

## Unported Modules by Complexity

### Trivial — < 100 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Small — 100–350 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Medium-Small — 350–550 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Medium — 550–900 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Medium-Large — 900–1,100 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Large — 1,000–1,500 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Very Large — > 1,500 LoC (4 files, 7,868 LoC)

| File | Lines | Category | Porting notes |
|------|------:|----------|---------------|
| `d_main.c` | 1,845 | Engine | Main initialization; orchestrates all subsystems |
| `p_saveg.c` | 1,891 | Game logic | Save/load game serialization; heavy struct layout |
| `g_game.c` | 2,303 | Game logic | Core game logic; largest single module |

## Recommended Porting Order

With only 3 modules left, the strategy shifts from "quick wins" to
**dependency-driven sequencing**: unblock the modules that the largest
orchestrators (`d_main.c`, `g_game.c`) depend on first, then tackle the
orchestrators themselves.

1. ~~**Finish the renderer** — `r_things.c` is now ported. The renderer pipeline
   (`r_data`, `r_draw`, `r_segs`, `r_main`, `r_plane`, `r_bsp`, `r_sky`,
   `r_things`) is fully Rust-native.~~
2. ~~**Map objects** — `p_mobj.c`. Needed by `p_enemy.c`, `g_game.c`, and
   `p_map.c`. Once ported, the thinker list becomes fully Rust-native.~~
3. ~~**Map utilities** — `p_maputl.c`. Building block for `p_map.c`; many of
   its types (`divline_t`, `intercept_t`, `mobj_t`) are already mirrored in
   `c_ffi.rs`.~~
4. ~~**Collision detection** — `p_map.c`. Required by `p_enemy.c` and
   `g_game.c`. Heavy geometry code, but its utility layer (`p_maputl.c`)
   should be done first.~~
5. ~~**Special actions** — `p_spec.c` is now ported. The sector/line
   special dispatcher (`p_spec.rs`) is fully Rust-native.~~
6. ~~**Enemy AI** — `p_enemy.c`. Complex state machines, but all dependencies
    (`p_mobj`, `p_map`, `p_maputl`, `p_spec`) should be in place by this
    point.~~
7. ~~**UI / display modules** — `st_stuff.c`, `i_scale.c`, and `wi_stuff.c`
       are now ported.~~
6. **Save/load** — `p_saveg.c`. Heavy struct-layout and serialization work.
   Best done after `p_map.c` is stable so the serialized types do not drift.
7. **Main orchestrators last** — `d_main.c`, `g_game.c`. These have the
   most cross-cutting dependencies and should be ported only when everything
   they call is already Rust.

## Porting Strategy Notes

- **C shims**: `m_menu_shim.c` and `m_misc_varargs.c` redirect variadic C
  calls to non-variadic Rust helpers (`M_StringJoinA`, `M_snprintf_clamp` in
  `m_misc.rs`). These shims will remain necessary until `d_main.c` and
  `g_game.c` (the last callers of variadic `I_Error` / `M_StringJoinA`) are
  ported, because Stable Rust does not support variadic function definitions.
- **`d_net.c` is already ported**: Since `FEATURE_MULTIPLAYER` is not
  defined, the original module contained only stubs; the Rust replacement
  (`room/src/doom/d_net.rs`) is already active.
- **`z_zone.c` is ported**: The zone memory allocator is now Rust-native
  (`room/src/doom/z_zone.rs`). Subsequent ports can rely on safe allocation
  and automatic zeroing.
- **Renderer inner loops**: `r_draw.c` and `r_things.c` were ported with
  explicit wrapping arithmetic to match C overflow semantics and preserve
  demo determinism.

(End of file - total 104 lines)
