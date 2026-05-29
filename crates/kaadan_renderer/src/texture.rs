pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

/// Number of mip levels for a texture of `width` x `height`.
///
/// `floor(log2(max(w, h))) + 1`, clamped to at least 1.
pub fn mip_level_count(width: u32, height: u32) -> u32 {
    let max_dim = width.max(height).max(1);
    32 - max_dim.leading_zeros()
}

impl Texture {
    /// Create a texture from encoded image bytes (PNG/JPEG).
    pub fn from_bytes(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        label: &str,
    ) -> Result<Self, kaadan_core::KaadanError> {
        let img =
            image::load_from_memory(bytes).map_err(|e| kaadan_core::KaadanError::AssetLoad {
                path: label.to_string(),
                reason: e.to_string(),
            })?;
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());
        Ok(Self::from_rgba8(device, queue, &rgba, width, height, label))
    }

    /// Create a texture from raw RGBA8 pixels.
    ///
    /// Allocates a full mipmap chain and generates the lower levels on the GPU
    /// so minified samples are trilinearly filtered (no aliasing).
    pub fn from_rgba8(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
        width: u32,
        height: u32,
        label: &str,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mip_count = mip_level_count(width, height);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: mip_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        if mip_count > 1 {
            generate_mipmaps(device, queue, &texture, format, mip_count);
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            width,
            height,
        }
    }
}

/// Generate mip levels 1..`mip_count` for `texture` by repeatedly downsampling
/// the previous level with a fullscreen-triangle blit pipeline.
fn generate_mipmaps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    format: wgpu::TextureFormat,
    mip_count: u32,
) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mipmap_blit_shader"),
        source: wgpu::ShaderSource::Wgsl(MIP_BLIT_SHADER.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mipmap_blit_bgl"),
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

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mipmap_blit_pipeline_layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mipmap_blit_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mipmap_blit_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // One view per mip level so we can bind level N-1 as input and render to N.
    let views: Vec<wgpu::TextureView> = (0..mip_count)
        .map(|level| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mipmap_blit_view"),
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mipmap_blit_encoder"),
    });

    for target_level in 1..mip_count as usize {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mipmap_blit_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[target_level - 1]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mipmap_blit_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &views[target_level],
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    queue.submit(std::iter::once(encoder.finish()));
}

/// Fullscreen-triangle blit/downsample shader used for mipmap generation.
const MIP_BLIT_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    // Oversized triangle covering the viewport.
    let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    out.uv = uv;
    out.position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    return out;
}

@group(0) @binding(0) var t_src: texture_2d<f32>;
@group(0) @binding(1) var s_src: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(t_src, s_src, in.uv);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_level_count_known_sizes() {
        assert_eq!(mip_level_count(256, 256), 9);
        assert_eq!(mip_level_count(1, 1), 1);
        assert_eq!(mip_level_count(2, 2), 2);
        assert_eq!(mip_level_count(1024, 1024), 11);
        // Non-square: based on the larger dimension.
        assert_eq!(mip_level_count(256, 1), 9);
        assert_eq!(mip_level_count(100, 100), 7);
    }
}
