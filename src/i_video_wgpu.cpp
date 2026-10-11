// SPDX-License-Identifier: GPL-2.0-only

#define WEBGPU_CPP_IMPLEMENTATION
#include <webgpu/webgpu.hpp>

#include <wgfx.h>
#include <sdl2webgpu.h>

#include "i_video_wgpu.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>

namespace
{
constexpr uint32_t kPaletteEntries = 256;
constexpr uint32_t kMaxWallColumns = 32768;
constexpr uint32_t kWallSourceWords = 32;
constexpr uint32_t kWallColormapWords = 64;

struct WallColumn
{
    uint32_t x;
    uint32_t yl;
    uint32_t yh;
    uint32_t frac;
    uint32_t fracstep;
};
static_assert(sizeof(WallColumn) == 5 * sizeof(uint32_t));

uint32_t gSourceWidth = 0;
uint32_t gSourceHeight = 0;
bool gReady = false;
bool gCollectWallColumns = false;
bool gWallSnapshotReady = false;
bool gReportedWallRasterizer = false;

wgfx::Compute* gCompute = nullptr;
wgfx::Compute* gWallCompute = nullptr;
wgfx::Uniform* gIndices = nullptr;
wgfx::Uniform* gPalette = nullptr;
wgfx::Uniform* gOutput = nullptr;
wgfx::Uniform* gWallSnapshotBuffer = nullptr;
wgfx::Uniform* gWallCommandsBuffer = nullptr;
wgfx::Uniform* gWallSourcesBuffer = nullptr;
wgfx::Uniform* gWallColormapsBuffer = nullptr;
wgfx::Uniform* gWallMetaBuffer = nullptr;

wgpu::Texture gFrameTexture = nullptr;
wgpu::TextureView gFrameView = nullptr;
wgpu::Sampler gFrameSampler = nullptr;
wgpu::RenderPipeline gPresentPipeline = nullptr;
wgpu::BindGroupLayout gPresentBindGroupLayout = nullptr;
wgpu::BindGroup gPresentBindGroup = nullptr;

wgfx::ColorTexture gSurfaceColor;
std::vector<uint32_t> gFrameIndices;
std::vector<uint32_t> gWallSnapshot;
std::vector<WallColumn> gWallColumns;
std::vector<uint32_t> gWallSources;
std::vector<uint32_t> gWallColormaps;
std::array<uint32_t, kPaletteEntries> gPackedPalette{};

constexpr const char* kComputeShader = R"WGSL(
@group(0) @binding(0) var<storage, read_write> indices: array<u32>;
@group(0) @binding(1) var<storage, read> palette: array<u32>;
@group(0) @binding(2) var<storage, read_write> outputPixels: array<u32>;

const SOURCE_WIDTH: u32 = 320u;
const SOURCE_HEIGHT: u32 = 200u;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= SOURCE_WIDTH || id.y >= SOURCE_HEIGHT) {
        return;
    }

    let pixel = id.y * SOURCE_WIDTH + id.x;
    let paletteIndex = indices[pixel] & 0xffu;
    outputPixels[pixel] = palette[paletteIndex];
}
)WGSL";

constexpr const char* kWallComputeShader = R"WGSL(
struct WallColumn {
    x: u32,
    yl: u32,
    yh: u32,
    frac: u32,
    fracstep: u32,
};

@group(0) @binding(0) var<storage, read_write> indices: array<u32>;
@group(0) @binding(1) var<storage, read> wallSnapshot: array<u32>;
@group(0) @binding(2) var<storage, read> columns: array<WallColumn>;
@group(0) @binding(3) var<storage, read> sources: array<u32>;
@group(0) @binding(4) var<storage, read> colormaps: array<u32>;
@group(0) @binding(5) var<storage, read> wallMeta: array<u32>;

const SOURCE_WIDTH: u32 = 320u;
const SOURCE_HEIGHT: u32 = 200u;

fn packedByte(words: ptr<storage, array<u32>, read>, base: u32, index: u32) -> u32 {
    let packed = (*words)[base + (index >> 2u)];
    return (packed >> ((index & 3u) * 8u)) & 0xffu;
}

@compute @workgroup_size(8, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    if (x >= SOURCE_WIDTH) {
        return;
    }

    var candidate: array<u32, 200>;
    for (var y = 0u; y < SOURCE_HEIGHT; y++) {
        candidate[y] = wallSnapshot[y * SOURCE_WIDTH + x];
    }

    let columnCount = wallMeta[0];
    for (var commandIndex = 0u; commandIndex < columnCount; commandIndex++) {
        let command = columns[commandIndex];
        if (command.x != x) {
            continue;
        }

        var frac = bitcast<i32>(command.frac);
        let fracstep = bitcast<i32>(command.fracstep);
        let sourceBase = commandIndex * 32u;
        let colormapBase = commandIndex * 64u;
        for (var y = command.yl; y <= command.yh && y < SOURCE_HEIGHT; y++) {
            let textureIndex = u32(frac >> 16) & 127u;
            let sourceIndex = packedByte(&sources, sourceBase, textureIndex);
            candidate[y] = packedByte(&colormaps, colormapBase, sourceIndex);
            frac += fracstep;
        }
    }

    for (var y = 0u; y < SOURCE_HEIGHT; y++) {
        let pixel = y * SOURCE_WIDTH + x;
        if (indices[pixel] == wallSnapshot[pixel]) {
            indices[pixel] = candidate[y];
        }
    }
}
)WGSL";

constexpr const char* kPresentShader = R"WGSL(
@group(0) @binding(0) var frameTexture: texture_2d<f32>;
@group(0) @binding(1) var frameSampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertexIndex: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(2.0, 1.0),
        vec2<f32>(0.0, -1.0)
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertexIndex], 0.0, 1.0);
    output.uv = uvs[vertexIndex];
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(frameTexture, frameSampler, input.uv);
}
)WGSL";

wgpu::ShaderModule CreateShader(const char* source)
{
    wgpu::ShaderModuleDescriptor descriptor{};
#ifdef WEBGPU_BACKEND_WGPU
    descriptor.hintCount = 0;
    descriptor.hints = nullptr;
#endif
    wgpu::ShaderModuleWGSLDescriptor wgsl{};
    wgsl.chain.next = nullptr;
    wgsl.chain.sType = wgpu::SType::ShaderModuleWGSLDescriptor;
    wgsl.code = source;
    descriptor.nextInChain = &wgsl.chain;
    return wgfx::device.createShaderModule(descriptor);
}

bool CreateFrameTexture()
{
    wgpu::TextureDescriptor descriptor{};
    descriptor.dimension = wgpu::TextureDimension::_2D;
    // Doom's PLAYPAL entries are sRGB-encoded bytes. Native swapchains expose
    // an sRGB render target, so decode while sampling and let the target encode
    // again. Browser canvas targets are plain *Unorm and are composited as
    // sRGB; keeping the sampled bytes encoded avoids presenting linear values
    // as if they were already sRGB (which makes the image far too dark).
#ifdef __EMSCRIPTEN__
    descriptor.format = wgpu::TextureFormat::RGBA8Unorm;
#else
    descriptor.format = wgpu::TextureFormat::RGBA8UnormSrgb;
#endif
    descriptor.size = {gSourceWidth, gSourceHeight, 1};
    descriptor.mipLevelCount = 1;
    descriptor.sampleCount = 1;
    descriptor.usage = wgpu::TextureUsage::CopyDst | wgpu::TextureUsage::TextureBinding;
    gFrameTexture = wgfx::device.createTexture(descriptor);
    if (!gFrameTexture)
        return false;

    gFrameView = gFrameTexture.createView();

    wgpu::SamplerDescriptor sampler{};
    sampler.addressModeU = wgpu::AddressMode::ClampToEdge;
    sampler.addressModeV = wgpu::AddressMode::ClampToEdge;
    sampler.addressModeW = wgpu::AddressMode::ClampToEdge;
    sampler.magFilter = wgpu::FilterMode::Nearest;
    sampler.minFilter = wgpu::FilterMode::Nearest;
    sampler.mipmapFilter = wgpu::MipmapFilterMode::Nearest;
    sampler.maxAnisotropy = 1;
    gFrameSampler = wgfx::device.createSampler(sampler);
    return gFrameView && gFrameSampler;
}

bool CreatePresentPipeline()
{
    std::array<wgpu::BindGroupLayoutEntry, 2> layoutEntries{};
    layoutEntries[0].binding = 0;
    layoutEntries[0].visibility = wgpu::ShaderStage::Fragment;
    layoutEntries[0].texture.sampleType = wgpu::TextureSampleType::Float;
    layoutEntries[0].texture.viewDimension = wgpu::TextureViewDimension::_2D;
    layoutEntries[0].texture.multisampled = false;
    layoutEntries[1].binding = 1;
    layoutEntries[1].visibility = wgpu::ShaderStage::Fragment;
    layoutEntries[1].sampler.type = wgpu::SamplerBindingType::Filtering;

    wgpu::BindGroupLayoutDescriptor bindGroupLayoutDescriptor{};
    bindGroupLayoutDescriptor.entryCount = layoutEntries.size();
    bindGroupLayoutDescriptor.entries = layoutEntries.data();
    gPresentBindGroupLayout = wgfx::device.createBindGroupLayout(bindGroupLayoutDescriptor);

    std::array<wgpu::BindGroupEntry, 2> bindGroupEntries{};
    bindGroupEntries[0].binding = 0;
    bindGroupEntries[0].textureView = gFrameView;
    bindGroupEntries[1].binding = 1;
    bindGroupEntries[1].sampler = gFrameSampler;

    wgpu::BindGroupDescriptor bindGroupDescriptor{};
    bindGroupDescriptor.layout = gPresentBindGroupLayout;
    bindGroupDescriptor.entryCount = bindGroupEntries.size();
    bindGroupDescriptor.entries = bindGroupEntries.data();
    gPresentBindGroup = wgfx::device.createBindGroup(bindGroupDescriptor);

    wgpu::PipelineLayoutDescriptor pipelineLayoutDescriptor{};
    pipelineLayoutDescriptor.bindGroupLayoutCount = 1;
    pipelineLayoutDescriptor.bindGroupLayouts =
        reinterpret_cast<WGPUBindGroupLayout*>(&gPresentBindGroupLayout);
    wgpu::PipelineLayout pipelineLayout =
        wgfx::device.createPipelineLayout(pipelineLayoutDescriptor);

    wgpu::ShaderModule shader = CreateShader(kPresentShader);
    wgpu::ColorTargetState colorTarget{};
    colorTarget.format = wgfx::surfaceFormat;
    colorTarget.writeMask = wgpu::ColorWriteMask::All;

    wgpu::FragmentState fragment{};
    fragment.module = shader;
    fragment.entryPoint = "fs_main";
    fragment.targetCount = 1;
    fragment.targets = &colorTarget;

    wgpu::RenderPipelineDescriptor pipelineDescriptor{};
    pipelineDescriptor.layout = pipelineLayout;
    pipelineDescriptor.vertex.module = shader;
    pipelineDescriptor.vertex.entryPoint = "vs_main";
    pipelineDescriptor.vertex.bufferCount = 0;
    pipelineDescriptor.vertex.buffers = nullptr;
    pipelineDescriptor.primitive.topology = wgpu::PrimitiveTopology::TriangleList;
    pipelineDescriptor.primitive.frontFace = wgpu::FrontFace::CCW;
    pipelineDescriptor.primitive.cullMode = wgpu::CullMode::None;
    pipelineDescriptor.fragment = &fragment;
    pipelineDescriptor.multisample.count = 1;
    pipelineDescriptor.multisample.mask = ~0u;
    pipelineDescriptor.multisample.alphaToCoverageEnabled = false;
    gPresentPipeline = wgfx::device.createRenderPipeline(pipelineDescriptor);

    shader.release();
    pipelineLayout.release();
    return gPresentPipeline && gPresentBindGroup;
}
} // namespace

extern "C" bool I_WGPUInit(SDL_Window* window, uint32_t sourceWidth,
                             uint32_t sourceHeight)
{
    if (sourceWidth != 320 || sourceHeight != 200)
    {
        std::cerr << "Compute presenter currently expects Doom's 320x200 framebuffer\n";
        return false;
    }

    gSourceWidth = sourceWidth;
    gSourceHeight = sourceHeight;
    const uint32_t pixelCount = sourceWidth * sourceHeight;
    gFrameIndices.resize(pixelCount);
    gWallSnapshot.resize(pixelCount);
    gWallColumns.reserve(kMaxWallColumns);
    gWallSources.reserve(kMaxWallColumns * kWallSourceWords);
    gWallColormaps.reserve(kMaxWallColumns * kWallColormapWords);

    wgpu::Surface surface = wgfx::getSurface(window);
    if (!surface)
        return false;
    wgfx::init(surface);
    if (!wgfx::device || !wgfx::queue)
        return false;

    gCompute = wgfx::loadCompute(kComputeShader);
    gIndices = wgfx::createStorage(0, gFrameIndices.size() * sizeof(uint32_t),
                                   gFrameIndices.data(), false);
    gPalette = wgfx::createStorage(1, gPackedPalette.size() * sizeof(uint32_t),
                                   gPackedPalette.data(), true);
    gOutput = wgfx::createStorage(2, sourceWidth * sourceHeight * sizeof(uint32_t),
                                  nullptr, false);
    gCompute->setStorage(gIndices);
    gCompute->setStorage(gPalette);
    gCompute->setStorage(gOutput);
    gCompute->init();

    gWallCompute = wgfx::loadCompute(kWallComputeShader);
    gWallSnapshotBuffer = wgfx::createStorage(
        1, gWallSnapshot.size() * sizeof(uint32_t), nullptr, true);
    gWallCommandsBuffer = wgfx::createStorage(
        2, kMaxWallColumns * sizeof(WallColumn), nullptr, true);
    gWallSourcesBuffer = wgfx::createStorage(
        3, kMaxWallColumns * kWallSourceWords * sizeof(uint32_t), nullptr, true);
    gWallColormapsBuffer = wgfx::createStorage(
        4, kMaxWallColumns * kWallColormapWords * sizeof(uint32_t), nullptr, true);
    const uint32_t noWallColumns = 0;
    gWallMetaBuffer = wgfx::createStorage(5, sizeof(uint32_t), &noWallColumns, true);
    gWallCompute->setStorage(gIndices);
    gWallCompute->setStorage(gWallSnapshotBuffer);
    gWallCompute->setStorage(gWallCommandsBuffer);
    gWallCompute->setStorage(gWallSourcesBuffer);
    gWallCompute->setStorage(gWallColormapsBuffer);
    gWallCompute->setStorage(gWallMetaBuffer);
    gWallCompute->init();

    gReady = CreateFrameTexture() && CreatePresentPipeline();
    return gReady;
}

extern "C" void I_WGPUBeginWallFrame(void)
{
    gWallColumns.clear();
    gWallSources.clear();
    gWallColormaps.clear();
    gWallSnapshotReady = false;
    gCollectWallColumns = gReady;
}

extern "C" void I_WGPUQueueWallColumn(int x, int yl, int yh, int32_t frac,
                                         int32_t fracstep,
                                         const uint8_t* source,
                                         const uint8_t* colormap)
{
    if (!gCollectWallColumns || !source || !colormap || x < 0 || yl < 0 ||
        yh < yl || static_cast<uint32_t>(x) >= gSourceWidth ||
        static_cast<uint32_t>(yl) >= gSourceHeight ||
        gWallColumns.size() >= kMaxWallColumns)
        return;

    yh = std::min(yh, static_cast<int>(gSourceHeight) - 1);
    gWallColumns.push_back({static_cast<uint32_t>(x), static_cast<uint32_t>(yl),
                            static_cast<uint32_t>(yh), static_cast<uint32_t>(frac),
                            static_cast<uint32_t>(fracstep)});

    for (uint32_t word = 0; word < kWallSourceWords; ++word)
    {
        const uint32_t byte = word * 4;
        gWallSources.push_back(uint32_t(source[byte]) |
                               (uint32_t(source[byte + 1]) << 8u) |
                               (uint32_t(source[byte + 2]) << 16u) |
                               (uint32_t(source[byte + 3]) << 24u));
    }
    for (uint32_t word = 0; word < kWallColormapWords; ++word)
    {
        const uint32_t byte = word * 4;
        gWallColormaps.push_back(uint32_t(colormap[byte]) |
                                 (uint32_t(colormap[byte + 1]) << 8u) |
                                 (uint32_t(colormap[byte + 2]) << 16u) |
                                 (uint32_t(colormap[byte + 3]) << 24u));
    }
}

extern "C" void I_WGPUEndWallFrame(const uint8_t* framebuffer)
{
    gCollectWallColumns = false;
    if (!gReady || !framebuffer || gWallColumns.empty())
        return;

    for (uint32_t i = 0; i < gSourceWidth * gSourceHeight; ++i)
        gWallSnapshot[i] = framebuffer[i];
    gWallSnapshotReady = true;
}

extern "C" void I_WGPUSetPalette(const uint8_t* palette)
{
    if (!palette)
        return;

    for (uint32_t i = 0; i < kPaletteEntries; ++i)
    {
        const uint32_t r = palette[i * 3 + 0];
        const uint32_t g = palette[i * 3 + 1];
        const uint32_t b = palette[i * 3 + 2];
        gPackedPalette[i] = r | (g << 8u) | (b << 16u) | 0xff000000u;
    }

    if (gReady)
        gCompute->updateStorageBuffer(gPalette, gPackedPalette.data(),
                                      gPackedPalette.size() * sizeof(uint32_t));
}

extern "C" void I_WGPUPresent(const uint8_t* framebuffer)
{
    if (!gReady || !framebuffer)
        return;

    const uint32_t pixelCount = gSourceWidth * gSourceHeight;
    for (uint32_t i = 0; i < pixelCount; ++i)
        gFrameIndices[i] = framebuffer[i];
    gCompute->updateStorageBuffer(gIndices, gFrameIndices.data(),
                                  gFrameIndices.size() * sizeof(uint32_t));

    if (gWallSnapshotReady && !gWallColumns.empty())
    {
        const uint32_t wallColumnCount = gWallColumns.size();
        if (!gReportedWallRasterizer)
        {
            std::cout << "WebGPU wall rasterizer active: " << wallColumnCount
                      << " BSP columns in first rendered frame\n";
            gReportedWallRasterizer = true;
        }
        gWallCompute->updateStorageBuffer(
            gWallSnapshotBuffer, gWallSnapshot.data(),
            gWallSnapshot.size() * sizeof(uint32_t));
        gWallCompute->updateStorageBuffer(
            gWallCommandsBuffer, gWallColumns.data(),
            gWallColumns.size() * sizeof(WallColumn));
        gWallCompute->updateStorageBuffer(
            gWallSourcesBuffer, gWallSources.data(),
            gWallSources.size() * sizeof(uint32_t));
        gWallCompute->updateStorageBuffer(
            gWallColormapsBuffer, gWallColormaps.data(),
            gWallColormaps.size() * sizeof(uint32_t));
        gWallCompute->updateStorageBuffer(
            gWallMetaBuffer, &wallColumnCount, sizeof(wallColumnCount));
    }

    int windowWidth = 0;
    int windowHeight = 0;
    SDL_GetWindowSize(wgfx::window, &windowWidth, &windowHeight);
    if (windowWidth != wgfx::width || windowHeight != wgfx::height)
        wgfx::initSurface();

    wgfx::touch(&gSurfaceColor);
    if (!gSurfaceColor.colorView || !wgfx::encoder)
        return;

    if (gWallSnapshotReady && !gWallColumns.empty())
    {
        wgfx::ComputePass wallPass;
        wallPass.prepare();
        wallPass.draw(gWallCompute, (gSourceWidth + 7) / 8, 1, 1);
        wallPass.end();
    }

    wgfx::ComputePass computePass;
    computePass.prepare();
    computePass.draw(gCompute, (gSourceWidth + 7) / 8,
                     (gSourceHeight + 7) / 8, 1);
    computePass.end();

    wgpu::ImageCopyBuffer source{};
    source.buffer = gOutput->buffer;
    source.layout.offset = 0;
    source.layout.bytesPerRow = gSourceWidth * sizeof(uint32_t);
    source.layout.rowsPerImage = gSourceHeight;
    wgpu::ImageCopyTexture destination{};
    destination.texture = gFrameTexture;
    destination.mipLevel = 0;
    destination.origin = {0, 0, 0};
    destination.aspect = wgpu::TextureAspect::All;
    wgfx::encoder.copyBufferToTexture(source, destination,
                                      {gSourceWidth, gSourceHeight, 1});

    wgpu::RenderPassColorAttachment attachment{};
    attachment.view = gSurfaceColor.colorView;
#ifdef __EMSCRIPTEN__
    attachment.depthSlice = WGPU_DEPTH_SLICE_UNDEFINED;
#endif
    attachment.loadOp = wgpu::LoadOp::Clear;
    attachment.storeOp = wgpu::StoreOp::Store;
    attachment.clearValue = {0.0, 0.0, 0.0, 1.0};
    wgpu::RenderPassDescriptor passDescriptor{};
    passDescriptor.colorAttachmentCount = 1;
    passDescriptor.colorAttachments = &attachment;
    wgpu::RenderPassEncoder pass = wgfx::encoder.beginRenderPass(passDescriptor);
    pass.setPipeline(gPresentPipeline);
    pass.setBindGroup(0, gPresentBindGroup, 0, nullptr);
    pass.draw(3, 1, 0, 0);
    pass.end();
    pass.release();

    wgfx::frame();
}

extern "C" void I_WGPUShutdown(void)
{
    gReady = false;
    if (gPresentBindGroup) gPresentBindGroup.release();
    if (gPresentBindGroupLayout) gPresentBindGroupLayout.release();
    if (gPresentPipeline) gPresentPipeline.release();
    if (gFrameSampler) gFrameSampler.release();
    if (gFrameView) gFrameView.release();
    if (gFrameTexture) gFrameTexture.release();
}
