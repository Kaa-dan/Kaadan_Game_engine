# 11 — 3D Rendering

## Description
Perspective camera, glTF model loading, mesh rendering with PBR materials, directional and point lighting, depth buffer. Extends the existing wgpu renderer and asset system for 3D content.

## Phase
5 — Physics & 3D

## Prerequisites
- Skill 04 (`04-renderer-core`) — wgpu device, pipelines, vertex buffers
- Skill 05 (`05-ecs-world`) — ECS World, system scheduling
- Skill 08 (`08-asset-pipeline`) — AssetServer for loading models/textures

## Complexity
High — PBR shading, glTF parsing, and 3D math are involved

## Architecture Decisions

### Why glTF?
- "JPEG of 3D" — the standard interchange format
- Supports meshes, materials (PBR), textures, animations, scene hierarchy
- Well-supported in Rust via the `gltf` crate
- Every 3D modeling tool (Blender, Maya, etc.) exports glTF

### PBR (Physically Based Rendering) basics
- **Albedo** (base color): the surface color
- **Metallic**: 0 = dielectric (plastic), 1 = metal
- **Roughness**: 0 = mirror smooth, 1 = completely rough
- **Normal map**: per-pixel surface detail without extra geometry
- This is the minimum viable PBR — matches glTF's material model

### Renderer architecture for 2D + 3D
- The renderer supports both 2D (sprites, UI) and 3D (meshes) in the same frame
- 3D renders first with depth testing
- 2D/UI renders on top with depth test disabled (overlay)
- Camera components determine which pipeline is used: `Camera2D` or `Camera3D`

## Step-by-Step Implementation

### 1. Camera3D

```rust
// crates/kaadan_renderer/src/camera3d.rs
use kaadan_math::{Mat4, Vec3};

/// 3D perspective camera.
pub struct Camera3D {
    pub position: Vec3,
    pub target: Vec3,        // Look-at target
    pub up: Vec3,
    pub fov_y: f32,          // Vertical field of view in radians
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera3D {
    pub fn new(aspect_ratio: f32) -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov_y: std::f32::consts::FRAC_PI_4, // 45 degrees
            aspect_ratio,
            near: 0.1,
            far: 1000.0,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position, self.target, self.up)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect_ratio, self.near, self.far)
    }

    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Simple orbit: rotate around target at fixed distance.
    pub fn orbit(&mut self, yaw: f32, pitch: f32) {
        let offset = self.position - self.target;
        let distance = offset.length();

        // Spherical coordinates
        let current_yaw = offset.z.atan2(offset.x);
        let current_pitch = (offset.y / distance).asin();

        let new_yaw = current_yaw + yaw;
        let new_pitch = (current_pitch + pitch).clamp(-1.4, 1.4); // Clamp to avoid gimbal lock

        self.position = self.target + Vec3::new(
            distance * new_pitch.cos() * new_yaw.cos(),
            distance * new_pitch.sin(),
            distance * new_pitch.cos() * new_yaw.sin(),
        );
    }
}
```

### 2. 3D Vertex Format

```rust
// crates/kaadan_renderer/src/vertex3d.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 4], // For normal mapping (xyz = tangent, w = handedness)
}

impl Vertex3D {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex3D>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },  // position
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 }, // normal
            wgpu::VertexAttribute { offset: 24, shader_location: 2, format: wgpu::VertexFormat::Float32x2 }, // uv
            wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 }, // tangent
        ],
    };
}
```

### 3. PBR Material

```rust
// crates/kaadan_renderer/src/material.rs
use kaadan_math::{Color, Handle};

/// PBR material matching glTF's metallic-roughness model.
pub struct PbrMaterial {
    pub albedo_color: Color,
    pub albedo_texture: Option<Handle<Texture>>,
    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: Option<Handle<Texture>>,
    pub normal_texture: Option<Handle<Texture>>,
    pub normal_scale: f32,
    pub emissive_color: Color,
    pub emissive_texture: Option<Handle<Texture>>,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

impl Default for PbrMaterial {
    fn default() -> Self {
        Self {
            albedo_color: Color::WHITE,
            albedo_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            emissive_color: Color::BLACK,
            emissive_texture: None,
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

/// GPU-side material uniform.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub albedo_color: [f32; 4],
    pub emissive_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub normal_scale: f32,
    pub alpha_cutoff: f32,
}
```

### 4. Lighting

```rust
// crates/kaadan_renderer/src/light.rs

/// Component: directional light (sun).
pub struct DirectionalLight {
    pub direction: kaadan_math::Vec3,
    pub color: kaadan_math::Color,
    pub intensity: f32,
}

/// Component: point light.
pub struct PointLight {
    pub color: kaadan_math::Color,
    pub intensity: f32,
    pub range: f32,
}

/// GPU uniform for lights — passed to the PBR shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub direction: [f32; 4],        // w unused
    pub color: [f32; 4],            // rgb + intensity in w
    pub camera_position: [f32; 4],  // w unused
    // Point lights would be an array
    pub point_lights: [PointLightData; 4],
    pub num_point_lights: u32,
    pub _padding: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointLightData {
    pub position: [f32; 4],  // xyz + range in w
    pub color: [f32; 4],     // rgb + intensity in w
}
```

### 5. PBR Shader (WGSL)

```wgsl
// assets/shaders/pbr.wgsl

struct CameraUniform {
    view_projection: mat4x4<f32>,
    view: mat4x4<f32>,
    position: vec4<f32>,
};

struct ModelUniform {
    model: mat4x4<f32>,
    normal_matrix: mat4x4<f32>, // transpose(inverse(model))
};

struct MaterialUniform {
    albedo_color: vec4<f32>,
    emissive_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    normal_scale: f32,
    alpha_cutoff: f32,
};

struct LightUniform {
    direction: vec4<f32>,
    color: vec4<f32>,
    camera_position: vec4<f32>,
    // ... point lights
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model: ModelUniform;
@group(2) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(1) var t_albedo: texture_2d<f32>;
@group(2) @binding(2) var s_albedo: sampler;
@group(3) @binding(0) var<uniform> lights: LightUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) tangent: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = model.model * vec4<f32>(position, 1.0);
    out.clip_position = camera.view_projection * world_pos;
    out.world_position = world_pos.xyz;
    out.world_normal = (model.normal_matrix * vec4<f32>(normal, 0.0)).xyz;
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample albedo
    let albedo = textureSample(t_albedo, s_albedo, in.uv) * material.albedo_color;

    // Simplified PBR: Lambertian diffuse + Blinn-Phong specular
    let N = normalize(in.world_normal);
    let L = normalize(-lights.direction.xyz);
    let V = normalize(lights.camera_position.xyz - in.world_position);
    let H = normalize(L + V);

    // Diffuse
    let NdotL = max(dot(N, L), 0.0);
    let diffuse = albedo.rgb * NdotL * lights.color.rgb * lights.color.w;

    // Specular (roughness-based)
    let shininess = mix(256.0, 1.0, material.roughness);
    let NdotH = max(dot(N, H), 0.0);
    let specular = pow(NdotH, shininess) * mix(0.04, 1.0, material.metallic) * lights.color.rgb * lights.color.w;

    // Ambient
    let ambient = albedo.rgb * 0.03;

    // Emissive
    let emissive = material.emissive_color.rgb;

    let color = ambient + diffuse + specular + emissive;
    return vec4<f32>(color, albedo.a);
}
```

### 6. glTF Loader

```rust
// crates/kaadan_renderer/src/gltf_loader.rs
use gltf::Gltf;

pub struct GltfModel {
    pub meshes: Vec<LoadedMesh>,
    pub materials: Vec<PbrMaterial>,
}

pub struct LoadedMesh {
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
}

pub fn load_gltf(bytes: &[u8], base_path: &str) -> Result<GltfModel, kaadan_core::KaadanError> {
    let gltf = Gltf::from_slice(bytes)
        .map_err(|e| kaadan_core::KaadanError::AssetLoad {
            path: base_path.to_string(),
            reason: e.to_string(),
        })?;

    let mut meshes = Vec::new();
    let mut materials = Vec::new();

    // Extract buffer data
    let blob = gltf.blob.as_deref();

    // Load materials
    for mat in gltf.materials() {
        let pbr = mat.pbr_metallic_roughness();
        materials.push(PbrMaterial {
            albedo_color: Color::new(
                pbr.base_color_factor()[0],
                pbr.base_color_factor()[1],
                pbr.base_color_factor()[2],
                pbr.base_color_factor()[3],
            ),
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            double_sided: mat.double_sided(),
            ..Default::default()
        });
    }

    // Load meshes
    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| {
                match buffer.source() {
                    gltf::buffer::Source::Bin => blob,
                    _ => None,
                }
            });

            let positions: Vec<_> = reader.read_positions().unwrap().collect();
            let normals: Vec<_> = reader.read_normals().unwrap_or_default().collect();
            let uvs: Vec<_> = reader.read_tex_coords(0)
                .map(|r| r.into_f32().collect())
                .unwrap_or_default();
            let indices: Vec<u32> = reader.read_indices()
                .map(|r| r.into_u32().collect())
                .unwrap_or_default();

            let vertices: Vec<Vertex3D> = positions.iter().enumerate().map(|(i, pos)| {
                Vertex3D {
                    position: *pos,
                    normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                    uv: uvs.get(i).copied().unwrap_or([0.0, 0.0]),
                    tangent: [1.0, 0.0, 0.0, 1.0],
                }
            }).collect();

            meshes.push(LoadedMesh {
                vertices,
                indices,
                material_index: primitive.material().index(),
            });
        }
    }

    Ok(GltfModel { meshes, materials })
}
```

### 7. 3D Render System

```rust
// System that renders all entities with Mesh3D + Transform + PbrMaterial
pub fn render_3d_system(world: &kaadan_ecs::World, renderer: &Renderer, camera: &Camera3D) {
    // 1. Update camera uniform buffer
    // 2. For each entity with (Mesh3D, Transform, PbrMaterial):
    //    a. Update model uniform (model matrix, normal matrix)
    //    b. Update material uniform
    //    c. Bind textures
    //    d. Draw mesh
}
```

## Deliverables Checklist

- [ ] `Camera3D` with perspective projection, look-at, orbit controls
- [ ] `Vertex3D` format with position, normal, UV, tangent
- [ ] `PbrMaterial` struct matching glTF's metallic-roughness model
- [ ] `DirectionalLight` and `PointLight` components
- [ ] PBR shader in WGSL with diffuse + specular + ambient + emissive
- [ ] glTF loader producing meshes + materials through AssetServer
- [ ] 3D render pipeline with depth testing
- [ ] Model + normal matrix uniform handling
- [ ] Demo: lit 3D scene with glTF model and orbiting camera
- [ ] Coexistence with 2D: render 3D scene then overlay 2D UI

## Common Pitfalls

1. **Normal matrix is NOT the model matrix** — It's `transpose(inverse(model))`. Using the model matrix for normals gives wrong lighting when objects are scaled non-uniformly.

2. **wgpu coordinate system** — wgpu uses a clip space where Y is up and Z is 0–1 (not -1 to 1 like OpenGL). `glam::Mat4::perspective_rh` handles this correctly.

3. **glTF uses right-handed Y-up coordinates** — Same as wgpu/Vulkan, so no conversion needed. But some models from Unity (left-handed) may need axis swapping.

4. **Texture formats in glTF** — Normal maps should be `Rgba8Unorm` (NOT sRGB). Albedo textures should be `Rgba8UnormSrgb`. Getting this wrong makes normals or colors look wrong.

5. **Bind group layout must match shader** — Each `@group(N) @binding(M)` in the shader must have a corresponding entry in the bind group layout. Mismatches cause hard-to-debug GPU errors.

6. **Depth buffer precision** — `Depth32Float` works well. For large scenes, use reverse-Z (near=1, far=0) for better precision at distance. Default is fine for starting out.

## References

- [glTF specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)
- [gltf crate docs](https://docs.rs/gltf/latest/gltf/)
- [Learn OpenGL PBR](https://learnopengl.com/PBR/Theory) (concepts transfer to wgpu)
- [wgpu examples - cube](https://github.com/gfx-rs/wgpu/tree/trunk/examples/src/cube)
- [Filament PBR whitepaper](https://google.github.io/filament/Filament.html) (advanced reference)
