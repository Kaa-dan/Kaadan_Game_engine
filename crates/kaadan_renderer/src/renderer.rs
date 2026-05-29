use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
}

pub struct FrameContext {
    pub output: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

impl Renderer {
    /// Create renderer from a platform window.
    /// Uses pollster to block on async wgpu init (acceptable at startup).
    pub fn new(
        window: &(impl HasWindowHandle + HasDisplayHandle + ?Sized),
        width: u32,
        height: u32,
    ) -> Result<Self, kaadan_core::KaadanError> {
        pollster::block_on(Self::new_async(window, width, height))
    }

    async fn new_async(
        window: &(impl HasWindowHandle + HasDisplayHandle + ?Sized),
        width: u32,
        height: u32,
    ) -> Result<Self, kaadan_core::KaadanError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // SAFETY: The caller must ensure the window outlives the surface.
        // In our engine, the Renderer is always dropped before the window.
        let surface = unsafe {
            let raw_display = window
                .display_handle()
                .map_err(|e| kaadan_core::KaadanError::Renderer(e.to_string()))?
                .as_raw();
            let raw_window = window
                .window_handle()
                .map_err(|e| kaadan_core::KaadanError::Renderer(e.to_string()))?
                .as_raw();
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: raw_display,
                    raw_window_handle: raw_window,
                })
                .map_err(|e| kaadan_core::KaadanError::Renderer(e.to_string()))?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| kaadan_core::KaadanError::Renderer("No GPU adapter found".into()))?;

        tracing::info!("GPU adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kaadan_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| kaadan_core::KaadanError::Renderer(e.to_string()))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let (depth_texture, depth_view) = Self::create_depth_texture(&device, width, height);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            depth_texture,
            depth_view,
        })
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
            let (tex, view) = Self::create_depth_texture(&self.device, width, height);
            self.depth_texture = tex;
            self.depth_view = view;
        }
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Begin a frame: acquire surface texture, create command encoder.
    pub fn begin_frame(&self) -> Result<FrameContext, wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });
        Ok(FrameContext {
            output,
            view,
            encoder,
        })
    }

    /// Submit commands and present.
    pub fn end_frame(&self, frame: FrameContext) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.output.present();
    }
}
