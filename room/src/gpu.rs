//! wgpu-based GPU rendering state for the Doom frame buffer.
//!
//! [`GpuState`] encapsulates all wgpu resources needed to upload and display
//! the 640 × 400 pixel Doom frame buffer produced by the engine's
//! `I_FinishUpdate` function.
//!
//! ## Rendering pipeline
//!
//! Each frame, the workflow is:
//!
//! 1. **Upload** – `DG_ScreenBuffer` (raw BGRA8 pixels) is written into a
//!    wgpu [`Texture`] via [`Queue::write_texture`].
//! 2. **Render** – A single fullscreen-quad draw call samples the texture and
//!    outputs to the swapchain surface.
//!
//! The WGSL shaders are embedded at compile time as string literals so the
//! binary is self-contained (no external shader files required).
//!
//! ## Pixel format
//!
//! The Doom engine's `I_FinishUpdate` function (in `i_video.c`) writes pixels
//! in BGRA byte order:
//!
//! | Byte offset | Channel |
//! |-------------|---------|
//! | 0 | Blue |
//! | 1 | Green |
//! | 2 | Red |
//! | 3 | Alpha (unused, always 0) |
//!
//! The intermediate [`Texture`] uses [`TextureFormat::Bgra8Unorm`] to match
//! this layout exactly, avoiding any CPU-side byte-swapping.
//!
//! [`Texture`]: wgpu::Texture
//! [`TextureFormat::Bgra8Unorm`]: wgpu::TextureFormat::Bgra8Unorm
//! [`Queue::write_texture`]: wgpu::Queue::write_texture

use std::sync::Arc;

use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::doom::doomgeneric::{DOOMGENERIC_RESX, DOOMGENERIC_RESY};

// ---------------------------------------------------------------------------
// WGSL shaders (embedded)
// ---------------------------------------------------------------------------

/// WGSL source for the fullscreen-quad vertex and fragment shaders.
///
/// The vertex shader generates a full-screen quad from the vertex index
/// without needing a vertex buffer.  The fragment shader samples the Doom
/// frame texture and writes it to the render target.
const SHADER_SOURCE: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

/// Generate a fullscreen quad (two triangles) from the vertex index.
///
/// Vertices are laid out as a counter-clockwise pair of triangles covering
/// clip space from (-1,-1) to (1,1).
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // NDC positions for a fullscreen quad (two triangles, 6 vertices).
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    // UV coordinates: Doom's origin is top-left; wgpu NDC bottom-left.
    // Flip the V axis so the image is not upside-down.
    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(positions[vi], 0.0, 1.0);
    out.tex_coords = tex_coords[vi];
    return out;
}

@group(0) @binding(0) var doom_texture: texture_2d<f32>;
@group(0) @binding(1) var doom_sampler: sampler;

/// Sample the Doom frame texture and write the result to the render target with full opacity.
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(doom_texture, doom_sampler, in.tex_coords);
    return vec4<f32>(color.rgb, 1.0);
}
"#;

/// World-space mesh. `uv` tiles inside the atlas rectangle carried on the vertex.
const WORLD_SHADER: &str = r#"
struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) atlas: vec4<f32>,
    @location(3) light: f32,
    @location(4) tint: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) atlas: vec4<f32>,
    @location(2) light: f32,
    @location(3) tint: vec3<f32>,
};

@vertex
fn vs_world(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    out.atlas = in.atlas;
    out.light = in.light;
    out.tint = in.tint;
    return out;
}

@fragment
fn fs_world(in: VertexOutput) -> @location(0) vec4<f32> {
    let tile = fract(in.uv);
    let atlas_uv = vec2<f32>(
        mix(in.atlas.x, in.atlas.z, tile.x),
        mix(in.atlas.y, in.atlas.w, tile.y),
    );
    // Level 0 avoids the huge derivative fract() creates at a tile edge,
    // which otherwise shimmers along walls and floors.
    let color = textureSampleLevel(atlas_tex, atlas_samp, atlas_uv, 0.0);
    if (color.a < 0.5) {
        discard;
    }
    return vec4<f32>(color.rgb * in.tint * in.light, 1.0);
}
"#;

const SVO_HEAD: &str = r#"
struct SvoCamera {
    inv_view: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // xyz is the cube's minimum corner. w is the cube's world length.
    origin: vec4<f32>,
    // The 3D viewport in framebuffer pixels: x, y, width, height.
    view: vec4<f32>,
    // x is the node texture width. y is the node count.
    tex: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: SvoCamera;
"#;

/// Browser path: one node per texel. The texture cannot be written, so the
/// hit counters stay at zero and the CPU opens the tree by distance.
#[cfg(target_arch = "wasm32")]
const SVO_NODES: &str = r#"
@group(0) @binding(1) var nodes: texture_2d<f32>;

fn load_node(i: u32) -> u32 {
    let width = u32(u.tex.x);
    let x = i % width;
    let y = i / width;
    let t = textureLoad(nodes, vec2<i32>(i32(x), i32(y)), 0);
    let r = u32(t.r * 255.0 + 0.5);
    let g = u32(t.g * 255.0 + 0.5);
    let b = u32(t.b * 255.0 + 0.5);
    let a = u32(t.a * 255.0 + 0.5);
    return r | (g << 8u) | (b << 16u) | (a << 24u);
}

fn touch_node(i: u32) {}
"#;

/// Native path: the same words in a storage buffer. Looking at a node adds
/// one to its low 4 bits, which is the tracer's hit counter.
#[cfg(not(target_arch = "wasm32"))]
const SVO_NODES: &str = r#"
@group(0) @binding(1) var<storage, read_write> node_words: array<u32>;

fn load_node(i: u32) -> u32 {
    return node_words[i];
}

fn touch_node(i: u32) {
    let value = node_words[i];
    if ((value & 15u) < 15u) {
        node_words[i] = value + 1u;
    }
}
"#;

const SVO_BODY: &str = r#"
const VOXEL_OFFSET: u32 = 134217728u;

struct Ray {
    pos: vec3<f32>,
    dir: vec3<f32>,
};

fn node_ptr(i: u32) -> u32 {
    return load_node(i) >> 4u;
}

@vertex
fn vs_svo(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var x = -1.0;
    var y = -1.0;
    if (index == 1u) { x = 1.0; }
    if (index == 2u) { y = 1.0; }
    if (index == 3u) { x = 1.0; y = 1.0; }
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn unproject(ndc: vec3<f32>) -> vec3<f32> {
    let p = u.inv_view * vec4<f32>(ndc, 1.0);
    return p.xyz / p.w;
}

fn to_oct(p: vec3<f32>) -> vec3<f32> {
    return (p - u.origin.xyz) / u.origin.w * 2.0 - vec3<f32>(1.0);
}

fn to_world(p: vec3<f32>) -> vec3<f32> {
    return u.origin.xyz + (p + vec3<f32>(1.0)) * 0.5 * u.origin.w;
}

fn ray_box(ro: vec3<f32>, rd: vec3<f32>, vmin: vec3<f32>, vmax: vec3<f32>) -> f32 {
    let v1 = (vmin.x - ro.x) / rd.x;
    let v2 = (vmax.x - ro.x) / rd.x;
    let v3 = (vmin.y - ro.y) / rd.y;
    let v4 = (vmax.y - ro.y) / rd.y;
    let v5 = (vmin.z - ro.z) / rd.z;
    let v6 = (vmax.z - ro.z) / rd.z;
    let t0 = max(max(min(v1, v2), min(v3, v4)), min(v5, v6));
    let t1 = min(min(max(v1, v2), max(v3, v4)), max(v5, v6));
    if (t1 < 0.0 || t0 > t1) {
        return 0.0;
    }
    return t0;
}

fn in_bounds(v: vec3<f32>) -> bool {
    let s = step(vec3<f32>(-1.0), v) - step(vec3<f32>(1.0), v);
    return (s.x * s.y * s.z) > 0.5;
}

struct Leaf {
    value: u32,
    pos: vec3<f32>,
    depth: u32,
};

fn find_leaf(pos: vec3<f32>) -> Leaf {
    var node_index = 0u;
    var node_pos = vec3<f32>(0.0);
    var depth = 0u;
    loop {
        depth = depth + 1u;
        let p = vec3<u32>(
            select(0u, 1u, pos.x >= node_pos.x),
            select(0u, 1u, pos.y >= node_pos.y),
            select(0u, 1u, pos.z >= node_pos.z),
        );
        let child = p.x * 4u + p.y * 2u + p.z;
        node_pos = node_pos + (vec3<f32>(p) * 2.0 - vec3<f32>(1.0)) / f32(1u << depth);
        let at = node_index + child;
        touch_node(at);
        let ptr = node_ptr(at);
        if (ptr >= VOXEL_OFFSET || depth > 24u) {
            return Leaf(ptr, node_pos, depth);
        }
        node_index = ptr;
    }
    return Leaf(VOXEL_OFFSET, node_pos, depth);
}

struct Hit {
    hit: bool,
    index: u32,
    pos: vec3<f32>,
    normal: vec3<f32>,
};

fn trace(ro: vec3<f32>, rd: vec3<f32>) -> Hit {
    var pos = ro;
    let dir = rd + vec3<f32>(
        select(0.0, 1.0, rd.x == 0.0),
        select(0.0, 1.0, rd.y == 0.0),
        select(0.0, 1.0, rd.z == 0.0),
    ) * 0.000001;
    if (!in_bounds(ro)) {
        let dist = ray_box(ro, dir, vec3<f32>(-1.0), vec3<f32>(1.0));
        if (dist == 0.0) {
            return Hit(false, 0u, ro, vec3<f32>(0.0));
        }
        pos = ro + dir * dist;
    }
    let sign_dir = sign(dir);
    var voxel_pos = pos;
    var steps = 0u;
    var normal = vec3<f32>(0.0, 1.0, 0.0);
    loop {
        let leaf = find_leaf(voxel_pos);
        let colour = leaf.value - VOXEL_OFFSET;
        if (colour > 0u) {
            return Hit(true, leaf.value, voxel_pos, normal);
        }
        let size = 2.0 / f32(1u << leaf.depth);
        let t_max = (leaf.pos - pos + sign_dir * size * 0.5) / dir;
        let mask = vec3<f32>(t_max.xyz <= min(t_max.yzx, t_max.zxy));
        normal = mask * -sign_dir;
        let t = min(min(t_max.x, t_max.y), t_max.z);
        voxel_pos = pos + dir * t - normal * 0.000002;
        if (!in_bounds(voxel_pos)) {
            return Hit(false, 0u, voxel_pos, normal);
        }
        steps = steps + 1u;
        if (steps > 160u) {
            return Hit(false, 0u, voxel_pos, normal);
        }
    }
    return Hit(false, 0u, voxel_pos, normal);
}

struct SvoOut {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_svo(@builtin(position) frag_pos: vec4<f32>) -> SvoOut {
    let nx = (frag_pos.x - u.view.x) / u.view.z * 2.0 - 1.0;
    let ny = 1.0 - (frag_pos.y - u.view.y) / u.view.w * 2.0;
    let near_w = unproject(vec3<f32>(nx, ny, 0.0));
    let far_w = unproject(vec3<f32>(nx, ny, 1.0));
    let dir = normalize(far_w - near_w);
    let hit = trace(to_oct(near_w), dir);
    if (!hit.hit) {
        discard;
    }
    let value = hit.index - VOXEL_OFFSET;
    let rgb = vec3<f32>(
        f32((value >> 16u) & 255u),
        f32((value >> 8u) & 255u),
        f32(value & 255u),
    ) / 255.0;
    let sun = normalize(vec3<f32>(0.35, 0.85, 0.25));
    let shade = 0.62 + 0.38 * max(dot(hit.normal, sun), 0.0);
    let world = to_world(hit.pos);
    let clip = u.view_proj * vec4<f32>(world, 1.0);
    var depth = 1.0;
    if (clip.w > 0.0001) {
        depth = clamp(clip.z / clip.w, 0.0, 1.0);
    }
    return SvoOut(vec4<f32>(rgb * shade, 1.0), depth);
}
"#;

fn svo_shader() -> String {
    format!("{SVO_HEAD}{SVO_NODES}{SVO_BODY}")
}

/// Queues leaves with a hit counter of 4 or more, and internal nodes whose
/// counter is still 0. Word 0 of each queue is the count.
#[cfg(not(target_arch = "wasm32"))]
const SVO_ADAPT: &str = r#"
const VOXEL_OFFSET: u32 = 134217728u;

struct AdaptUniform {
    node_length: u32,
    max_depth: u32,
    pad0: u32,
    pad1: u32,
};

struct Queue {
    counter: atomic<u32>,
    data: array<u32>,
};

@group(0) @binding(0) var<uniform> u: AdaptUniform;
@group(0) @binding(1) var<storage, read> node_words: array<u32>;
@group(0) @binding(2) var<storage, read_write> sub: Queue;
@group(0) @binding(3) var<storage, read_write> unsub: Queue;

@compute @workgroup_size(64)
fn cs_adapt(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    if (id >= u.node_length) {
        return;
    }
    let node = node_words[id];
    if (node == 0u) {
        return;
    }
    let counter = node & 15u;
    let ptr = node >> 4u;
    if (counter == 0u && ptr < VOXEL_OFFSET) {
        let index = atomicAdd(&unsub.counter, 1u);
        if (index < 1023999u) {
            unsub.data[index] = id;
        }
    } else if (counter >= 4u && ptr > VOXEL_OFFSET) {
        let index = atomicAdd(&sub.counter, 1u);
        if (index < 1023999u) {
            sub.data[index] = id;
        }
    }
}
"#;

/// Software frame with the 3D viewport punched out, so the status bar and
/// border stay while the triangle scene shows through.
const HUD_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
};

struct HudRect {
    rect: vec4<f32>,
};

@group(0) @binding(0) var doom_texture: texture_2d<f32>;
@group(0) @binding(1) var doom_sampler: sampler;
@group(1) @binding(0) var<uniform> hud: HudRect;

@vertex
fn vs_hud(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );
    var tex_coords = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(1.0, 0.0),
    );
    var out: VertexOutput;
    out.clip_position = vec4<f32>(positions[vi], 0.0, 1.0);
    out.tex_coords = tex_coords[vi];
    return out;
}

@fragment
fn fs_hud(in: VertexOutput) -> @location(0) vec4<f32> {
    let r = hud.rect;
    if (in.tex_coords.x >= r.x && in.tex_coords.x <= r.x + r.z
        && in.tex_coords.y >= r.y && in.tex_coords.y <= r.y + r.w) {
        discard;
    }
    let color = textureSample(doom_texture, doom_sampler, in.tex_coords);
    return vec4<f32>(color.rgb, 1.0);
}
"#;

struct WorldGpu {
    atlas: wgpu::Texture,
    atlas_size: u32,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    weapon_pipeline: wgpu::RenderPipeline,
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    uniform: wgpu::Buffer,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    vertex_cap: u64,
    index_cap: u64,
    svo_pipeline: wgpu::RenderPipeline,
    svo_layout: wgpu::BindGroupLayout,
    svo_uniform: wgpu::Buffer,
    svo_tex: wgpu::Texture,
    svo_view: wgpu::TextureView,
    svo_bind: wgpu::BindGroup,
    svo_width: u32,
    svo_height: u32,
    /// Node words, when the device can increment hit counters from the trace.
    node_buf: Option<wgpu::Buffer>,
    adapt: Option<SvoAdapt>,
}

struct SvoAdapt {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    sub: wgpu::Buffer,
    unsub: wgpu::Buffer,
    sub_read: wgpu::Buffer,
    unsub_read: wgpu::Buffer,
}

// ---------------------------------------------------------------------------
// GpuState
// ---------------------------------------------------------------------------

/// All wgpu resources required to render the Doom frame buffer to a window.
pub struct GpuState {
    /// The wgpu surface tied to the OS window.
    surface: wgpu::Surface<'static>,
    /// The logical GPU device.
    device: wgpu::Device,
    /// The command queue used to submit GPU work.
    queue: wgpu::Queue,
    /// Current swapchain configuration (updated on resize).
    config: wgpu::SurfaceConfiguration,
    /// Intermediate texture that receives the Doom screen pixels each frame.
    doom_texture: wgpu::Texture,
    /// Bind group binding `doom_texture` and its sampler to the shader.
    bind_group: wgpu::BindGroup,
    /// The render pipeline executing the fullscreen-quad blit.
    render_pipeline: wgpu::RenderPipeline,
    /// Software frame with the 3D view discarded, drawn on top of the mesh.
    hud_pipeline: wgpu::RenderPipeline,
    hud_bind_group: wgpu::BindGroup,
    hud_rect_group: wgpu::BindGroup,
    hud_uniform: wgpu::Buffer,
    /// Triangle scene resources, created on the first level frame.
    world: Option<WorldGpu>,
    /// CPU octree the hit counters split and merge. Native only uses it.
    octree: Option<crate::svo::Built>,
    octree_dirty: bool,
}

/// Constructors and frame-time operations for [`GpuState`].
impl GpuState {
    /// Create a new [`GpuState`] for the given window.
    ///
    /// This performs the following wgpu setup:
    ///
    /// 1. Creates a [`wgpu::Instance`].
    /// 2. Creates a [`wgpu::Surface`] from the window.
    /// 3. Requests a [`wgpu::Adapter`] and [`wgpu::Device`] / [`wgpu::Queue`].
    /// 4. Configures the surface.
    /// 5. Creates the intermediate Doom-frame [`wgpu::Texture`].
    /// 6. Creates the render pipeline with the embedded WGSL shaders.
    ///
    /// # Errors
    ///
    /// Returns an error string if any wgpu initialisation step fails.
    pub fn new(window: Arc<Window>) -> Result<Self, String> {
        pollster::block_on(Self::new_async(window))
    }

    /// Asynchronous wgpu initialization, required for WebAssembly targets where blocking is forbidden.
    pub async fn new_async(window: Arc<Window>) -> Result<Self, String> {
        let mut size = window.inner_size();
        // winit's web backend reports 0×0 until its resize observer runs.
        // Configuring a 1×1 swapchain from that leaves a black canvas.
        #[cfg(target_arch = "wasm32")]
        if size.width < 2 || size.height < 2 {
            size = web_canvas_size();
        }
        // Use the default backends (Vulkan, Metal, DX12, OpenGL, WebGPU, WebGL2).
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        // `Surface<'static>` requires the window to outlive the surface.
        // `Arc<Window>` is `'static`, so this is sound.
        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("create_surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("no suitable GPU adapter found: {e}"))?;

        let adapter_info = adapter.get_info();
        log::info!(
            "GPU Adapter: backend={:?}, name={}, device_type={:?}",
            adapter_info.backend,
            adapter_info.name,
            adapter_info.device_type
        );

        #[cfg(target_arch = "wasm32")]
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        #[cfg(not(target_arch = "wasm32"))]
        let limits = wgpu::Limits::default();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("doom_device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                memory_hints: wgpu::MemoryHints::default(),
                // Disable experimental features and API tracing for production use.
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("request_device: {e}"))?;
        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer a non-sRGB format so colours match the palette exactly.
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        // Created with RGBA8Unorm to match the standard RGBA pixel format written by
        // the engine's I_FinishUpdate function. Supported universally across WebGPU and WebGL2.
        let doom_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("doom_frame_texture"),
            size: wgpu::Extent3d {
                width: DOOMGENERIC_RESX as u32,
                height: DOOMGENERIC_RESY as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let doom_texture_view = doom_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let doom_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("doom_sampler"),
            // Use nearest-neighbour filtering to preserve pixel sharpness.
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("doom_bind_group_layout"),
            entries: &[
                // Binding 0: the Doom frame texture.
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Binding 1: the texture sampler.
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("doom_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&doom_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&doom_sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("doom_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("doom_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("doom_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // No vertex buffer; positions come from vertex index.
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let hud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("doom_hud_shader"),
            source: wgpu::ShaderSource::Wgsl(HUD_SHADER.into()),
        });
        let hud_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_hud_uniform"),
            size: 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("doom_hud_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let hud_rect_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("doom_hud_rect_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let hud_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("doom_hud_bind_group"),
            layout: &hud_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&doom_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&doom_sampler),
                },
            ],
        });
        let hud_rect_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("doom_hud_rect_group"),
            layout: &hud_rect_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: hud_uniform.as_entire_binding(),
            }],
        });
        let hud_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("doom_hud_pipeline_layout"),
            bind_group_layouts: &[Some(&hud_layout), Some(&hud_rect_layout)],
            immediate_size: 0,
        });
        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("doom_hud_pipeline"),
            layout: Some(&hud_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &hud_shader,
                entry_point: Some("vs_hud"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_shader,
                entry_point: Some("fs_hud"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Ok(Self {
            surface,
            device,
            queue,
            config,
            doom_texture,
            bind_group,
            render_pipeline,
            hud_pipeline,
            hud_bind_group,
            hud_rect_group,
            hud_uniform,
            world: None,
            octree: None,
            octree_dirty: false,
        })
    }

    /// Reconfigure the surface after a window resize.
    ///
    /// This must be called whenever the window's inner size changes, before
    /// the next call to [`render`].
    ///
    /// [`render`]: Self::render
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    /// Upload `pixels` to the Doom frame texture and blit it to the surface.
    ///
    /// `pixels` must be exactly `DOOMGENERIC_RESX * DOOMGENERIC_RESY * 4`
    /// bytes in BGRA8 format.
    ///
    /// # Errors
    ///
    /// Returns an error string if the swapchain texture cannot be acquired
    /// (e.g. the surface is lost or the window is minimised).
    pub fn render(&self, pixels: &[u8]) -> Result<(), String> {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.doom_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((DOOMGENERIC_RESX * 4) as u32),
                rows_per_image: Some(DOOMGENERIC_RESY as u32),
            },
            wgpu::Extent3d {
                width: DOOMGENERIC_RESX as u32,
                height: DOOMGENERIC_RESY as u32,
                depth_or_array_layers: 1,
            },
        );
        let mut reconfigured = false;
        let output = loop {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) => break t,
                wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                    // Surface is suboptimal but still usable; present this frame.
                    break t;
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    return Err("surface timeout".to_owned());
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    // Window is minimised or hidden; skip this frame.
                    return Ok(());
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    // The canvas size often changes after the first configure on
                    // the web. Rebuild the swapchain once and present this frame.
                    if reconfigured || self.config.width == 0 || self.config.height == 0 {
                        return Err("surface lost or outdated".to_owned());
                    }
                    reconfigured = true;
                    self.surface.configure(&self.device, &self.config);
                }
                // Handle future variants without breaking the build.
                _ => {
                    return Err("surface error: unknown status".to_owned());
                }
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("doom_frame_encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("doom_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            // Draw 6 vertices (2 triangles) without a vertex buffer.
            render_pass.draw(0..6, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Draw a triangle scene, then composite the software HUD on top.
    pub fn render_world(&mut self, pixels: &[u8], scene: &mut crate::hw::Scene) -> Result<(), String> {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.doom_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((DOOMGENERIC_RESX * 4) as u32),
                rows_per_image: Some(DOOMGENERIC_RESY as u32),
            },
            wgpu::Extent3d {
                width: DOOMGENERIC_RESX as u32,
                height: DOOMGENERIC_RESY as u32,
                depth_or_array_layers: 1,
            },
        );
        self.queue
            .write_buffer(&self.hud_uniform, 0, bytemuck::cast_slice(&scene.view_rect));

        let atlas = scene
            .atlas
            .as_ref()
            .map(|(size, rgba)| (*size, rgba.as_slice()));
        if let Some(frame) = scene.svo.as_mut() {
            if let Some(tree) = frame.tree.take() {
                self.octree = Some(tree);
                self.octree_dirty = true;
            }
        }
        let node_upload = if self.octree_dirty {
            self.octree_dirty = false;
            self.octree.as_ref().map(|tree| tree.nodes().to_vec())
        } else {
            None
        };
        self.ensure_world(atlas);
        if let Some(world) = self.world.as_mut() {
            upload_mesh(world, &self.device, &self.queue, scene);
            self.queue
                .write_buffer(&world.uniform, 0, bytemuck::cast_slice(&scene.view_proj));
            if self.config.width != world.depth_size.0 || self.config.height != world.depth_size.1 {
                let (depth, depth_view) =
                    depth_target(&self.device, self.config.width, self.config.height);
                world.depth = depth;
                world.depth_view = depth_view;
                world.depth_size = (self.config.width, self.config.height);
            }
            if let Some(frame) = &scene.svo {
                let surf_w = self.config.width as f32;
                let surf_h = self.config.height as f32;
                let rect = scene.view_rect;
                let view = [
                    rect[0] * surf_w,
                    rect[1] * surf_h,
                    (rect[2] * surf_w).max(1.0),
                    (rect[3] * surf_h).max(1.0),
                ];
                sync_svo(
                    world,
                    &self.device,
                    &self.queue,
                    frame,
                    view,
                    node_upload.as_deref(),
                );
            }
        }

        let mut reconfigured = false;
        let output = loop {
            match self.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) => break t,
                wgpu::CurrentSurfaceTexture::Suboptimal(t) => break t,
                wgpu::CurrentSurfaceTexture::Timeout => return Err("surface timeout".to_owned()),
                wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    if reconfigured || self.config.width == 0 || self.config.height == 0 {
                        return Err("surface lost or outdated".to_owned());
                    }
                    reconfigured = true;
                    self.surface.configure(&self.device, &self.config);
                }
                _ => return Err("surface error: unknown status".to_owned()),
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("doom_world_encoder"),
            });

        let rect = scene.view_rect;
        let surf_w = self.config.width as f32;
        let surf_h = self.config.height as f32;
        let vx = (rect[0] * surf_w).round().clamp(0.0, surf_w - 1.0);
        let vy = (rect[1] * surf_h).round().clamp(0.0, surf_h - 1.0);
        let vw = (rect[2] * surf_w).round().clamp(1.0, surf_w - vx);
        let vh = (rect[3] * surf_h).round().clamp(1.0, surf_h - vy);

        if let Some(world) = self.world.as_ref() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("doom_world_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.03,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &world.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);
            pass.set_scissor_rect(vx as u32, vy as u32, vw as u32, vh as u32);
            pass.set_pipeline(&world.pipeline);
            pass.set_bind_group(0, &world.bind_group, &[]);
            pass.set_vertex_buffer(0, world.vertices.slice(..));
            pass.set_index_buffer(world.indices.slice(..), wgpu::IndexFormat::Uint32);
            let sky_n = scene.sky_indices.min(scene.indices.len() as u32);
            if sky_n > 0 {
                pass.draw_indexed(0..sky_n, 0, 0..1);
            }
            if scene.svo.is_some() {
                pass.set_pipeline(&world.svo_pipeline);
                pass.set_bind_group(0, &world.svo_bind, &[]);
                pass.draw(0..4, 0..1);
            }
            let world_n = scene.world_indices.min(scene.indices.len() as u32);
            if world_n > sky_n {
                pass.set_pipeline(&world.pipeline);
                pass.set_bind_group(0, &world.bind_group, &[]);
                pass.set_vertex_buffer(0, world.vertices.slice(..));
                pass.set_index_buffer(world.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(sky_n..world_n, 0, 0..1);
            }
            let total = scene.indices.len() as u32;
            if total > world_n {
                pass.set_pipeline(&world.weapon_pipeline);
                pass.set_bind_group(0, &world.bind_group, &[]);
                pass.set_vertex_buffer(0, world.vertices.slice(..));
                pass.set_index_buffer(world.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(world_n..total, 0, 0..1);
            }
        }

        {
            let mut hud = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("doom_hud_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            hud.set_pipeline(&self.hud_pipeline);
            hud.set_bind_group(0, &self.hud_bind_group, &[]);
            hud.set_bind_group(1, &self.hud_rect_group, &[]);
            hud.draw(0..6, 0..1);
        }

        #[cfg(not(target_arch = "wasm32"))]
        let adapt_count = self.octree.as_ref().map(|tree| tree.nodes().len()).unwrap_or(0);
        #[cfg(not(target_arch = "wasm32"))]
        if adapt_count > 0 {
            if let Some(world) = self.world.as_mut() {
                record_adapt(world, &self.queue, &mut encoder, adapt_count as u32);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        #[cfg(not(target_arch = "wasm32"))]
        if adapt_count > 0 {
            finish_adapt(self);
        }
        Ok(())
    }

    fn ensure_world(&mut self, atlas: Option<(u32, &[u8])>) {
        let Some((size, rgba)) = atlas else {
            return;
        };
        if self.world.as_ref().is_some_and(|w| w.atlas_size == size) {
            let world = self.world.as_ref().unwrap();
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &world.atlas,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
            return;
        }

        let atlas_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("doom_atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 4),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("doom_atlas_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("doom_world_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_camera"),
            size: 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("doom_world_bind_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("doom_world_shader"),
                source: wgpu::ShaderSource::Wgsl(WORLD_SHADER.into()),
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("doom_world_pipeline_layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let buffers = [vertex_layout()];
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("doom_world_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_world"),
                    buffers: &buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_world"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(true, wgpu::CompareFunction::Less)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });
        let weapon_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("doom_weapon_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_world"),
                    buffers: &buffers,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_world"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(false, wgpu::CompareFunction::Always)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });
        let svo_shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("doom_svo_shader"),
            source: wgpu::ShaderSource::Wgsl(svo_shader().into()),
        });
        let svo_layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("doom_svo_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: svo_node_binding(),
                        count: None,
                    },
                ],
            });
        let svo_pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("doom_svo_pipeline_layout"),
                bind_group_layouts: &[Some(&svo_layout)],
                immediate_size: 0,
            });
        let svo_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("doom_svo_pipeline"),
                layout: Some(&svo_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &svo_shader,
                    entry_point: Some("vs_svo"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &svo_shader,
                    entry_point: Some("fs_svo"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(true, wgpu::CompareFunction::Less)),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });
        let svo_uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_svo_uniform"),
            size: std::mem::size_of::<SvoUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (svo_tex, svo_view) = svo_texture(&self.device, SVO_TEX_WIDTH, 1);
        let (node_buf, adapt, svo_bind) = svo_nodes_and_bind(
            &self.device,
            &svo_layout,
            &svo_uniform,
            &svo_view,
        );
        let (depth, depth_view) = depth_target(&self.device, self.config.width, self.config.height);
        let vertices = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_vertices"),
            size: 64 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indices = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_indices"),
            size: 64 * 1024,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.world = Some(WorldGpu {
            atlas: atlas_tex,
            atlas_size: size,
            bind_group,
            pipeline,
            weapon_pipeline,
            depth,
            depth_view,
            depth_size: (self.config.width, self.config.height),
            uniform,
            vertices,
            indices,
            vertex_cap: 64 * 1024,
            index_cap: 64 * 1024,
            svo_pipeline,
            svo_layout,
            svo_uniform,
            svo_tex,
            svo_view,
            svo_bind,
            svo_width: SVO_TEX_WIDTH,
            svo_height: 1,
            node_buf,
            adapt,
        });
    }
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 5] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 12,
        shader_location: 1,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 20,
        shader_location: 2,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32,
        offset: 36,
        shader_location: 3,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 40,
        shader_location: 4,
    },
];

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<crate::hw::Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &VERTEX_ATTRS,
    }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24Plus,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn depth_target(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("doom_depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24Plus,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = depth.create_view(&wgpu::TextureViewDescriptor::default());
    (depth, view)
}

fn upload_mesh(
    world: &mut WorldGpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scene: &crate::hw::Scene,
) {
    let v_bytes = bytemuck::cast_slice(&scene.verts);
    let i_bytes = bytemuck::cast_slice(&scene.indices);
    if v_bytes.len() as u64 > world.vertex_cap {
        let size = (v_bytes.len() as u64).next_power_of_two().max(64 * 1024);
        world.vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_vertices"),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        world.vertex_cap = size;
    }
    if i_bytes.len() as u64 > world.index_cap {
        let size = (i_bytes.len() as u64).next_power_of_two().max(64 * 1024);
        world.indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_indices"),
            size,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        world.index_cap = size;
    }
    if !v_bytes.is_empty() {
        queue.write_buffer(&world.vertices, 0, v_bytes);
    }
    if !i_bytes.is_empty() {
        queue.write_buffer(&world.indices, 0, i_bytes);
    }
}

const SVO_TEX_WIDTH: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SvoUniform {
    inv_view: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    origin: [f32; 4],
    view: [f32; 4],
    tex: [f32; 4],
}

fn svo_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("doom_svo"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn svo_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("doom_svo_bind"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(view),
            },
        ],
    })
}

fn sync_svo(
    world: &mut WorldGpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame: &crate::hw::SvoFrame,
    view: [f32; 4],
    uploaded: Option<&[u32]>,
) {
    let nodes = uploaded.or(frame.nodes.as_deref());
    if world.node_buf.is_some() {
        if let Some(nodes) = nodes {
            upload_svo_nodes(world, device, queue, nodes);
        }
    } else if let Some(nodes) = nodes {
        let width = SVO_TEX_WIDTH;
        let height = ((nodes.len() as u32) + width - 1) / width;
        let height = height.max(1);
        if height != world.svo_height || width != world.svo_width {
            let (tex, tex_view) = svo_texture(device, width, height);
            world.svo_bind = svo_bind_group(device, &world.svo_layout, &world.svo_uniform, &tex_view);
            world.svo_tex = tex;
            world.svo_view = tex_view;
            world.svo_width = width;
            world.svo_height = height;
        }
        let mut bytes = vec![0u8; (world.svo_width * world.svo_height * 4) as usize];
        for (i, word) in nodes.iter().enumerate() {
            let at = i * 4;
            bytes[at..at + 4].copy_from_slice(&word.to_le_bytes());
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &world.svo_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(world.svo_width * 4),
                rows_per_image: Some(world.svo_height),
            },
            wgpu::Extent3d {
                width: world.svo_width,
                height: world.svo_height,
                depth_or_array_layers: 1,
            },
        );
    }
    let uniform = SvoUniform {
        inv_view: frame.inv_view,
        view_proj: frame.view_proj,
        origin: [frame.origin[0], frame.origin[1], frame.origin[2], frame.cube],
        view,
        tex: [
            world.svo_width as f32,
            frame.nodes.as_ref().map(|n| n.len()).unwrap_or(0) as f32,
            0.0,
            0.0,
        ],
    };
    queue.write_buffer(&world.svo_uniform, 0, bytemuck::cast_slice(&[uniform]));
}

fn svo_node_binding() -> wgpu::BindingType {
    #[cfg(target_arch = "wasm32")]
    {
        wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        }
    }
}

fn svo_storage_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    nodes: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("doom_svo_bind"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: nodes.as_entire_binding(),
            },
        ],
    })
}

fn svo_nodes_and_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    view: &wgpu::TextureView,
) -> (Option<wgpu::Buffer>, Option<SvoAdapt>, wgpu::BindGroup) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = device;
        (None, None, svo_bind_group(device, layout, uniform, view))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = view;
        let nodes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_svo_nodes"),
            size: 65_536,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = svo_storage_bind(device, layout, uniform, &nodes);
        let adapt = SvoAdapt::new(device, &nodes);
        (Some(nodes), Some(adapt), bind)
    }
}

fn upload_svo_nodes(
    world: &mut WorldGpu,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    nodes: &[u32],
) {
    let bytes = (nodes.len() as u64).saturating_mul(4).max(4);
    let fits = world.node_buf.as_ref().is_some_and(|buf| buf.size() >= bytes);
    if !fits {
        let cap = bytes.next_power_of_two().max(65_536);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_svo_nodes"),
            size: cap,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        world.svo_bind = svo_storage_bind(device, &world.svo_layout, &world.svo_uniform, &buf);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(adapt) = world.adapt.as_mut() {
            adapt.rebind(device, &buf);
        }
        world.node_buf = Some(buf);
    }
    if let Some(buf) = &world.node_buf {
        queue.write_buffer(buf, 0, bytemuck::cast_slice(nodes));
    }
}

#[cfg(not(target_arch = "wasm32"))]
const ADAPT_QUEUE: u64 = 1_024_000 * 4;

#[cfg(not(target_arch = "wasm32"))]
impl SvoAdapt {
    fn new(device: &wgpu::Device, nodes: &wgpu::Buffer) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("doom_svo_adapt"),
            source: wgpu::ShaderSource::Wgsl(SVO_ADAPT.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("doom_svo_adapt_layout"),
            entries: &[
                storage_layout(0, true, false),
                storage_layout(1, false, true),
                storage_layout(2, false, false),
                storage_layout(3, false, false),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("doom_svo_adapt_pipeline"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("doom_svo_adapt"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_adapt"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("doom_svo_adapt_uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sub = queue_buffer(device, "doom_svo_sub", false);
        let unsub = queue_buffer(device, "doom_svo_unsub", false);
        let sub_read = queue_buffer(device, "doom_svo_sub_read", true);
        let unsub_read = queue_buffer(device, "doom_svo_unsub_read", true);
        let bind = adapt_bind(device, &layout, &uniform, nodes, &sub, &unsub);
        Self {
            pipeline,
            layout,
            uniform,
            bind,
            sub,
            unsub,
            sub_read,
            unsub_read,
        }
    }

    fn rebind(&mut self, device: &wgpu::Device, nodes: &wgpu::Buffer) {
        self.bind = adapt_bind(device, &self.layout, &self.uniform, nodes, &self.sub, &self.unsub);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_layout(binding: u32, uniform: bool, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: if uniform {
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            }
        } else {
            wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            }
        },
        count: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn queue_buffer(device: &wgpu::Device, label: &str, mappable: bool) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: ADAPT_QUEUE,
        usage: if mappable {
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST
        } else {
            wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST
        },
        mapped_at_creation: false,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn adapt_bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    nodes: &wgpu::Buffer,
    sub: &wgpu::Buffer,
    unsub: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("doom_svo_adapt_bind"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: nodes.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: sub.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: unsub.as_entire_binding(),
            },
        ],
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn record_adapt(
    world: &WorldGpu,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    count: u32,
) {
    let Some(adapt) = world.adapt.as_ref() else {
        return;
    };
    let uniform = [count, 24u32, 0, 0];
    queue.write_buffer(&adapt.uniform, 0, bytemuck::cast_slice(&uniform));
    queue.write_buffer(&adapt.sub, 0, &[0, 0, 0, 0]);
    queue.write_buffer(&adapt.unsub, 0, &[0, 0, 0, 0]);
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("doom_svo_adapt"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&adapt.pipeline);
        pass.set_bind_group(0, &adapt.bind, &[]);
        pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&adapt.sub, 0, &adapt.sub_read, 0, adapt.sub.size());
    encoder.copy_buffer_to_buffer(&adapt.unsub, 0, &adapt.unsub_read, 0, adapt.unsub.size());
}

#[cfg(not(target_arch = "wasm32"))]
fn finish_adapt(state: &mut GpuState) {
    let (sub, unsub) = {
        let Some(adapt) = state.world.as_ref().and_then(|world| world.adapt.as_ref()) else {
            return;
        };
        let device = state.device.clone();
        (
            map_ids(&device, &adapt.sub_read),
            map_ids(&device, &adapt.unsub_read),
        )
    };
    if let Some(tree) = state.octree.as_mut() {
        tree.apply_hits(&sub, &unsub);
    }
    let Some(words) = state.octree.as_ref().map(|tree| tree.nodes().to_vec()) else {
        return;
    };
    let device = state.device.clone();
    let queue = state.queue.clone();
    if let Some(world) = state.world.as_mut() {
        upload_svo_nodes(world, &device, &queue, &words);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn map_ids(device: &wgpu::Device, buf: &wgpu::Buffer) -> Vec<u32> {
    let slice = buf.slice(..);
    let (send, recv) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = send.send(result);
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    if recv.recv().ok().and_then(|result| result.ok()).is_none() {
        return Vec::new();
    }
    let data = slice.get_mapped_range();
    let words: Vec<u32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    buf.unmap();
    if words.is_empty() {
        return Vec::new();
    }
    let count = (words[0] as usize).min(words.len() - 1).min(1_023_999);
    words[1..=count].to_vec()
}

/// CSS size of the page canvas, used when winit has not reported a real size yet.
#[cfg(target_arch = "wasm32")]
fn web_canvas_size() -> PhysicalSize<u32> {
    let fallback = PhysicalSize::new(640, 400);
    let Some(window) = web_sys::window() else {
        return fallback;
    };
    let Some(document) = window.document() else {
        return fallback;
    };
    let Some(canvas) = document.get_element_by_id("doom-canvas") else {
        return fallback;
    };
    let width = canvas.client_width();
    let height = canvas.client_height();
    if width < 2 || height < 2 {
        fallback
    } else {
        PhysicalSize::new(width as u32, height as u32)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod adapt_pipeline {
    use super::*;

    #[test]
    fn hit_counter_pipelines_build() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })) else {
            return;
        };
        let Ok((device, _)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("svo_adapt_test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        })) else {
            return;
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("svo"),
            source: wgpu::ShaderSource::Wgsl(svo_shader().into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("svo"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: svo_node_binding(),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("svo"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let _trace = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("svo"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_svo"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_svo"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(depth_state(true, wgpu::CompareFunction::LessEqual)),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let nodes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("nodes"),
            size: 256,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let _adapt = SvoAdapt::new(&device, &nodes);
    }
}
