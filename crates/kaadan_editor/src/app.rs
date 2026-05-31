//! Dedicated winit event loop for the editor. Unlike `kaadan_app::Engine`, this
//! owns the raw `Window` and `WindowEvent`s so egui-winit can consume them.

use std::sync::Arc;
use std::time::Instant;

use egui_wgpu::ScreenDescriptor;
use kaadan_renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::state::EditorState;
use crate::viewport::Viewport;

/// GPU + egui state, created once the window exists on `resumed`.
struct Gfx {
    window: Arc<Window>,
    renderer: Renderer,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    viewport: Viewport,
    /// egui handle to the viewport's offscreen color texture.
    viewport_tex: Option<egui::TextureId>,
    /// Timestamp of the previous frame, for play-mode delta time.
    last_frame: Instant,
}

#[derive(Default)]
pub struct EditorApp {
    gfx: Option<Gfx>,
    state: EditorState,
}

impl ApplicationHandler for EditorApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("KaadanEngine Editor")
            .with_inner_size(LogicalSize::new(1280.0, 800.0));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        let size = window.inner_size();
        let renderer = Renderer::new(window.as_ref(), size.width.max(1), size.height.max(1))
            .expect("failed to create renderer");

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            None,
            Some(renderer.device.limits().max_texture_dimension_2d as usize),
        );
        let egui_renderer =
            egui_wgpu::Renderer::new(&renderer.device, renderer.surface_format(), None, 1, false);

        let viewport = Viewport::new(&renderer.device, &renderer.queue, renderer.surface_format());

        self.gfx = Some(Gfx {
            window,
            renderer,
            egui_ctx,
            egui_state,
            egui_renderer,
            viewport,
            viewport_tex: None,
            last_frame: Instant::now(),
        });
        tracing::info!("editor initialized at {}x{}", size.width, size.height);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(gfx) = self.gfx.as_mut() else {
            return;
        };

        let response = gfx.egui_state.on_window_event(&gfx.window, &event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                gfx.renderer.resize(size.width, size.height);
                gfx.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                gfx.render(&mut self.state);
                if self.state.should_exit {
                    event_loop.exit();
                }
            }
            _ => {
                if response.repaint {
                    gfx.window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gfx) = self.gfx.as_ref() {
            gfx.window.request_redraw();
        }
    }
}

impl Gfx {
    fn render(&mut self, state: &mut EditorState) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        if state.playing {
            crate::play::tick(&mut self.viewport.world, dt);
        }

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let viewport_tex = self.viewport_tex;
        let world = &mut self.viewport.world;
        let cam2d = &self.viewport.camera2d;
        let cam3d = &self.viewport.camera3d;
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            crate::ui::build(ctx, state, world, cam2d, cam3d, viewport_tex)
        });
        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);

        // Save/load needs the GPU device (to rebuild assets), so it runs here
        // rather than inside the egui closure.
        if let Some(request) = state.io_request.take() {
            let path = "kaadan_scene.ron";
            match request {
                crate::scene_io::IoRequest::Save => {
                    if let Err(e) = self.viewport.save_scene(path) {
                        tracing::error!("save failed: {e}");
                    }
                }
                crate::scene_io::IoRequest::Load => {
                    match self.viewport.load_scene(
                        &self.renderer.device,
                        &self.renderer.queue,
                        path,
                    ) {
                        Ok(()) => state.selected = None,
                        Err(e) => tracing::error!("load failed: {e}"),
                    }
                }
            }
        }

        // Play/Stop: snapshot on start, restore on stop.
        if let Some(request) = state.play_request.take() {
            match request {
                crate::play::PlayRequest::Start => {
                    state.play_snapshot = Some(self.viewport.to_scene());
                    state.playing = true;
                }
                crate::play::PlayRequest::Stop => {
                    if let Some(scene) = state.play_snapshot.take() {
                        self.viewport.apply_scene(
                            &self.renderer.device,
                            &self.renderer.queue,
                            &scene,
                        );
                    }
                    state.playing = false;
                    state.selected = None;
                    state.gizmo_drag = None;
                }
            }
        }

        let ppp = full_output.pixels_per_point;

        // Size the offscreen target to the viewport panel (points -> pixels) and
        // (re)bind it to a stable egui texture id.
        let target_w = (state.viewport_size.x * ppp).round().max(1.0) as u32;
        let target_h = (state.viewport_size.y * ppp).round().max(1.0) as u32;
        let recreated = self
            .viewport
            .ensure_size(&self.renderer.device, target_w, target_h);
        match self.viewport_tex {
            None => {
                let id = self.egui_renderer.register_native_texture(
                    &self.renderer.device,
                    self.viewport.color_view(),
                    wgpu::FilterMode::Linear,
                );
                self.viewport_tex = Some(id);
            }
            Some(id) if recreated => {
                self.egui_renderer.update_egui_texture_from_wgpu_texture(
                    &self.renderer.device,
                    self.viewport.color_view(),
                    wgpu::FilterMode::Linear,
                    id,
                );
            }
            _ => {}
        }

        let paint_jobs = self.egui_ctx.tessellate(full_output.shapes, ppp);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [
                self.renderer.surface_config.width,
                self.renderer.surface_config.height,
            ],
            pixels_per_point: ppp,
        };

        let mut frame = match self.renderer.begin_frame() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                let (w, h) = (
                    self.renderer.surface_config.width,
                    self.renderer.surface_config.height,
                );
                self.renderer.resize(w, h);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                state.should_exit = true;
                return;
            }
            Err(e) => {
                tracing::warn!("surface error: {e:?}");
                return;
            }
        };

        // Draw the scene into the offscreen target first; the egui pass below
        // samples it via `viewport_tex`.
        self.viewport.render(
            &self.renderer.device,
            &self.renderer.queue,
            &mut frame.encoder,
        );

        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(
                &self.renderer.device,
                &self.renderer.queue,
                *id,
                image_delta,
            );
        }
        let prep_buffers = self.egui_renderer.update_buffers(
            &self.renderer.device,
            &self.renderer.queue,
            &mut frame.encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut pass = frame
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.02,
                                g: 0.02,
                                b: 0.03,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.egui_renderer
                .render(&mut pass, &paint_jobs, &screen_descriptor);
        }

        // egui's prep command buffers (texture uploads) must run before the pass.
        self.renderer.queue.submit(prep_buffers);
        self.renderer.end_frame(frame);

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
    }
}
