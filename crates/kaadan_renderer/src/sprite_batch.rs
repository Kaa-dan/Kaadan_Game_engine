use bytemuck::{Pod, Zeroable};
use kaadan_math::Handle;

use crate::texture::Texture;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SpriteVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl SpriteVertex {
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<SpriteVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 20,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

/// A pending draw call grouped by texture.
pub struct DrawCall {
    pub texture_handle: Handle<Texture>,
    pub index_start: u32,
    pub index_count: u32,
}

/// Collects Sprite + Transform entities, sorts by z_order and texture,
/// builds vertex buffers, and issues draw calls.
pub struct SpriteBatch {
    pub vertices: Vec<SpriteVertex>,
    pub indices: Vec<u32>,
    pub draw_calls: Vec<DrawCall>,
}

impl Default for SpriteBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl SpriteBatch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            draw_calls: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.draw_calls.clear();
    }

    /// Add a quad to the batch.
    pub fn push_quad(
        &mut self,
        texture_handle: Handle<Texture>,
        positions: [[f32; 3]; 4],
        uvs: [[f32; 2]; 4],
        color: [f32; 4],
    ) {
        let base = self.vertices.len() as u32;

        for i in 0..4 {
            self.vertices.push(SpriteVertex {
                position: positions[i],
                uv: uvs[i],
                color,
            });
        }

        // Two triangles: 0-1-2, 2-3-0
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);

        // Extend or create draw call
        if let Some(last) = self.draw_calls.last_mut() {
            if last.texture_handle == texture_handle {
                last.index_count += 6;
                return;
            }
        }
        self.draw_calls.push(DrawCall {
            texture_handle,
            index_start: self.indices.len() as u32 - 6,
            index_count: 6,
        });
    }
}
