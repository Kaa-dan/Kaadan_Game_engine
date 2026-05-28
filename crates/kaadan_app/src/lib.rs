//! Application framework and engine orchestration.
//!
//! Ties together all KaadanEngine subsystems into a cohesive runtime.

mod frame_pacer;
mod lifecycle;

pub use frame_pacer::{FramePacer, FrameStats};
pub use lifecycle::AppLifecycleState;

// Re-export all engine crates for convenient access
pub use kaadan_assets;
pub use kaadan_audio;
pub use kaadan_core;
pub use kaadan_ecs;
pub use kaadan_input;
pub use kaadan_math;
pub use kaadan_physics;
pub use kaadan_platform;
pub use kaadan_renderer;
pub use kaadan_scene;
pub use kaadan_ui;
