use std::sync::Arc;
use std::time::Instant;

use raw_window_handle::{DisplayHandle, HasDisplayHandle, HasWindowHandle, WindowHandle};
use winit::application::ApplicationHandler;
use winit::event::{TouchPhase as WinitTouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::android::activity::AndroidApp;
use winit::platform::android::EventLoopBuilderExtAndroid;
use winit::window::{Window, WindowAttributes, WindowId};

use crate::input_event::*;
use crate::platform::{AppHandler, PlatformWindow, WindowConfig};

struct AndroidWindow {
    window: Arc<Window>,
}

impl HasWindowHandle for AndroidWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        self.window.window_handle()
    }
}

impl HasDisplayHandle for AndroidWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.window.display_handle()
    }
}

impl PlatformWindow for AndroidWindow {
    fn width(&self) -> u32 {
        self.window.inner_size().width
    }

    fn height(&self) -> u32 {
        self.window.inner_size().height
    }

    fn scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }
}

struct AndroidWinitApp<H: AppHandler> {
    handler: H,
    window: Option<Arc<Window>>,
    last_frame: Instant,
    pending_events: Vec<InputEvent>,
    initialized: bool,
}

impl<H: AppHandler> AndroidWinitApp<H> {
    fn new(handler: H) -> Self {
        Self {
            handler,
            window: None,
            last_frame: Instant::now(),
            pending_events: Vec::new(),
            initialized: false,
        }
    }
}

impl<H: AppHandler> ApplicationHandler for AndroidWinitApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Android hands us a fresh native window each time the app returns to the
        // foreground. Create it, then either run one-time init (first time) or
        // rebuild the rendering surface against the new window (subsequent times).
        let window = Arc::new(
            event_loop
                .create_window(WindowAttributes::default())
                .expect("Failed to create Android window"),
        );
        self.window = Some(window.clone());
        let platform_window = AndroidWindow { window };

        if !self.initialized {
            self.handler.init(&platform_window);
            self.initialized = true;
        } else {
            self.handler.surface_created(&platform_window);
        }

        self.handler.lifecycle(LifecycleEvent::Resumed);
        self.last_frame = Instant::now();
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // The native window is about to be destroyed: drop our surface first, then
        // release the window handle.
        self.handler.surface_destroyed();
        self.handler.lifecycle(LifecycleEvent::Suspended);
        self.window = None;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.pending_events.push(InputEvent::CloseRequested);
            }
            WindowEvent::Resized(size) => {
                self.handler.resize(size.width, size.height);
                self.pending_events.push(InputEvent::Resize {
                    width: size.width,
                    height: size.height,
                });
            }
            WindowEvent::Touch(touch) => {
                let phase = match touch.phase {
                    WinitTouchPhase::Started => TouchPhase::Started,
                    WinitTouchPhase::Moved => TouchPhase::Moved,
                    WinitTouchPhase::Ended => TouchPhase::Ended,
                    WinitTouchPhase::Cancelled => TouchPhase::Cancelled,
                };
                self.pending_events.push(InputEvent::Touch(TouchEvent {
                    id: touch.id,
                    phase,
                    position: kaadan_math::Vec2::new(
                        touch.location.x as f32,
                        touch.location.y as f32,
                    ),
                }));
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;

                let events: Vec<InputEvent> = self.pending_events.drain(..).collect();
                self.handler.update(&events, dt);

                if self.handler.should_exit() {
                    event_loop.exit();
                    return;
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// Run the engine on Android, driven by the `AndroidApp` passed to `android_main`.
///
/// `config` is accepted for parity with [`crate::desktop::run`]; Android windows
/// are full-screen so title/size are not applied.
pub fn run(android_app: AndroidApp, _config: WindowConfig, handler: impl AppHandler + 'static) {
    let event_loop = EventLoop::builder()
        .with_android_app(android_app)
        .build()
        .expect("Failed to create Android event loop");
    let mut app = AndroidWinitApp::new(handler);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
