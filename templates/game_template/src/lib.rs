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
//!
//! This template authors gameplay the **Unity-style way**: a [`Spinner`]
//! [`Behaviour`] with a `start`/`update` lifecycle, rather than a raw ECS
//! system. The driver system that runs behaviours is wired up automatically by
//! [`ScriptContext::register_behaviour`]; `build` attaches a `Spinner` to the
//! mesh entities the host/editor provides.

use kaadan_ecs::Entity;
use kaadan_math::{Quat, Transform};
use kaadan_renderer::Mesh3D;
use kaadan_script::{kaadan_game, Behaviour, BehaviourContext, ScriptComponent, ScriptContext};

/// A Unity-like behaviour that spins its own entity around the Y axis.
///
/// State lives on the behaviour instance (`speed`), demonstrating that a
/// behaviour is a plain stateful object. Note that this instance state is reset
/// on hot-reload (a fresh `Spinner` is attached) — durable state belongs in
/// ordinary components.
#[derive(Default)]
struct Spinner {
    /// Angular speed in radians/second.
    speed: f32,
}

impl Behaviour for Spinner {
    fn start(&mut self, _ctx: &mut BehaviourContext) {
        // Establish the spin rate once when the behaviour first runs.
        self.speed = 1.0;
    }

    fn update(&mut self, ctx: &mut BehaviourContext) {
        let step = Quat::from_rotation_y(self.speed * ctx.dt);
        if let Ok(mut transform) = ctx.world.get_mut::<Transform>(ctx.entity) {
            transform.rotation = step * transform.rotation;
        }
    }

    fn type_name(&self) -> &'static str {
        "Spinner"
    }
}

/// Register this game's behaviours and attach them to the scene's mesh entities.
///
/// This is the only entry point gameplay needs to expose; it works identically
/// whether the crate is loaded as a dylib or linked statically. It is called
/// again on every hot-reload, so it must be **idempotent**: the host clears all
/// behaviours before reloading, then `build` re-registers the factory and
/// re-attaches a fresh [`Spinner`] to any mesh entity that lost its behaviour.
pub fn build(ctx: &mut ScriptContext) {
    // Make the behaviour constructible by name (editor "Add Behaviour", scene
    // load) and start the driver system that runs `start`/`update`.
    ctx.register_behaviour_default::<Spinner>("Spinner");

    // Attach a Spinner to every mesh entity that doesn't already carry one. On a
    // fresh load this wires up the initial scene; on reload the host has stripped
    // the old ScriptComponents, so this re-attaches fresh instances without ever
    // duplicating an entity.
    let world = ctx.world();
    let unattached: Vec<Entity> = world
        .query::<&Mesh3D>()
        .iter()
        .filter(|(e, _)| world.get::<ScriptComponent>(*e).is_err())
        .map(|(e, _)| e)
        .collect();
    for entity in unattached {
        let _ = world
            .inner_mut()
            .insert_one(entity, ScriptComponent::new(Spinner::default()));
    }
}

// Export the `kaadan_register` FFI symbol for the hot-reload host.
kaadan_game!(build);

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_ecs::{App, Time};
    use kaadan_math::{HandleAllocator, Vec3};
    use kaadan_renderer::Mesh3DGpu;
    use std::time::Duration;

    /// Spawn a mesh entity without a GPU: the handle is a plain generational id.
    fn spawn_mesh(app: &mut App) -> Entity {
        let mut alloc: HandleAllocator<Mesh3DGpu> = HandleAllocator::new();
        app.world.spawn((
            Mesh3D::new(alloc.allocate()),
            Transform::from_position(Vec3::ZERO),
        ))
    }

    /// Proves the STATIC-LINK path: register `build` directly against an `App`
    /// (no dylib, no `ScriptHost`) and confirm the `Spinner` behaviour is driven
    /// and rotates its mesh entity. This mirrors how gameplay ships on mobile.
    #[test]
    fn static_link_build_and_run() {
        let mut app = App::new();
        let mesh = spawn_mesh(&mut app);

        let mut ctx = ScriptContext::new(&mut app);
        build(&mut ctx);
        // `register_behaviour` wires up the driver system under the hood.
        assert!(
            ctx.registered()
                .iter()
                .any(|s| s == kaadan_script::BEHAVIOUR_DRIVER_SYSTEM),
            "the behaviour driver system must be registered"
        );
        drop(ctx); // release the &mut App borrow

        // Advance a known delta and tick once so start + update run.
        app.resources
            .get_mut::<Time>()
            .unwrap()
            .advance(Duration::from_millis(100));
        app.tick();

        let rot = app.world.get::<Transform>(mesh).unwrap().rotation;
        assert_ne!(
            rot,
            Quat::IDENTITY,
            "the Spinner behaviour should have rotated the mesh"
        );
    }

    /// Proves `build` is idempotent: calling it a second time on a world whose
    /// behaviours were cleared (simulating a hot-reload) re-attaches the Spinner
    /// without duplicating the mesh entity or leaving it behaviour-less.
    #[test]
    fn build_is_idempotent_across_reload() {
        let mut app = App::new();
        spawn_mesh(&mut app);

        {
            let mut ctx = ScriptContext::new(&mut app);
            build(&mut ctx);
        }
        assert_eq!(app.world.query::<&ScriptComponent>().iter().count(), 1);

        // Simulate the host's reload teardown, then build again.
        kaadan_script::clear_all_behaviours(&mut app.world, &mut app.resources);
        assert_eq!(app.world.query::<&ScriptComponent>().iter().count(), 0);
        {
            let mut ctx = ScriptContext::new(&mut app);
            build(&mut ctx);
        }

        assert_eq!(
            app.world.query::<&Mesh3D>().iter().count(),
            1,
            "reload must not duplicate the mesh entity"
        );
        assert_eq!(
            app.world.query::<&ScriptComponent>().iter().count(),
            1,
            "the Spinner must be re-attached after reload"
        );
    }
}
