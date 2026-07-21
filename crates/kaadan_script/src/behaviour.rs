//! Unity-style scripted behaviours layered over the raw ECS-system model.
//!
//! A [`Behaviour`] is a stateful object attached to an entity via a
//! [`ScriptComponent`]. Its [`start`](Behaviour::start) runs once, the first
//! frame it is seen; its [`update`](Behaviour::update) runs every frame after.
//! This gives gameplay authors the familiar "script with `start`/`update`"
//! ergonomics of Unity's `MonoBehaviour` without abandoning the underlying ECS:
//! a single driver system ([`behaviour_driver_system`]) walks the
//! `ScriptComponent`s and dispatches their lifecycle calls.
//!
//! # Hot-reload safety
//!
//! A `Box<dyn Behaviour>` carries a vtable pointing into the code that defined
//! the concrete type — for gameplay that is the loaded plugin dylib. Those
//! instances must **not** outlive the dylib, or the vtable dangles. The host
//! therefore calls [`clear_all_behaviours`] on reload (before dropping the old
//! library); the fresh plugin re-attaches behaviours from its `build`. Behaviour
//! *instance* state does not survive a reload — only data stored in ordinary
//! components from shared crates does. See `docs/scripting/abi.md`.

use kaadan_ecs::{Entity, Resources, Time, World};

/// Context handed to a [`Behaviour`]'s lifecycle methods.
///
/// Exposes the owning `entity` plus mutable access to the whole `world` and the
/// shared `resources`, so a behaviour can move itself, spawn/despawn entities,
/// read input, and so on. `dt` is the frame delta in seconds.
pub struct BehaviourContext<'a> {
    /// The entity this behaviour is attached to.
    pub entity: Entity,
    /// The full ECS world.
    pub world: &'a mut World,
    /// Shared resources (input, time, custom game state, ...).
    pub resources: &'a mut Resources,
    /// Seconds elapsed since the previous frame.
    pub dt: f32,
}

/// A stateful, per-entity gameplay script with a `start`/`update` lifecycle.
///
/// Implement this for your gameplay objects and attach them with
/// [`ScriptComponent::new`]. All methods have empty defaults, so implementors
/// override only what they need.
pub trait Behaviour: Send + Sync + 'static {
    /// Called once, the first frame this behaviour is driven.
    fn start(&mut self, _ctx: &mut BehaviourContext) {}

    /// Called every frame after [`start`](Behaviour::start).
    fn update(&mut self, _ctx: &mut BehaviourContext) {}

    /// Called when the behaviour is torn down (its `ScriptComponent` cleared,
    /// e.g. on hot-reload). Not called on entity despawn — the component is
    /// gone by then.
    fn on_destroy(&mut self, _ctx: &mut BehaviourContext) {}

    /// Stable name used as the reload / editor key. Defaults to the Rust type
    /// name; override to pin a serialization-stable identifier.
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Component that holds one [`Behaviour`] instance on an entity.
///
/// The behaviour is stored in an `Option` so the driver can move it out for the
/// duration of a lifecycle call (the call needs `&mut World`, which would
/// otherwise alias the component borrow) and move it back afterwards.
pub struct ScriptComponent {
    behaviour: Option<Box<dyn Behaviour>>,
    started: bool,
}

impl ScriptComponent {
    /// Attach `behaviour` to an entity.
    pub fn new(behaviour: impl Behaviour) -> Self {
        Self {
            behaviour: Some(Box::new(behaviour)),
            started: false,
        }
    }

    /// Attach an already-boxed behaviour (e.g. one produced by a
    /// [`BehaviourRegistry`] factory).
    pub fn from_box(behaviour: Box<dyn Behaviour>) -> Self {
        Self {
            behaviour: Some(behaviour),
            started: false,
        }
    }

    /// The behaviour's [`type_name`](Behaviour::type_name), or `None` if it is
    /// currently moved out (mid lifecycle call).
    pub fn type_name(&self) -> Option<&'static str> {
        self.behaviour.as_ref().map(|b| b.type_name())
    }
}

/// Name the [`behaviour_driver_system`] is registered under. Used so the
/// scripting host can remove it by name on reload.
pub const BEHAVIOUR_DRIVER_SYSTEM: &str = "kaadan::behaviour_driver";

/// System that drives every [`ScriptComponent`]: runs `start` once, then
/// `update` each frame.
///
/// Registered automatically the first time gameplay calls
/// [`ScriptContext::register_behaviour`](crate::ScriptContext::register_behaviour)
/// or [`ScriptContext::enable_behaviours`](crate::ScriptContext::enable_behaviours).
pub fn behaviour_driver_system(world: &mut World, resources: &mut Resources) {
    let dt = resources
        .get::<Time>()
        .map(|t| t.delta_seconds())
        .unwrap_or(0.0);

    // Snapshot the entities up front so mutating the world inside a lifecycle
    // call (spawn/despawn) can't disturb iteration. Entities created this frame
    // start next frame; entities despawned this frame are simply skipped below.
    let entities: Vec<Entity> = world
        .query::<&ScriptComponent>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for entity in entities {
        // Move the behaviour out (dropping the component borrow) so the call
        // below can take `&mut World` without aliasing.
        let taken = match world.get_mut::<ScriptComponent>(entity) {
            Ok(mut sc) => sc.behaviour.take().map(|b| (b, sc.started)),
            Err(_) => None,
        };
        let Some((mut behaviour, started)) = taken else {
            continue;
        };

        {
            let mut ctx = BehaviourContext {
                entity,
                world,
                resources,
                dt,
            };
            if !started {
                behaviour.start(&mut ctx);
            }
            behaviour.update(&mut ctx);
        }

        // Move it back. The entity may have been despawned (or its component
        // removed) by the call; if so, drop the behaviour instead of leaking it.
        if let Ok(mut sc) = world.get_mut::<ScriptComponent>(entity) {
            sc.behaviour = Some(behaviour);
            sc.started = true;
        }
    }
}

/// Remove every [`ScriptComponent`] from the world, calling each behaviour's
/// [`on_destroy`](Behaviour::on_destroy) first.
///
/// The host calls this on hot-reload, **before** unloading the plugin dylib, so
/// no behaviour vtable outlives the code that defined it. See the module docs.
pub fn clear_all_behaviours(world: &mut World, resources: &mut Resources) {
    let entities: Vec<Entity> = world
        .query::<&ScriptComponent>()
        .iter()
        .map(|(e, _)| e)
        .collect();

    for entity in entities {
        let taken = match world.get_mut::<ScriptComponent>(entity) {
            Ok(mut sc) => sc.behaviour.take(),
            Err(_) => None,
        };
        if let Some(mut behaviour) = taken {
            let mut ctx = BehaviourContext {
                entity,
                world,
                resources,
                dt: 0.0,
            };
            behaviour.on_destroy(&mut ctx);
        }
        // Drop the (now behaviour-less) component so the vtable is gone.
        let _ = world.inner_mut().remove_one::<ScriptComponent>(entity);
    }
}

/// Factory function producing a fresh boxed [`Behaviour`].
pub type BehaviourFactory = fn() -> Box<dyn Behaviour>;

/// Maps behaviour names to factories, so tooling (the editor's "Add Behaviour"
/// menu, scene deserialization) can construct behaviours by string name without
/// knowing the concrete type.
///
/// Stored as a resource; populated by
/// [`ScriptContext::register_behaviour`](crate::ScriptContext::register_behaviour).
#[derive(Default)]
pub struct BehaviourRegistry {
    entries: Vec<(String, BehaviourFactory)>,
}

impl BehaviourRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register `factory` under `name`, replacing any previous entry with the
    /// same name (so a reload re-registering the same behaviour is idempotent).
    pub fn register(&mut self, name: impl Into<String>, factory: BehaviourFactory) {
        let name = name.into();
        if let Some(slot) = self.entries.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = factory;
        } else {
            self.entries.push((name, factory));
        }
    }

    /// Construct a fresh behaviour for `name`, or `None` if unregistered.
    pub fn create(&self, name: &str) -> Option<Box<dyn Behaviour>> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, f)| f())
    }

    /// Iterate the registered behaviour names.
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.entries.iter().map(|(n, _)| n.as_str())
    }

    /// Remove all registrations. The host calls this on reload so names from an
    /// unloaded plugin don't linger.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_ecs::App;
    use std::time::Duration;

    /// Shared tally the test behaviour writes into, so we can observe lifecycle
    /// dispatch without downcasting the trait object.
    #[derive(Default)]
    struct Tally {
        starts: u32,
        updates: u32,
    }

    #[derive(Default)]
    struct Counter;

    impl Behaviour for Counter {
        fn start(&mut self, ctx: &mut BehaviourContext) {
            if let Some(t) = ctx.resources.get_mut::<Tally>() {
                t.starts += 1;
            }
        }
        fn update(&mut self, ctx: &mut BehaviourContext) {
            if let Some(t) = ctx.resources.get_mut::<Tally>() {
                t.updates += 1;
            }
        }
        fn type_name(&self) -> &'static str {
            "Counter"
        }
    }

    fn tick_with_dt(app: &mut App) {
        app.resources
            .get_mut::<Time>()
            .unwrap()
            .advance(Duration::from_millis(16));
        app.tick();
    }

    #[test]
    fn start_runs_once_update_runs_each_frame() {
        let mut app = App::new();
        app.insert_resource(Tally::default());
        app.add_system(BEHAVIOUR_DRIVER_SYSTEM, behaviour_driver_system);
        app.world.spawn((ScriptComponent::new(Counter),));

        tick_with_dt(&mut app);
        tick_with_dt(&mut app);
        tick_with_dt(&mut app);

        let tally = app.resources.get::<Tally>().unwrap();
        assert_eq!(tally.starts, 1, "start must run exactly once");
        assert_eq!(tally.updates, 3, "update must run every frame");
    }

    #[test]
    fn registry_creates_by_name() {
        let mut registry = BehaviourRegistry::new();
        registry.register("Counter", || Box::new(Counter));

        let names: Vec<_> = registry.names().collect();
        assert_eq!(names, vec!["Counter"]);

        let made = registry.create("Counter");
        assert!(made.is_some());
        assert_eq!(made.unwrap().type_name(), "Counter");
        assert!(registry.create("Missing").is_none());
    }

    #[test]
    fn clear_all_behaviours_removes_components() {
        let mut app = App::new();
        let e = app.world.spawn((ScriptComponent::new(Counter),));
        assert!(app.world.get::<ScriptComponent>(e).is_ok());

        clear_all_behaviours(&mut app.world, &mut app.resources);
        assert!(app.world.get::<ScriptComponent>(e).is_err());
    }
}
