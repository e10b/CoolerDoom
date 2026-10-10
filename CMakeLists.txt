cmake_minimum_required(VERSION 3.16...3.25)

project(CustomDoomPort LANGUAGES C CXX)

set(SDL3_DIR "" CACHE PATH "SDL3 CMake directory")
list(APPEND CMAKE_PREFIX_PATH "${SDL3_DIR}")
find_package(SDL3 REQUIRED CONFIG COMPONENTS SDL3-shared)

include(FetchContent)
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
target_link_libraries(sdl2webgpu PUBLIC SDL3::SDL3-shared webgpu)
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
target_link_libraries(wgfx PUBLIC webgpu sdl2webgpu SDL3::SDL3-shared)

file(GLOB_RECURSE doom_c_sources CONFIGURE_DEPENDS "src/*.c")
file(GLOB_RECURSE doom_cpp_sources CONFIGURE_DEPENDS "src/*.cpp")
list(FILTER doom_c_sources EXCLUDE REGEX "src/i_main.c$")

add_executable(CustomDoom
    ${doom_c_sources}
    ${doom_cpp_sources}
    src/i_main.c
)

target_include_directories(CustomDoom PRIVATE
    src
    deps/wgfx/src
)
target_link_libraries(CustomDoom PRIVATE
    wgfx::wgfx
    webgpu
    sdl2webgpu
    SDL3::SDL3-shared
)
target_compile_features(CustomDoom PRIVATE cxx_std_17)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/$<CONFIGURATION>")
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/$<CONFIGURATION>")

target_copy_webgpu_binaries(CustomDoom)

