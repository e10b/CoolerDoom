// SPDX-License-Identifier: GPL-2.0-only

#define WEBGPU_CPP_IMPLEMENTATION
#include <webgpu/webgpu.hpp>

#include <wgfx.h>
#include <sdl2webgpu.h>

#include "i_video_wgpu.h"
extern "C" {
#include "m_argv.h"
}

#include <algorithm>
#include <array>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <memory>
#include <string>
#include <unordered_map>
#include <vector>

namespace
{
constexpr uint32_t kPaletteEntries = 256;
constexpr uint32_t kMaxWallColumns = 32768;
constexpr uint32_t kWallSourceWords = 32;
constexpr uint32_t kWallColormapWords = 64;
constexpr uint32_t kMaskWords = (320 * 200 + 31) / 32;

struct WallColumn
{
    uint32_t x;
    uint32_t yl;
    uint32_t yh;
    uint32_t frac;
    uint32_t fracstep;
    uint32_t sourceIndex;
    uint32_t colormapIndex;
};
static_assert(sizeof(WallColumn) == 7 * sizeof(uint32_t));

uint32_t gSourceWidth = 0;
uint32_t gSourceHeight = 0;
bool gReady = false;
bool gCollectWallColumns = false;
bool gWallSnapshotReady = false;
bool gPostWallPhase = false;
bool gCPUWallsReady = false;
bool gReportedWallRasterizer = false;
bool gValidationMode = false;
bool gCPUWallMode = false;
bool gShuttingDown = false;
uint64_t gValidationFrame = 0;
uint64_t gValidationCompletedFrames = 0;
uint64_t gValidationFailedFrames = 0;
uint64_t gValidationComparedPixels = 0;
uint64_t gValidationMismatchPixels = 0;
uint64_t gValidationReadbackFailures = 0;

wgfx::Compute* gCompute = nullptr;
wgfx::Compute* gWallCompute = nullptr;
wgfx::Uniform* gIndices = nullptr;
wgfx::Uniform* gPalette = nullptr;
wgfx::Uniform* gOutput = nullptr;
wgfx::Uniform* gWallSnapshotBuffer = nullptr;
wgfx::Uniform* gWallCommandsBuffer = nullptr;
wgfx::Uniform* gWallSourcesBuffer = nullptr;
wgfx::Uniform* gWallColormapsBuffer = nullptr;
wgfx::Uniform* gWallOffsetsBuffer = nullptr;
wgfx::Uniform* gPostWallMaskBuffer = nullptr;

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
template <size_t N> struct ByteBlock
{
    std::array<uint8_t, N> bytes;
    bool operator==(const ByteBlock& other) const { return bytes == other.bytes; }
};
template <size_t N> struct ByteBlockHash
{
    size_t operator()(const ByteBlock<N>& block) const
    {
        size_t hash = sizeof(size_t) == 8 ? 1469598103934665603ull : 2166136261u;
        for (uint8_t byte : block.bytes)
            hash = (hash ^ byte) * (sizeof(size_t) == 8 ? 1099511628211ull : 16777619u);
        return hash;
    }
};
using SourceBlock = ByteBlock<128>;
using ColormapBlock = ByteBlock<256>;
std::unordered_map<SourceBlock, uint32_t, ByteBlockHash<128>> gWallSourceIndices;
std::unordered_map<ColormapBlock, uint32_t, ByteBlockHash<256>> gWallColormapIndices;
std::array<std::vector<WallColumn>, 320> gWallBuckets;
size_t gWallColumnCount = 0;
size_t gUploadedSourceWords = 0;
size_t gUploadedColormapWords = 0;
uint64_t gWallBucketMicros = 0;
std::array<uint32_t, 321> gWallOffsets{};
std::array<uint32_t, kMaskWords> gPostWallMask{};
std::array<uint32_t, kPaletteEntries> gPackedPalette{};
std::chrono::steady_clock::time_point gCPUWallStart;
uint64_t gCPUWallMicros = 0;
uint64_t gCPUWallLoopNanos = 0;
std::chrono::steady_clock::time_point gCPUWallLoopStart;

struct ValidationSample
{
    uint64_t frame = 0;
    uint64_t cpuWallMicros = 0;
    uint64_t cpuWallLoopNanos = 0;
    uint64_t referenceCopyMicros = 0;
    uint64_t uploadMicros = 0;
    uint64_t submitMicros = 0;
    std::chrono::steady_clock::time_point gpuStart;
    std::chrono::steady_clock::time_point mapStart;
    std::vector<uint32_t> expected;
    wgpu::Buffer readback = nullptr;
    std::unique_ptr<wgpu::QueueWorkDoneCallback> workDoneCallback;
    std::unique_ptr<wgpu::BufferMapCallback> mapCallback;
    bool cancelled = false;
    bool finished = false;

    ~ValidationSample()
    {
        if (readback)
        {
            readback.destroy();
            readback.release();
        }
    }
};
std::vector<std::shared_ptr<ValidationSample>> gValidationSamples;

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
    sourceIndex: u32,
    colormapIndex: u32,
};

@group(0) @binding(0) var<storage, read_write> indices: array<u32>;
@group(0) @binding(1) var<storage, read> wallSnapshot: array<u32>;
@group(0) @binding(2) var<storage, read> columns: array<WallColumn>;
@group(0) @binding(3) var<storage, read> sources: array<u32>;
@group(0) @binding(4) var<storage, read> colormaps: array<u32>;
@group(0) @binding(5) var<storage, read> columnOffsets: array<u32>;
@group(0) @binding(6) var<storage, read> postWallMask: array<u32>;

const SOURCE_WIDTH: u32 = 320u;
const SOURCE_HEIGHT: u32 = 200u;

fn sourceByte(base: u32, index: u32) -> u32 {
    let packed = sources[base + (index >> 2u)];
    return (packed >> ((index & 3u) * 8u)) & 0xffu;
}

fn colormapByte(base: u32, index: u32) -> u32 {
    let packed = colormaps[base + (index >> 2u)];
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

    let firstCommand = columnOffsets[x];
    let lastCommand = columnOffsets[x + 1u];
    for (var commandIndex = firstCommand; commandIndex < lastCommand; commandIndex++) {
        let command = columns[commandIndex];

        var frac = bitcast<i32>(command.frac);
        let fracstep = bitcast<i32>(command.fracstep);
        let sourceBase = command.sourceIndex * 32u;
        let colormapBase = command.colormapIndex * 64u;
        for (var y = command.yl; y <= command.yh && y < SOURCE_HEIGHT; y++) {
            let textureIndex = u32(frac >> 16) & 127u;
            let sourceIndex = sourceByte(sourceBase, textureIndex);
            candidate[y] = colormapByte(colormapBase, sourceIndex);
            frac += fracstep;
        }
    }

    for (var y = 0u; y < SOURCE_HEIGHT; y++) {
        let pixel = y * SOURCE_WIDTH + x;
        let wasDrawnAfterWalls = (postWallMask[pixel >> 5u] &
                                  (1u << (pixel & 31u))) != 0u;
        if (!wasDrawnAfterWalls) {
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
    gShuttingDown = false;
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

    gValidationMode = M_CheckParm(const_cast<char*>("-wgpuvalidate")) != 0;
    gCPUWallMode = M_CheckParm(const_cast<char*>("-cpuwalls")) != 0;
    if (gCPUWallMode && gValidationMode)
    {
        std::cerr << "-cpuwalls and -wgpuvalidate are mutually exclusive\n";
        return false;
    }
    if (gCPUWallMode)
        std::cout << "CPU wall benchmark mode: original software wall loops active\n";
    if (gValidationMode)
        std::cout << "WebGPU validation mode: original software renderer and GPU "
                     "will be compared every eligible frame\n";

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
    gWallOffsetsBuffer = wgfx::createStorage(
        5, gWallOffsets.size() * sizeof(uint32_t), gWallOffsets.data(), true);
    gPostWallMaskBuffer = wgfx::createStorage(
        6, gPostWallMask.size() * sizeof(uint32_t), gPostWallMask.data(), true);
    gWallCompute->setStorage(gIndices);
    gWallCompute->setStorage(gWallSnapshotBuffer);
    gWallCompute->setStorage(gWallCommandsBuffer);
    gWallCompute->setStorage(gWallSourcesBuffer);
    gWallCompute->setStorage(gWallColormapsBuffer);
    gWallCompute->setStorage(gWallOffsetsBuffer);
    gWallCompute->setStorage(gPostWallMaskBuffer);
    gWallCompute->init();

    gReady = CreateFrameTexture() && CreatePresentPipeline();
    return gReady;
}

extern "C" void I_WGPUBeginWallFrame(const uint8_t* framebuffer)
{
    gWallColumns.clear();
    // Texture columns and colormaps are immutable content-addressed resources.
    // Keep them across frames and upload only entries discovered this frame.
    for (auto& bucket : gWallBuckets)
        bucket.clear();
    gWallColumnCount = 0;
    gPostWallMask.fill(0);
    gWallSnapshotReady = false;
    gPostWallPhase = false;
    gCPUWallsReady = false;
    gCPUWallLoopNanos = 0;
    gCollectWallColumns = gReady && framebuffer && !gCPUWallMode;
    if (gCollectWallColumns)
    {
        if (gValidationMode)
            gCPUWallStart = std::chrono::steady_clock::now();
        for (uint32_t i = 0; i < gSourceWidth * gSourceHeight; ++i)
            gWallSnapshot[i] = framebuffer[i];
    }
}

template <size_t N>
static uint32_t InternBytes(const uint8_t* bytes,
                            std::unordered_map<ByteBlock<N>, uint32_t,
                                               ByteBlockHash<N>>& indices,
                            std::vector<uint32_t>& packed)
{
    ByteBlock<N> block{};
    std::memcpy(block.bytes.data(), bytes, N);
    const auto found = indices.find(block);
    if (found != indices.end())
        return found->second;
    const uint32_t index = static_cast<uint32_t>(indices.size());
    indices.emplace(block, index);
    for (uint32_t word = 0; word < N / 4; ++word)
    {
        const uint32_t byte = word * 4;
        packed.push_back(uint32_t(bytes[byte]) |
                         (uint32_t(bytes[byte + 1]) << 8u) |
                         (uint32_t(bytes[byte + 2]) << 16u) |
                         (uint32_t(bytes[byte + 3]) << 24u));
    }
    return index;
}

extern "C" bool I_WGPUQueueWallColumn(int x, int yl, int yh, int32_t frac,
                                         int32_t fracstep,
                                         const uint8_t* source,
                                         const uint8_t* colormap)
{
    if (!gCollectWallColumns || !source || !colormap || x < 0 || yl < 0 ||
        yh < yl || static_cast<uint32_t>(x) >= gSourceWidth ||
        static_cast<uint32_t>(yl) >= gSourceHeight ||
        gWallColumnCount >= kMaxWallColumns)
        return false;

    yh = std::min(yh, static_cast<int>(gSourceHeight) - 1);
    const uint32_t sourceIndex = InternBytes<128>(source,
        gWallSourceIndices, gWallSources);
    const uint32_t colormapIndex = InternBytes<256>(colormap,
        gWallColormapIndices, gWallColormaps);
    gWallBuckets[x].push_back({static_cast<uint32_t>(x), static_cast<uint32_t>(yl),
                            static_cast<uint32_t>(yh), static_cast<uint32_t>(frac),
                            static_cast<uint32_t>(fracstep), sourceIndex,
                            colormapIndex});
    ++gWallColumnCount;
    // Validation deliberately continues through DOOM's original software
    // pixel loop, producing a reference independent of this command packing.
    return !gValidationMode;
}

extern "C" void I_WGPUEndWallFrame(void)
{
    gCollectWallColumns = false;
    gPostWallPhase = true;
    if (!gReady || gWallColumnCount == 0)
        return;

    const auto bucketStart = std::chrono::steady_clock::now();
    gWallColumns.reserve(gWallColumnCount);
    for (uint32_t x = 0; x < gSourceWidth; ++x)
    {
        gWallOffsets[x] = static_cast<uint32_t>(gWallColumns.size());
        gWallColumns.insert(gWallColumns.end(), gWallBuckets[x].begin(),
                            gWallBuckets[x].end());
    }
    gWallOffsets[gSourceWidth] = static_cast<uint32_t>(gWallColumns.size());
    gWallBucketMicros = std::chrono::duration_cast<std::chrono::microseconds>(
        std::chrono::steady_clock::now() - bucketStart).count();
    gWallSnapshotReady = true;
    if (gValidationMode)
    {
        gCPUWallMicros = std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - gCPUWallStart).count();
        gCPUWallsReady = true;
    }
}

extern "C" void I_WGPUMarkPostWallRect(int x, int y, int width, int height)
{
    if (!gPostWallPhase || !gWallSnapshotReady || width <= 0 || height <= 0)
        return;
    const int x0 = std::max(0, x), y0 = std::max(0, y);
    const int x1 = std::min<int>(gSourceWidth, x + width);
    const int y1 = std::min<int>(gSourceHeight, y + height);
    for (int py = y0; py < y1; ++py)
        for (int px = x0; px < x1; ++px)
        {
            const uint32_t pixel = py * gSourceWidth + px;
            gPostWallMask[pixel >> 5u] |= 1u << (pixel & 31u);
        }
}

extern "C" void I_WGPUMarkPostWallColumn(int x, int yl, int yh)
{
    I_WGPUMarkPostWallRect(x, yl, 1, yh - yl + 1);
}

static void ReplayWallColumns(uint8_t* framebuffer)
{
    for (const WallColumn& column : gWallColumns)
    {
        int32_t frac = static_cast<int32_t>(column.frac);
        const int32_t step = static_cast<int32_t>(column.fracstep);
        for (uint32_t y = column.yl; y <= column.yh; ++y)
        {
            const uint32_t textureIndex = (frac >> 16) & 127;
            const uint32_t sourceWord =
                gWallSources[column.sourceIndex * kWallSourceWords +
                             (textureIndex >> 2)];
            const uint32_t sourceIndex =
                (sourceWord >> ((textureIndex & 3) * 8)) & 0xff;
            const uint32_t mapWord =
                gWallColormaps[column.colormapIndex * kWallColormapWords +
                               (sourceIndex >> 2)];
            framebuffer[y * gSourceWidth + column.x] =
                (mapWord >> ((sourceIndex & 3) * 8)) & 0xff;
            frac += step;
        }
    }
}

extern "C" void I_WGPURequireCPUWalls(uint8_t* framebuffer)
{
    if (gWallSnapshotReady && !gCPUWallsReady && framebuffer)
    {
        ReplayWallColumns(framebuffer);
        gCPUWallsReady = true;
    }
}

extern "C" void I_WGPUCPUWallLoopStart(void)
{
    if (gValidationMode && gCollectWallColumns)
        gCPUWallLoopStart = std::chrono::steady_clock::now();
}

extern "C" void I_WGPUCPUWallLoopEnd(void)
{
    if (gValidationMode && gCollectWallColumns)
        gCPUWallLoopNanos +=
            std::chrono::duration_cast<std::chrono::nanoseconds>(
                std::chrono::steady_clock::now() - gCPUWallLoopStart).count();
}

static std::shared_ptr<ValidationSample> CreateValidationSample(
    const uint8_t* framebuffer)
{
    auto sample = std::make_shared<ValidationSample>();
    sample->frame = ++gValidationFrame;
    sample->cpuWallMicros = gCPUWallMicros;
    sample->cpuWallLoopNanos = gCPUWallLoopNanos;
    sample->expected.resize(gSourceWidth * gSourceHeight);
    const auto copyStart = std::chrono::steady_clock::now();
    for (uint32_t i = 0; i < sample->expected.size(); ++i)
        sample->expected[i] = framebuffer[i];
    sample->referenceCopyMicros =
        std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - copyStart).count();

    wgpu::BufferDescriptor descriptor{};
    descriptor.size = sample->expected.size() * sizeof(uint32_t);
    descriptor.usage = wgpu::BufferUsage::CopyDst | wgpu::BufferUsage::MapRead;
    descriptor.mappedAtCreation = false;
    sample->readback = wgfx::device.createBuffer(descriptor);
    gValidationSamples.push_back(sample);
    return sample;
}

static void ReportValidation(const std::shared_ptr<ValidationSample>& sample,
                             wgpu::BufferMapAsyncStatus status,
                             uint64_t queueMicros)
{
    const auto mappedAt = std::chrono::steady_clock::now();
    if (sample->cancelled || gShuttingDown)
    {
        if (status == wgpu::BufferMapAsyncStatus::Success)
            sample->readback.unmap();
        sample->finished = true;
        return;
    }
    if (status != wgpu::BufferMapAsyncStatus::Success)
    {
        std::cerr << "WebGPU validation readback mapping failed\n";
        ++gValidationReadbackFailures;
        sample->finished = true;
        return;
    }

    const uint32_t* actual = static_cast<const uint32_t*>(
        sample->readback.getConstMappedRange(
            0, sample->expected.size() * sizeof(uint32_t)));
    uint32_t mismatches = 0;
    std::array<uint32_t, 8> firstMismatches{};
    for (uint32_t pixel = 0; pixel < sample->expected.size(); ++pixel)
        if ((actual[pixel] & 0xffu) != (sample->expected[pixel] & 0xffu))
        {
            if (mismatches < firstMismatches.size())
                firstMismatches[mismatches] = pixel;
            ++mismatches;
        }

    ++gValidationCompletedFrames;
    gValidationComparedPixels += sample->expected.size();
    gValidationMismatchPixels += mismatches;
    if (mismatches != 0)
        ++gValidationFailedFrames;

    const uint64_t mapMicros =
        std::chrono::duration_cast<std::chrono::microseconds>(mappedAt - sample->mapStart)
            .count();
    const uint64_t cpuRasterMicros = sample->cpuWallLoopNanos / 1000u;
    const uint64_t gpuEndToEndMicros =
        (sample->cpuWallMicros > cpuRasterMicros
             ? sample->cpuWallMicros - cpuRasterMicros
             : 0u) + sample->uploadMicros + sample->submitMicros + queueMicros;
    std::cout << "WebGPU validation frame " << sample->frame << ": "
              << mismatches << " / " << sample->expected.size()
              << " indexed pixels differ; CPU wall path " << sample->cpuWallMicros
              << " us (raster loops " << cpuRasterMicros
              << " us), estimated GPU wall path " << gpuEndToEndMicros
              << " us, reference copy " << sample->referenceCopyMicros
              << " us, uploads " << sample->uploadMicros << " us, submit "
              << sample->submitMicros << " us, queue completion/sync " << queueMicros
              << " us, map/readback " << mapMicros << " us\n";
    for (uint32_t i = 0; i < std::min<uint32_t>(mismatches, firstMismatches.size()); ++i)
    {
        const uint32_t pixel = firstMismatches[i];
        std::cout << "  mismatch (" << pixel % gSourceWidth << ","
                  << pixel / gSourceWidth << "): CPU "
                  << (sample->expected[pixel] & 0xffu) << ", GPU "
                  << (actual[pixel] & 0xffu) << "\n";
    }
    sample->readback.unmap();
    sample->finished = true;
}

extern "C" void I_WGPUReportValidationSummary(void)
{
    if (!gValidationMode)
        return;

#ifdef WEBGPU_BACKEND_WGPU
    // Native validation may block at demo end so the aggregate cannot omit
    // callbacks which are still in flight.
    for (int poll = 0; poll < 8; ++poll)
    {
        const bool pending = std::any_of(
            gValidationSamples.begin(), gValidationSamples.end(),
            [](const std::shared_ptr<ValidationSample>& sample)
            { return !sample->finished; });
        if (!pending)
            break;
        wgfx::device.poll(true);
    }
#endif

    const uint64_t pending = std::count_if(
        gValidationSamples.begin(), gValidationSamples.end(),
        [](const std::shared_ptr<ValidationSample>& sample)
        { return !sample->finished; });
    const bool passed = gValidationCompletedFrames == gValidationFrame &&
        gValidationFailedFrames == 0 && gValidationReadbackFailures == 0 &&
        pending == 0;
    std::cout << "WebGPU validation summary: "
              << (passed ? "PASS" : "FAIL") << "; submitted "
              << gValidationFrame << " frames, completed "
              << gValidationCompletedFrames << ", failed "
              << gValidationFailedFrames << ", compared "
              << gValidationComparedPixels << " pixels, mismatched "
              << gValidationMismatchPixels << ", readback failures "
              << gValidationReadbackFailures << ", pending " << pending
              << "\n";
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

    gValidationSamples.erase(
        std::remove_if(gValidationSamples.begin(), gValidationSamples.end(),
            [](const std::shared_ptr<ValidationSample>& sample)
            { return sample->finished; }),
        gValidationSamples.end());
    const bool validateThisFrame = gValidationMode && gWallSnapshotReady &&
                                   !gWallColumns.empty();
    std::shared_ptr<ValidationSample> validationSample;
    if (validateThisFrame)
        validationSample = CreateValidationSample(framebuffer);

    const auto uploadStart = std::chrono::steady_clock::now();
    const uint32_t pixelCount = gSourceWidth * gSourceHeight;
    for (uint32_t i = 0; i < pixelCount; ++i)
        gFrameIndices[i] = framebuffer[i];
    gCompute->updateStorageBuffer(gIndices, gFrameIndices.data(),
                                  gFrameIndices.size() * sizeof(uint32_t));

    if (gWallSnapshotReady && !gWallColumns.empty())
    {
        if (!gReportedWallRasterizer)
        {
            const size_t uploadBytes = gWallColumns.size() * sizeof(WallColumn) +
                gWallSources.size() * sizeof(uint32_t) +
                gWallColormaps.size() * sizeof(uint32_t) +
                sizeof(gWallOffsets) + sizeof(gPostWallMask);
            std::cout << "WebGPU wall rasterizer active: " << gWallColumns.size()
                      << " columns, " << gWallSourceIndices.size()
                      << " texture columns, " << gWallColormapIndices.size()
                      << " colormaps, " << (uploadBytes + 1023) / 1024
                      << " KiB of wall-command data, " << gWallBucketMicros
                      << " us bucket flatten in first rendered frame\n";
            gReportedWallRasterizer = true;
        }
        gWallCompute->updateStorageBuffer(
            gWallSnapshotBuffer, gWallSnapshot.data(),
            gWallSnapshot.size() * sizeof(uint32_t));
        gWallCompute->updateStorageBuffer(
            gWallCommandsBuffer, gWallColumns.data(),
            gWallColumns.size() * sizeof(WallColumn));
        if (gWallSources.size() > gUploadedSourceWords)
        {
            gWallCompute->updateStorageBuffer(
                gWallSourcesBuffer, gWallSources.data() + gUploadedSourceWords,
                (gWallSources.size() - gUploadedSourceWords) * sizeof(uint32_t),
                gUploadedSourceWords * sizeof(uint32_t));
            gUploadedSourceWords = gWallSources.size();
        }
        if (gWallColormaps.size() > gUploadedColormapWords)
        {
            gWallCompute->updateStorageBuffer(
                gWallColormapsBuffer,
                gWallColormaps.data() + gUploadedColormapWords,
                (gWallColormaps.size() - gUploadedColormapWords) * sizeof(uint32_t),
                gUploadedColormapWords * sizeof(uint32_t));
            gUploadedColormapWords = gWallColormaps.size();
        }
        gWallCompute->updateStorageBuffer(
            gWallOffsetsBuffer, gWallOffsets.data(), sizeof(gWallOffsets));
        gWallCompute->updateStorageBuffer(
            gPostWallMaskBuffer, gPostWallMask.data(), sizeof(gPostWallMask));
    }
    if (validationSample)
        validationSample->uploadMicros =
            std::chrono::duration_cast<std::chrono::microseconds>(
            std::chrono::steady_clock::now() - uploadStart).count();

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
        if (validationSample)
        {
            wgfx::encoder.copyBufferToBuffer(
                gIndices->buffer, 0, validationSample->readback, 0,
                pixelCount * sizeof(uint32_t));

            // Submit wall work separately so queue completion measures the
            // upload + wall dispatch + readback-copy path, not presentation.
            wgpu::CommandBufferDescriptor commandDescriptor{};
            commandDescriptor.label = "Validated wall rasterization";
            wgpu::CommandBuffer wallCommands =
                wgfx::encoder.finish(commandDescriptor);
            wgfx::encoder.release();
            wgfx::encoder = nullptr;
            validationSample->gpuStart = std::chrono::steady_clock::now();
            const auto wallSubmitStart = validationSample->gpuStart;
            wgfx::queue.submit(1, &wallCommands);
            validationSample->submitMicros =
                std::chrono::duration_cast<std::chrono::microseconds>(
                    std::chrono::steady_clock::now() - wallSubmitStart).count();
            wallCommands.release();

            std::weak_ptr<ValidationSample> weakSample = validationSample;
            validationSample->workDoneCallback = wgfx::queue.onSubmittedWorkDone(
                [weakSample](wgpu::QueueWorkDoneStatus status)
                {
                    const auto sample = weakSample.lock();
                    if (!sample)
                        return;
                    const auto completedAt = std::chrono::steady_clock::now();
                    const uint64_t queueMicros =
                        std::chrono::duration_cast<std::chrono::microseconds>(
                            completedAt - sample->gpuStart).count();
                    if (status != wgpu::QueueWorkDoneStatus::Success)
                    {
                        if (!sample->cancelled)
                        {
                            std::cerr << "WebGPU validation queue completion failed\n";
                            ++gValidationReadbackFailures;
                        }
                        sample->finished = true;
                        return;
                    }
                    if (sample->cancelled || gShuttingDown)
                    {
                        sample->finished = true;
                        return;
                    }
                    sample->mapStart = completedAt;
                    std::weak_ptr<ValidationSample> mapWeakSample = sample;
                    sample->mapCallback = sample->readback.mapAsync(
                        wgpu::MapMode::Read, 0,
                        sample->expected.size() * sizeof(uint32_t),
                        [mapWeakSample, queueMicros](
                            wgpu::BufferMapAsyncStatus mapStatus)
                        {
                            if (const auto mappedSample = mapWeakSample.lock())
                                ReportValidation(mappedSample, mapStatus,
                                                 queueMicros);
                        });
                });

            wgpu::CommandEncoderDescriptor encoderDescriptor{};
            encoderDescriptor.label = "Palette and presentation";
            wgfx::encoder = wgfx::device.createCommandEncoder(encoderDescriptor);
        }
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
    gWallSnapshotReady = false;
    gPostWallPhase = false;
}

extern "C" void I_WGPUShutdown(void)
{
    gShuttingDown = true;
    for (const auto& sample : gValidationSamples)
        sample->cancelled = true;
    gReady = false;
    if (gPresentBindGroup) gPresentBindGroup.release();
    if (gPresentBindGroupLayout) gPresentBindGroupLayout.release();
    if (gPresentPipeline) gPresentPipeline.release();
    if (gFrameSampler) gFrameSampler.release();
    if (gFrameView) gFrameView.release();
    if (gFrameTexture) gFrameTexture.release();
}
