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
    /// Called once when the window is ready and surface is available.
    fn init(&mut self, window: &dyn PlatformWindow);
    /// Called every frame with accumulated input events.
    fn update(&mut self, events: &[InputEvent], dt: f32);
    /// Called when the surface is resized.
    fn resize(&mut self, width: u32, height: u32);
    /// Called on lifecycle events (suspend/resume).
    fn lifecycle(&mut self, event: LifecycleEvent);
    /// Return true to exit the event loop.
    fn should_exit(&self) -> bool;
}

/// Abstraction over the platform window.
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn scale_factor(&self) -> f64;
}
