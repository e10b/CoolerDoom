# Porting Progress — Missing Conversions

This document lists every C module that has not yet been ported to Rust,
ordered by size (lines of code).  Use it to plan incremental porting work.

> A module is considered "ported" when the `.c` file is removed from
> `doomgeneric-sys/build.rs` and a Rust replacement exists in the `room`
> crate.  The checklist in [README.md](README.md#porting-progress) is the
> source of truth.

## Summary

| Metric | Value |
|--------|------:|
| Remaining C modules | 21 |
| Total remaining LoC | 27,070 |
| Already ported LoC | ~28,630 (est.) |
| Port completeness | ~51.4% (by line count) |

## Unported Modules by Complexity

### Trivial — < 100 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Small — 100–350 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

**Recently ported**: `r_segs.c` (743 LoC), `f_finale.c` (718 LoC), `p_switch.c` (648 LoC), `hu_stuff.c` (641 LoC), `r_main.c` (891 LoC), `v_video.c` (932 LoC), `i_system.c` (578 LoC), `w_wad.c` (612 LoC), `z_zone.c` (488 LoC), `i_video.c` (495 LoC), `r_bsp.c` (573 LoC), `r_plane.c` (446 LoC), `p_tick.c` (151 LoC), `d_net.c` (281 LoC), `f_wipe.c` (294 LoC), `p_lights.c` (350 LoC), `st_lib.c` (284 LoC), `p_telept.c` (133 LoC), `p_sight.c` (350 LoC), `p_floor.c` (546 LoC), `p_user.c` (379 LoC), `hu_lib.c` (347 LoC), `i_input.c` (341 LoC), `p_ceilng.c` (324 LoC), `p_plats.c` (304 LoC).

### Medium-Small — 350–550 LoC (0 files, 0 LoC)

_All modules in this bucket have been ported._

### Medium — 550–900 LoC (5 files, 4,195 LoC)

| File | Lines | Category | Porting notes |
|------|------:|----------|---------------|
| `p_doors.c` | 778 | Game logic | Door action specials |
| `d_loop.c` | 826 | Engine | Main game loop; net sync even without MP |
| `d_iwad.c` | 848 | Engine | IWAD discovery and validation |
| `p_setup.c` | 855 | Game logic | Level/map loading and initialization |
| `p_pspr.c` | 888 | Game logic | Player weapon sprite (psprite) logic |

### Medium-Large — 900–1,100 LoC (5 files, 4,792 LoC)

| File | Lines | Category | Porting notes |
|------|------:|----------|---------------|
| `r_data.c` | 912 | Renderer | Texture/flat/colormap data management |
| `p_inter.c` | 922 | Game logic | Player/item interactions and damage |
| `r_draw.c` | 975 | Renderer | Column/span drawing (inner loop) |
| `r_things.c` | 982 | Renderer | Sprite rendering and scaling |
| `p_maputl.c` | 1,001 | Game logic | Map collision utilities (P_PathTraverse, etc.) |

### Large — 1,000–1,500 LoC (6 files, 8,209 LoC)

| File | Lines | Category | Porting notes |
|------|------:|----------|---------------|
| `p_mobj.c` | 1,049 | Game logic | Map object (mobj) creation, movement, spawning |
| `am_map.c` | 1,355 | Automap | Full automap implementation |
| `st_stuff.c` | 1,416 | Status bar | Full status bar logic |
| `p_map.c` | 1,448 | Game logic | Map collision detection; dense geometry code |
| `i_scale.c` | 1,452 | Platform | Screen scaling algorithms |
| `p_spec.c` | 1,489 | Game logic | Special sector/line action dispatcher |

### Very Large — > 1,500 LoC (5 files, 9,874 LoC)

| File | Lines | Category | Porting notes |
|------|------:|----------|---------------|
| `wi_stuff.c` | 1,829 | Intermission | Victory/intermission screens and stats |
| `d_main.c` | 1,845 | Engine | Main initialization; orchestrates all subsystems |
| `p_saveg.c` | 1,891 | Game logic | Save/load game serialization; heavy struct layout |
| `p_enemy.c` | 2,006 | Game logic | Enemy AI; complex state machines and behavior |
| `g_game.c` | 2,303 | Game logic | Core game logic; largest single module |

## Recommended Porting Order

1. **Quick wins** — Small tier: `hu_lib.c`, `i_input.c`, `p_ceilng.c`, `p_plats.c`.
2. **Self-contained modules** — `p_user.c`, `r_plane.c`, `r_bsp.c` are now complete. Good next candidates: `hu_lib.c`, `p_ceilng.c`, `p_plats.c`.
3. **Building blocks** — `z_zone.c` and `v_video.c` are now ported. Next: `r_data.c`.
4. **Renderer pipeline** — `r_data.c`, `r_draw.c`, `r_things.c`, `r_main.c`.
5. **Game logic** — Start with smaller `p_*` modules, work up to `p_map.c`, `p_mobj.c`, `p_spec.c`.
6. **Large orchestrators** — `d_main.c`, `g_game.c`, `p_enemy.c`, `p_saveg.c` last (most dependencies).

## Porting Strategy Notes

- **C shims**: The previous `m_menu_shim.c` (replaced by `room/src/doom/d_player.rs`) and `m_misc_varargs.c` (replaced by Rust `M_StringJoinA` / `M_snprintf_clamp` + macro shims in `m_misc.h`) have already been eliminated. Stable Rust lacks variadic function definitions, so the remaining variadic-style calls from C code are redirected to non-variadic Rust helpers via C preprocessor macros.
- **`i_video.c` overlap**: The Rust platform layer already provides window/video output via winit/wgpu. Porting `i_video.c` means merging its logic into the existing Rust platform callbacks.
- **`d_net.c` is a stub**: Since `FEATURE_MULTIPLAYER` is not defined, this module contains only stubs. It can be ported trivially once the build no longer references it.
- **`z_zone.c` is now ported**: The zone memory allocator is available in Rust (`room/src/doom/z_zone.rs`). This simplifies subsequent work by providing a safe allocation layer.
- **Renderer inner loops**: `r_draw.c` contains the hottest rendering paths. Consider whether to port to idiomatic Rust or leverage SIMD/wgpu for these.

(End of file - total 104 lines)
