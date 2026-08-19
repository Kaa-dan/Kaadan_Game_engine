//! Rust gameplay scripting for KaadanEngine.
//!
//! Gameplay is written as an ordinary Rust crate that registers ECS systems
//! against a [`ScriptContext`]. Two linkage models are supported from the same
//! source:
//!
//! * **Hot-reload (desktop dev):** the gameplay crate is built as a `cdylib`
//!   and loaded at runtime by a [`ScriptHost`] (feature `hot_reload`, default).
//!   The host owns the [`App`](kaadan_ecs::App) so the world/resources survive
//!   a code swap; on reload it removes the plugin's systems *by name*, drops the
//!   old library, and re-registers from the fresh build.
//! * **Static link (mobile / iOS):** the gameplay crate is built as an `rlib`
//!   and its `build` function is called directly. No `libloading`, no dylib.
//!   Compile this crate with `--no-default-features` for that target.
//!
//! The seam between the two is the exported `kaadan_register` symbol, generated
//! by the [`kaadan_game!`] macro.

mod behaviour;
mod context;
mod registry;

/// Version of the host↔plugin ABI.
///
/// Every plugin exports this value via [`kaadan_game!`]; [`ScriptHost`] refuses
/// to load a plugin whose value differs from the host's. **Bump this whenever
/// anything reachable through [`ScriptContext`] changes shape** — that includes
/// `App`, `World`, `Resources`, `Stage`, and the `Behaviour` trait's vtable.
///
/// This catches the common failure: a stale dylib left in `target/` from an
/// older build being silently loaded and reinterpreted. It does **not** catch
/// toolchain drift — two builds from the same source with different `rustc`
/// versions still report the same ABI version while having different layouts,
/// because Rust has no stable ABI. Building host and plugin from one workspace
/// in one cargo invocation remains the actual contract.
pub const ABI_VERSION: u64 = 1;

pub use behaviour::{
    behaviour_driver_system, clear_all_behaviours, Behaviour, BehaviourContext, BehaviourFactory,
    BehaviourRegistry, ScriptComponent, BEHAVIOUR_DRIVER_SYSTEM,
};
pub use context::ScriptContext;
pub use registry::ComponentRegistry;

// Re-export `Stage` so gameplay/host code depends only on `kaadan_script` for
// scheduling, not `kaadan_ecs` directly.
pub use kaadan_ecs::Stage;

#[cfg(feature = "hot_reload")]
mod host;
#[cfg(feature = "hot_reload")]
pub use host::{ScriptError, ScriptHost};

/// Define the gameplay plugin entry point.
///
/// Pass the path to a `fn(&mut ScriptContext)` build function. This emits the
/// `#[no_mangle] extern "C" fn kaadan_register` symbol the [`ScriptHost`] looks
/// up. It is harmless (and useful) to also export this symbol in static builds.
///
/// ```ignore
/// use kaadan_script::{kaadan_game, ScriptContext};
///
/// pub fn build(ctx: &mut ScriptContext) {
///     ctx.add_system("hello", |_w, _r| { /* ... */ });
/// }
///
/// kaadan_game!(build);
/// ```
#[macro_export]
macro_rules! kaadan_game {
    ($build:path) => {
        /// Plugin registration entry point resolved by the scripting host.
        ///
        /// # Safety
        /// Called across the host/plugin FFI boundary. Sound only when host and
        /// plugin are built with the same toolchain and dependency versions, so
        /// that `ScriptContext` has an identical layout on both sides.
        #[no_mangle]
        pub extern "C" fn kaadan_register(ctx: &mut $crate::ScriptContext) {
            $build(ctx);
        }

        /// ABI version this plugin was built against.
        ///
        /// The host reads this *before* calling `kaadan_register` and refuses to
        /// load on mismatch, so a stale dylib is rejected rather than having its
        /// `ScriptContext` reinterpreted at the wrong layout.
        #[no_mangle]
        pub extern "C" fn kaadan_abi_version() -> u64 {
            $crate::ABI_VERSION
        }
    };
}
