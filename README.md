# room

A faithful Rust port of [doomgeneric](https://github.com/ozkl/doomgeneric) using
[winit](https://github.com/rust-windowing/winit) and [wgpu](https://github.com/gfx-rs/wgpu)
for the platform layer.

## What is this?

`room` is a Rust reimplementation of the classic DOOM engine.  The port follows
a *functional approximation* approach: the initial implementation compiles the
doomgeneric C source via the `cc` crate and provides all six platform callbacks
(`DG_Init`, `DG_DrawFrame`, `DG_SleepMs`, `DG_GetTicksMs`, `DG_GetKey`,
`DG_SetWindowTitle`) in pure Rust using modern, cross-platform libraries.

As noted in the doomgeneric README, sound is hard – so we skip it for now.

## Project layout

```
room/
├── Cargo.toml               Workspace manifest
├── vendor/
│   └── doomgeneric/         Vendored C source from ozkl/doomgeneric
├── doomgeneric-sys/         Raw FFI bindings crate
│   ├── build.rs             Compiles the C source via the `cc` crate
│   └── src/lib.rs           Minimal extern "C" declarations
└── room/                    Rust binary crate
    └── src/
        ├── main.rs          winit ApplicationHandler and entry point
        ├── gpu.rs           wgpu renderer (texture upload + fullscreen blit)
        └── platform/
            ├── mod.rs       DG_* C-callable platform callbacks
            └── keys.rs      winit KeyCode → Doom key byte mapping
```

## Prerequisites

- A Rust toolchain (stable, ≥ 1.75).
- A C compiler (`gcc` or `clang`).
- A Doom WAD file (`doom1.wad`, `doom.wad`, `doom2.wad`, `freedoom1.wad`, …).
  The shareware episode is freely available from many sources.

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release -- -iwad /path/to/doom1.wad
```

Any arguments after `--` are forwarded to the Doom engine unchanged.

## How it works

The engine runs inside the winit event loop:

1. `ApplicationHandler::resumed` creates the window and initialises the wgpu
   renderer, storing both in thread-local statics accessible to the `DG_*`
   callbacks.
2. `ApplicationHandler::about_to_wait` calls `doomgeneric_Create` on the first
   invocation (which initialises all Doom subsystems and runs the first tick),
   then `doomgeneric_Tick` on every subsequent invocation.
3. `DG_DrawFrame` (called from inside `doomgeneric_Tick`) uploads the 640 × 400
   BGRA pixel buffer produced by `I_FinishUpdate` to a wgpu texture and blits
   it to the swapchain surface via a fullscreen-quad render pass.
4. Keyboard events collected by `ApplicationHandler::window_event` are placed
   into a `VecDeque`; `DG_GetKey` pops them one at a time per tick.

## Known limitations

- **No sound.**  Sound output is deliberately omitted in this initial port.
- **No mouse support.**  Mouse aiming / strafing are not yet implemented.
- **No joystick support.**
- **Single player only.**  Networking (`FEATURE_MULTIPLAYER`) is not compiled in.
- The renderer performs a nearest-neighbour upscale from the native 640 × 400
  resolution to the window size; the window is currently fixed at 640 × 400.

## License

This repository includes the unmodified Doom shareware IWAD, `doom1.wad`,
copyright id Software. It is included under the Doom shareware distribution
terms. The full registered/commercial Doom IWADs are not included and are
not redistributable.
