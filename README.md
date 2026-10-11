# wDoom

Classic Doom rendered through a WebGPU compute shader, with SDL 3 handling
the platform and input layer.

## WebGPU rendering

wDoom does more than upload Doom's finished framebuffer. During BSP traversal,
the engine records the visible wall columns with their screen bounds, 16.16
texture coordinates, texture texels, and Doom colormap. A WGSL compute pass
replays the original fixed-point wall-column rasterizer on the GPU before the
palette conversion and presentation passes.

The CPU BSP remains authoritative for visibility and currently keeps a mirrored
software result as a compatibility fallback. The compositor preserves pixels
drawn later by floors, ceilings, masked textures, sprites, weapons, the HUD,
and status bar. This makes wall rasterization a real GPU rendering stage while
retaining Doom's original rendering order and indexed-color behavior.

## Lineage

- **DOOM** — the original engine and GPL source release by id Software.
- **CoolDoom** — brubid's SDL 3 source port and the direct base of this fork.
- **wDoom** — e10b's continuation with macOS support, a mywgfx WebGPU compute
  renderer, and an Emscripten browser build.

## Browser build

The browser backend currently targets Emscripten 3.1.61, which matches the
WebGPU C API used by the pinned `webgpu-distribution` and `mywgfx` submodules.

```sh
git submodule update --init --recursive
source /path/to/emsdk/emsdk_env.sh
emcmake cmake -S . -B out/build/web -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DWEBGPU_BACKEND=EMSCRIPTEN
cmake --build out/build/web -j
python3 -m http.server 8765 --bind 127.0.0.1 --directory out/build/web
```

Open <http://127.0.0.1:8765/index.html> in a WebGPU-capable browser. The build
packages the repository's verified shareware IWAD as `/doom1.wad`.

## License and game data

wDoom is free software distributed under the **GNU General Public License,
version 2 only**. The complete license is in [LICENSE](LICENSE), with project
lineage and modification notices in [NOTICE.md](NOTICE.md). Corresponding
source is this repository, including its pinned submodules.

The GPL covers the engine source code. It does **not** cover the bundled
`doom1.wad`, which is the original, unmodified DOOM shareware v1.0 Episode 1
IWAD (4,207,819 bytes; SHA-256
`c4e9e6e6cc24a1c5a21c382c396ee3e2d5513d3f196ff464dcdfb2a003fbcc2e`).
That file is Copyright © 1993 id Software, Inc. and remains under id Software's
separate shareware terms in [LICENSE-DOOM1](LICENSE-DOOM1). Do not charge for
access to or use of the shareware content. No commercial `DOOM.WAD` is included.
