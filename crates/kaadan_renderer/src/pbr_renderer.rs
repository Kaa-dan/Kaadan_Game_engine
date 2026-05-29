use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use kaadan_math::{Handle, Mat3, Mat4, Transform, Vec3};

use crate::camera3d::Camera3D;
use crate::material::{DirectionalLight, PbrMaterial, PointLight};
use crate::mesh3d::{Mesh3D, Mesh3DGpu};
use crate::pipeline::create_pbr_pipeline;
use crate::texture::Texture;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform3D {
    view_projection: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    position: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ModelUniform {
    model: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    emissive: [f32; 4],
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    // x = albedo, y = metallic-roughness, z = normal, w = emissive present.
    flags: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointLightData {
    position: [f32; 4], // xyz, w = range
    color: [f32; 4],    // rgb, w = intensity
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LightUniform {
    direction: [f32; 4],
    dir_color: [f32; 4],
    camera_position: [f32; 4],
    point_lights: [PointLightData; 4],
    num_point_lights: u32,
    _pad: [u32; 3],
}

/// Identifies the set of textures referenced by a material so material-texture
/// bind groups can be cached across frames (one bind group per distinct
/// combination, created lazily). `None` slots fall back to default textures.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct MaterialTextureKey {
    albedo: Option<Handle<Texture>>,
    metallic_roughness: Option<Handle<Texture>>,
    normal: Option<Handle<Texture>>,
    emissive: Option<Handle<Texture>>,
}

impl MaterialTextureKey {
    fn from_material(material: &PbrMaterial) -> Self {
        Self {
            albedo: material.base_color_texture,
            metallic_roughness: material.metallic_roughness_texture,
            normal: material.normal_texture,
            emissive: material.emissive_texture,
        }
    }
}

/// 1x1 fallback textures used when a material omits a texture handle.
struct DefaultTextures {
    white: Texture,
    normal: Texture,
    black: Texture,
}

/// Owns the GPU resources for PBR-lit 3D rendering: pipeline, persistent
/// camera/light uniforms, default textures, a growable per-entity uniform buffer
/// addressed with dynamic offsets, and a cache of material-texture bind groups.
///
/// In steady state there are no per-frame GPU allocations: the uniform buffer is
/// reused (only growing when the entity count exceeds its capacity) and material
/// bind groups are created once per distinct texture combination.
pub struct PbrRenderer {
    pipeline: wgpu::RenderPipeline,
    model_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    defaults: DefaultTextures,

    /// Byte stride between consecutive per-entity uniform blocks, rounded up to
    /// the device's `min_uniform_buffer_offset_alignment`.
    uniform_stride: u64,
    /// Single growable buffer holding the model uniform of every entity, one
    /// `uniform_stride`-aligned block each. Bound with a dynamic offset.
    model_uniform_buffer: wgpu::Buffer,
    model_dynamic_bind_group: wgpu::BindGroup,
    /// Single growable buffer holding the material uniform of every entity.
    material_uniform_buffer: wgpu::Buffer,
    /// Number of per-entity blocks the buffers can currently hold.
    capacity: u32,

    /// Material-texture bind groups keyed by their texture combination. The
    /// material uniform binding inside each is dynamic, so a single cached bind
    /// group serves every entity sharing that texture set.
    material_bind_groups: HashMap<MaterialTextureKey, wgpu::BindGroup>,

    /// Scratch buffers reused across frames to avoid per-frame heap allocation.
    model_scratch: Vec<u8>,
    material_scratch: Vec<u8>,
}

impl PbrRenderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        shader_source: &str,
    ) -> Self {
        let camera_layout = uniform_layout(
            device,
            "pbr_camera_bgl",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            false,
        );
        // Model uniform is bound with a dynamic offset (per-entity block).
        let model_layout =
            uniform_layout(device, "pbr_model_bgl", wgpu::ShaderStages::VERTEX, true);
        let material_layout = material_layout(device);
        let light_layout =
            uniform_layout(device, "pbr_light_bgl", wgpu::ShaderStages::FRAGMENT, false);

        let pipeline = create_pbr_pipeline(
            device,
            format,
            shader_source,
            [
                &camera_layout,
                &model_layout,
                &material_layout,
                &light_layout,
            ],
        );

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pbr_camera_buffer"),
            size: std::mem::size_of::<CameraUniform3D>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pbr_camera_bind_group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pbr_light_buffer"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pbr_light_bind_group"),
            layout: &light_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_buffer.as_entire_binding(),
            }],
        });

        let defaults = DefaultTextures {
            // White albedo and white metallic-roughness (factors used instead
            // via has_mr=0). Flat normal points straight out of the surface.
            white: Texture::from_rgba8(device, queue, &[255, 255, 255, 255], 1, 1, "pbr_white"),
            normal: Texture::from_rgba8(device, queue, &[128, 128, 255, 255], 1, 1, "pbr_normal"),
            black: Texture::from_rgba8(device, queue, &[0, 0, 0, 255], 1, 1, "pbr_black"),
        };

        // Per-entity uniform stride: max(model, material) rounded up to the
        // device's min uniform offset alignment so dynamic offsets are legal.
        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let block =
            std::mem::size_of::<ModelUniform>().max(std::mem::size_of::<MaterialUniform>()) as u64;
        let uniform_stride = block.div_ceil(align) * align;

        let capacity = 16u32;
        let model_uniform_buffer = create_dynamic_uniform_buffer(
            device,
            "pbr_model_uniform_buffer",
            uniform_stride * capacity as u64,
        );
        let material_uniform_buffer = create_dynamic_uniform_buffer(
            device,
            "pbr_material_uniform_buffer",
            uniform_stride * capacity as u64,
        );
        let model_dynamic_bind_group =
            create_model_bind_group(device, &model_layout, &model_uniform_buffer);

        Self {
            pipeline,
            model_layout,
            material_layout,
            camera_buffer,
            camera_bind_group,
            light_buffer,
            light_bind_group,
            defaults,
            uniform_stride,
            model_uniform_buffer,
            model_dynamic_bind_group,
            material_uniform_buffer,
            capacity,
            material_bind_groups: HashMap::new(),
            model_scratch: Vec::new(),
            material_scratch: Vec::new(),
        }
    }

    /// Ensure the uniform buffers can hold `count` per-entity blocks, growing
    /// (and invalidating cached bind groups that referenced the old buffer) only
    /// when necessary. Amortized: no allocation in steady state.
    fn ensure_capacity(&mut self, device: &wgpu::Device, count: u32) {
        if count <= self.capacity {
            return;
        }
        let new_capacity = count.next_power_of_two();
        let size = self.uniform_stride * new_capacity as u64;
        self.model_uniform_buffer =
            create_dynamic_uniform_buffer(device, "pbr_model_uniform_buffer", size);
        self.material_uniform_buffer =
            create_dynamic_uniform_buffer(device, "pbr_material_uniform_buffer", size);
        self.model_dynamic_bind_group =
            create_model_bind_group(device, &self.model_layout, &self.model_uniform_buffer);
        // Cached material bind groups referenced the now-replaced material
        // buffer, so they must be rebuilt against the new one.
        self.material_bind_groups.clear();
        self.capacity = new_capacity;
    }

    /// Resolve a material texture handle to a view/sampler, falling back to the
    /// given default when the handle is missing or unregistered.
    fn resolve<'a>(
        textures: &'a HashMap<Handle<Texture>, Texture>,
        handle: Option<Handle<Texture>>,
        default: &'a Texture,
    ) -> &'a Texture {
        handle.and_then(|h| textures.get(&h)).unwrap_or(default)
    }

    /// Get (or lazily create and cache) the material-texture bind group for the
    /// material's texture combination.
    fn material_bind_group(
        &mut self,
        device: &wgpu::Device,
        key: MaterialTextureKey,
        textures: &HashMap<Handle<Texture>, Texture>,
    ) -> &wgpu::BindGroup {
        if !self.material_bind_groups.contains_key(&key) {
            let albedo = Self::resolve(textures, key.albedo, &self.defaults.white);
            let mr = Self::resolve(textures, key.metallic_roughness, &self.defaults.white);
            let normal = Self::resolve(textures, key.normal, &self.defaults.normal);
            let emissive = Self::resolve(textures, key.emissive, &self.defaults.black);

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pbr_material_bind_group"),
                layout: &self.material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &self.material_uniform_buffer,
                            offset: 0,
                            size: Some(
                                std::num::NonZeroU64::new(
                                    std::mem::size_of::<MaterialUniform>() as u64
                                )
                                .unwrap(),
                            ),
                        }),
                    },
                    tex_entry(1, albedo),
                    sampler_entry(2, albedo),
                    tex_entry(3, mr),
                    sampler_entry(4, mr),
                    tex_entry(5, normal),
                    sampler_entry(6, normal),
                    tex_entry(7, emissive),
                    sampler_entry(8, emissive),
                ],
            });
            self.material_bind_groups.insert(key, bind_group);
        }
        &self.material_bind_groups[&key]
    }

    /// Draw all `(Mesh3D, Transform, PbrMaterial)` entities into `pass`.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &kaadan_ecs::World,
        meshes: &HashMap<Handle<Mesh3DGpu>, Mesh3DGpu>,
        textures: &HashMap<Handle<Texture>, Texture>,
        camera: &Camera3D,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        let camera_uniform = CameraUniform3D {
            view_projection: camera.view_projection().to_cols_array_2d(),
            view: camera.view_matrix().to_cols_array_2d(),
            position: [camera.position.x, camera.position.y, camera.position.z, 1.0],
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        let light_uniform = build_light_uniform(world, camera.position);
        queue.write_buffer(&self.light_buffer, 0, bytemuck::bytes_of(&light_uniform));

        // First pass over entities: count drawable ones and grow buffers once.
        let drawable: Vec<(Mesh3D, Transform, PbrMaterial)> = world
            .query::<(&Mesh3D, &Transform, &PbrMaterial)>()
            .iter()
            .filter(|(_e, (m, _t, _mat))| meshes.contains_key(&m.handle))
            .map(|(_e, (m, t, mat))| (*m, *t, mat.clone()))
            .collect();

        if drawable.is_empty() {
            return;
        }
        self.ensure_capacity(device, drawable.len() as u32);

        let stride = self.uniform_stride as usize;
        self.model_scratch.clear();
        self.material_scratch.clear();
        self.model_scratch.resize(stride * drawable.len(), 0);
        self.material_scratch.resize(stride * drawable.len(), 0);

        // Stage every entity's model + material uniform into the scratch buffers
        // at its dynamic-offset block, then upload each in a single write.
        for (i, (_mesh, transform, material)) in drawable.iter().enumerate() {
            let model_mat = transform.to_matrix();
            let normal_mat = Mat4::from_mat3(Mat3::from_mat4(model_mat).inverse().transpose());
            let model_uniform = ModelUniform {
                model: model_mat.to_cols_array_2d(),
                normal_matrix: normal_mat.to_cols_array_2d(),
            };
            let off = i * stride;
            self.model_scratch[off..off + std::mem::size_of::<ModelUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&model_uniform));

            let material_uniform = MaterialUniform {
                base_color: material.base_color.to_array(),
                emissive: material.emissive.to_array(),
                metallic: material.metallic,
                roughness: material.roughness,
                _pad0: 0.0,
                _pad1: 0.0,
                flags: [
                    presence(material.base_color_texture, textures),
                    presence(material.metallic_roughness_texture, textures),
                    presence(material.normal_texture, textures),
                    presence(material.emissive_texture, textures),
                ],
            };
            self.material_scratch[off..off + std::mem::size_of::<MaterialUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&material_uniform));
        }
        queue.write_buffer(&self.model_uniform_buffer, 0, &self.model_scratch);
        queue.write_buffer(&self.material_uniform_buffer, 0, &self.material_scratch);

        // Ensure every needed material-texture bind group exists before the draw
        // loop, so the loop only needs immutable cache lookups (no borrow split).
        for (_mesh, _transform, material) in &drawable {
            let key = MaterialTextureKey::from_material(material);
            self.material_bind_group(device, key, textures);
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(3, &self.light_bind_group, &[]);

        for (i, (mesh3d, _transform, material)) in drawable.iter().enumerate() {
            let mesh = &meshes[&mesh3d.handle];
            let dyn_off = (i * stride) as u32;

            let key = MaterialTextureKey::from_material(material);
            let material_bg = &self.material_bind_groups[&key];

            // Model + material uniforms share the same per-entity dynamic offset.
            pass.set_bind_group(1, &self.model_dynamic_bind_group, &[dyn_off]);
            pass.set_bind_group(2, material_bg, &[dyn_off]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}

/// Presence flag (1.0/0.0) for a material texture, true only if the handle is
/// set and actually registered in the texture map.
fn presence(handle: Option<Handle<Texture>>, textures: &HashMap<Handle<Texture>, Texture>) -> f32 {
    match handle {
        Some(h) if textures.contains_key(&h) => 1.0,
        _ => 0.0,
    }
}

fn tex_entry(binding: u32, texture: &Texture) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::TextureView(&texture.view),
    }
}

fn sampler_entry(binding: u32, texture: &Texture) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: wgpu::BindingResource::Sampler(&texture.sampler),
    }
}

fn create_dynamic_uniform_buffer(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_model_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    // With a dynamic offset the bound range is one element: the model struct.
    let size = std::num::NonZeroU64::new(std::mem::size_of::<ModelUniform>() as u64).unwrap();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pbr_model_bind_group"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: Some(size),
            }),
        }],
    })
}

fn uniform_layout(
    device: &wgpu::Device,
    label: &str,
    visibility: wgpu::ShaderStages,
    has_dynamic_offset: bool,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: None,
        },
        count: None,
    }];
    // Four texture+sampler pairs at bindings 1..=8.
    for pair in 0..4u32 {
        let tex_binding = 1 + pair * 2;
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: tex_binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: tex_binding + 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pbr_material_bgl"),
        entries: &entries,
    })
}

fn build_light_uniform(world: &kaadan_ecs::World, camera_position: Vec3) -> LightUniform {
    let mut light = LightUniform::zeroed();
    light.camera_position = [camera_position.x, camera_position.y, camera_position.z, 1.0];

    if let Some((_e, dl)) = world.query::<&DirectionalLight>().iter().next() {
        let d = dl.direction.normalize_or_zero();
        light.direction = [d.x, d.y, d.z, 0.0];
        light.dir_color = [dl.color.r, dl.color.g, dl.color.b, dl.intensity];
    } else {
        light.direction = [0.0, -1.0, -0.3, 0.0];
        light.dir_color = [1.0, 1.0, 1.0, 1.0];
    }

    let mut count = 0usize;
    for (_e, (pl, transform)) in world.query::<(&PointLight, &Transform)>().iter() {
        if count >= 4 {
            break;
        }
        let p = transform.position;
        light.point_lights[count] = PointLightData {
            position: [p.x, p.y, p.z, pl.range],
            color: [pl.color.r, pl.color.g, pl.color.b, pl.intensity],
        };
        count += 1;
    }
    light.num_point_lights = count as u32;
    light
}

#[cfg(test)]
mod tests {
    use naga::valid::{Capabilities, ValidationFlags, Validator};

    /// Validate that both PBR shaders parse and pass naga validation without a
    /// GPU. naga 23.x matches the wgsl front-end shipped with wgpu 23.
    fn validate(name: &str, source: &str) {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{name} failed to parse: {e}"));
        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name} failed to validate: {e:?}"));
    }

    #[test]
    fn pbr_shader_is_valid_wgsl() {
        validate("PBR_SHADER", crate::PBR_SHADER);
    }

    #[test]
    fn pbr_mobile_shader_is_valid_wgsl() {
        validate("PBR_MOBILE_SHADER", crate::PBR_MOBILE_SHADER);
    }
}
