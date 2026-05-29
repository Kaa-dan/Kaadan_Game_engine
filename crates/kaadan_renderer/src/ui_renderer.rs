use bytemuck::{Pod, Zeroable};
use kaadan_math::{Color, Mat4, Rect, Vec2};

use crate::texture::Texture;

/// A screen-space quad. `rect` is in pixels, top-left origin, y-down.
/// `uv` is the 0..1 sub-region of the bound texture to sample.
pub struct UiQuad {
    pub rect: Rect,
    pub uv: Rect,
    pub color: Color,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UiVertex {
    /// Position in pixels (top-left origin, y-down).
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl UiVertex {
    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2,
            },
            wgpu::VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: wgpu::VertexFormat::Float32x4,
            },
        ],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ProjectionUniform {
    projection: [[f32; 4]; 4],
}

/// Build a screen-space orthographic projection.
///
/// Maps pixel space with a top-left origin (y-down) to wgpu clip space:
/// (0, 0) -> (-1, 1) (top-left), (w, h) -> (1, -1) (bottom-right). z is 0.
pub(crate) fn screen_ortho(w: f32, h: f32) -> Mat4 {
    // Column-major. clip.x = (2/w) px - 1 ; clip.y = -(2/h) py + 1.
    Mat4::from_cols_array(&[
        2.0 / w,
        0.0,
        0.0,
        0.0, // col 0
        0.0,
        -2.0 / h,
        0.0,
        0.0, // col 1
        0.0,
        0.0,
        1.0,
        0.0, // col 2
        -1.0,
        1.0,
        0.0,
        1.0, // col 3 (translation)
    ])
}

const INITIAL_VERTICES: u64 = 1024;
const INITIAL_INDICES: u64 = 1536;

/// Renderer for 2D screen-space UI quads (panels, buttons, images, bars).
///
/// Modeled on [`crate::SpriteRenderer`]: growable vertex/index buffers, a
/// projection uniform, and a texture+sampler bind group. Alpha-blended with no
/// depth attachment.
pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    proj_buffer: wgpu::Buffer,
    proj_bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    /// Built-in 1x1 white texture used when the caller passes `texture: None`.
    white_texture: Texture,
    white_bind_group: wgpu::BindGroup,
    /// Whether the 1x1 white pixel still needs its one-time GPU upload.
    white_uploaded: bool,
    /// Scratch buffers reused each frame to avoid per-frame allocation.
    vertices: Vec<UiVertex>,
    indices: Vec<u32>,
}

impl UiRenderer {
    /// `shader` is the WGSL source (engine passes `kaadan_renderer::UI_SHADER`).
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat, shader: &str) -> Self {
        let proj_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui_proj_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui_texture_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui_shader"),
            source: wgpu::ShaderSource::Wgsl(shader.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui_pipeline_layout"),
            bind_group_layouts: &[&proj_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[UiVertex::LAYOUT],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let proj_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_proj_buffer"),
            size: std::mem::size_of::<ProjectionUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let proj_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui_proj_bind_group"),
            layout: &proj_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: proj_buffer.as_entire_binding(),
            }],
        });

        let vertex_buffer = Self::alloc_vertex_buffer(device, INITIAL_VERTICES);
        let index_buffer = Self::alloc_index_buffer(device, INITIAL_INDICES);

        // Note: no queue available here; a 1x1 white texture needs an upload.
        // Texture::from_rgba8 takes a queue, so build it lazily would require
        // a queue — instead we accept a device-only path by uploading via the
        // device's default queue is not possible, so require it in new()? The
        // public signature has no queue, so we create the white texture using a
        // staging-free approach: write happens through `render`'s queue is too
        // late. We therefore create it here using a one-shot encoder + a
        // mapped staging buffer is overkill; instead we keep a CPU copy and
        // upload it the first time `render` runs.
        let white_texture = Self::create_white_texture(device);
        let white_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui_white_bind_group"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&white_texture.sampler),
                },
            ],
        });

        Self {
            pipeline,
            texture_layout,
            proj_buffer,
            proj_bind_group,
            vertex_buffer,
            index_buffer,
            vertex_capacity: INITIAL_VERTICES,
            index_capacity: INITIAL_INDICES,
            white_texture,
            white_bind_group,
            white_uploaded: false,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn alloc_vertex_buffer(device: &wgpu::Device, vertices: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_vertex_buffer"),
            size: vertices * std::mem::size_of::<UiVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn alloc_index_buffer(device: &wgpu::Device, indices: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_index_buffer"),
            size: indices * std::mem::size_of::<u32>() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Allocate the built-in 1x1 white texture. The single white pixel is
    /// uploaded lazily on the first [`render`](Self::render) call (which has a
    /// queue), since the constructor only has a device.
    fn create_white_texture(device: &wgpu::Device) -> Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui_white_texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ui_white_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Texture {
            texture,
            view,
            sampler,
            width: 1,
            height: 1,
        }
    }

    /// Draw `quads` for a screen of `screen_size` pixels. If `texture` is None,
    /// quads are filled with a built-in 1x1 white texture (solid color =
    /// `quad.color`). Alpha-blended, no depth. Builds a screen-space ortho
    /// projection internally (top-left origin, y-down).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_size: Vec2,
        quads: &[UiQuad],
        texture: Option<&Texture>,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        // Upload the 1x1 white pixel once, now that we have a queue.
        if !self.white_uploaded {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.white_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &[255u8, 255, 255, 255],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            self.white_uploaded = true;
        }

        // Refresh the projection every frame so resizes are handled.
        let projection = screen_ortho(screen_size.x.max(1.0), screen_size.y.max(1.0));
        queue.write_buffer(
            &self.proj_buffer,
            0,
            bytemuck::bytes_of(&ProjectionUniform {
                projection: projection.to_cols_array_2d(),
            }),
        );

        if quads.is_empty() {
            return;
        }

        self.vertices.clear();
        self.indices.clear();
        self.vertices.reserve(quads.len() * 4);
        self.indices.reserve(quads.len() * 6);

        for quad in quads {
            let base = self.vertices.len() as u32;
            let color = quad.color.to_array();
            let (rmin, rmax) = (quad.rect.min, quad.rect.max);
            let (umin, umax) = (quad.uv.min, quad.uv.max);
            // Corners: top-left, top-right, bottom-right, bottom-left (y-down).
            let corners = [
                ([rmin.x, rmin.y], [umin.x, umin.y]),
                ([rmax.x, rmin.y], [umax.x, umin.y]),
                ([rmax.x, rmax.y], [umax.x, umax.y]),
                ([rmin.x, rmax.y], [umin.x, umax.y]),
            ];
            for (position, uv) in corners {
                self.vertices.push(UiVertex {
                    position,
                    uv,
                    color,
                });
            }
            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }

        if self.vertices.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (self.vertices.len() as u64 * 3 / 2).max(INITIAL_VERTICES);
            self.vertex_buffer = Self::alloc_vertex_buffer(device, self.vertex_capacity);
        }
        if self.indices.len() as u64 > self.index_capacity {
            self.index_capacity = (self.indices.len() as u64 * 3 / 2).max(INITIAL_INDICES);
            self.index_buffer = Self::alloc_index_buffer(device, self.index_capacity);
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.indices));

        // Pick (and cache) the texture bind group.
        let texture_bind_group = match texture {
            Some(tex) => self.make_texture_bind_group(device, tex),
            None => None,
        };
        let bind_group = texture_bind_group
            .as_ref()
            .unwrap_or(&self.white_bind_group);

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.proj_bind_group, &[]);
        pass.set_bind_group(1, bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.indices.len() as u32, 0, 0..1);
    }

    fn make_texture_bind_group(
        &self,
        device: &wgpu::Device,
        texture: &Texture,
    ) -> Option<wgpu::BindGroup> {
        Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui_texture_bind_group"),
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
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(m: &Mat4, x: f32, y: f32) -> (f32, f32) {
        let v = m.mul_vec4(kaadan_math::Vec4::new(x, y, 0.0, 1.0));
        (v.x / v.w, v.y / v.w)
    }

    #[test]
    fn screen_ortho_maps_corners() {
        let m = screen_ortho(800.0, 600.0);
        let (tlx, tly) = clip(&m, 0.0, 0.0);
        assert!((tlx - -1.0).abs() < 1e-5, "tlx={tlx}");
        assert!((tly - 1.0).abs() < 1e-5, "tly={tly}");

        let (brx, bry) = clip(&m, 800.0, 600.0);
        assert!((brx - 1.0).abs() < 1e-5, "brx={brx}");
        assert!((bry - -1.0).abs() < 1e-5, "bry={bry}");

        // Center maps to the clip-space origin.
        let (cx, cy) = clip(&m, 400.0, 300.0);
        assert!(cx.abs() < 1e-5, "cx={cx}");
        assert!(cy.abs() < 1e-5, "cy={cy}");
    }
}
