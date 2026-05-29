use std::collections::HashMap;

use kaadan_ecs::{App, Resources, World};
use kaadan_input::InputState;
use kaadan_math::{Color, Handle, HandleAllocator, Mat4};
use kaadan_platform::{AppHandler, InputEvent, LifecycleEvent, PlatformWindow};
use kaadan_renderer::{
    Camera2D, Camera3D, Mesh3DGpu, PbrRenderer, Renderer, SpriteBatch, SpriteRenderer, Texture,
};

use crate::frame_pacer::{FramePacer, FrameStats};

type InitCallback = Box<dyn FnOnce(&mut EngineSetup)>;

/// GPU-side state, created once the window/surface exists.
struct RenderCtx {
    renderer: Renderer,
    sprite: SpriteRenderer,
    sprite_batch: SpriteBatch,
    pbr: PbrRenderer,
    textures: HashMap<Handle<Texture>, Texture>,
    texture_alloc: HandleAllocator<Texture>,
    meshes: HashMap<Handle<Mesh3DGpu>, Mesh3DGpu>,
    mesh_alloc: HandleAllocator<Mesh3DGpu>,
}

/// Borrowed view passed to the user's init callback so they can load GPU
/// assets and spawn entities once the renderer is live.
pub struct EngineSetup<'a> {
    pub world: &'a mut World,
    pub resources: &'a mut Resources,
    ctx: &'a mut RenderCtx,
}

impl EngineSetup<'_> {
    /// Decode image bytes into a GPU texture, register it for sprite drawing,
    /// and return its handle.
    pub fn load_texture(
        &mut self,
        bytes: &[u8],
        label: &str,
    ) -> Result<Handle<Texture>, kaadan_core::KaadanError> {
        let texture = Texture::from_bytes(
            &self.ctx.renderer.device,
            &self.ctx.renderer.queue,
            bytes,
            label,
        )?;
        let handle = self.ctx.texture_alloc.allocate();
        self.ctx
            .sprite
            .register_texture(&self.ctx.renderer.device, handle, &texture);
        self.ctx.textures.insert(handle, texture);
        Ok(handle)
    }

    /// Create a GPU texture from raw RGBA8 pixels (procedural textures).
    pub fn create_texture_rgba(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        label: &str,
    ) -> Handle<Texture> {
        let texture = Texture::from_rgba8(
            &self.ctx.renderer.device,
            &self.ctx.renderer.queue,
            pixels,
            width,
            height,
            label,
        );
        let handle = self.ctx.texture_alloc.allocate();
        self.ctx
            .sprite
            .register_texture(&self.ctx.renderer.device, handle, &texture);
        self.ctx.textures.insert(handle, texture);
        handle
    }

    /// Upload a procedural unit cube mesh and return its handle.
    pub fn create_cube(&mut self, half_extent: f32) -> Handle<Mesh3DGpu> {
        let mesh = kaadan_renderer::create_cube_mesh(&self.ctx.renderer.device, half_extent);
        let handle = self.ctx.mesh_alloc.allocate();
        self.ctx.meshes.insert(handle, mesh);
        handle
    }

    /// Load a `.glb`/`.gltf` model. Returns the first mesh's handle and the
    /// material it references (defaulting if absent). Requires the renderer's
    /// `gltf` feature.
    #[cfg(feature = "gltf")]
    pub fn load_gltf_first_mesh(
        &mut self,
        bytes: &[u8],
        label: &str,
    ) -> Result<(Handle<Mesh3DGpu>, kaadan_renderer::PbrMaterial), kaadan_core::KaadanError> {
        let model = kaadan_renderer::load_gltf(bytes, label)?;
        let mesh =
            model
                .meshes
                .into_iter()
                .next()
                .ok_or_else(|| kaadan_core::KaadanError::AssetLoad {
                    path: label.to_string(),
                    reason: "glTF contains no mesh primitives".to_string(),
                })?;
        let gpu = Mesh3DGpu::new(&self.ctx.renderer.device, &mesh.vertices, &mesh.indices);
        let handle = self.ctx.mesh_alloc.allocate();
        self.ctx.meshes.insert(handle, gpu);
        let material = mesh
            .material_index
            .and_then(|i| model.materials.into_iter().nth(i))
            .unwrap_or_default();
        Ok((handle, material))
    }

    pub fn camera2d(&mut self) -> Option<&mut Camera2D> {
        self.resources.get_mut::<Camera2D>()
    }

    pub fn camera3d(&mut self) -> Option<&mut Camera3D> {
        self.resources.get_mut::<Camera3D>()
    }
}

/// Top-level engine runtime. Implements the platform [`AppHandler`], owning the
/// ECS world, renderer, and per-frame input. Build it, register systems, then
/// hand it to [`kaadan_platform::run`].
pub struct Engine {
    app: App,
    render: Option<RenderCtx>,
    pacer: FramePacer,
    stats: FrameStats,
    should_exit: bool,
    clear_color: Color,
    on_init: Option<InitCallback>,
}

impl Engine {
    pub fn new(target_fps: u32) -> Self {
        let mut app = App::new();
        app.insert_resource(InputState::new());
        Self {
            app,
            render: None,
            pacer: FramePacer::new(target_fps),
            stats: FrameStats::new(),
            should_exit: false,
            clear_color: Color::new(0.05, 0.05, 0.08, 1.0),
            on_init: None,
        }
    }

    /// Run a one-time setup callback after the renderer is initialized.
    pub fn on_init<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut EngineSetup) + 'static,
    {
        self.on_init = Some(Box::new(f));
        self
    }

    /// Register a per-frame system.
    pub fn add_system(
        mut self,
        name: impl Into<String>,
        system: impl FnMut(&mut World, &mut Resources) + 'static,
    ) -> Self {
        self.app.add_system(name, system);
        self
    }

    /// Insert a global resource.
    pub fn insert_resource<T: 'static>(mut self, resource: T) -> Self {
        self.app.insert_resource(resource);
        self
    }

    pub fn with_clear_color(mut self, color: Color) -> Self {
        self.clear_color = color;
        self
    }

    /// Average FPS over the last frames.
    pub fn fps(&self) -> f32 {
        self.stats.average_fps()
    }
}

impl AppHandler for Engine {
    fn init(&mut self, window: &dyn PlatformWindow) {
        let width = window.width().max(1);
        let height = window.height().max(1);

        let renderer = match Renderer::new(window, width, height) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("renderer initialization failed: {e}");
                self.should_exit = true;
                return;
            }
        };
        let sprite = SpriteRenderer::new(
            &renderer.device,
            renderer.surface_format(),
            kaadan_renderer::SPRITE_SHADER,
        );
        let pbr = PbrRenderer::new(
            &renderer.device,
            &renderer.queue,
            renderer.surface_format(),
            kaadan_renderer::PBR_SHADER,
        );

        self.app
            .resources
            .insert(Camera2D::new(width as f32, height as f32));
        self.app
            .resources
            .insert(Camera3D::new(width as f32 / height as f32));
        self.app
            .resources
            .insert(kaadan_ui::UiScreen::new(width as f32, height as f32));

        let mut ctx = RenderCtx {
            renderer,
            sprite,
            sprite_batch: SpriteBatch::new(),
            pbr,
            textures: HashMap::new(),
            texture_alloc: HandleAllocator::new(),
            meshes: HashMap::new(),
            mesh_alloc: HandleAllocator::new(),
        };

        if let Some(callback) = self.on_init.take() {
            let mut setup = EngineSetup {
                world: &mut self.app.world,
                resources: &mut self.app.resources,
                ctx: &mut ctx,
            };
            callback(&mut setup);
        }

        self.render = Some(ctx);
        tracing::info!("engine initialized at {width}x{height}");
    }

    fn update(&mut self, events: &[InputEvent], dt: f32) {
        self.pacer.begin_frame();

        if events
            .iter()
            .any(|e| matches!(e, InputEvent::CloseRequested))
        {
            self.should_exit = true;
        }
        if let Some(input) = self.app.resources.get_mut::<InputState>() {
            input.begin_frame();
            for event in events {
                input.process_event(event);
            }
        }

        self.app.tick();

        let clear = self.clear_color;
        if let Some(ctx) = self.render.as_mut() {
            let cam2d = self.app.resources.get::<Camera2D>();
            let cam3d = self.app.resources.get::<Camera3D>();
            if let (Some(cam2d), Some(cam3d)) = (cam2d, cam3d) {
                ctx.sprite_batch.collect(&self.app.world, cam2d);
                let view_proj = cam2d.view_projection();
                render_frame(
                    ctx,
                    &self.app.world,
                    cam3d,
                    view_proj,
                    clear,
                    &mut self.should_exit,
                );
            }
        }

        self.stats.record_frame(dt);
    }

    fn resize(&mut self, width: u32, height: u32) {
        let Some(ctx) = self.render.as_mut() else {
            return;
        };
        let width = width.max(1);
        let height = height.max(1);
        ctx.renderer.resize(width, height);
        if let Some(cam) = self.app.resources.get_mut::<Camera2D>() {
            cam.viewport_size = kaadan_math::Vec2::new(width as f32, height as f32);
        }
        if let Some(cam) = self.app.resources.get_mut::<Camera3D>() {
            cam.aspect_ratio = width as f32 / height as f32;
        }
        if let Some(screen) = self.app.resources.get_mut::<kaadan_ui::UiScreen>() {
            screen.size = kaadan_math::Vec2::new(width as f32, height as f32);
        }
    }

    fn lifecycle(&mut self, event: LifecycleEvent) {
        tracing::debug!("lifecycle event: {event:?}");
    }

    fn should_exit(&self) -> bool {
        self.should_exit
    }
}

fn render_frame(
    ctx: &mut RenderCtx,
    world: &World,
    cam3d: &Camera3D,
    view_proj_2d: Mat4,
    clear: Color,
    should_exit: &mut bool,
) {
    match ctx.renderer.begin_frame() {
        Ok(mut frame) => {
            let clear_color = wgpu::Color {
                r: clear.r as f64,
                g: clear.g as f64,
                b: clear.b as f64,
                a: clear.a as f64,
            };

            // 3D pass: clears color + depth and writes depth.
            {
                let mut pass = frame
                    .encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("pbr_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(clear_color),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &ctx.renderer.depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                ctx.pbr.render(
                    &ctx.renderer.device,
                    &ctx.renderer.queue,
                    world,
                    &ctx.meshes,
                    &ctx.textures,
                    cam3d,
                    &mut pass,
                );
            }

            // 2D pass: depth-less overlay; loads (keeps) the 3D color result.
            {
                let mut pass = frame
                    .encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sprite_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &frame.view,
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
                ctx.sprite.render(
                    &ctx.renderer.device,
                    &ctx.renderer.queue,
                    &ctx.sprite_batch,
                    view_proj_2d,
                    &mut pass,
                );
            }

            ctx.renderer.end_frame(frame);
        }
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            let w = ctx.renderer.surface_config.width;
            let h = ctx.renderer.surface_config.height;
            ctx.renderer.resize(w, h);
        }
        Err(wgpu::SurfaceError::OutOfMemory) => *should_exit = true,
        Err(e) => tracing::warn!("surface error: {e:?}"),
    }
}
