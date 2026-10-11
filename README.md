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

Pass `-wgpuvalidate` to enable comprehensive validation (`?wgpuvalidate` in the
browser). In this mode every eligible frame continues through DOOM's original
software wall loops, preserving that completed indexed framebuffer as the
independent reference. WebGPU separately consumes the generated command stream,
copies its post-compute index buffer into a per-frame staging buffer, and compares
all 64,000 pixels after queue completion. This catches command-generation,
resource-packing, shader, and composition errors rather than merely comparing
two implementations that share prepared texture data.

Reports include mismatch count, the first differing coordinates and palette
indices, the complete CPU BSP/command/software-wall phase, time spent specifically
inside the original wall raster loops, an estimated equivalent GPU wall path
(CPU traversal/command preparation without those loops, plus upload, submission,
and wall-only queue completion), reference-copy time, and mapping/readback. The
wall compute and readback copy are submitted separately from palette conversion
and presentation so the completion boundary does not include those later passes.
Samples own their reference data, staging buffer, and callback
state. Shutdown and queue/device failure cancel samples without releasing memory
still reachable by asynchronous callbacks. Queue completion is deliberately not
described as pure shader time: browser scheduling and synchronization are part
of that wall-clock measurement. For a reproducible run, record a demo with this
engine and replay it with `-wgpuvalidate -playdemo <name>`.

Validation ends with an aggregate `PASS`/`FAIL` line containing submitted,
completed, and failed frame counts, total compared and mismatched pixels,
readback failures, and callbacks still pending. A run passes only if every
submitted frame completed, no pixel differed, and no asynchronous work remains.

For an end-to-end performance comparison, replay the same demo twice with
`-timedemo`: once normally for GPU walls and once with `-cpuwalls` for DOOM's
original software wall loops. Both modes retain the same WebGPU palette and
presentation path, while the timedemo result includes command preparation,
uploads, submission, and the native backend's queue synchronization. Do not
combine `-cpuwalls` with `-wgpuvalidate`.

### Measured baseline

On 2026-10-10, the native wgpu backend was tested on an Apple M3 Pro (14 GPU
cores), arm64 macOS 15.6.1, using the locally supplied Ultimate DOOM v1.9 IWAD
(`6fdf361847b46228cfebd9f3af09cd844282ac75f3edbb61ca4cb27103ce2e7f`). The
commercial IWAD is test input only and is not distributed by this repository.

All four complete embedded demos passed `-wgpuvalidate -timedemo`: 8,738 of
8,738 eligible frames completed, 559,232,000 indexed framebuffer pixels were
compared, and there were zero mismatches, readback failures, or pending
callbacks. This establishes equivalence for that deterministic demo corpus; it
is not a proof covering every possible game state.

Five end-to-end runs of demo 1 produced these realtic counts (35 realtics per
second):

| wall path | five runs | mean | mean throughput |
| --- | --- | ---: | ---: |
| WebGPU | 251, 261, 294, 195, 232 | 246.6 | 242.7 frames/s |
| original CPU | 169, 170, 168, 163, 163 | 166.6 | 359.2 frames/s |

For this baseline the WebGPU path is **not faster**: it takes 48.0% longer
end-to-end and delivers 32.4% less throughput. The comparison deliberately
leaves validation and readback disabled and changes only `-cpuwalls`; both runs
still use the same WebGPU palette conversion, presentation, and native queue
synchronization.

```sh
./wDoom -iwad /path/to/DOOM.WAD -wgpuvalidate -timedemo demo1
./wDoom -iwad /path/to/DOOM.WAD -timedemo demo1
./wDoom -iwad /path/to/DOOM.WAD -cpuwalls -timedemo demo1
```

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
