//! Platform abstraction for windowing, event loops, and OS lifecycle.
//!
//! Wraps [`winit`] to provide a unified interface across Android, iOS, and desktop.

#[cfg(target_os = "android")]
mod android;
mod input_event;
mod platform;

#[cfg(not(target_os = "android"))]
mod desktop;

pub use input_event::*;
pub use platform::*;

#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

/// Run the engine with the given config and handler on desktop (Linux/macOS/
/// Windows). On Android, use [`run_android`] instead — its entry point receives
/// the `AndroidApp` from the platform.
#[cfg(not(target_os = "android"))]
pub fn run(config: WindowConfig, handler: impl AppHandler + 'static) {
    desktop::run(config, handler);
}

/// Run the engine on Android. Call this from the game's `android_main`, passing
/// the [`AndroidApp`] the `android-activity` glue provides.
#[cfg(target_os = "android")]
pub fn run_android(
    android_app: AndroidApp,
    config: WindowConfig,
    handler: impl AppHandler + 'static,
) {
    android::run(android_app, config, handler);
}
