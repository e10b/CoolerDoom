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
    pub fn render_world(&mut self, pixels: &[u8], scene: &crate::hw::Scene) -> Result<(), String> {
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
            let world_n = scene.world_indices.min(scene.indices.len() as u32);
            if world_n > 0 {
                pass.draw_indexed(0..world_n, 0, 0..1);
            }
            let total = scene.indices.len() as u32;
            if total > world_n {
                pass.set_pipeline(&world.weapon_pipeline);
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

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
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
