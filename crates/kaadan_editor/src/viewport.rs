//! The scene viewport: owns an ECS world and the 2D/3D renderers, draws the
//! scene into an offscreen [`RenderTarget`], and exposes its color view so the
//! UI layer can show it inside an egui panel. Also owns the asset registry that
//! maps GPU handles to serializable [`TextureSource`]/[`MeshSource`] descriptors.

use std::collections::HashMap;

use kaadan_ecs::{Entity, World};
use kaadan_math::{Color, Handle, HandleAllocator, Transform, Vec2, Vec3};
use kaadan_renderer::{
    create_cube_mesh, Camera2D, Camera3D, DirectionalLight, Mesh3D, Mesh3DGpu, PbrMaterial,
    PbrRenderer, PointLight, RenderTarget, Sprite, SpriteBatch, SpriteRenderer, Texture,
    PBR_SHADER, SPRITE_SHADER,
};
use kaadan_scene::{set_parent, Children, Parent};

use crate::components::Name;
use crate::scene_io::{
    DirLightDesc, EditorScene, EntityDesc, MaterialDesc, MeshSource, PointLightDesc, SpriteDesc,
    TextureSource, TransformDesc,
};

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
    // Asset registry: dedup by source key, plus reverse maps for serialization.
    texture_keys: HashMap<String, Handle<Texture>>,
    texture_sources: HashMap<Handle<Texture>, TextureSource>,
    mesh_keys: HashMap<String, Handle<Mesh3DGpu>>,
    mesh_sources: HashMap<Handle<Mesh3DGpu>, MeshSource>,
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
            texture_keys: HashMap::new(),
            texture_sources: HashMap::new(),
            mesh_keys: HashMap::new(),
            mesh_sources: HashMap::new(),
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

    // --- Asset registry ---------------------------------------------------

    pub fn get_or_create_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &TextureSource,
    ) -> Handle<Texture> {
        let key = source.key();
        if let Some(handle) = self.texture_keys.get(&key) {
            return *handle;
        }
        let texture = build_texture(device, queue, source);
        let handle = self.texture_alloc.allocate();
        self.sprite.register_texture(device, handle, &texture);
        self.textures.insert(handle, texture);
        self.texture_keys.insert(key, handle);
        self.texture_sources.insert(handle, source.clone());
        handle
    }

    pub fn get_or_create_mesh(
        &mut self,
        device: &wgpu::Device,
        source: &MeshSource,
    ) -> Handle<Mesh3DGpu> {
        let key = source.key();
        if let Some(handle) = self.mesh_keys.get(&key) {
            return *handle;
        }
        let mesh = match source {
            MeshSource::Cube { half_extent } => create_cube_mesh(device, *half_extent),
            MeshSource::Gltf { path } => {
                tracing::warn!("glTF loading not supported in editor yet ({path}); using cube");
                create_cube_mesh(device, 1.0)
            }
        };
        let handle = self.mesh_alloc.allocate();
        self.meshes.insert(handle, mesh);
        self.mesh_keys.insert(key, handle);
        self.mesh_sources.insert(handle, source.clone());
        handle
    }

    fn texture_source(&self, handle: Handle<Texture>) -> Option<TextureSource> {
        self.texture_sources.get(&handle).cloned()
    }

    fn mesh_source(&self, handle: Handle<Mesh3DGpu>) -> Option<MeshSource> {
        self.mesh_sources.get(&handle).cloned()
    }

    // --- Scene save / load ------------------------------------------------

    pub fn save_scene(&self, path: &str) -> Result<(), String> {
        let scene = self.to_scene();
        let ron = scene.to_ron()?;
        std::fs::write(path, ron).map_err(|e| e.to_string())?;
        tracing::info!("saved scene to {path}");
        Ok(())
    }

    pub fn load_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> Result<(), String> {
        let ron = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let scene = EditorScene::from_ron(&ron)?;
        let roots = scene.entities.len();
        self.apply_scene(device, queue, &scene);
        tracing::info!("loaded scene from {path} ({roots} roots)");
        Ok(())
    }

    /// Replace the current world contents with `scene` (used by load and by
    /// Play/Stop to restore the pre-play snapshot).
    pub fn apply_scene(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &EditorScene) {
        let all: Vec<Entity> = self.world.inner().iter().map(|e| e.entity()).collect();
        for entity in all {
            let _ = self.world.despawn(entity);
        }
        for desc in &scene.entities {
            self.spawn_desc(device, queue, desc, None);
        }
    }

    pub fn to_scene(&self) -> EditorScene {
        let mut roots: Vec<Entity> = self
            .world
            .inner()
            .iter()
            .filter(|e| !e.has::<Parent>())
            .map(|e| e.entity())
            .collect();
        roots.sort_by_key(|e| e.id());
        let entities = roots.iter().map(|&e| self.entity_desc(e)).collect();
        EditorScene {
            name: "scene".to_string(),
            entities,
        }
    }

    fn entity_desc(&self, entity: Entity) -> EntityDesc {
        let name = self.world.get::<Name>(entity).ok().map(|n| n.0.clone());
        let transform = self
            .world
            .get::<Transform>(entity)
            .ok()
            .map(|t| TransformDesc::from(&*t));
        let sprite = self.world.get::<Sprite>(entity).ok().and_then(|s| {
            self.texture_source(s.texture).map(|texture| SpriteDesc {
                texture,
                color: s.color.to_array(),
                size: s.size.map(|v| [v.x, v.y]),
                anchor: [s.anchor.x, s.anchor.y],
                z_order: s.z_order,
                flip_x: s.flip_x,
                flip_y: s.flip_y,
            })
        });
        let mesh = self
            .world
            .get::<Mesh3D>(entity)
            .ok()
            .and_then(|m| self.mesh_source(m.handle));
        let material = self.world.get::<PbrMaterial>(entity).ok().map(|m| {
            let emissive = m.emissive.to_array();
            MaterialDesc {
                base_color: m.base_color.to_array(),
                base_color_texture: m.base_color_texture.and_then(|h| self.texture_source(h)),
                metallic: m.metallic,
                roughness: m.roughness,
                emissive: [emissive[0], emissive[1], emissive[2]],
            }
        });
        let dir_light = self
            .world
            .get::<DirectionalLight>(entity)
            .ok()
            .map(|d| DirLightDesc::from(&*d));
        let point_light = self
            .world
            .get::<PointLight>(entity)
            .ok()
            .map(|p| PointLightDesc::from(&*p));
        let children = self
            .world
            .get::<Children>(entity)
            .ok()
            .map(|c| c.0.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|&c| self.world.is_alive(c))
            .map(|c| self.entity_desc(c))
            .collect();
        EntityDesc {
            name,
            transform,
            sprite,
            mesh,
            material,
            dir_light,
            point_light,
            children,
        }
    }

    fn spawn_desc(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        desc: &EntityDesc,
        parent: Option<Entity>,
    ) -> Entity {
        // Resolve GPU assets first (each borrows &mut self), then build the entity.
        let sprite = desc.sprite.as_ref().map(|s| {
            let handle = self.get_or_create_texture(device, queue, &s.texture);
            s.to_sprite(handle)
        });
        let mesh = desc
            .mesh
            .as_ref()
            .map(|m| self.get_or_create_mesh(device, m));
        let material = desc.material.as_ref().map(|m| {
            let base_color_texture = m
                .base_color_texture
                .as_ref()
                .map(|t| self.get_or_create_texture(device, queue, t));
            PbrMaterial {
                base_color: Color::new(
                    m.base_color[0],
                    m.base_color[1],
                    m.base_color[2],
                    m.base_color[3],
                ),
                base_color_texture,
                metallic: m.metallic,
                roughness: m.roughness,
                metallic_roughness_texture: None,
                normal_texture: None,
                emissive: Color::new(m.emissive[0], m.emissive[1], m.emissive[2], 1.0),
                emissive_texture: None,
            }
        });

        let entity = self.world.spawn(());
        {
            let w = self.world.inner_mut();
            if let Some(name) = &desc.name {
                let _ = w.insert_one(entity, Name::new(name.clone()));
            }
            if let Some(t) = &desc.transform {
                let _ = w.insert_one(entity, Transform::from(t));
            }
            if let Some(s) = sprite {
                let _ = w.insert_one(entity, s);
            }
            if let Some(handle) = mesh {
                let _ = w.insert_one(entity, Mesh3D::new(handle));
            }
            if let Some(m) = material {
                let _ = w.insert_one(entity, m);
            }
            if let Some(d) = &desc.dir_light {
                let _ = w.insert_one(entity, DirectionalLight::from(d));
            }
            if let Some(p) = &desc.point_light {
                let _ = w.insert_one(entity, PointLight::from(p));
            }
        }
        if let Some(parent) = parent {
            set_parent(&mut self.world, entity, parent);
        }
        for child in &desc.children {
            self.spawn_desc(device, queue, child, Some(entity));
        }
        entity
    }

    /// Spawn a small starter scene (lit cube + 2D sprite grid) so the editor has
    /// something real to display and manipulate.
    fn seed_demo(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let cube = self.get_or_create_mesh(device, &MeshSource::Cube { half_extent: 1.0 });
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

        let texture = self.get_or_create_texture(
            device,
            queue,
            &TextureSource::Checker { size: 64, cells: 8 },
        );
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

fn build_texture(device: &wgpu::Device, queue: &wgpu::Queue, source: &TextureSource) -> Texture {
    match source {
        TextureSource::Checker { size, cells } => Texture::from_rgba8(
            device,
            queue,
            &checkerboard(*size, *cells),
            *size,
            *size,
            "checker",
        ),
        TextureSource::Solid { rgba } => Texture::from_rgba8(device, queue, rgba, 1, 1, "solid"),
        TextureSource::File { path } => match std::fs::read(path) {
            Ok(bytes) => Texture::from_bytes(device, queue, &bytes, path).unwrap_or_else(|e| {
                tracing::error!("failed to decode texture {path}: {e}");
                missing_texture(device, queue)
            }),
            Err(e) => {
                tracing::error!("failed to read texture {path}: {e}");
                missing_texture(device, queue)
            }
        },
    }
}

fn missing_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> Texture {
    Texture::from_rgba8(device, queue, &[255, 0, 255, 255], 1, 1, "missing")
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
