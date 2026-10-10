# CoolerDoom

Classic Doom rendered through a WebGPU compute shader, with SDL 3 handling
the platform and input layer.

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
