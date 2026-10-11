#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
emsdk_root="${repo_root}/.vercel-cache/emsdk"
build_root="${repo_root}/out/build/vercel"
output_root="${repo_root}/out/vercel"

git -C "${repo_root}" submodule sync --recursive
git -C "${repo_root}" submodule update --init --recursive --depth 1

if [[ ! -x "${emsdk_root}/emsdk" ]]; then
    mkdir -p "$(dirname "${emsdk_root}")"
    git clone --depth 1 --branch 3.1.61 \
        https://github.com/emscripten-core/emsdk.git "${emsdk_root}"
fi

"${emsdk_root}/emsdk" install 3.1.61
"${emsdk_root}/emsdk" activate 3.1.61

# emsdk_env defines the compiler/tool paths used by emcmake.
source "${emsdk_root}/emsdk_env.sh"

emcmake cmake -S "${repo_root}" -B "${build_root}" \
    -DCMAKE_BUILD_TYPE=Release \
    -DWEBGPU_BACKEND=EMSCRIPTEN
cmake --build "${build_root}" --parallel

cmake -E remove_directory "${output_root}"
cmake -E make_directory "${output_root}"
cmake -E copy \
    "${build_root}/index.html" \
    "${build_root}/index.js" \
    "${build_root}/index.wasm" \
    "${build_root}/index.data" \
    "${build_root}/LICENSE" \
    "${build_root}/LICENSE-DOOM1" \
    "${build_root}/NOTICE.md" \
    "${output_root}"
cmake -E copy_directory "${build_root}/title" "${output_root}/title"

test -f "${output_root}/index.html"
test -f "${output_root}/index.js"
test -f "${output_root}/index.wasm"
test -f "${output_root}/index.data"
