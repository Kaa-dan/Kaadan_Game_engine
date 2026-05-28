//! Platform abstraction for windowing, event loops, and OS lifecycle.
//!
//! Wraps [`winit`] to provide a unified interface across Android, iOS, and desktop.

mod desktop;
mod input_event;
mod platform;

pub use input_event::*;
pub use platform::*;

/// Run the engine with the given config and handler on the current platform.
pub fn run(config: WindowConfig, handler: impl AppHandler + 'static) {
    desktop::run(config, handler);
}
