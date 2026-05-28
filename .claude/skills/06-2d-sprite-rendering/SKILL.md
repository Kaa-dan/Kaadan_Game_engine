# 06 — 2D Sprite Rendering

## Description
Texture loading (PNG/JPEG), texture atlases, sprite batching, 2D camera with orthographic projection. Integrates with ECS so `Sprite` + `Transform` entities auto-render via a sprite batch system.

## Phase
3 — ECS & Sprites

## Prerequisites
- Skill 04 (`04-renderer-core`) — wgpu device, pipeline, vertex buffers
- Skill 05 (`05-ecs-world`) — World, Entity, system scheduling

## Complexity
Medium — sprite batching is the critical performance technique

## Architecture Decisions

### Why sprite batching?
- Mobile GPUs are draw-call limited (~100-500 draws per frame at 60fps)
- Batching collects all sprites sharing a texture into ONE draw call
- 1000 sprites with 4 textures = 4 draw calls instead of 1000
- This is the single most important mobile rendering optimization

### Rendering pipeline for 2D
```
Sprite + Transform components
    → SpriteBatch system collects and sorts
    → Groups by texture
    → Builds vertex buffer (4 verts per sprite, instancing or quad expansion)
    → One draw call per texture group
    → Orthographic camera projection
```

### Texture atlas strategy
- Packing many small sprites into one large atlas reduces texture switches
- Atlas regions defined by name → UV rect mapping
- Can be generated offline or at load time

## Step-by-Step Implementation

### 1. Texture Abstraction

```rust
// crates/kaadan_renderer/src/texture.rs
pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

impl Texture {
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self, kaadan_core::KaadanError> {
        let img = image::load_from_memory(bytes)
            .map_err(|e| kaadan_core::KaadanError::AssetLoad {
                path: label.to_string(),
                reason: e.to_string(),
            })?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1, // Add mipmap generation later
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest, // Pixel art friendly
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self { texture, view, sampler, width, height })
    }
}
```

### 2. Texture Atlas

```rust
// crates/kaadan_renderer/src/atlas.rs
use kaadan_math::Rect;
use std::collections::HashMap;

/// A region within a texture atlas, in UV coordinates (0.0–1.0).
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    pub uv_min: kaadan_math::Vec2,
    pub uv_max: kaadan_math::Vec2,
    pub pixel_size: kaadan_math::Vec2,
}

/// Maps named sprite regions to UV coordinates within a texture.
pub struct TextureAtlas {
    pub texture_handle: kaadan_math::Handle<Texture>,
    regions: HashMap<String, AtlasRegion>,
}

impl TextureAtlas {
    pub fn new(texture_handle: kaadan_math::Handle<Texture>) -> Self {
        Self { texture_handle, regions: HashMap::new() }
    }

    /// Add a region defined in pixel coordinates.
    pub fn add_region(&mut self, name: impl Into<String>, x: u32, y: u32, w: u32, h: u32, tex_width: u32, tex_height: u32) {
        let tw = tex_width as f32;
        let th = tex_height as f32;
        self.regions.insert(name.into(), AtlasRegion {
            uv_min: kaadan_math::Vec2::new(x as f32 / tw, y as f32 / th),
            uv_max: kaadan_math::Vec2::new((x + w) as f32 / tw, (y + h) as f32 / th),
            pixel_size: kaadan_math::Vec2::new(w as f32, h as f32),
        });
    }

    pub fn get(&self, name: &str) -> Option<&AtlasRegion> {
        self.regions.get(name)
    }
}
```

### 3. Sprite Component

```rust
// crates/kaadan_renderer/src/sprite.rs

/// Component: a 2D sprite attached to an entity.
pub struct Sprite {
    /// Handle to the texture (or atlas texture)
    pub texture: kaadan_math::Handle<Texture>,
    /// UV region within the texture (full texture if None)
    pub region: Option<AtlasRegion>,
    /// Tint color (multiplied with texture)
    pub color: kaadan_math::Color,
    /// Sprite size in world units (if None, uses texture pixel size)
    pub size: Option<kaadan_math::Vec2>,
    /// Anchor/pivot point (0,0 = bottom-left, 0.5,0.5 = center)
    pub anchor: kaadan_math::Vec2,
    /// Draw order — higher values render on top
    pub z_order: i32,
    /// Flip horizontally
    pub flip_x: bool,
    /// Flip vertically
    pub flip_y: bool,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            texture: unsafe { std::mem::zeroed() }, // Must be set
            region: None,
            color: kaadan_math::Color::WHITE,
            size: None,
            anchor: kaadan_math::Vec2::new(0.5, 0.5), // Center
            z_order: 0,
            flip_x: false,
            flip_y: false,
        }
    }
}
```

### 4. Camera2D

```rust
// crates/kaadan_renderer/src/camera2d.rs
use kaadan_math::{Mat4, Vec2, Vec3};

/// 2D orthographic camera.
pub struct Camera2D {
    pub position: Vec2,
    pub zoom: f32,
    pub viewport_size: Vec2,
}

impl Camera2D {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            viewport_size: Vec2::new(viewport_width, viewport_height),
        }
    }

    /// Orthographic projection matrix.
    /// Origin at center, Y-up, zoom affects visible area.
    pub fn projection_matrix(&self) -> Mat4 {
        let half_w = self.viewport_size.x / (2.0 * self.zoom);
        let half_h = self.viewport_size.y / (2.0 * self.zoom);
        Mat4::orthographic_rh(
            -half_w, half_w,
            -half_h, half_h,
            -1000.0, 1000.0,
        )
    }

    /// View matrix (inverse camera transform).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(
            Vec3::new(self.position.x, self.position.y, 1.0),
            Vec3::new(self.position.x, self.position.y, 0.0),
            Vec3::Y,
        )
    }

    /// Combined view-projection matrix.
    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Convert screen coordinates to world coordinates.
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        let ndc_x = (screen_pos.x / self.viewport_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / self.viewport_size.y) * 2.0;
        Vec2::new(
            ndc_x * self.viewport_size.x / (2.0 * self.zoom) + self.position.x,
            ndc_y * self.viewport_size.y / (2.0 * self.zoom) + self.position.y,
        )
    }
}
```

### 5. SpriteBatch System

```rust
// crates/kaadan_renderer/src/sprite_batch.rs

/// Collects Sprite + Transform entities, sorts by z_order and texture,
/// builds vertex buffers, and issues draw calls.
pub struct SpriteBatch {
    vertices: Vec<SpriteVertex>,
    indices: Vec<u32>,
    // Groups sorted by texture handle for batching
    draw_calls: Vec<DrawCall>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteVertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

struct DrawCall {
    texture_handle: kaadan_math::Handle<Texture>,
    index_start: u32,
    index_count: u32,
}

impl SpriteBatch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            draw_calls: Vec::new(),
        }
    }

    /// Collect all Sprite + Transform entities from the world.
    /// Sort by z_order then by texture to minimize state changes.
    pub fn collect(&mut self, world: &kaadan_ecs::World, camera: &Camera2D) {
        self.vertices.clear();
        self.indices.clear();
        self.draw_calls.clear();

        // 1. Collect sprite data into a sortable vec
        let mut sprites: Vec<_> = world
            .query::<(&Sprite, &kaadan_math::Transform)>()
            .iter()
            .map(|(_, (sprite, transform))| (sprite, transform))
            .collect();

        // 2. Sort by z_order, then by texture handle (for batching)
        sprites.sort_by(|a, b| {
            a.0.z_order.cmp(&b.0.z_order)
                .then_with(|| a.0.texture.index().cmp(&b.0.texture.index()))
        });

        // 3. Build vertex/index buffers, grouping by texture
        let vp = camera.view_projection();
        // ... build quads for each sprite, track draw call boundaries
    }
}
```

### 6. Sprite Shader (WGSL)

```wgsl
// assets/shaders/sprite.wgsl
struct CameraUniform {
    view_projection: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_projection * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(t_diffuse, s_diffuse, in.uv);
    return tex_color * in.color;
}
```

## Deliverables Checklist

- [ ] `Texture` loading PNG/JPEG into `wgpu::Texture`
- [ ] `TextureAtlas` with named regions and UV mapping
- [ ] `Sprite` component with texture, color, size, anchor, z_order, flip
- [ ] `SpriteBatch` collecting all sprites into minimal draw calls
- [ ] `Camera2D` with orthographic projection, position, zoom
- [ ] Screen-to-world coordinate conversion
- [ ] Sprite WGSL shader with camera uniform and texture sampling
- [ ] Demo: 1000+ animated sprites at 60fps on desktop
- [ ] Draw order sorting (z_order, then texture grouping)

## Common Pitfalls

1. **Texture format must match** — If you load an sRGB PNG but create a `Rgba8Unorm` (non-sRGB) texture, colors will look wrong. Use `Rgba8UnormSrgb` for color textures.

2. **Y-axis convention** — Screen Y goes down, but world Y typically goes up. The camera projection handles this, but be consistent.

3. **Sprite anchor affects position** — A center-anchored sprite at (0,0) is centered there. A bottom-left anchored sprite at (0,0) has its bottom-left corner there. Document and be consistent.

4. **Batch breaking** — Every texture switch is a new draw call. Sort by texture to minimize switches. Using a texture atlas eliminates switches entirely for sprites in the same atlas.

5. **Dynamic vertex buffer** — Rebuild the vertex buffer each frame (sprites move). Use `queue.write_buffer()` instead of creating a new buffer each frame to avoid allocation churn.

6. **Index buffer indices** — Each sprite quad is 4 vertices, 6 indices (two triangles). Index pattern: `[0,1,2, 2,3,0]` offset by `sprite_index * 4`.

## References

- [wgpu texture tutorial](https://sotrh.github.io/learn-wgpu/beginner/tutorial5-textures/)
- [image crate docs](https://docs.rs/image/latest/image/)
- [Sprite batching explained](https://www.gamedev.net/tutorials/programming/graphics/2d-sprite-batching-in-opengl-r3900/)
- [Orthographic projection](https://learnopengl.com/Getting-started/Coordinate-Systems)
