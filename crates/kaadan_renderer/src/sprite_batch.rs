use bytemuck::{Pod, Zeroable};
use kaadan_math::{Handle, Transform, Vec2, Vec3};

use crate::camera2d::Camera2D;
use crate::sprite::Sprite;
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

    /// Gather all `(Sprite, Transform)` entities, sort by `z_order` then
    /// texture (so equal-texture runs merge into a single draw call), cull
    /// against the camera's visible area, and build the vertex/index buffers.
    pub fn collect(&mut self, world: &kaadan_ecs::World, camera: &Camera2D) {
        self.clear();

        // Camera-visible world rect (center origin, Y-up).
        let half = camera.viewport_size / (2.0 * camera.zoom);
        let cam_min = camera.position - half;
        let cam_max = camera.position + half;

        let mut query = world.query::<(&Sprite, &Transform)>();
        let mut items: Vec<(&Sprite, &Transform)> = query.iter().map(|(_e, c)| c).collect();

        // Stable sort: primary z_order (back-to-front), secondary texture index
        // so identical textures are adjacent and merge in `push_quad`.
        items.sort_by(|(a, _), (b, _)| {
            a.z_order
                .cmp(&b.z_order)
                .then(a.texture.index().cmp(&b.texture.index()))
        });

        for (sprite, transform) in items {
            let size = sprite.size.unwrap_or_else(|| {
                sprite
                    .region
                    .map(|r| r.pixel_size)
                    .unwrap_or(Vec2::splat(1.0))
            });
            let (positions, min, max) = quad_corners(transform, size, sprite.anchor);

            // Cull sprites whose AABB does not overlap the camera rect.
            if max.x < cam_min.x || min.x > cam_max.x || max.y < cam_min.y || min.y > cam_max.y {
                continue;
            }

            let uvs = quad_uvs(sprite);
            self.push_quad(sprite.texture, positions, uvs, sprite.color.to_array());
        }
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

/// Quad corner fractions (bottom-left, bottom-right, top-right, top-left).
const CORNERS: [Vec2; 4] = [
    Vec2::new(0.0, 0.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(0.0, 1.0),
];

/// World-space corners of a sprite quad plus its XY AABB.
fn quad_corners(transform: &Transform, size: Vec2, anchor: Vec2) -> ([[f32; 3]; 4], Vec2, Vec2) {
    let matrix = transform.to_matrix();
    let mut positions = [[0.0_f32; 3]; 4];
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for (i, frac) in CORNERS.iter().enumerate() {
        let local = (*frac - anchor) * size;
        let world = matrix.transform_point3(Vec3::new(local.x, local.y, 0.0));
        positions[i] = [world.x, world.y, world.z];
        let xy = world.truncate();
        min = min.min(xy);
        max = max.max(xy);
    }
    (positions, min, max)
}

/// UVs matching [`CORNERS`], honoring atlas region and flip flags.
fn quad_uvs(sprite: &Sprite) -> [[f32; 2]; 4] {
    let (mut u_min, mut v_min, mut u_max, mut v_max) = match sprite.region {
        Some(r) => (r.uv_min.x, r.uv_min.y, r.uv_max.x, r.uv_max.y),
        None => (0.0, 0.0, 1.0, 1.0),
    };
    if sprite.flip_x {
        std::mem::swap(&mut u_min, &mut u_max);
    }
    if sprite.flip_y {
        std::mem::swap(&mut v_min, &mut v_max);
    }
    [
        [u_min, v_max], // bottom-left
        [u_max, v_max], // bottom-right
        [u_max, v_min], // top-right
        [u_min, v_min], // top-left
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_math::HandleAllocator;

    #[test]
    fn collect_sorts_and_merges_by_texture() {
        let mut alloc = HandleAllocator::<Texture>::new();
        let tex_a = alloc.allocate();
        let tex_b = alloc.allocate();

        let mut world = kaadan_ecs::World::new();
        // Spawn interleaved textures across z-orders; after sorting by
        // (z_order, texture) the two tex_a sprites should merge.
        world.spawn((sprite(tex_b, 1), Transform::from_position_2d(0.0, 0.0)));
        world.spawn((sprite(tex_a, 0), Transform::from_position_2d(0.0, 0.0)));
        world.spawn((sprite(tex_a, 0), Transform::from_position_2d(10.0, 0.0)));

        let camera = Camera2D::new(800.0, 600.0);
        let mut batch = SpriteBatch::new();
        batch.collect(&world, &camera);

        // 3 sprites -> 12 vertices, 18 indices.
        assert_eq!(batch.vertices.len(), 12);
        assert_eq!(batch.indices.len(), 18);
        // tex_a (z=0) sprites are adjacent -> merged; tex_b (z=1) separate.
        assert_eq!(batch.draw_calls.len(), 2);
        assert!(batch.draw_calls[0].texture_handle == tex_a);
        assert_eq!(batch.draw_calls[0].index_count, 12);
        assert!(batch.draw_calls[1].texture_handle == tex_b);
        assert_eq!(batch.draw_calls[1].index_count, 6);
    }

    #[test]
    fn collect_culls_offscreen_sprites() {
        let mut alloc = HandleAllocator::<Texture>::new();
        let tex = alloc.allocate();
        let mut world = kaadan_ecs::World::new();
        world.spawn((sprite(tex, 0), Transform::from_position_2d(0.0, 0.0)));
        world.spawn((sprite(tex, 0), Transform::from_position_2d(100_000.0, 0.0)));

        let camera = Camera2D::new(800.0, 600.0);
        let mut batch = SpriteBatch::new();
        batch.collect(&world, &camera);

        // Only the on-screen sprite survives.
        assert_eq!(batch.vertices.len(), 4);
    }

    fn sprite(texture: Handle<Texture>, z: i32) -> Sprite {
        let mut s = Sprite::new(texture);
        s.size = Some(Vec2::splat(32.0));
        s.z_order = z;
        s
    }
}
