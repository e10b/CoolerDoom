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
void I_WGPUBeginWallFrame(const uint8_t* indexed_framebuffer);
bool I_WGPUQueueWallColumn(int x, int yl, int yh, int32_t frac,
                           int32_t fracstep, const uint8_t* source,
                           const uint8_t* colormap);
void I_WGPUEndWallFrame(void);
void I_WGPUMarkPostWallRect(int x, int y, int width, int height);
void I_WGPUMarkPostWallColumn(int x, int yl, int yh);
void I_WGPURequireCPUWalls(uint8_t* indexed_framebuffer);
void I_WGPUCPUWallLoopStart(void);
void I_WGPUCPUWallLoopEnd(void);
void I_WGPUPresent(const uint8_t* indexed_framebuffer);
void I_WGPUShutdown(void);

#ifdef __cplusplus
}
#endif

#endif
