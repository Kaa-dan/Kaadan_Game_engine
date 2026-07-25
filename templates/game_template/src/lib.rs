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
//! # Writing & attaching scripts (the editor loop)
//!
//! 1. Define a struct and `impl Behaviour` for it (`start`/`update`), like
//!    [`Spinner`] or [`Player`] below.
//! 2. Register it **by name** in [`build`] with `register_behaviour_default`.
//! 3. In the editor: **Code ▸ Build**, then select an entity and use the
//!    Inspector's **Scripts ▸ Add behaviour** dropdown (press ↻ after building)
//!    to attach it by name. Press **▶ Play** to run it.
//!
//! Attachment is **data-driven**: the editor stores which behaviours an entity
//! carries and attaches them on Play. `build` therefore only *registers*
//! behaviours — it must not attach them itself.

use kaadan_input::InputState;
use kaadan_math::{Quat, Transform, Vec3};
use kaadan_platform::KeyCode;
use kaadan_script::{kaadan_game, Behaviour, BehaviourContext, ScriptContext};

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

/// A WASD/arrow-key character controller. Reads the shared [`InputState`] and
/// translates its own entity on the XZ plane. Attach this to an entity and press
/// Play, then drive it with the keyboard.
struct Player {
    /// Movement speed in world units/second.
    speed: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self { speed: 4.0 }
    }
}

impl Behaviour for Player {
    fn update(&mut self, ctx: &mut BehaviourContext) {
        let mut dir = Vec3::ZERO;
        if let Some(input) = ctx.resources.get::<InputState>() {
            if input.key_pressed(KeyCode::W) || input.key_pressed(KeyCode::ArrowUp) {
                dir.z -= 1.0;
            }
            if input.key_pressed(KeyCode::S) || input.key_pressed(KeyCode::ArrowDown) {
                dir.z += 1.0;
            }
            if input.key_pressed(KeyCode::A) || input.key_pressed(KeyCode::ArrowLeft) {
                dir.x -= 1.0;
            }
            if input.key_pressed(KeyCode::D) || input.key_pressed(KeyCode::ArrowRight) {
                dir.x += 1.0;
            }
        }
        if dir != Vec3::ZERO {
            let step = dir.normalize() * self.speed * ctx.dt;
            if let Ok(mut transform) = ctx.world.get_mut::<Transform>(ctx.entity) {
                transform.position += step;
            }
        }
    }

    fn type_name(&self) -> &'static str {
        "Player"
    }
}

/// Register this game's behaviours by name. Called on first load and on every
/// hot-reload, so it must be idempotent — registering a name simply replaces the
/// previous factory. It does **not** attach behaviours to entities; the editor
/// (or a scene loader) does that from saved per-entity script assignments.
pub fn build(ctx: &mut ScriptContext) {
    ctx.register_behaviour_default::<Spinner>("Spinner");
    ctx.register_behaviour_default::<Player>("Player");
}

// Export the `kaadan_register` FFI symbol for the hot-reload host.
kaadan_game!(build);

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_ecs::{App, Entity, Time};
    use kaadan_math::{HandleAllocator, Vec3};
    use kaadan_platform::{InputEvent, KeyEvent};
    use kaadan_renderer::{Mesh3D, Mesh3DGpu};
    use kaadan_script::{BehaviourRegistry, ScriptComponent};
    use std::time::Duration;

    fn spawn_mesh(app: &mut App) -> Entity {
        let mut alloc: HandleAllocator<Mesh3DGpu> = HandleAllocator::new();
        app.world.spawn((
            Mesh3D::new(alloc.allocate()),
            Transform::from_position(Vec3::ZERO),
        ))
    }

    /// Attach a registered behaviour by name (mirrors what the editor does on
    /// Play): look it up in the `BehaviourRegistry` and insert a component.
    fn attach(app: &mut App, entity: Entity, name: &str) {
        let behaviour = app
            .resources
            .get::<BehaviourRegistry>()
            .and_then(|r| r.create(name))
            .expect("behaviour registered");
        let _ = app
            .world
            .inner_mut()
            .insert_one(entity, ScriptComponent::from_box(behaviour));
    }

    fn tick_ms(app: &mut App, ms: u64) {
        app.resources
            .get_mut::<Time>()
            .unwrap()
            .advance(Duration::from_millis(ms));
        app.tick();
    }

    /// STATIC-LINK path: register `build` directly, attach a Spinner by name, and
    /// confirm the driver runs it and rotates the mesh.
    #[test]
    fn spinner_registers_and_runs() {
        let mut app = App::new();
        let mesh = spawn_mesh(&mut app);
        {
            let mut ctx = ScriptContext::new(&mut app);
            build(&mut ctx);
            assert!(
                ctx.registered()
                    .iter()
                    .any(|s| s == kaadan_script::BEHAVIOUR_DRIVER_SYSTEM),
                "the behaviour driver system must be registered"
            );
        }
        attach(&mut app, mesh, "Spinner");

        tick_ms(&mut app, 100);

        let rot = app.world.get::<Transform>(mesh).unwrap().rotation;
        assert_ne!(rot, Quat::IDENTITY, "Spinner should have rotated the mesh");
    }

    /// The `Player` behaviour moves its entity while a movement key is held.
    #[test]
    fn player_moves_with_input() {
        let mut app = App::new();
        app.insert_resource(InputState::new());
        let entity = app.world.spawn((Transform::from_position(Vec3::ZERO),));
        {
            let mut ctx = ScriptContext::new(&mut app);
            build(&mut ctx);
        }
        attach(&mut app, entity, "Player");

        // Press D (move +X).
        app.resources.get_mut::<InputState>().unwrap().begin_frame();
        app.resources
            .get_mut::<InputState>()
            .unwrap()
            .process_event(&InputEvent::Key(KeyEvent {
                key: KeyCode::D,
                pressed: true,
            }));
        tick_ms(&mut app, 100);

        let pos = app.world.get::<Transform>(entity).unwrap().position;
        assert!(pos.x > 0.0, "Player should have moved along +X, got {pos:?}");
    }

    /// `build` is idempotent across a simulated hot-reload: re-registering the
    /// factories does not error and the names remain constructible.
    #[test]
    fn build_is_idempotent_across_reload() {
        let mut app = App::new();
        {
            let mut ctx = ScriptContext::new(&mut app);
            build(&mut ctx);
        }
        kaadan_script::clear_all_behaviours(&mut app.world, &mut app.resources);
        if let Some(registry) = app.resources.get_mut::<BehaviourRegistry>() {
            registry.clear();
        }
        {
            let mut ctx = ScriptContext::new(&mut app);
            build(&mut ctx);
        }
        let names: Vec<String> = app
            .resources
            .get::<BehaviourRegistry>()
            .unwrap()
            .names()
            .map(str::to_string)
            .collect();
        assert!(names.contains(&"Spinner".to_string()));
        assert!(names.contains(&"Player".to_string()));
    }
}
