# wDoom

Classic Doom rendered through a WebGPU compute shader, with SDL 3 handling
the platform and input layer.

## WebGPU rendering

wDoom does more than upload Doom's finished framebuffer. During BSP traversal,
the engine records compact visible-wall commands with screen bounds, 16.16
texture coordinates, and indices into content-addressed texture-column and
colormap tables. The owned byte keys remain correct even if the engine later
reuses an address for different data. These tables persist on the GPU, so each
frame uploads only newly encountered entries. Commands are appended directly to
per-column buckets, and a 321-entry offset table lets each WGSL invocation
process only its own commands in the original drawing order without sorting.

The CPU BSP remains authoritative for visibility, but normal wall calls return
before Doom's software pixel loop: the compute pass independently rasterizes
them. A per-pixel bit mask identifies pixels subsequently owned by floors,
ceilings, masked textures, sprites, weapons, the HUD, or the status bar, avoiding
ambiguous palette-value comparisons during composition. Frames containing a
visible fuzz/invisibility sprite use a narrow CPU wall replay because Doom's
classic fuzz effect samples neighboring framebuffer pixels.

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
