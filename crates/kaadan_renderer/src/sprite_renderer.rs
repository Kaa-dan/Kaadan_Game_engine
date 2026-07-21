use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use kaadan_math::{Handle, Mat4};

use crate::pipeline::{
    create_sprite_pipeline, sprite_camera_bind_group_layout, sprite_texture_bind_group_layout,
};
use crate::sprite_batch::SpriteBatch;
use crate::texture::Texture;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Camera2DUniform {
    view_projection: [[f32; 4]; 4],
}

/// Owns the GPU resources for drawing batched 2D sprites: the pipeline, a
/// camera uniform, growable vertex/index buffers, and a per-texture bind
/// group cache keyed by [`Handle<Texture>`].
pub struct SpriteRenderer {
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    texture_bind_groups: HashMap<Handle<Texture>, wgpu::BindGroup>,
}

const INITIAL_VERTICES: u64 = 4096;
const INITIAL_INDICES: u64 = 6144;

impl SpriteRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, shader_source: &str) -> Self {
        let camera_layout = sprite_camera_bind_group_layout(device);
        let texture_layout = sprite_texture_bind_group_layout(device);
        let pipeline = create_sprite_pipeline(
            device,
            format,
            shader_source,
            &camera_layout,
            &texture_layout,
        );

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_camera_buffer"),
            size: std::mem::size_of::<Camera2DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_camera_bind_group"),
            layout: &camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let vertex_buffer = Self::alloc_vertex_buffer(device, INITIAL_VERTICES);
        let index_buffer = Self::alloc_index_buffer(device, INITIAL_INDICES);

        Self {
            pipeline,
            texture_layout,
            camera_buffer,
            camera_bind_group,
            vertex_buffer,
            index_buffer,
            vertex_capacity: INITIAL_VERTICES,
            index_capacity: INITIAL_INDICES,
            texture_bind_groups: HashMap::new(),
        }
    }

    fn alloc_vertex_buffer(device: &wgpu::Device, vertices: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_vertex_buffer"),
            size: vertices * std::mem::size_of::<crate::sprite_batch::SpriteVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn alloc_index_buffer(device: &wgpu::Device, indices: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprite_index_buffer"),
            size: indices * std::mem::size_of::<u32>() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Register (or replace) the bind group for a texture handle. Call this
    /// once when a texture is created/loaded so the batch can reference it.
    pub fn register_texture(
        &mut self,
        device: &wgpu::Device,
        handle: Handle<Texture>,
        texture: &Texture,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_texture_bind_group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ],
        });
        self.texture_bind_groups.insert(handle, bind_group);
    }

    /// Upload the batch and record its draw calls into `pass`.
    pub fn render<'p>(
        &'p mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        batch: &SpriteBatch,
        view_projection: Mat4,
        pass: &mut wgpu::RenderPass<'p>,
    ) {
        if batch.draw_calls.is_empty() {
            // Still refresh the camera so the next frame is correct.
            queue.write_buffer(
                &self.camera_buffer,
                0,
                bytemuck::bytes_of(&Camera2DUniform {
                    view_projection: view_projection.to_cols_array_2d(),
                }),
            );
            return;
        }

        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&Camera2DUniform {
                view_projection: view_projection.to_cols_array_2d(),
            }),
        );

        if batch.vertices.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (batch.vertices.len() as u64 * 3 / 2).max(INITIAL_VERTICES);
            self.vertex_buffer = Self::alloc_vertex_buffer(device, self.vertex_capacity);
        }
        if batch.indices.len() as u64 > self.index_capacity {
            self.index_capacity = (batch.indices.len() as u64 * 3 / 2).max(INITIAL_INDICES);
            self.index_buffer = Self::alloc_index_buffer(device, self.index_capacity);
        }
        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&batch.vertices),
        );
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&batch.indices));

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for call in &batch.draw_calls {
            let Some(bind_group) = self.texture_bind_groups.get(&call.texture_handle) else {
                tracing::warn!("no bind group for texture handle; skipping draw call");
                continue;
            };
            pass.set_bind_group(1, bind_group, &[]);
            let start = call.index_start;
            pass.draw_indexed(start..start + call.index_count, 0, 0..1);
        }
    }
}
