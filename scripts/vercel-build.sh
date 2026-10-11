#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
emsdk_root="${repo_root}/.vercel-cache/emsdk"
tools_root="${repo_root}/.vercel-cache/build-tools"
build_root="${repo_root}/out/build/vercel-deploy"
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
export EMSDK_QUIET=1
source "${emsdk_root}/emsdk_env.sh"

# Vercel's generic Linux image can expose a non-executable CMake. Install
# official pinned binaries when the host tools cannot actually be run.
if ! cmake --version >/dev/null 2>&1 || ! ninja --version >/dev/null 2>&1; then
    if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
        echo "Executable CMake and Ninja are required on non-Vercel hosts." >&2
        exit 1
    fi
    # This directory contains only disposable downloaded build tools.
    rm -rf -- "${tools_root}"
    mkdir -p "${tools_root}/bin"
    curl --fail --location --retry 3 \
        https://github.com/Kitware/CMake/releases/download/v3.30.5/cmake-3.30.5-linux-x86_64.tar.gz \
        --output "${tools_root}/cmake.tar.gz"
    tar -xzf "${tools_root}/cmake.tar.gz" --strip-components=1 \
        -C "${tools_root}"
    curl --fail --location --retry 3 \
        https://github.com/ninja-build/ninja/releases/download/v1.11.1/ninja-linux.zip \
        --output "${tools_root}/ninja.zip"
    "${EMSDK_PYTHON}" -c \
        'import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])' \
        "${tools_root}/ninja.zip" "${tools_root}/bin"
    chmod +x "${tools_root}/bin/ninja"
    export PATH="${tools_root}/bin:${PATH}"
fi

cmake --version
ninja --version

emcmake cmake -S "${repo_root}" -B "${build_root}" -G Ninja \
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
