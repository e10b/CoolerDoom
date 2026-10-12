# SPDX-License-Identifier: GPL-2.0-only

cmake_minimum_required(VERSION 3.16...3.25)

project(wDoom LANGUAGES C CXX)

include(FetchContent)

if(EMSCRIPTEN)
    # Emscripten 3.1.61 matches the WebGPU C API used by mywgfx, but predates
    # the bundled SDL3 port. Build SDL3's browser backend from source instead.
    set(SDL_SHARED OFF CACHE BOOL "" FORCE)
    set(SDL_STATIC ON CACHE BOOL "" FORCE)
    set(SDL_TEST_LIBRARY OFF CACHE BOOL "" FORCE)
    set(SDL_TESTS OFF CACHE BOOL "" FORCE)
    set(SDL_EXAMPLES OFF CACHE BOOL "" FORCE)
    FetchContent_Declare(SDL3
        GIT_REPOSITORY https://github.com/libsdl-org/SDL.git
        GIT_TAG release-3.4.2
        GIT_SHALLOW TRUE
    )
    FetchContent_MakeAvailable(SDL3)
    set(DOOM_SDL_TARGET SDL3::SDL3-static)
else()
    set(SDL3_DIR "" CACHE PATH "SDL3 CMake directory")
    list(APPEND CMAKE_PREFIX_PATH "${SDL3_DIR}")
    find_package(SDL3 REQUIRED CONFIG COMPONENTS SDL3-shared)
    set(DOOM_SDL_TARGET SDL3::SDL3-shared)
endif()

FetchContent_Declare(stb
    GIT_REPOSITORY https://github.com/nothings/stb.git
    GIT_TAG 2c980bb59875b0d32144a71867fbdebb2f77cd20
    GIT_SHALLOW TRUE
)
FetchContent_MakeAvailable(stb)

# Match the dependency layout used by mywgfx applications: WebGPU and the
# SDL surface bridge are consumer-owned, while wgfx is the reusable layer.
add_subdirectory(deps/webgpu)
add_subdirectory(deps/wgfx)

add_library(sdl2webgpu STATIC compat/sdl2webgpu.c)
target_include_directories(sdl2webgpu PUBLIC compat)
target_link_libraries(sdl2webgpu PUBLIC ${DOOM_SDL_TARGET} webgpu)
if(APPLE)
    target_compile_options(sdl2webgpu PRIVATE -x objective-c)
    target_link_libraries(sdl2webgpu PRIVATE
        "-framework Cocoa"
        "-framework CoreVideo"
        "-framework IOKit"
        "-framework QuartzCore"
    )
endif()

# Current mywgfx releases retain the historic sdl2webgpu include name.
target_include_directories(wgfx PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/compat"
    "${stb_SOURCE_DIR}"
)
target_link_libraries(wgfx PUBLIC webgpu sdl2webgpu ${DOOM_SDL_TARGET})

file(GLOB_RECURSE doom_c_sources CONFIGURE_DEPENDS "src/*.c")
file(GLOB_RECURSE doom_cpp_sources CONFIGURE_DEPENDS "src/*.cpp")
list(FILTER doom_c_sources EXCLUDE REGEX "src/i_main.c$")

add_executable(wDoom
    ${doom_c_sources}
    ${doom_cpp_sources}
    src/i_main.c
)

target_include_directories(wDoom PRIVATE
    src
    deps/wgfx/src
)
target_link_libraries(wDoom PRIVATE
    wgfx::wgfx
    webgpu
    sdl2webgpu
    ${DOOM_SDL_TARGET}
)
target_compile_features(wDoom PRIVATE cxx_std_17)

if(EMSCRIPTEN)
    if(DEFINED ENV{VERCEL_GIT_COMMIT_SHA} AND NOT "$ENV{VERCEL_GIT_COMMIT_SHA}" STREQUAL "")
        set(WDOOM_BUILD_VERSION "$ENV{VERCEL_GIT_COMMIT_SHA}")
    else()
        execute_process(
            COMMAND git rev-parse --short HEAD
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
            OUTPUT_VARIABLE WDOOM_BUILD_VERSION
            OUTPUT_STRIP_TRAILING_WHITESPACE
            ERROR_QUIET
        )
        if(NOT WDOOM_BUILD_VERSION)
            set(WDOOM_BUILD_VERSION "development")
        endif()
    endif()
    set(DOOM1_WAD "${CMAKE_CURRENT_SOURCE_DIR}/doom1.wad")
    set(DOOM1_WAD_SHA256 "c4e9e6e6cc24a1c5a21c382c396ee3e2d5513d3f196ff464dcdfb2a003fbcc2e")
    if(NOT EXISTS "${DOOM1_WAD}")
        message(FATAL_ERROR "The bundled shareware doom1.wad is missing")
    endif()
    file(SHA256 "${DOOM1_WAD}" DOOM1_WAD_ACTUAL_SHA256)
    if(NOT DOOM1_WAD_ACTUAL_SHA256 STREQUAL DOOM1_WAD_SHA256)
        message(FATAL_ERROR "doom1.wad is not the original unmodified shareware v1.0 IWAD")
    endif()
    set_target_properties(wDoom PROPERTIES
        OUTPUT_NAME index
        SUFFIX ".html"
        LINK_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/web/shell.html"
    )
    configure_file(LICENSE "${CMAKE_BINARY_DIR}/LICENSE" COPYONLY)
    configure_file(LICENSE-DOOM1 "${CMAKE_BINARY_DIR}/LICENSE-DOOM1" COPYONLY)
    configure_file(NOTICE.md "${CMAKE_BINARY_DIR}/NOTICE.md" COPYONLY)
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/title")
    foreach(TITLE_FRAME RANGE 1 5)
        configure_file(
            "${CMAKE_CURRENT_SOURCE_DIR}/title/${TITLE_FRAME}.jpeg"
            "${CMAKE_BINARY_DIR}/title/${TITLE_FRAME}.jpeg"
            COPYONLY
        )
    endforeach()
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/title/a.flac"
        "${CMAKE_BINARY_DIR}/title/a.flac"
        COPYONLY
    )
    target_link_options(wDoom PRIVATE
        "--shell-file"
        "${CMAKE_CURRENT_SOURCE_DIR}/web/shell.html"
        "--preload-file"
        "${DOOM1_WAD}@/doom1.wad"
        "-sALLOW_MEMORY_GROWTH=1"
        "-sASYNCIFY=1"
        "-sEXIT_RUNTIME=0"
        "-sEXPORTED_RUNTIME_METHODS=FS,callMain"
    )
    add_custom_command(TARGET wDoom POST_BUILD
        COMMAND "${CMAKE_COMMAND}"
            -DHTML=$<TARGET_FILE:wDoom>
            -DVERSION=${WDOOM_BUILD_VERSION}
            -P "${CMAKE_CURRENT_SOURCE_DIR}/scripts/stamp-web-build.cmake"
        COMMENT "Stamping web asset URLs with build ${WDOOM_BUILD_VERSION}"
    )
endif()

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/$<CONFIGURATION>")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/$<CONFIGURATION>")

target_copy_webgpu_binaries(wDoom)

