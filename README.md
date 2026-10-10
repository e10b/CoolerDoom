# wDoom

Classic Doom rendered through a WebGPU compute shader, with SDL 3 handling
the platform and input layer.

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
  -DWEBGPU_BACKEND=EMSCRIPTEN \
  -DDOOM_WAD=/absolute/path/to/DOOM.WAD
cmake --build out/build/web -j
python3 -m http.server 8765 --bind 127.0.0.1 --directory out/build/web
```

Open <http://127.0.0.1:8765/index.html> in a WebGPU-capable browser. The build
packages the selected IWAD as `/doom.wad`; WAD files and generated output are
not tracked by Git.

## License and game data

wDoom is free software distributed under the **GNU General Public License,
version 2 only**. The complete license is in [LICENSE](LICENSE), with project
lineage and modification notices in [NOTICE.md](NOTICE.md). Corresponding
source is this repository, including its pinned submodules.

The GPL covers the source code, not commercial DOOM game data. No IWAD is
included in the repository. Build and distribute only with game data you are
legally permitted to use and redistribute; in particular, do not publish a
web build containing a retail `DOOM.WAD`.
