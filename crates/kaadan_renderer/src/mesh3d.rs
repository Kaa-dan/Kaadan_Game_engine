use kaadan_math::Handle;
use wgpu::util::DeviceExt;

use crate::vertex3d::Vertex3D;

/// GPU-resident 3D mesh (positions/normals/uv/tangent + u32 indices).
pub struct Mesh3DGpu {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Mesh3DGpu {
    pub fn new(device: &wgpu::Device, vertices: &[Vertex3D], indices: &[u32]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh3d_vertex_buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh3d_index_buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }
}

/// ECS component: references an uploaded [`Mesh3DGpu`] by handle.
#[derive(Clone, Copy)]
pub struct Mesh3D {
    pub handle: Handle<Mesh3DGpu>,
}

impl Mesh3D {
    pub fn new(handle: Handle<Mesh3DGpu>) -> Self {
        Self { handle }
    }
}

/// Build a unit cube (side length `2 * half`) centered at the origin with
/// outward-facing normals, suitable as an offline fallback when no glTF model
/// is available.
pub fn create_cube_mesh(device: &wgpu::Device, half: f32) -> Mesh3DGpu {
    let h = half;
    // (normal, [four CCW-from-outside corners])
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        // +Z
        (
            [0.0, 0.0, 1.0],
            [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]],
        ),
        // -Z
        (
            [0.0, 0.0, -1.0],
            [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]],
        ),
        // +X
        (
            [1.0, 0.0, 0.0],
            [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]],
        ),
        // -X
        (
            [-1.0, 0.0, 0.0],
            [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]],
        ),
        // +Y
        (
            [0.0, 1.0, 0.0],
            [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]],
        ),
        // -Y
        (
            [0.0, -1.0, 0.0],
            [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]],
        ),
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (normal, corners) in faces {
        let base = vertices.len() as u32;
        for (i, position) in corners.iter().enumerate() {
            vertices.push(Vertex3D {
                position: *position,
                normal,
                uv: uvs[i],
                tangent: [1.0, 0.0, 0.0, 1.0],
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh3DGpu::new(device, &vertices, &indices)
}
