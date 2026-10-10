#define WEBGPU_CPP_IMPLEMENTATION
#include <webgpu/webgpu.hpp>

#include <wgfx.h>
#include <sdl2webgpu.h>

#include "i_video_wgpu.h"

#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>

namespace
{
constexpr uint32_t kPaletteEntries = 256;

uint32_t gSourceWidth = 0;
uint32_t gSourceHeight = 0;
uint32_t gPackedIndexCount = 0;
bool gReady = false;

wgfx::Compute* gCompute = nullptr;
wgfx::Uniform* gIndices = nullptr;
wgfx::Uniform* gPalette = nullptr;
wgfx::Uniform* gOutput = nullptr;

wgpu::Texture gFrameTexture = nullptr;
wgpu::TextureView gFrameView = nullptr;
wgpu::Sampler gFrameSampler = nullptr;
wgpu::RenderPipeline gPresentPipeline = nullptr;
wgpu::BindGroupLayout gPresentBindGroupLayout = nullptr;
wgpu::BindGroup gPresentBindGroup = nullptr;

wgfx::ColorTexture gSurfaceColor;
std::vector<uint32_t> gPackedIndices;
std::array<uint32_t, kPaletteEntries> gPackedPalette{};

constexpr const char* kComputeShader = R"WGSL(
@group(0) @binding(0) var<storage, read> indices: array<u32>;
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
    let packed = indices[pixel >> 2u];
    let shift = (pixel & 3u) * 8u;
    let paletteIndex = (packed >> shift) & 0xffu;
    outputPixels[pixel] = palette[paletteIndex];
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
    // Doom's PLAYPAL entries are sRGB-encoded bytes. Sampling through an
    // sRGB view decodes them to linear before the presentation pass writes
    // to the swapchain, avoiding a second gamma encoding by the compositor.
    descriptor.format = wgpu::TextureFormat::RGBA8UnormSrgb;
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
    gPackedIndexCount = (sourceWidth * sourceHeight + 3) / 4;
    gPackedIndices.resize(gPackedIndexCount);

    wgpu::Surface surface = wgfx::getSurface(window);
    if (!surface)
        return false;
    wgfx::init(surface);
    if (!wgfx::device || !wgfx::queue)
        return false;

    gCompute = wgfx::loadCompute(kComputeShader);
    gIndices = wgfx::createStorage(0, gPackedIndices.size() * sizeof(uint32_t),
                                   gPackedIndices.data(), true);
    gPalette = wgfx::createStorage(1, gPackedPalette.size() * sizeof(uint32_t),
                                   gPackedPalette.data(), true);
    gOutput = wgfx::createStorage(2, sourceWidth * sourceHeight * sizeof(uint32_t),
                                  nullptr, false);
    gCompute->setStorage(gIndices);
    gCompute->setStorage(gPalette);
    gCompute->setStorage(gOutput);
    gCompute->init();

    gReady = CreateFrameTexture() && CreatePresentPipeline();
    return gReady;
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
    for (uint32_t i = 0; i < gPackedIndexCount; ++i)
    {
        const uint32_t base = i * 4;
        uint32_t packed = 0;
        for (uint32_t lane = 0; lane < 4 && base + lane < pixelCount; ++lane)
            packed |= uint32_t(framebuffer[base + lane]) << (lane * 8u);
        gPackedIndices[i] = packed;
    }
    gCompute->updateStorageBuffer(gIndices, gPackedIndices.data(),
                                  gPackedIndices.size() * sizeof(uint32_t));

    int windowWidth = 0;
    int windowHeight = 0;
    SDL_GetWindowSize(wgfx::window, &windowWidth, &windowHeight);
    if (windowWidth != wgfx::width || windowHeight != wgfx::height)
        wgfx::initSurface();

    wgfx::touch(&gSurfaceColor);
    if (!gSurfaceColor.colorView || !wgfx::encoder)
        return;

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
