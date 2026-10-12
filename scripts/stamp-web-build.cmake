# SPDX-License-Identifier: GPL-2.0-only

if(NOT DEFINED HTML OR NOT EXISTS "${HTML}")
    message(FATAL_ERROR "HTML must name the generated Emscripten page")
endif()
if(NOT DEFINED VERSION OR VERSION STREQUAL "")
    message(FATAL_ERROR "VERSION must identify this web build")
endif()

file(READ "${HTML}" contents)
string(REPLACE "__WDOOM_BUILD_VERSION__" "${VERSION}" contents "${contents}")
string(REPLACE "src=\"index.js\"" "src=\"index.js?v=${VERSION}\"" contents "${contents}")
string(REPLACE "src=index.js" "src=\"index.js?v=${VERSION}\"" contents "${contents}")
file(WRITE "${HTML}" "${contents}")
