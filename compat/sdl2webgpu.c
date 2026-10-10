// SPDX-License-Identifier: GPL-2.0-only

#include "sdl2webgpu.h"

#if defined(__EMSCRIPTEN__)
#include <emscripten/html5.h>
#elif defined(SDL_PLATFORM_MACOS)
#include <Cocoa/Cocoa.h>
#include <Foundation/Foundation.h>
#include <QuartzCore/CAMetalLayer.h>
#endif

#if defined(SDL_PLATFORM_WIN32)
#include <windows.h>
#endif

WGPUSurface SDL_GetWGPUSurface(WGPUInstance instance, SDL_Window* window)
{
#if defined(__EMSCRIPTEN__)
    (void)window;
    WGPUSurfaceDescriptorFromCanvasHTMLSelector canvasDescriptor = {
        .chain = {
            .next = NULL,
            .sType = WGPUSType_SurfaceDescriptorFromCanvasHTMLSelector,
        },
        .selector = "#canvas",
    };
    WGPUSurfaceDescriptor descriptor = {
        .nextInChain = (const WGPUChainedStruct*)&canvasDescriptor,
        .label = NULL,
    };
    return wgpuInstanceCreateSurface(instance, &descriptor);
#elif defined(SDL_PLATFORM_MACOS)
    id metalLayer = NULL;
    NSWindow* nativeWindow = (__bridge NSWindow*)SDL_GetPointerProperty(
        SDL_GetWindowProperties(window),
        SDL_PROP_WINDOW_COCOA_WINDOW_POINTER,
        NULL);
    [nativeWindow.contentView setWantsLayer:YES];
    metalLayer = [CAMetalLayer layer];
    [nativeWindow.contentView setLayer:metalLayer];

    WGPUSurfaceDescriptorFromMetalLayer metalDescriptor = {
        .chain = {
            .next = NULL,
            .sType = WGPUSType_SurfaceDescriptorFromMetalLayer,
        },
        .layer = metalLayer,
    };
    WGPUSurfaceDescriptor descriptor = {
        .nextInChain = (const WGPUChainedStruct*)&metalDescriptor,
        .label = NULL,
    };
    return wgpuInstanceCreateSurface(instance, &descriptor);
#elif defined(SDL_PLATFORM_WIN32)
    HWND hwnd = (HWND)SDL_GetPointerProperty(
        SDL_GetWindowProperties(window),
        SDL_PROP_WINDOW_WIN32_HWND_POINTER,
        NULL);
    WGPUSurfaceDescriptorFromWindowsHWND windowsDescriptor = {
        .chain = {
            .next = NULL,
            .sType = WGPUSType_SurfaceDescriptorFromWindowsHWND,
        },
        .hinstance = GetModuleHandle(NULL),
        .hwnd = hwnd,
    };
    WGPUSurfaceDescriptor descriptor = {
        .nextInChain = (const WGPUChainedStruct*)&windowsDescriptor,
        .label = NULL,
    };
    return wgpuInstanceCreateSurface(instance, &descriptor);
#else
    (void)instance;
    (void)window;
    return NULL;
#endif
}
