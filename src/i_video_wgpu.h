// SPDX-License-Identifier: GPL-2.0-only

#ifndef I_VIDEO_WGPU_H
#define I_VIDEO_WGPU_H

#include <stdbool.h>
#include <stdint.h>
#include <SDL3/SDL.h>

#ifdef __cplusplus
extern "C" {
#endif

bool I_WGPUInit(SDL_Window* window, uint32_t source_width, uint32_t source_height);
void I_WGPUSetPalette(const uint8_t* rgb_palette);
void I_WGPUPresent(const uint8_t* indexed_framebuffer);
void I_WGPUShutdown(void);

#ifdef __cplusplus
}
#endif

#endif
