//! Application framework and engine orchestration.
//!
//! Ties together all KaadanEngine subsystems into a cohesive runtime.

mod engine;
mod frame_pacer;
mod lifecycle;

pub use engine::{Engine, EngineSetup};
pub use frame_pacer::{FramePacer, FrameStats, ThermalState};
pub use lifecycle::{AppLifecycleState, LifecycleManager};

#[cfg(feature = "profiling")]
#[doc(hidden)]
pub use puffin as __puffin;

/// Mark a profiling scope. With the `profiling` feature enabled this emits a
/// [`puffin`] scope spanning the enclosing block; otherwise it compiles away.
#[macro_export]
macro_rules! profile_scope {
    ($name:expr) => {
        #[cfg(feature = "profiling")]
        $crate::__puffin::profile_scope!($name);
    };
}

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
