use std::sync::Arc;
use std::time::Instant;

use raw_window_handle::{DisplayHandle, HasDisplayHandle, HasWindowHandle, WindowHandle};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::input_event::*;
use crate::platform::{AppHandler, PlatformWindow, WindowConfig};

/// Wrapper around a winit Window that implements PlatformWindow.
struct DesktopWindow {
    window: Arc<Window>,
}

impl HasWindowHandle for DesktopWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        self.window.window_handle()
    }
}

impl HasDisplayHandle for DesktopWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, raw_window_handle::HandleError> {
        self.window.display_handle()
    }
}

impl PlatformWindow for DesktopWindow {
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

struct WinitApp<H: AppHandler> {
    config: WindowConfig,
    handler: H,
    window: Option<Arc<Window>>,
    last_frame: Instant,
    pending_events: Vec<InputEvent>,
    initialized: bool,
}

impl<H: AppHandler> WinitApp<H> {
    fn new(config: WindowConfig, handler: H) -> Self {
        Self {
            config,
            handler,
            window: None,
            last_frame: Instant::now(),
            pending_events: Vec::new(),
            initialized: false,
        }
    }
}

impl<H: AppHandler> ApplicationHandler for WinitApp<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title(&self.config.title)
                .with_inner_size(winit::dpi::LogicalSize::new(
                    self.config.width,
                    self.config.height,
                ))
                .with_resizable(self.config.resizable);

            let window = Arc::new(
                event_loop
                    .create_window(attrs)
                    .expect("Failed to create window"),
            );
            self.window = Some(window.clone());

            if !self.initialized {
                let desktop_window = DesktopWindow { window };
                self.handler.init(&desktop_window);
                self.initialized = true;
            }

            self.last_frame = Instant::now();
        }

        self.handler.lifecycle(LifecycleEvent::Resumed);
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.handler.lifecycle(LifecycleEvent::Suspended);
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
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                let key = convert_key(&event.logical_key);
                self.pending_events
                    .push(InputEvent::Key(KeyEvent { key, pressed }));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pending_events.push(InputEvent::Touch(TouchEvent {
                    id: 0,
                    phase: TouchPhase::Moved,
                    position: kaadan_math::Vec2::new(position.x as f32, position.y as f32),
                }));
            }
            WindowEvent::MouseInput { state, .. } => {
                let phase = match state {
                    ElementState::Pressed => TouchPhase::Started,
                    ElementState::Released => TouchPhase::Ended,
                };
                // Position will be filled from last CursorMoved
                self.pending_events.push(InputEvent::Touch(TouchEvent {
                    id: 0,
                    phase,
                    position: kaadan_math::Vec2::ZERO,
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

fn convert_key(key: &Key) -> KeyCode {
    match key {
        Key::Character(c) => match c.as_str() {
            "a" | "A" => KeyCode::A,
            "b" | "B" => KeyCode::B,
            "c" | "C" => KeyCode::C,
            "d" | "D" => KeyCode::D,
            "e" | "E" => KeyCode::E,
            "f" | "F" => KeyCode::F,
            "g" | "G" => KeyCode::G,
            "h" | "H" => KeyCode::H,
            "i" | "I" => KeyCode::I,
            "j" | "J" => KeyCode::J,
            "k" | "K" => KeyCode::K,
            "l" | "L" => KeyCode::L,
            "m" | "M" => KeyCode::M,
            "n" | "N" => KeyCode::N,
            "o" | "O" => KeyCode::O,
            "p" | "P" => KeyCode::P,
            "q" | "Q" => KeyCode::Q,
            "r" | "R" => KeyCode::R,
            "s" | "S" => KeyCode::S,
            "t" | "T" => KeyCode::T,
            "u" | "U" => KeyCode::U,
            "v" | "V" => KeyCode::V,
            "w" | "W" => KeyCode::W,
            "x" | "X" => KeyCode::X,
            "y" | "Y" => KeyCode::Y,
            "z" | "Z" => KeyCode::Z,
            "0" => KeyCode::Key0,
            "1" => KeyCode::Key1,
            "2" => KeyCode::Key2,
            "3" => KeyCode::Key3,
            "4" => KeyCode::Key4,
            "5" => KeyCode::Key5,
            "6" => KeyCode::Key6,
            "7" => KeyCode::Key7,
            "8" => KeyCode::Key8,
            "9" => KeyCode::Key9,
            _ => KeyCode::Unknown,
        },
        Key::Named(named) => match named {
            NamedKey::Space => KeyCode::Space,
            NamedKey::Enter => KeyCode::Enter,
            NamedKey::Escape => KeyCode::Escape,
            NamedKey::Backspace => KeyCode::Backspace,
            NamedKey::Tab => KeyCode::Tab,
            NamedKey::ArrowUp => KeyCode::ArrowUp,
            NamedKey::ArrowDown => KeyCode::ArrowDown,
            NamedKey::ArrowLeft => KeyCode::ArrowLeft,
            NamedKey::ArrowRight => KeyCode::ArrowRight,
            NamedKey::Shift => KeyCode::ShiftLeft,
            NamedKey::Control => KeyCode::ControlLeft,
            _ => KeyCode::Unknown,
        },
        _ => KeyCode::Unknown,
    }
}

/// Run the engine on desktop using winit.
pub fn run(config: WindowConfig, handler: impl AppHandler + 'static) {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = WinitApp::new(config, handler);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
