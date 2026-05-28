# 04 — Renderer Core

## Description
Build `kaadan_renderer` — a wgpu-based rendering backend with device/surface init, render loop, vertex/index buffers, WGSL shader compilation, and basic render pipelines. Clears to a color and renders a triangle.

## Phase
2 — Platform & Pixels

## Prerequisites
- Skill 03 (`03-platform-abstraction`) — needs `raw_window_handle` from `PlatformWindow`

## Complexity
High — GPU programming with wgpu, shader writing in WGSL

## Architecture Decisions

### Why wgpu?
- Abstracts Vulkan (Android), Metal (iOS), DX12 (Windows) behind one safe Rust API
- Naga shader compiler handles WGSL → SPIR-V (Vulkan) / MSL (Metal) / HLSL (DX12) automatically
- No unsafe code needed for GPU access
- Same API works on desktop and mobile without `#[cfg]` in rendering code

### Why WGSL instead of GLSL/SPIR-V?
- WGSL is wgpu's native shader language — no external toolchain needed
- Naga compiles WGSL to all backend shader formats at runtime
- GLSL requires `naga` feature flags or external `glslc` compilation step

### Renderer architecture
- `Renderer` struct owns the wgpu device, queue, surface
- Render passes are built per-frame (wgpu is stateless — no persistent render state)
- Vertex layout uses `bytemuck` for zero-copy GPU uploads
- Pipeline caching will be added later; for now, pipelines are created at init

## Step-by-Step Implementation

### 1. Crate Setup

```toml
# crates/kaadan_renderer/Cargo.toml
[package]
name = "kaadan_renderer"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
wgpu = { workspace = true }
pollster = { workspace = true }
bytemuck = { workspace = true }
tracing = { workspace = true }
raw-window-handle = { workspace = true }
```

### 2. Renderer Initialization

```rust
// crates/kaadan_renderer/src/renderer.rs
use wgpu::*;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub struct Renderer {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    pub depth_texture: Texture,
    pub depth_view: TextureView,
}

impl Renderer {
    /// Create renderer from a platform window.
    /// Uses pollster to block on async wgpu init (acceptable at startup).
    pub fn new(
        window: &(impl HasWindowHandle + HasDisplayHandle),
        width: u32,
        height: u32,
    ) -> Result<Self, kaadan_core::KaadanError> {
        pollster::block_on(Self::new_async(window, width, height))
    }

    async fn new_async(
        window: &(impl HasWindowHandle + HasDisplayHandle),
        width: u32,
        height: u32,
    ) -> Result<Self, kaadan_core::KaadanError> {
        // 1. Create instance with all backends
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        // 2. Create surface from window handle
        let surface = instance.create_surface(/* window */)
            .map_err(|e| kaadan_core::KaadanError::Renderer(e.to_string()))?;

        // 3. Request adapter (GPU) — prefer high-performance
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| kaadan_core::KaadanError::Renderer("No GPU adapter found".into()))?;

        tracing::info!("GPU adapter: {:?}", adapter.get_info());

        // 4. Request device with mobile-friendly limits
        let (device, queue) = adapter
            .request_device(&DeviceDescriptor {
                label: Some("kaadan_device"),
                required_features: Features::empty(),
                required_limits: Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            }, None)
            .await
            .map_err(|e| kaadan_core::KaadanError::Renderer(e.to_string()))?;

        // 5. Configure surface
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let surface_config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // 6. Create depth texture
        let (depth_texture, depth_view) = Self::create_depth_texture(&device, width, height);

        Ok(Self {
            device, queue, surface, surface_config,
            depth_texture, depth_view,
        })
    }

    fn create_depth_texture(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let size = Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("depth_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            let (tex, view) = Self::create_depth_texture(&self.device, width, height);
            self.depth_texture = tex;
            self.depth_view = view;
        }
    }

    pub fn surface_format(&self) -> TextureFormat {
        self.surface_config.format
    }
}
```

### 3. Vertex Type

```rust
// crates/kaadan_renderer/src/vertex.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl Vertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            // position
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            // color
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x4,
            },
            // uv
            wgpu::VertexAttribute {
                offset: 28,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x2,
            },
        ],
    };
}
```

### 4. Mesh Abstraction

```rust
// crates/kaadan_renderer/src/mesh.rs
use wgpu::*;

pub struct Mesh {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
}

impl Mesh {
    pub fn new(device: &Device, vertices: &[Vertex], indices: &[u16]) -> Self {
        let vertex_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: BufferUsages::INDEX,
        });
        Self { vertex_buffer, index_buffer, index_count: indices.len() as u32 }
    }
}
```

### 5. WGSL Shader

```wgsl
// assets/shaders/basic.wgsl
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
```

### 6. Render Pipeline

```rust
// crates/kaadan_renderer/src/pipeline.rs
pub fn create_basic_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    shader_source: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("basic_shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("basic_pipeline_layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("basic_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Vertex::LAYOUT],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
```

### 7. Frame Rendering

```rust
// crates/kaadan_renderer/src/frame.rs
impl Renderer {
    /// Begin a frame: acquire surface texture, create command encoder.
    pub fn begin_frame(&self) -> Result<FrameContext, wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_encoder"),
        });
        Ok(FrameContext { output, view, encoder })
    }

    /// Submit commands and present.
    pub fn end_frame(&self, frame: FrameContext) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.output.present();
    }
}

pub struct FrameContext {
    pub output: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}
```

## Deliverables Checklist

- [ ] `Renderer` struct wrapping `wgpu::Device`, `Queue`, `Surface`
- [ ] Clear-color render loop at 60fps on desktop
- [ ] `Vertex` struct (position + color + UV) with `bytemuck` derive
- [ ] `Mesh` abstraction with vertex + index buffer upload
- [ ] WGSL vertex + fragment shader rendering a colored triangle
- [ ] `RenderPipeline` with depth/stencil and alpha blending
- [ ] `begin_frame()` / `end_frame()` lifecycle
- [ ] Resize handling (recreate surface + depth texture)
- [ ] `cargo build` succeeds

## Common Pitfalls

1. **`Limits::downlevel_webgl2_defaults()`** — Use this instead of `Limits::default()` for mobile compatibility. Default limits request features not available on all mobile GPUs.

2. **Surface format must be sRGB** — Prefer `TextureFormat::Bgra8UnormSrgb` or similar. If you use a non-sRGB format, colors will look wrong because the GPU won't do gamma correction on output.

3. **`SurfaceError::Lost`** — On Android, the surface is lost when the app goes to background. Handle this by reconfiguring: `surface.configure(&device, &config)`.

4. **`SurfaceError::OutOfMemory`** — On mobile, this is a real concern. Don't allocate massive textures. Start with reasonable sizes.

5. **Shader compilation errors are runtime errors** — WGSL is compiled by Naga at `create_shader_module()` time. Unlike compiled shaders, bad WGSL will panic at runtime. Test shaders thoroughly.

6. **Don't forget `bytemuck::cast_slice`** — Vertex and index data must be cast to `&[u8]` for buffer creation. `bytemuck` does this safely for `Pod` types.

7. **Depth buffer format** — Use `Depth32Float` for simplicity. `Depth24PlusStencil8` saves memory but isn't available on all mobile GPUs without checking capabilities first.

8. **Present mode** — Use `PresentMode::AutoVsync` for mobile to avoid battery drain. `Fifo` (strict vsync) is the safe fallback if auto isn't supported.

## References

- [wgpu docs](https://docs.rs/wgpu/latest/wgpu/)
- [Learn wgpu tutorial](https://sotrh.github.io/learn-wgpu/)
- [WGSL specification](https://www.w3.org/TR/WGSL/)
- [wgpu examples](https://github.com/gfx-rs/wgpu/tree/trunk/examples)
- [bytemuck docs](https://docs.rs/bytemuck/latest/bytemuck/)
