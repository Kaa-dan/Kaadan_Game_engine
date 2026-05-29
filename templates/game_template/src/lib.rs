//! Template gameplay crate for KaadanEngine.
//!
//! Copy this crate to start a new game. It is built two ways from one source:
//!
//! * as a `cdylib`, loaded at runtime by `kaadan_script::ScriptHost` for
//!   hot-reload during development, and
//! * as an `rlib`, statically linked into the engine binary for shipping
//!   (the iOS / mobile model), where [`build`] is called directly.
//!
//! The [`kaadan_game!`] macro at the bottom exports the `kaadan_register`
//! symbol the host resolves; [`build`] is the single registration entry point.

use kaadan_ecs::{Resources, Time, World};
use kaadan_math::{Quat, Transform};
use kaadan_renderer::Mesh3D;
use kaadan_script::{kaadan_game, ScriptContext};

/// Register this game's systems and resources.
///
/// This is the only entry point gameplay needs to expose; it works identically
/// whether the crate is loaded as a dylib or linked statically.
pub fn build(ctx: &mut ScriptContext) {
    ctx.add_system("spin", spin);
}

/// Rotate every entity that has both a [`Mesh3D`] and a [`Transform`] around the
/// Y axis at a fixed angular speed, scaled by the frame delta.
fn spin(world: &mut World, resources: &mut Resources) {
    // ~1 radian/second.
    const SPEED: f32 = 1.0;
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(0.0);
    let step = Quat::from_rotation_y(SPEED * dt);

    for (_e, (_mesh, transform)) in world.query::<(&Mesh3D, &mut Transform)>().iter() {
        transform.rotation = step * transform.rotation;
    }
}

// Export the `kaadan_register` FFI symbol for the hot-reload host.
kaadan_game!(build);

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_ecs::App;
    use kaadan_math::{HandleAllocator, Vec3};
    use kaadan_renderer::Mesh3DGpu;
    use std::time::Duration;

    /// Proves the STATIC-LINK path: register `build` directly against an `App`
    /// (no dylib, no `ScriptHost`) and confirm the `spin` system runs and
    /// rotates a `Mesh3D` + `Transform` entity. This mirrors how gameplay ships
    /// on mobile.
    #[test]
    fn static_link_build_and_run() {
        let mut app = App::new();

        let mut ctx = ScriptContext::new(&mut app);
        build(&mut ctx);
        assert_eq!(ctx.registered(), &["spin".to_string()]);
        drop(ctx); // release the &mut App borrow

        // Spawn a mesh entity. The handle is a plain generational id, so no GPU
        // is required to attach a `Mesh3D` component.
        let mut alloc: HandleAllocator<Mesh3DGpu> = HandleAllocator::new();
        let e = app
            .world
            .spawn((Mesh3D::new(alloc.allocate()), Transform::IDENTITY));

        // Also spawn a mesh-less entity to confirm the query filter excludes it.
        let lone = app.world.spawn((Transform::from_position(Vec3::X),));

        // Advance a known delta and tick once.
        app.resources
            .get_mut::<Time>()
            .unwrap()
            .advance(Duration::from_millis(100));
        app.tick();

        let rot = app.world.get::<Transform>(e).unwrap().rotation;
        assert_ne!(rot, Quat::IDENTITY, "spin should have rotated the mesh");

        let lone_rot = app.world.get::<Transform>(lone).unwrap().rotation;
        assert_eq!(
            lone_rot,
            Quat::IDENTITY,
            "entity without Mesh3D must be untouched"
        );
    }
}
