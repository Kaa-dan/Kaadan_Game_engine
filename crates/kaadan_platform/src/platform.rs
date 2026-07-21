use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::input_event::{InputEvent, LifecycleEvent};

/// Configuration for window creation.
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "KaadanEngine".to_string(),
            width: 800,
            height: 600,
            resizable: true,
        }
    }
}

/// Callback driven by the platform event loop.
pub trait AppHandler {
    /// Called once when the window is ready and the surface is first available.
    /// Full one-time setup (GPU device, pipelines, initial scene) lives here.
    fn init(&mut self, window: &dyn PlatformWindow);
    /// Called every frame with accumulated input events.
    fn update(&mut self, events: &[InputEvent], dt: f32);
    /// Called when the surface is resized.
    fn resize(&mut self, width: u32, height: u32);
    /// Called on lifecycle events (suspend/resume).
    fn lifecycle(&mut self, event: LifecycleEvent);
    /// A rendering surface became available again *after* a suspend. On mobile
    /// (Android) the native window — and thus the surface — is destroyed when
    /// the app is backgrounded and a fresh one is handed back on resume; the
    /// handler must rebuild its surface from `window`. [`init`](Self::init)
    /// covers the very first surface, this covers every subsequent one.
    ///
    /// Default: no-op — desktop keeps a single surface for its whole lifetime.
    fn surface_created(&mut self, _window: &dyn PlatformWindow) {}
    /// The surface is about to be destroyed (mobile suspend). The handler should
    /// drop its surface; the GPU device and resources remain valid. Default: no-op.
    fn surface_destroyed(&mut self) {}
    /// Return true to exit the event loop.
    fn should_exit(&self) -> bool;
}

/// Abstraction over the platform window.
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn scale_factor(&self) -> f64;
}
