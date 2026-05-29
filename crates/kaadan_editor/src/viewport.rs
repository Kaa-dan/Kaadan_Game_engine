//! The scene viewport: owns an ECS world and the 2D/3D renderers, draws the
//! scene into an offscreen [`RenderTarget`], and exposes its color view so the
//! UI layer can show it inside an egui panel.

use std::collections::HashMap;

use kaadan_ecs::World;
use kaadan_math::{Color, Handle, HandleAllocator, Transform, Vec2, Vec3};
use kaadan_renderer::{
    create_cube_mesh, Camera2D, Camera3D, DirectionalLight, Mesh3D, Mesh3DGpu, PbrMaterial,
    PbrRenderer, RenderTarget, Sprite, SpriteBatch, SpriteRenderer, Texture, PBR_SHADER,
    SPRITE_SHADER,
};

use crate::components::Name;

pub struct Viewport {
    pub world: World,
    pub camera2d: Camera2D,
    pub camera3d: Camera3D,
    sprite: SpriteRenderer,
    sprite_batch: SpriteBatch,
    pbr: PbrRenderer,
    textures: HashMap<Handle<Texture>, Texture>,
    texture_alloc: HandleAllocator<Texture>,
    meshes: HashMap<Handle<Mesh3DGpu>, Mesh3DGpu>,
    mesh_alloc: HandleAllocator<Mesh3DGpu>,
    target: RenderTarget,
    clear: Color,
}

impl Viewport {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let sprite = SpriteRenderer::new(device, format, SPRITE_SHADER);
        let pbr = PbrRenderer::new(device, queue, format, PBR_SHADER);
        let target = RenderTarget::new(device, 1280, 720, format);

        let mut viewport = Self {
            world: World::new(),
            camera2d: Camera2D::new(1280.0, 720.0),
            camera3d: Camera3D::new(1280.0 / 720.0),
            sprite,
            sprite_batch: SpriteBatch::new(),
            pbr,
            textures: HashMap::new(),
            texture_alloc: HandleAllocator::new(),
            meshes: HashMap::new(),
            mesh_alloc: HandleAllocator::new(),
            target,
            clear: Color::new(0.02, 0.02, 0.04, 1.0),
        };
        viewport.seed_demo(device, queue);
        viewport
    }

    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.target.color_view
    }

    /// Resize the offscreen target to match the panel. Returns `true` if the
    /// underlying texture was recreated (the egui texture id must be rebound).
    pub fn ensure_size(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let recreated = self.target.resize(device, width, height);
        if recreated {
            self.camera2d.viewport_size =
                Vec2::new(self.target.width as f32, self.target.height as f32);
            self.camera3d.aspect_ratio = self.target.width as f32 / self.target.height as f32;
        }
        recreated
    }

    /// Record the 3D (clear + depth) and 2D (overlay) passes into `encoder`,
    /// both targeting the offscreen color texture.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        self.sprite_batch.collect(&self.world, &self.camera2d);
        let view_proj_2d = self.camera2d.view_projection();
        let clear = wgpu::Color {
            r: self.clear.r as f64,
            g: self.clear.g as f64,
            b: self.clear.b as f64,
            a: self.clear.a as f64,
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_pbr_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.pbr.render(
                device,
                queue,
                &self.world,
                &self.meshes,
                &self.textures,
                &self.camera3d,
                &mut pass,
            );
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viewport_sprite_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.sprite
                .render(device, queue, &self.sprite_batch, view_proj_2d, &mut pass);
        }
    }

    fn create_texture_rgba(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
        width: u32,
        height: u32,
        label: &str,
    ) -> Handle<Texture> {
        let texture = Texture::from_rgba8(device, queue, pixels, width, height, label);
        let handle = self.texture_alloc.allocate();
        self.sprite.register_texture(device, handle, &texture);
        self.textures.insert(handle, texture);
        handle
    }

    fn create_cube(&mut self, device: &wgpu::Device, half_extent: f32) -> Handle<Mesh3DGpu> {
        let mesh = create_cube_mesh(device, half_extent);
        let handle = self.mesh_alloc.allocate();
        self.meshes.insert(handle, mesh);
        handle
    }

    /// Spawn a small starter scene (lit cube + 2D sprite grid) so the editor has
    /// something real to display and manipulate.
    fn seed_demo(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let cube = self.create_cube(device, 1.0);
        self.world.spawn((
            Mesh3D::new(cube),
            Transform::from_position(Vec3::ZERO),
            PbrMaterial {
                base_color: Color::from_hex(0x4477FF),
                metallic: 0.1,
                roughness: 0.35,
                ..Default::default()
            },
            Name::new("Cube"),
        ));
        self.world.spawn((
            DirectionalLight {
                direction: Vec3::new(-0.4, -1.0, -0.6).normalize(),
                color: Color::WHITE,
                intensity: 1.2,
            },
            Name::new("Sun"),
        ));
        self.camera3d.position = Vec3::new(0.0, 2.5, 6.0);
        self.camera3d.target = Vec3::ZERO;

        let texture =
            self.create_texture_rgba(device, queue, &checkerboard(64, 8), 64, 64, "checker");
        for i in 0..12 {
            let col = (i % 6) as f32 - 2.5;
            let mut sprite = Sprite::new(texture);
            sprite.size = Some(Vec2::splat(48.0));
            sprite.color = Color::from_hex(0x66FFAA);
            self.world.spawn((
                sprite,
                Transform::from_position_2d(col * 70.0, -300.0),
                Name::new(format!("Tile {i}")),
            ));
        }
        let mut player = Sprite::new(texture);
        player.size = Some(Vec2::splat(72.0));
        player.color = Color::WHITE;
        player.z_order = 10;
        self.world.spawn((
            player,
            Transform::from_position_2d(0.0, 280.0),
            Name::new("Player"),
        ));
    }
}

fn checkerboard(size: u32, cells: u32) -> Vec<u8> {
    let cell = (size / cells).max(1);
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let on = ((x / cell) + (y / cell)) % 2 == 0;
            let (r, g, b) = if on { (235, 235, 245) } else { (70, 70, 95) };
            pixels.extend_from_slice(&[r, g, b, 255]);
        }
    }
    pixels
}
