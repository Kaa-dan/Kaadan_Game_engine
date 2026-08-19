use kaadan_math::{Handle, Vec3};
use wgpu::util::DeviceExt;

use crate::vertex3d::Vertex3D;

use std::f32::consts::PI;

const DUMMY_TANGENT: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

fn push_tri(vertices: &[Vertex3D], indices: &mut Vec<u32>, a: u32, b: u32, c: u32) {
    let pos = |i: u32| Vec3::from_array(vertices[i as usize].position);
    let nrm = |i: u32| Vec3::from_array(vertices[i as usize].normal);
    let face = (pos(b) - pos(a)).cross(pos(c) - pos(a));
    let outward = nrm(a) + nrm(b) + nrm(c);
    if face.dot(outward) >= 0.0 {
        indices.extend_from_slice(&[a, b, c]);
    } else {
        indices.extend_from_slice(&[a, c, b]);
    }
}

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

/// Build a UV sphere of the given `radius` centered at the origin.
pub fn create_sphere_mesh(device: &wgpu::Device, radius: f32) -> Mesh3DGpu {
    let sectors: u32 = 32; // longitude divisions
    let stacks: u32 = 16; // latitude divisions
    let mut vertices = Vec::new();
    for i in 0..=stacks {
        let theta = PI * (i as f32 / stacks as f32); // 0 (north pole) -> PI (south)
        let (st, ct) = theta.sin_cos();
        for j in 0..=sectors {
            let phi = 2.0 * PI * (j as f32 / sectors as f32);
            let (sp, cp) = phi.sin_cos();
            let n = [st * cp, ct, st * sp];
            vertices.push(Vertex3D {
                position: [radius * n[0], radius * n[1], radius * n[2]],
                normal: n,
                uv: [j as f32 / sectors as f32, i as f32 / stacks as f32],
                tangent: DUMMY_TANGENT,
            });
        }
    }
    let stride = sectors + 1;
    let mut indices = Vec::new();
    for i in 0..stacks {
        for j in 0..sectors {
            let a = i * stride + j;
            let b = a + stride;
            push_tri(&vertices, &mut indices, a, b, a + 1);
            push_tri(&vertices, &mut indices, a + 1, b, b + 1);
        }
    }
    Mesh3DGpu::new(device, &vertices, &indices)
}

/// Build a flat square plane on the XZ axis (normal +Y), side length `2 * half`.
pub fn create_plane_mesh(device: &wgpu::Device, half: f32) -> Mesh3DGpu {
    let n = [0.0, 1.0, 0.0];
    let corners = [
        ([-half, 0.0, -half], [0.0, 0.0]),
        ([half, 0.0, -half], [1.0, 0.0]),
        ([half, 0.0, half], [1.0, 1.0]),
        ([-half, 0.0, half], [0.0, 1.0]),
    ];
    let mut vertices = Vec::new();
    for (position, uv) in corners {
        vertices.push(Vertex3D {
            position,
            normal: n,
            uv,
            tangent: DUMMY_TANGENT,
        });
    }
    let mut indices = Vec::new();
    push_tri(&vertices, &mut indices, 0, 1, 2);
    push_tri(&vertices, &mut indices, 0, 2, 3);
    Mesh3DGpu::new(device, &vertices, &indices)
}

fn push_ring(
    vertices: &mut Vec<Vertex3D>,
    sectors: u32,
    radius: f32,
    st: f32,
    ct: f32,
    y: f32,
    v: f32,
) -> u32 {
    let start = vertices.len() as u32;
    for j in 0..=sectors {
        let phi = 2.0 * PI * (j as f32 / sectors as f32);
        let (sp, cp) = phi.sin_cos();
        vertices.push(Vertex3D {
            position: [radius * st * cp, y, radius * st * sp],
            normal: [st * cp, ct, st * sp],
            uv: [j as f32 / sectors as f32, v],
            tangent: DUMMY_TANGENT,
        });
    }
    start
}

fn bridge_rings(vertices: &[Vertex3D], indices: &mut Vec<u32>, r0: u32, r1: u32, sectors: u32) {
    for j in 0..sectors {
        let a0 = r0 + j;
        let a1 = r1 + j;
        push_tri(vertices, indices, a0, a1, a0 + 1);
        push_tri(vertices, indices, a0 + 1, a1, a1 + 1);
    }
}

/// Build a cylinder of `radius` and total height `2 * half_height` around Y,
/// with flat end caps.
pub fn create_cylinder_mesh(device: &wgpu::Device, radius: f32, half_height: f32) -> Mesh3DGpu {
    let sectors: u32 = 32;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Side wall: two rings with purely radial normals.
    let top = push_ring(&mut vertices, sectors, radius, 1.0, 0.0, half_height, 0.0);
    let bottom = push_ring(&mut vertices, sectors, radius, 1.0, 0.0, -half_height, 1.0);
    bridge_rings(&vertices, &mut indices, top, bottom, sectors);

    // Top cap (fan from center).
    let top_center = vertices.len() as u32;
    vertices.push(Vertex3D {
        position: [0.0, half_height, 0.0],
        normal: [0.0, 1.0, 0.0],
        uv: [0.5, 0.5],
        tangent: DUMMY_TANGENT,
    });
    let top_ring = push_ring(&mut vertices, sectors, radius, 0.0, 1.0, half_height, 0.0);
    for j in 0..sectors {
        push_tri(
            &vertices,
            &mut indices,
            top_center,
            top_ring + j,
            top_ring + j + 1,
        );
    }

    // Bottom cap.
    let bot_center = vertices.len() as u32;
    vertices.push(Vertex3D {
        position: [0.0, -half_height, 0.0],
        normal: [0.0, -1.0, 0.0],
        uv: [0.5, 0.5],
        tangent: DUMMY_TANGENT,
    });
    let bot_ring = push_ring(&mut vertices, sectors, radius, 0.0, -1.0, -half_height, 0.0);
    for j in 0..sectors {
        push_tri(
            &vertices,
            &mut indices,
            bot_center,
            bot_ring + j,
            bot_ring + j + 1,
        );
    }

    Mesh3DGpu::new(device, &vertices, &indices)
}

/// Build a capsule: a cylinder of half-length `half_height` around Y, capped by
/// two hemispheres of `radius`.
pub fn create_capsule_mesh(device: &wgpu::Device, radius: f32, half_height: f32) -> Mesh3DGpu {
    let sectors: u32 = 32;
    let stacks: u32 = 8; // per hemisphere
    let mut vertices = Vec::new();
    let mut ring_starts: Vec<u32> = Vec::new();

    // Top hemisphere: polar angle 0 -> PI/2, centered at (0, +half_height, 0).
    for i in 0..=stacks {
        let theta = (PI / 2.0) * (i as f32 / stacks as f32);
        let (st, ct) = theta.sin_cos();
        let y = half_height + radius * ct;
        ring_starts.push(push_ring(&mut vertices, sectors, radius, st, ct, y, 0.0));
    }
    // Bottom hemisphere: polar angle PI/2 -> PI, centered at (0, -half_height, 0).
    for i in 0..=stacks {
        let theta = PI / 2.0 + (PI / 2.0) * (i as f32 / stacks as f32);
        let (st, ct) = theta.sin_cos();
        let y = -half_height + radius * ct;
        ring_starts.push(push_ring(&mut vertices, sectors, radius, st, ct, y, 1.0));
    }

    let mut indices = Vec::new();
    for w in ring_starts.windows(2) {
        bridge_rings(&vertices, &mut indices, w[0], w[1], sectors);
    }
    Mesh3DGpu::new(device, &vertices, &indices)
}
