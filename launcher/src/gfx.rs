/*
    written by: @corescripts
*/

//! wgpu renderer: shadow map, HDR scene pass, post (BlurEffect/ColorCorrection/bloom), UI.

// >> imports
use crate::mesh::{self, MeshData, Vertex};
use crate::rbxl::CFrame;
use crate::scene::{self, LightKind, MeshKey, Scene};
use crate::ui::{UiFrame, UiVertex};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// samples per pixel for the 3D passes (anti-aliased edges), resolved before post-processing
const MSAA: u32 = 4;
const SHADOW_SIZE: u32 = 2048;
const MAX_LIGHTS: usize = 48;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuLight {
    pos_range: [f32; 4],
    color_kind: [f32; 4],
    dir_angle: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    shadow_mat: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    ambient: [f32; 4],
    outdoor: [f32; 4],
    shift_top: [f32; 4],
    shift_bottom: [f32; 4],
    fog_color: [f32; 4],
    fog: [f32; 4],
    lights: [GpuLight; MAX_LIGHTS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkyGlobals {
    inv_view_proj: [[f32; 4]; 4],
    sun: [f32; 4],
    misc: [f32; 4],
    cam_pos: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceData {
    m: [[f32; 4]; 4],
    scale_mat: [f32; 4],
    color: [f32; 4],
    extra: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
    emission: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostParams {
    texel: [f32; 2],
    dir: [f32; 2],
    sigma: f32,
    threshold: f32,
    _p: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CcParams {
    brightness: f32,
    contrast: f32,
    saturation: f32,
    blur_mix: f32,
    tint: [f32; 4],
    bloom: [f32; 4],
}

pub struct PostFx {
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub tint: Vec3,
    pub blur: f32,
    /// radius (fraction of screen height) left unfiltered in the middle; 0 filters everything
    pub clear: f32,
    /// bloom strength
    pub bloom: f32,
}

impl Default for PostFx {
    fn default() -> Self {
        PostFx { brightness: 0.0, contrast: 0.0, saturation: 0.0, tint: Vec3::ONE, blur: 0.0, clear: 0.0, bloom: 0.45 }
    }
}

impl PostFx {
    /// The menu's dark blue background filter at rest (Flythrough between transitions).
    pub fn menu() -> Self {
        PostFx { brightness: 0.3, contrast: 0.5, saturation: -1.0, tint: crate::scene::BACKGROUND_TINT, blur: 10.0, ..Default::default() }
    }
}

struct GpuMesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    count: u32,
}

enum MeshState {
    Pending,
    Ready(GpuMesh),
    Failed,
}

struct Target {
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    w: u32,
    h: u32,
}

/// Offscreen pass for the 3D avatar: its own targets, globals and instances, shown as a UI texture.
struct AvatarPass {
    color: Target,
    color_ms: Target,
    depth: Target,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    instance_buf: wgpu::Buffer,
}

pub const AVATAR_SIZE: u32 = 1024;

pub struct UiTexture {
    pub bind: wgpu::BindGroup,
    pub tex: wgpu::Texture,
    pub w: u32,
    pub h: u32,
}

pub struct Gfx {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,

    tex_layout: wgpu::BindGroupLayout,
    linear_repeat: wgpu::Sampler,
    linear_clamp: wgpu::Sampler,
    white: wgpu::BindGroup,

    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    shadow_view: wgpu::TextureView,
    scene_opaque: wgpu::RenderPipeline,
    scene_transparent: wgpu::RenderPipeline,
    /// no back-face culling: the avatar, so the inside of hats, hair and sleeves shows
    scene_opaque_double: wgpu::RenderPipeline,
    scene_transparent_double: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    shadow_bg: wgpu::BindGroup,
    quad_alpha: wgpu::RenderPipeline,
    quad_add: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    sky_buf: wgpu::Buffer,
    sky_uniform_bg: wgpu::BindGroup,
    sky_tex_layout: wgpu::BindGroupLayout,
    sky_cache: HashMap<String, wgpu::BindGroup>,
    sky_placeholder: wgpu::BindGroup,

    meshes: HashMap<MeshKey, MeshState>,
    textures: HashMap<String, Option<wgpu::BindGroup>>,
    instance_buf: wgpu::Buffer,
    quad_buf: wgpu::Buffer,

    hdr: Target,
    /// the multisampled scene colour, resolved into `hdr`
    hdr_ms: Target,
    depth: Target,
    half_a: Target,
    half_b: Target,
    quarter_a: Target,
    quarter_b: Target,
    post_layout: wgpu::BindGroupLayout,
    comp_layout: wgpu::BindGroupLayout,
    down_pipe: wgpu::RenderPipeline,
    blur_pipe: wgpu::RenderPipeline,
    comp_pipe: wgpu::RenderPipeline,
    cc_buf: wgpu::Buffer,
    post_bufs: Vec<wgpu::Buffer>,

    ui_pipe: wgpu::RenderPipeline,
    ui_buf: wgpu::Buffer,
    ui_ubg: wgpu::BindGroup,
    ui_ubuf: wgpu::Buffer,
    pub ui_textures: Vec<UiTexture>,
    ui_vb: wgpu::Buffer,

    pub shadows: bool,
    pub capture: Option<std::path::PathBuf>,
    avatar: AvatarPass,
    /// ui_textures index of the avatar render
    pub avatar_ui: usize,
}

fn target(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat, label: &str) -> Target {
    target_ms(device, w, h, format, label, 1)
}

/// A render target with `samples` per pixel (multisampled ones are only drawn into and resolved).
fn target_ms(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat, label: &str, samples: u32) -> Target {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: if samples > 1 { wgpu::TextureUsages::RENDER_ATTACHMENT } else { wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING },
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    Target { tex, view, w: w.max(1), h: h.max(1) }
}

fn tex_entry(binding: u32, kind: wgpu::TextureSampleType, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type: kind, view_dimension: dim, multisampled: false },
        count: None,
    }
}

fn uniform_entry(binding: u32, vis: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
        count: None,
    }
}

fn sampler_entry(binding: u32, ty: wgpu::SamplerBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(ty), count: None }
}

const FLOAT: wgpu::TextureSampleType = wgpu::TextureSampleType::Float { filterable: true };

impl Gfx {
    pub fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow::anyhow!("no GPU adapter"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor { label: None, required_features: wgpu::Features::empty(), required_limits: wgpu::Limits::default(), memory_hints: Default::default() },
            None,
        ))?;
        let size = window.inner_size();
        let mut config = surface.get_default_config(&adapter, size.width.max(1), size.height.max(1)).ok_or_else(|| anyhow::anyhow!("surface unsupported"))?;
        // gamma-space output like the original engine
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().copied().find(|f| !f.is_srgb()) {
            config.format = f;
        }
        config.present_mode = wgpu::PresentMode::AutoVsync;
        if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
            config.usage |= wgpu::TextureUsages::COPY_SRC;
        }
        surface.configure(&device, &config);

        let linear_repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            // sharp textures on surfaces seen at an angle
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let linear_clamp = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tex"),
            entries: &[tex_entry(0, FLOAT, wgpu::TextureViewDimension::D2), sampler_entry(1, wgpu::SamplerBindingType::Filtering)],
        });

        let white_tex = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("white"),
                size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            Default::default(),
            &[255, 255, 255, 255],
        );
        let white = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &tex_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&white_tex.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&linear_repeat) },
            ],
        });

        // >> scene
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow"),
            size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&Default::default());
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                tex_entry(1, wgpu::TextureSampleType::Depth, wgpu::TextureViewDimension::D2),
                sampler_entry(2, wgpu::SamplerBindingType::Comparison),
            ],
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &globals_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
            ],
        });

        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/scene.wgsl").into()),
        });
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&globals_layout, &tex_layout],
            push_constant_ranges: &[],
        });
        let shadow_layout_only = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow globals"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let shadow_globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &shadow_layout_only,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals_buf.as_entire_binding() }],
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Unorm8x4],
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Uint32x4],
        };
        let depth_state = |write: bool| wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: write,
            depth_compare: wgpu::CompareFunction::Greater,
            stencil: Default::default(),
            bias: Default::default(),
        };
        let mk_scene = |blend: Option<wgpu::BlendState>, write: bool, cull: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene"),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &scene_shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[vertex_layout.clone(), instance_layout.clone()],
                },
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
                depth_stencil: Some(depth_state(write)),
                multisample: wgpu::MultisampleState { count: MSAA, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &scene_shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: HDR, blend, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let scene_opaque = mk_scene(None, true, Some(wgpu::Face::Back));
        let scene_transparent = mk_scene(Some(wgpu::BlendState::ALPHA_BLENDING), false, Some(wgpu::Face::Back));
        let scene_opaque_double = mk_scene(None, true, None);
        let scene_transparent_double = mk_scene(Some(wgpu::BlendState::ALPHA_BLENDING), false, None);

        let shadow_pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&shadow_layout_only],
            push_constant_ranges: &[],
        });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_pipe_layout),
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[vertex_layout.clone(), instance_layout.clone()],
            },
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: Default::default(),
            fragment: None,
            multiview: None,
            cache: None,
        });

        let quad_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x4, 3 => Float32],
        };
        let mk_quad = |blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("quads"),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &scene_shader,
                    entry_point: Some("vs_quad"),
                    compilation_options: Default::default(),
                    buffers: &[quad_layout.clone()],
                },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(depth_state(false)),
                multisample: wgpu::MultisampleState { count: MSAA, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &scene_shader,
                    entry_point: Some("fs_quad"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: HDR, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let quad_alpha = mk_quad(wgpu::BlendState::ALPHA_BLENDING);
        let quad_add = mk_quad(wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent::OVER,
        });

        // >> sky
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
        });
        let sky_ulayout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky u"),
            entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let sky_tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky tex"),
            entries: &[tex_entry(0, FLOAT, wgpu::TextureViewDimension::D2Array), sampler_entry(1, wgpu::SamplerBindingType::Filtering)],
        });
        let sky_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky"),
            size: std::mem::size_of::<SkyGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &sky_ulayout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: sky_buf.as_entire_binding() }],
        });
        let sky_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&sky_ulayout, &sky_tex_layout],
            push_constant_ranges: &[],
        });
        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&sky_layout),
            vertex: wgpu::VertexState { module: &sky_shader, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState { count: MSAA, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: HDR, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });
        let placeholder_layers: Vec<u8> = (0..6).flat_map(|_| [110u8, 140, 190, 255]).collect();
        let sky_placeholder = make_sky_bg(&device, &queue, &sky_tex_layout, &linear_clamp, 1, &placeholder_layers);

        // >> post
        let post_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/post.wgsl").into()),
        });
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post"),
            entries: &[tex_entry(0, FLOAT, wgpu::TextureViewDimension::D2), sampler_entry(1, wgpu::SamplerBindingType::Filtering), uniform_entry(2, wgpu::ShaderStages::FRAGMENT)],
        });
        let comp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[
                tex_entry(0, FLOAT, wgpu::TextureViewDimension::D2),
                tex_entry(1, FLOAT, wgpu::TextureViewDimension::D2),
                tex_entry(2, FLOAT, wgpu::TextureViewDimension::D2),
                uniform_entry(3, wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let post_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&post_layout], push_constant_ranges: &[] });
        let comp_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&post_layout, &comp_layout], push_constant_ranges: &[] });
        let mk_post = |layout: &wgpu::PipelineLayout, fs: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(layout),
                vertex: wgpu::VertexState { module: &post_shader, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &post_shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview: None,
                cache: None,
            })
        };
        let down_pipe = mk_post(&post_pl, "fs_down", HDR);
        let blur_pipe = mk_post(&post_pl, "fs_blur", HDR);
        let comp_pipe = mk_post(&comp_pl, "fs_composite", config.format);
        let cc_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cc"),
            size: std::mem::size_of::<CcParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post_bufs = (0..8)
            .map(|_| {
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("post params"),
                    size: std::mem::size_of::<PostParams>() as u64,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                })
            })
            .collect();

        // >> ui
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });
        let ui_ulayout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("ui u"), entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)] });
        let ui_ubuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui u"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ui_ubg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &ui_ulayout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ui_ubuf.as_entire_binding() }],
        });
        let ui_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[&ui_ulayout, &tex_layout], push_constant_ranges: &[] });
        let ui_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&ui_pl),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                }],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview: None,
            cache: None,
        });

        let (w, h) = (config.width, config.height);
        let buf = |label: &str, size: u64, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
        };
        let instance_buf = buf("instances", 1 << 20, wgpu::BufferUsages::VERTEX);
        let quad_buf = buf("quads", 1 << 20, wgpu::BufferUsages::VERTEX);
        let ui_vb = buf("ui", 1 << 20, wgpu::BufferUsages::VERTEX);
        let ui_buf = buf("ui2", 16, wgpu::BufferUsages::UNIFORM);

        let avatar_globals = buf("avatar globals", std::mem::size_of::<Globals>() as u64, wgpu::BufferUsages::UNIFORM);
        let avatar = AvatarPass {
            color: target(&device, AVATAR_SIZE, AVATAR_SIZE, HDR, "avatar"),
            color_ms: target_ms(&device, AVATAR_SIZE, AVATAR_SIZE, HDR, "avatar ms", MSAA),
            depth: target_ms(&device, AVATAR_SIZE, AVATAR_SIZE, DEPTH, "avatar depth", MSAA),
            globals_bg: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &globals_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: avatar_globals.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&shadow_view) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
                ],
            }),
            globals_buf: avatar_globals,
            instance_buf: buf("avatar instances", 1 << 16, wgpu::BufferUsages::VERTEX),
        };

        let mut g = Gfx {
            window,
            surface,
            hdr: target(&device, w, h, HDR, "hdr"),
            hdr_ms: target_ms(&device, w, h, HDR, "hdr ms", MSAA),
            depth: target_ms(&device, w, h, DEPTH, "depth", MSAA),
            half_a: target(&device, w / 2, h / 2, HDR, "half a"),
            half_b: target(&device, w / 2, h / 2, HDR, "half b"),
            quarter_a: target(&device, w / 4, h / 4, HDR, "q a"),
            quarter_b: target(&device, w / 4, h / 4, HDR, "q b"),
            device,
            queue,
            config,
            tex_layout,
            linear_repeat,
            linear_clamp,
            white,
            globals_buf,
            globals_bg,
            shadow_view,
            scene_opaque,
            scene_transparent,
            scene_opaque_double,
            scene_transparent_double,
            shadow_pipe,
            shadow_bg: shadow_globals_bg,
            quad_alpha,
            quad_add,
            sky_pipe,
            sky_buf,
            sky_uniform_bg,
            sky_tex_layout,
            sky_cache: HashMap::new(),
            sky_placeholder,
            meshes: HashMap::new(),
            textures: HashMap::new(),
            instance_buf,
            quad_buf,
            post_layout,
            comp_layout,
            down_pipe,
            blur_pipe,
            comp_pipe,
            cc_buf,
            post_bufs,
            ui_pipe,
            ui_buf,
            ui_ubg,
            ui_ubuf,
            ui_textures: Vec::new(),
            ui_vb,
            shadows: true,
            capture: None,
            avatar,
            avatar_ui: 0,
        };
        // UI texture 0 = white
        g.add_ui_texture(1, 1, &[255, 255, 255, 255]);
        let bind = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &g.tex_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&g.avatar.color.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&g.linear_clamp) },
            ],
        });
        g.ui_textures.push(UiTexture { bind, tex: g.avatar.color.tex.clone(), w: AVATAR_SIZE, h: AVATAR_SIZE });
        g.avatar_ui = g.ui_textures.len() - 1;
        for k in [MeshKey::Box, MeshKey::Wedge, MeshKey::CornerWedge, MeshKey::CylX, MeshKey::CylY, MeshKey::Sphere, MeshKey::Head] {
            let m = match k {
                MeshKey::Box => mesh::box_mesh(),
                MeshKey::Wedge => mesh::wedge_mesh(),
                MeshKey::CornerWedge => mesh::corner_wedge_mesh(),
                MeshKey::CylX => mesh::cylinder_x(24),
                MeshKey::CylY => mesh::cylinder_y(24),
                MeshKey::Sphere => mesh::sphere(16, 24),
                _ => mesh::head_mesh(),
            };
            g.upload_mesh(k, &m);
        }
        Ok(g)
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.hdr = target(&self.device, w, h, HDR, "hdr");
        self.hdr_ms = target_ms(&self.device, w, h, HDR, "hdr ms", MSAA);
        self.depth = target_ms(&self.device, w, h, DEPTH, "depth", MSAA);
        self.half_a = target(&self.device, w / 2, h / 2, HDR, "half a");
        self.half_b = target(&self.device, w / 2, h / 2, HDR, "half b");
        self.quarter_a = target(&self.device, w / 4, h / 4, HDR, "q a");
        self.quarter_b = target(&self.device, w / 4, h / 4, HDR, "q b");
    }

    // >> resources

    pub fn upload_mesh(&mut self, key: MeshKey, m: &MeshData) {
        if m.indices.is_empty() {
            self.meshes.insert(key, MeshState::Failed);
            return;
        }
        let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh vb"),
            contents: bytemuck::cast_slice(&m.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let ib = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh ib"),
            contents: bytemuck::cast_slice(&m.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.meshes.insert(key, MeshState::Ready(GpuMesh { vb, ib, count: m.indices.len() as u32 }));
    }

    pub fn mesh_failed(&mut self, key: MeshKey) {
        self.meshes.insert(key, MeshState::Failed);
    }

    /// Mesh keys the scene references that have not been requested yet.
    pub fn missing_meshes(&mut self, scene: &Scene) -> Vec<MeshKey> {
        let mut out = Vec::new();
        for p in &scene.parts {
            if !self.meshes.contains_key(&p.mesh) {
                self.meshes.insert(p.mesh, MeshState::Pending);
                out.push(p.mesh);
            }
        }
        out
    }

    pub fn forget_mesh(&mut self, key: MeshKey) {
        self.meshes.remove(&key);
    }

    pub fn forget_texture(&mut self, key: &str) {
        self.textures.remove(key);
    }

    pub fn forget_sky(&mut self, key: &str) {
        self.sky_cache.remove(key);
    }

    pub fn pending_meshes(&self) -> usize {
        self.meshes.values().filter(|m| matches!(m, MeshState::Pending)).count()
    }

    fn make_texture(&self, w: u32, h: u32, rgba: &[u8], mips: bool) -> wgpu::Texture {
        let levels = if mips { (w.max(h) as f32).log2().floor() as u32 + 1 } else { 1 };
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut level_data = rgba.to_vec();
        let (mut lw, mut lh) = (w, h);
        for level in 0..levels {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: level, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                &level_data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * lw), rows_per_image: Some(lh) },
                wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
            );
            if level + 1 < levels {
                let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
                let mut next = vec![0u8; (nw * nh * 4) as usize];
                for y in 0..nh {
                    for x in 0..nw {
                        for c in 0..4 {
                            let mut s = 0u32;
                            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                                let sx = (x * 2 + dx).min(lw - 1);
                                let sy = (y * 2 + dy).min(lh - 1);
                                s += level_data[((sy * lw + sx) * 4 + c) as usize] as u32;
                            }
                            next[((y * nw + x) * 4 + c) as usize] = (s / 4) as u8;
                        }
                    }
                }
                level_data = next;
                lw = nw;
                lh = nh;
            }
        }
        tex
    }

    pub fn set_scene_texture(&mut self, key: &str, img: Option<&crate::assets::Rgba>) {
        let bg = img.map(|img| {
            let tex = self.make_texture(img.w, img.h, &img.data, true);
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex.create_view(&Default::default())) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.linear_repeat) },
                ],
            })
        });
        self.textures.insert(key.to_string(), bg);
    }

    /// Scene texture keys referenced but not yet requested.
    pub fn missing_textures(&mut self, scene: &Scene) -> Vec<String> {
        let mut out = Vec::new();
        let mut want = |k: &String, out: &mut Vec<String>, map: &mut HashMap<String, Option<wgpu::BindGroup>>| {
            if !map.contains_key(k) {
                map.insert(k.clone(), None);
                out.push(k.clone());
            }
        };
        for p in &scene.parts {
            if let Some(t) = &p.texture {
                want(t, &mut out, &mut self.textures);
            }
        }
        for d in &scene.decals {
            want(&d.texture, &mut out, &mut self.textures);
        }
        for e in &scene.emitters {
            want(&e.texture, &mut out, &mut self.textures);
        }
        out
    }

    pub fn has_sky(&self, key: &str) -> bool {
        self.sky_cache.contains_key(key)
    }

    pub fn set_sky(&mut self, key: &str, faces: &[crate::assets::Rgba; 6]) {
        let size = faces.iter().map(|f| f.w.max(f.h)).max().unwrap_or(1).clamp(1, 1024);
        let mut layers = Vec::with_capacity((size * size * 4 * 6) as usize);
        for f in faces {
            if f.w == size && f.h == size {
                layers.extend_from_slice(&f.data);
            } else {
                let img = image::RgbaImage::from_raw(f.w, f.h, f.data.clone()).unwrap_or_else(|| image::RgbaImage::new(1, 1));
                let r = image::imageops::resize(&img, size, size, image::imageops::FilterType::Triangle);
                layers.extend_from_slice(r.as_raw());
            }
        }
        let bg = make_sky_bg(&self.device, &self.queue, &self.sky_tex_layout, &self.linear_clamp, size, &layers);
        self.sky_cache.insert(key.to_string(), bg);
    }

    pub fn add_ui_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> usize {
        let tex = self.make_texture(w, h, rgba, false);
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.tex_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex.create_view(&Default::default())) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.linear_clamp) },
            ],
        });
        self.ui_textures.push(UiTexture { bind, tex, w, h });
        self.ui_textures.len() - 1
    }

    pub fn update_ui_texture_rows(&self, idx: usize, y0: u32, y1: u32, rgba: &[u8]) {
        let t = &self.ui_textures[idx];
        let rows = y1 - y0;
        if rows == 0 {
            return;
        }
        let start = (y0 * t.w * 4) as usize;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &t.tex, mip_level: 0, origin: wgpu::Origin3d { x: 0, y: y0, z: 0 }, aspect: wgpu::TextureAspect::All },
            &rgba[start..start + (rows * t.w * 4) as usize],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * t.w), rows_per_image: Some(rows) },
            wgpu::Extent3d { width: t.w, height: rows, depth_or_array_layers: 1 },
        );
    }

    fn ensure_buffer(device: &wgpu::Device, buf: &mut wgpu::Buffer, size: u64, usage: wgpu::BufferUsages) {
        if buf.size() < size {
            *buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: size.next_power_of_two(),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
    }

    // >> frame

    pub fn render(&mut self, scene: Option<(&Scene, &CFrame, &PostFx)>, avatar: Option<(&[crate::scene::Part], &CFrame)>, extra: &[crate::scene::Part], ui: &UiFrame, background: Option<(usize, f32)>) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(_) => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.device.create_command_encoder(&Default::default());

        if let Some((scene, cam, fx)) = scene {
            self.render_scene(&mut enc, scene, cam, extra);
            self.render_post(&mut enc, &view, fx);
        } else {
            let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }
        let _ = background;
        if let Some((parts, cam)) = avatar {
            self.render_avatar(&mut enc, parts, cam);
        }
        self.render_ui(&mut enc, &view, ui);
        let capture = self.capture.take().map(|p| {
            let (w, h) = (self.config.width, self.config.height);
            let bpr = (w * 4 + 255) / 256 * 256;
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("capture"),
                size: (bpr * h) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            enc.copy_texture_to_buffer(
                frame.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: Some(h) } },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            (p, buf, bpr, w, h)
        });
        self.queue.submit([enc.finish()]);
        if let Some((path, buf, bpr, w, h)) = capture {
            let slice = buf.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            let _ = self.device.poll(wgpu::Maintain::Wait);
            let data = slice.get_mapped_range();
            let bgra = matches!(self.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
            let mut img = image::RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let i = (y * bpr + x * 4) as usize;
                    let px = if bgra { [data[i + 2], data[i + 1], data[i], 255] } else { [data[i], data[i + 1], data[i + 2], 255] };
                    img.put_pixel(x, y, image::Rgba(px));
                }
            }
            drop(data);
            buf.unmap();
            let _ = img.save(&path);
        }
        self.window.pre_present_notify();
        frame.present();
    }

    fn render_scene(&mut self, enc: &mut wgpu::CommandEncoder, scene: &Scene, cam: &CFrame, extra: &[crate::scene::Part]) {
        let (w, h) = (self.config.width as f32, self.config.height as f32);
        let aspect = w / h;
        let fov = 70f32.to_radians();
        let eye = cam.pos;
        let look = cam.look_vector();
        let up = cam.rot.y_axis;
        let view = Mat4::look_to_rh(eye, look, up);
        let proj = Mat4::perspective_infinite_reverse_rh(fov, aspect, 0.5);
        let view_proj = proj * view;

        let lighting = &scene.lighting;
        let sun = scene.sun_direction();
        let sun_visible = if sun.y > -0.05 { 1.0 } else { 0.0 };

        // shadow camera fitted around the area in front of the viewer, snapped to texels
        let radius = 260.0;
        let center = eye + look * 160.0;
        let light_view = Mat4::look_to_rh(Vec3::ZERO, -sun, if sun.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y });
        let mut c_ls = light_view.transform_point3(center);
        let texel = radius * 2.0 / SHADOW_SIZE as f32;
        c_ls.x = (c_ls.x / texel).floor() * texel;
        c_ls.y = (c_ls.y / texel).floor() * texel;
        let shadow_proj = Mat4::orthographic_rh(c_ls.x - radius, c_ls.x + radius, c_ls.y - radius, c_ls.y + radius, -c_ls.z - 1200.0, -c_ls.z + 1200.0);
        let shadow_mat = shadow_proj * light_view;

        // lights nearest the camera
        let mut lights: Vec<(f32, GpuLight)> = Vec::new();
        for (li, l) in scene.lights.iter().enumerate() {
            if !scene.light_active(li) {
                continue;
            }
            let part = &scene.parts[l.part];
            let mut pos = part.cf.pos;
            let mut dir = Vec3::ZERO;
            let mut kind = 0.0;
            if l.kind != LightKind::Point {
                let n = scene::face_normal(l.face);
                dir = (part.cf.rot * n).normalize();
                if l.kind == LightKind::Surface {
                    pos = part.cf.point(n * part.size * 0.5);
                }
                kind = 1.0;
            }
            let d = (pos - eye).length() - l.range;
            let c = l.color * l.brightness.min(10.0) * 0.6;
            lights.push((d, GpuLight { pos_range: [pos.x, pos.y, pos.z, l.range], color_kind: [c.x, c.y, c.z, kind], dir_angle: [dir.x, dir.y, dir.z, l.angle.max(1.0)] }));
        }
        lights.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        lights.truncate(MAX_LIGHTS);
        let mut glights = [GpuLight::zeroed(); MAX_LIGHTS];
        for (i, l) in lights.iter().enumerate() {
            glights[i] = l.1;
        }
        let shadows_on = self.shadows && lighting.shadows && sun_visible > 0.0;
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            shadow_mat: shadow_mat.to_cols_array_2d(),
            cam_pos: [eye.x, eye.y, eye.z, 0.0],
            sun_dir: [sun.x, sun.y, sun.z, lighting.brightness],
            ambient: [lighting.ambient.x, lighting.ambient.y, lighting.ambient.z, if shadows_on { 1.0 } else { 0.0 }],
            outdoor: [lighting.outdoor_ambient.x, lighting.outdoor_ambient.y, lighting.outdoor_ambient.z, lights.len() as f32],
            shift_top: lighting.shift_top.extend(0.0).into(),
            shift_bottom: lighting.shift_bottom.extend(0.0).into(),
            fog_color: lighting.fog_color.extend(1.0).into(),
            fog: [lighting.fog_start, lighting.fog_end, sun_visible, 1.0 / SHADOW_SIZE as f32],
            lights: glights,
        };
        self.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
        let sky_key = scene.zone.clone();
        let inv = view_proj.inverse();
        let sky = SkyGlobals {
            inv_view_proj: inv.to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, scene.sky.sun_size],
            misc: [if scene.sky.celestial && sun_visible > 0.0 { 1.0 } else { 0.0 }, 1.0, 0.0, 0.0],
            cam_pos: [eye.x, eye.y, eye.z, 0.0],
        };
        self.queue.write_buffer(&self.sky_buf, 0, bytemuck::bytes_of(&sky));

        // build instances
        let frustum = Frustum::new(&view_proj);
        let mut opaque: Vec<(MeshKey, Option<&str>, InstanceData)> = Vec::new();
        let mut opaque_extra: Vec<(MeshKey, Option<&str>, InstanceData)> = Vec::new();
        let mut transparent: Vec<(f32, MeshKey, Option<&str>, InstanceData, bool)> = Vec::new();
        // extra parts (the avatar standing in the editor room) render with the scene, double sided
        let n_scene = scene.parts.len();
        for (pi, p) in scene.parts.iter().chain(extra.iter()).enumerate() {
            let is_extra = pi >= n_scene;
            if !p.visible || p.alpha <= 0.001 {
                continue;
            }
            let (key, scale) = match self.meshes.get(&p.mesh) {
                Some(MeshState::Ready(_)) => (p.mesh, p.mesh_scale),
                Some(MeshState::Failed) => (MeshKey::Box, p.fallback_scale),
                _ => continue,
            };
            let radius = p.size.length().max(scale.length()) * 0.5 + p.mesh_offset.length();
            if !frustum.sphere(p.cf.pos, radius) {
                continue;
            }
            let model = p.cf.to_mat4() * Mat4::from_translation(p.mesh_offset);
            let tex = p.texture.as_deref().filter(|t| matches!(self.textures.get(*t), Some(Some(_))));
            let mut flags = p.flags;
            if tex.is_none() {
                flags &= !scene::FLAG_TEXTURED;
            }
            if key == MeshKey::Box && p.mesh != MeshKey::Box {
                flags &= !scene::FLAG_VERTEX_COLOR;
            }
            let inst = InstanceData {
                m: model.to_cols_array_2d(),
                scale_mat: [scale.x, scale.y, scale.z, p.material as f32],
                color: [p.color.x, p.color.y, p.color.z, p.alpha],
                extra: [p.surfaces, flags, p.reflectance.to_bits(), 0],
            };
            if p.alpha < 0.999 || p.material == 1568 {
                transparent.push(((p.cf.pos - eye).length_squared(), key, tex, inst, is_extra));
            } else if is_extra {
                opaque_extra.push((key, tex, inst));
            } else {
                opaque.push((key, tex, inst));
            }
        }
        opaque.sort_by(|a, b| (format!("{:?}", a.0), a.1).cmp(&(format!("{:?}", b.0), b.1)));
        transparent.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let mut instances: Vec<InstanceData> = opaque.iter().map(|o| o.2).collect();
        let topaque_scene = instances.len();
        instances.extend(opaque_extra.iter().map(|o| o.2));
        let topaque = instances.len();
        // mesh of every opaque instance, scene then avatar (the shadow pass casts both)
        let opaque_keys: Vec<MeshKey> = opaque.iter().chain(opaque_extra.iter()).map(|o| o.0).collect();
        instances.extend(transparent.iter().map(|t| t.3));
        let isize = (instances.len() * std::mem::size_of::<InstanceData>()) as u64;
        Self::ensure_buffer(&self.device, &mut self.instance_buf, isize.max(256), wgpu::BufferUsages::VERTEX);
        if !instances.is_empty() {
            self.queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&instances));
        }

        // particles + decals as world quads
        let mut quads: Vec<QuadVertex> = Vec::new();
        let mut quad_draws: Vec<(String, u32, u32, bool)> = Vec::new();
        let right = cam.rot.x_axis;
        for d in &scene.decals {
            let p = &scene.parts[d.part];
            if !p.visible || !matches!(self.textures.get(&d.texture), Some(Some(_))) {
                continue;
            }
            let n = scene::face_normal(d.face);
            let (t1, t2) = face_axes(d.face);
            let half = p.size * 0.5;
            let (e1, e2) = ((t1 * half).length(), (t2 * half).length());
            let c = n * half + n * 0.02;
            let corners = [c - t1 * e1 + t2 * e2, c + t1 * e1 + t2 * e2, c + t1 * e1 - t2 * e2, c - t1 * e1 - t2 * e2];
            let (tu, tv) = match d.tile {
                Some(t) => (e1 * 2.0 / t.x.max(0.01), e2 * 2.0 / t.y.max(0.01)),
                None => (1.0, 1.0),
            };
            let uvs = [[0.0, 0.0], [tu, 0.0], [tu, tv], [0.0, tv]];
            let start = quads.len() as u32;
            for (k, idx) in [0usize, 1, 2, 0, 2, 3].iter().enumerate() {
                let _ = k;
                let wp = p.cf.point(corners[*idx]);
                quads.push(QuadVertex { pos: wp.into(), uv: uvs[*idx], color: [d.color.x, d.color.y, d.color.z, d.alpha * p.alpha], emission: 0.0 });
            }
            quad_draws.push((d.texture.clone(), start, 6, false));
        }
        let mut emitters: Vec<(f32, &scene::Emitter)> = scene
            .emitters
            .iter()
            .filter(|e| !e.particles.is_empty() && matches!(self.textures.get(&e.texture), Some(Some(_))))
            .map(|e| ((scene.parts[e.part].cf.pos - eye).length_squared(), e))
            .collect();
        emitters.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        for (_, e) in emitters {
            let start = quads.len() as u32;
            let mut ps: Vec<&scene::Particle> = e.particles.iter().collect();
            ps.sort_by(|a, b| (b.pos - eye).length_squared().partial_cmp(&(a.pos - eye).length_squared()).unwrap());
            for p in ps {
                let t = p.age / p.life;
                let size = scene::eval_num(&e.size, t);
                let alpha = 1.0 - scene::eval_num(&e.transparency, t);
                let col = scene::eval_color(&e.color, t);
                let to_cam = (eye - p.pos).normalize_or_zero();
                let center = p.pos + to_cam * e.z_offset;
                let (s, c) = p.rot.sin_cos();
                let r = right * c + up * s;
                let u = up * c - right * s;
                let corners = [center - r * size + u * size, center + r * size + u * size, center + r * size - u * size, center - r * size - u * size];
                let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                for idx in [0usize, 1, 2, 0, 2, 3] {
                    quads.push(QuadVertex { pos: corners[idx].into(), uv: uvs[idx], color: [col.x, col.y, col.z, alpha], emission: 1.0 });
                }
            }
            let count = quads.len() as u32 - start;
            quad_draws.push((e.texture.clone(), start, count, e.light_emission > 0.0));
        }
        let qsize = (quads.len() * std::mem::size_of::<QuadVertex>()) as u64;
        Self::ensure_buffer(&self.device, &mut self.quad_buf, qsize.max(256), wgpu::BufferUsages::VERTEX);
        if !quads.is_empty() {
            self.queue.write_buffer(&self.quad_buf, 0, bytemuck::cast_slice(&quads));
        }

        // shadow pass
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if shadows_on {
                pass.set_pipeline(&self.shadow_pipe);
                pass.set_bind_group(0, &self.shadow_bg, &[]);
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
                let mut i = 0;
                while i < topaque {
                    let key = opaque_keys[i];
                    let mut j = i;
                    while j < topaque && opaque_keys[j] == key {
                        j += 1;
                    }
                    if let Some(MeshState::Ready(m)) = self.meshes.get(&key) {
                        pass.set_vertex_buffer(0, m.vb.slice(..));
                        pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..m.count, 0, i as u32..j as u32);
                    }
                    i = j;
                }
            }
        }

        // main pass
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_ms.view,
                    resolve_target: Some(&self.hdr.view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.sky_pipe);
            pass.set_bind_group(0, &self.sky_uniform_bg, &[]);
            pass.set_bind_group(1, self.sky_cache.get(&sky_key).unwrap_or(&self.sky_placeholder), &[]);
            pass.draw(0..3, 0..1);

            pass.set_pipeline(&self.scene_opaque);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            let all_opaque: Vec<(MeshKey, Option<&str>)> = opaque.iter().chain(opaque_extra.iter()).map(|o| (o.0, o.1)).collect();
            let mut i = 0;
            while i < topaque {
                if i == topaque_scene {
                    pass.set_pipeline(&self.scene_opaque_double);
                }
                let (key, tex) = all_opaque[i];
                let end = if i < topaque_scene { topaque_scene } else { topaque };
                let mut j = i;
                while j < end && all_opaque[j].0 == key && all_opaque[j].1 == tex {
                    j += 1;
                }
                if let Some(MeshState::Ready(m)) = self.meshes.get(&key) {
                    let bg = tex.and_then(|t| self.textures.get(t)).and_then(|t| t.as_ref()).unwrap_or(&self.white);
                    pass.set_bind_group(1, bg, &[]);
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.count, 0, i as u32..j as u32);
                }
                i = j;
            }

            if !quads.is_empty() {
                pass.set_vertex_buffer(0, self.quad_buf.slice(..));
                for (tex, start, count, additive) in quad_draws.iter().filter(|q| !q.3) {
                    let _ = additive;
                    if let Some(Some(bg)) = self.textures.get(tex) {
                        pass.set_pipeline(&self.quad_alpha);
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                        pass.set_bind_group(1, bg, &[]);
                        pass.draw(*start..start + count, 0..1);
                    }
                }
            }

            pass.set_pipeline(&self.scene_transparent);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            for (k, t) in transparent.iter().enumerate() {
                pass.set_pipeline(if t.4 { &self.scene_transparent_double } else { &self.scene_transparent });
                if let Some(MeshState::Ready(m)) = self.meshes.get(&t.1) {
                    let bg = t.2.and_then(|x| self.textures.get(x)).and_then(|x| x.as_ref()).unwrap_or(&self.white);
                    pass.set_bind_group(1, bg, &[]);
                    pass.set_vertex_buffer(0, m.vb.slice(..));
                    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                    let idx = (topaque + k) as u32;
                    pass.draw_indexed(0..m.count, 0, idx..idx + 1);
                }
            }

            if !quads.is_empty() {
                pass.set_vertex_buffer(0, self.quad_buf.slice(..));
                for (tex, start, count, _) in quad_draws.iter().filter(|q| q.3) {
                    if let Some(Some(bg)) = self.textures.get(tex) {
                        pass.set_pipeline(&self.quad_add);
                        pass.set_bind_group(0, &self.globals_bg, &[]);
                        pass.set_bind_group(1, bg, &[]);
                        pass.draw(*start..start + count, 0..1);
                    }
                }
            }
        }
    }

    /// The avatar on a transparent background, lit like the catalog renders (key light from the
    /// upper front-left, soft ambient), no sky, fog or shadows.
    fn render_avatar(&mut self, enc: &mut wgpu::CommandEncoder, parts: &[crate::scene::Part], cam: &CFrame) {
        let fov = 30f32.to_radians();
        let view = Mat4::look_to_rh(cam.pos, cam.look_vector(), cam.rot.y_axis);
        let proj = Mat4::perspective_infinite_reverse_rh(fov, 1.0, 0.1);
        let sun = Vec3::new(-0.45, 0.75, -0.5).normalize();
        let globals = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            shadow_mat: Mat4::IDENTITY.to_cols_array_2d(),
            cam_pos: [cam.pos.x, cam.pos.y, cam.pos.z, 0.0],
            sun_dir: [sun.x, sun.y, sun.z, 1.1],
            ambient: [0.5, 0.5, 0.52, 0.0],
            outdoor: [0.45, 0.45, 0.45, 0.0],
            shift_top: [0.0; 4],
            shift_bottom: [0.0; 4],
            fog_color: [0.0, 0.0, 0.0, 1.0],
            fog: [100000.0, 200000.0, 1.0, 1.0 / SHADOW_SIZE as f32],
            lights: [GpuLight::zeroed(); MAX_LIGHTS],
        };
        self.queue.write_buffer(&self.avatar.globals_buf, 0, bytemuck::bytes_of(&globals));
        let mut draws: Vec<(MeshKey, Option<&str>, InstanceData)> = Vec::new();
        for p in parts.iter().filter(|p| p.visible) {
            if !matches!(self.meshes.get(&p.mesh), Some(MeshState::Ready(_))) {
                continue;
            }
            let tex = p.texture.as_deref().filter(|t| matches!(self.textures.get(*t), Some(Some(_))));
            let mut flags = p.flags;
            if tex.is_none() {
                flags &= !scene::FLAG_TEXTURED;
            }
            let model = p.cf.to_mat4() * Mat4::from_translation(p.mesh_offset);
            draws.push((
                p.mesh,
                tex,
                InstanceData {
                    m: model.to_cols_array_2d(),
                    scale_mat: [p.mesh_scale.x, p.mesh_scale.y, p.mesh_scale.z, p.material as f32],
                    color: [p.color.x, p.color.y, p.color.z, p.alpha],
                    extra: [0, flags, 0f32.to_bits(), 0],
                },
            ));
        }
        let instances: Vec<InstanceData> = draws.iter().map(|d| d.2).collect();
        let size = (instances.len() * std::mem::size_of::<InstanceData>()) as u64;
        Self::ensure_buffer(&self.device, &mut self.avatar.instance_buf, size.max(256), wgpu::BufferUsages::VERTEX);
        if !instances.is_empty() {
            self.queue.write_buffer(&self.avatar.instance_buf, 0, bytemuck::cast_slice(&instances));
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("avatar"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.avatar.color_ms.view,
                resolve_target: Some(&self.avatar.color.view),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.avatar.depth.view,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.scene_opaque);
        pass.set_bind_group(0, &self.avatar.globals_bg, &[]);
        pass.set_vertex_buffer(1, self.avatar.instance_buf.slice(..));
        for (i, (key, tex, _)) in draws.iter().enumerate() {
            if let Some(MeshState::Ready(m)) = self.meshes.get(key) {
                let bg = tex.and_then(|t| self.textures.get(t)).and_then(|t| t.as_ref()).unwrap_or(&self.white);
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, m.vb.slice(..));
                pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.count, 0, i as u32..i as u32 + 1);
            }
        }
    }

    fn post_pass(&self, enc: &mut wgpu::CommandEncoder, pipe: &wgpu::RenderPipeline, src: &Target, dst: &Target, buf_idx: usize, params: PostParams) {
        self.queue.write_buffer(&self.post_bufs[buf_idx], 0, bytemuck::bytes_of(&params));
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.post_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.linear_clamp) },
                wgpu::BindGroupEntry { binding: 2, resource: self.post_bufs[buf_idx].as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("post"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &dst.view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(pipe);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..3, 0..1);
    }

    fn render_post(&mut self, enc: &mut wgpu::CommandEncoder, out: &wgpu::TextureView, fx: &PostFx) {
        let h = self.config.height as f32;
        let p = |src: &Target, dir: [f32; 2], sigma: f32, threshold: f32| PostParams {
            texel: [1.0 / src.w as f32, 1.0 / src.h as f32],
            dir,
            sigma,
            threshold,
            _p: [0.0; 2],
        };
        // BlurEffect: Size is roughly a pixel radius at 1080p
        let sigma_full = fx.blur * 0.55 * (h / 1080.0);
        let sigma_half = sigma_full * 0.5;
        self.post_pass(enc, &self.down_pipe, &self.hdr, &self.half_a, 0, p(&self.hdr, [0.0, 0.0], 0.0, 0.0));
        if sigma_half > 0.3 {
            self.post_pass(enc, &self.blur_pipe, &self.half_a, &self.half_b, 1, p(&self.half_a, [1.0, 0.0], sigma_half, 0.0));
            self.post_pass(enc, &self.blur_pipe, &self.half_b, &self.half_a, 2, p(&self.half_b, [0.0, 1.0], sigma_half, 0.0));
        }
        // bloom from bright (neon / lights)
        self.post_pass(enc, &self.down_pipe, &self.half_a, &self.quarter_a, 3, p(&self.half_a, [0.0, 0.0], 0.0, 1.05));
        self.post_pass(enc, &self.blur_pipe, &self.quarter_a, &self.quarter_b, 4, p(&self.quarter_a, [1.0, 0.0], 4.0, 0.0));
        self.post_pass(enc, &self.blur_pipe, &self.quarter_b, &self.quarter_a, 5, p(&self.quarter_b, [0.0, 1.0], 4.0, 0.0));

        let cc = CcParams {
            brightness: fx.brightness,
            contrast: fx.contrast,
            saturation: fx.saturation,
            blur_mix: if sigma_half > 0.3 { (sigma_half / 1.5).min(1.0) } else { 0.0 },
            tint: fx.tint.extend(1.0).into(),
            bloom: [fx.bloom, fx.clear, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.cc_buf, 0, bytemuck::bytes_of(&cc));
        self.queue.write_buffer(&self.post_bufs[6], 0, bytemuck::bytes_of(&p(&self.hdr, [0.0, 0.0], 0.0, 0.0)));
        let bg0 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.post_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.hdr.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.linear_clamp) },
                wgpu::BindGroupEntry { binding: 2, resource: self.post_bufs[6].as_entire_binding() },
            ],
        });
        let bg1 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.comp_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.hdr.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.half_a.view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.quarter_a.view) },
                wgpu::BindGroupEntry { binding: 3, resource: self.cc_buf.as_entire_binding() },
            ],
        });
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.comp_pipe);
        pass.set_bind_group(0, &bg0, &[]);
        pass.set_bind_group(1, &bg1, &[]);
        pass.draw(0..3, 0..1);
    }

    fn render_ui(&mut self, enc: &mut wgpu::CommandEncoder, out: &wgpu::TextureView, ui: &UiFrame) {
        if ui.vertices.is_empty() {
            return;
        }
        let (w, h) = (self.config.width, self.config.height);
        self.queue.write_buffer(&self.ui_ubuf, 0, bytemuck::cast_slice(&[w as f32, h as f32, 0.0, 0.0]));
        let size = (ui.vertices.len() * std::mem::size_of::<UiVertex>()) as u64;
        Self::ensure_buffer(&self.device, &mut self.ui_vb, size, wgpu::BufferUsages::VERTEX);
        self.queue.write_buffer(&self.ui_vb, 0, bytemuck::cast_slice(&ui.vertices));
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: out,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.ui_pipe);
        pass.set_bind_group(0, &self.ui_ubg, &[]);
        pass.set_vertex_buffer(0, self.ui_vb.slice(..));
        for b in &ui.batches {
            let Some(t) = self.ui_textures.get(b.texture) else { continue };
            match b.clip {
                Some([x, y, cw, ch]) => {
                    let x0 = x.clamp(0.0, w as f32) as u32;
                    let y0 = y.clamp(0.0, h as f32) as u32;
                    let x1 = (x + cw).clamp(0.0, w as f32) as u32;
                    let y1 = (y + ch).clamp(0.0, h as f32) as u32;
                    if x1 <= x0 || y1 <= y0 {
                        continue;
                    }
                    pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
                }
                None => pass.set_scissor_rect(0, 0, w, h),
            }
            pass.set_bind_group(1, &t.bind, &[]);
            pass.draw(b.start..b.start + b.count, 0..1);
        }
        let _ = &self.ui_buf;
    }
}

fn face_axes(face: u32) -> (Vec3, Vec3) {
    // (right, up) as seen looking at the face from outside
    match face {
        0 => (-Vec3::Z, Vec3::Y),
        1 => (Vec3::X, -Vec3::Z),
        2 => (Vec3::X, Vec3::Y),
        3 => (Vec3::Z, Vec3::Y),
        4 => (Vec3::X, Vec3::Z),
        _ => (-Vec3::X, Vec3::Y),
    }
}

fn make_sky_bg(device: &wgpu::Device, queue: &wgpu::Queue, layout: &wgpu::BindGroupLayout, smp: &wgpu::Sampler, size: u32, layers: &[u8]) -> wgpu::BindGroup {
    let tex = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some("sky"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 6 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        Default::default(),
        layers,
    );
    let view = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(smp) }],
    })
}

struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    fn new(m: &Mat4) -> Self {
        let r = m.transpose();
        let (r0, r1, r2, r3) = (r.x_axis, r.y_axis, r.z_axis, r.w_axis);
        let norm = |p: Vec4| p / p.truncate().length();
        // reverse-z infinite: skip far plane; near is r3 - r2
        Frustum { planes: [norm(r3 + r0), norm(r3 - r0), norm(r3 + r1), norm(r3 - r1), norm(r3 - r2)] }
    }
    fn sphere(&self, c: Vec3, r: f32) -> bool {
        self.planes.iter().all(|p| p.truncate().dot(c) + p.w >= -r)
    }
}

// >> end << //
