use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use kaadan_math::{Handle, Mat3, Mat4, Transform, Vec3};
use wgpu::util::DeviceExt;

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
    has_albedo_tex: f32,
    _pad: f32,
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

/// Owns the GPU resources for PBR-lit 3D rendering: pipeline, persistent
/// camera/light uniforms, and a default white texture. Per-entity model and
/// material bind groups are created each frame (fine at demo scale).
pub struct PbrRenderer {
    pipeline: wgpu::RenderPipeline,
    model_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    light_bind_group: wgpu::BindGroup,
    default_texture: Texture,
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
        );
        let model_layout = uniform_layout(device, "pbr_model_bgl", wgpu::ShaderStages::VERTEX);
        let material_layout = material_layout(device);
        let light_layout = uniform_layout(device, "pbr_light_bgl", wgpu::ShaderStages::FRAGMENT);

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

        let default_texture = Texture::from_rgba8(
            device,
            queue,
            &[255, 255, 255, 255],
            1,
            1,
            "pbr_default_white",
        );

        Self {
            pipeline,
            model_layout,
            material_layout,
            camera_buffer,
            camera_bind_group,
            light_buffer,
            light_bind_group,
            default_texture,
        }
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

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(3, &self.light_bind_group, &[]);

        let mut query = world.query::<(&Mesh3D, &Transform, &PbrMaterial)>();
        for (_entity, (mesh3d, transform, material)) in query.iter() {
            let Some(mesh) = meshes.get(&mesh3d.handle) else {
                continue;
            };

            let model_mat = transform.to_matrix();
            let normal_mat = Mat4::from_mat3(Mat3::from_mat4(model_mat).inverse().transpose());
            let model_uniform = ModelUniform {
                model: model_mat.to_cols_array_2d(),
                normal_matrix: normal_mat.to_cols_array_2d(),
            };
            let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pbr_model_buffer"),
                contents: bytemuck::bytes_of(&model_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let model_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pbr_model_bind_group"),
                layout: &self.model_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_buffer.as_entire_binding(),
                }],
            });

            let (albedo, has_albedo_tex) =
                match material.base_color_texture.and_then(|h| textures.get(&h)) {
                    Some(texture) => (texture, 1.0),
                    None => (&self.default_texture, 0.0),
                };
            let material_uniform = MaterialUniform {
                base_color: material.base_color.to_array(),
                emissive: material.emissive.to_array(),
                metallic: material.metallic,
                roughness: material.roughness,
                has_albedo_tex,
                _pad: 0.0,
            };
            let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("pbr_material_buffer"),
                contents: bytemuck::bytes_of(&material_uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pbr_material_bind_group"),
                layout: &self.material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: material_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&albedo.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&albedo.sampler),
                    },
                ],
            });

            pass.set_bind_group(1, &model_bind_group, &[]);
            pass.set_bind_group(2, &material_bind_group, &[]);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}

fn uniform_layout(
    device: &wgpu::Device,
    label: &str,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("pbr_material_bgl"),
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
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
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
