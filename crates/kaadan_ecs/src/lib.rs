//! Entity-Component-System built on [`hecs`], with a staged, single-threaded
//! system scheduler and a fixed-timestep main loop.
//!
//! Manages game world state, entity spawning, and component queries.

mod app;
mod events;
mod resources;
mod schedule;
mod time;
mod world;

pub use app::{App, Plugin};
pub use events::Events;
pub use resources::Resources;
pub use schedule::{Schedule, Stage, SystemFn};
pub use time::Time;
pub use world::{Component, Entity, Query, QueryBorrow, Ref, RefMut, World};

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_math::{Transform, Vec3};

    #[test]
    fn spawn_and_query() {
        let mut world = World::new();
        let e = world.spawn((Transform::from_position(Vec3::X),));
        assert!(world.is_alive(e));

        let t = world.get::<Transform>(e).unwrap();
        assert_eq!(t.position, Vec3::X);
    }

    #[test]
    fn despawn_entity() {
        let mut world = World::new();
        let e = world.spawn((42u32,));
        assert_eq!(world.len(), 1);
        world.despawn(e).unwrap();
        assert_eq!(world.len(), 0);
        assert!(!world.is_alive(e));
    }

    #[test]
    fn resource_insert_get() {
        let mut res = Resources::new();
        res.insert(42u32);
        assert_eq!(*res.get::<u32>().unwrap(), 42);
        *res.get_mut::<u32>().unwrap() = 100;
        assert_eq!(*res.get::<u32>().unwrap(), 100);
    }

    #[test]
    fn resource_remove() {
        let mut res = Resources::new();
        res.insert("hello".to_string());
        assert!(res.contains::<String>());
        let val = res.remove::<String>().unwrap();
        assert_eq!(val, "hello");
        assert!(!res.contains::<String>());
    }

    #[test]
    fn app_tick_updates_time() {
        let mut app = App::new();
        app.tick();
        let time = app.resources.get::<Time>().unwrap();
        assert_eq!(time.frame_count(), 1);
    }

    #[test]
    fn system_modifies_world() {
        struct Velocity(Vec3);

        fn movement(world: &mut World, resources: &mut Resources) {
            let dt = resources.get::<Time>().unwrap().delta_seconds();
            for (_e, (t, v)) in world.query::<(&mut Transform, &Velocity)>().iter() {
                t.position += v.0 * dt;
            }
        }

        let mut app = App::new();
        app.add_system("movement", movement);

        for i in 0..100 {
            app.world.spawn((
                Transform::from_position(Vec3::new(i as f32, 0.0, 0.0)),
                Velocity(Vec3::Y),
            ));
        }

        app.tick();
        assert_eq!(app.world.len(), 100);
    }

    #[test]
    fn remove_system_stops_it_running() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let runs = Rc::new(RefCell::new(0u32));
        let r = runs.clone();

        let mut app = App::new();
        app.add_system("counter", move |_, _| *r.borrow_mut() += 1);

        app.tick();
        assert_eq!(*runs.borrow(), 1);

        app.remove_system("counter");
        app.tick();
        assert_eq!(*runs.borrow(), 1, "removed system must not run again");
    }

    #[test]
    fn fixed_update_is_capped_per_tick() {
        use std::cell::RefCell;
        use std::rc::Rc;
        use std::time::Duration;

        // Count FixedUpdate invocations within a single tick.
        let counter = Rc::new(RefCell::new(0u32));
        let c = counter.clone();

        let mut app = App::new();
        app.add_system_to_stage(Stage::FixedUpdate, "count", move |_, _| {
            *c.borrow_mut() += 1;
        });

        // Prime a huge backlog (clamped to 250ms by Time = ~15 steps at 60Hz),
        // so the per-tick cap (MAX_FIXED_STEPS = 8) is what limits the count.
        // This makes the assertion deterministic regardless of wall-clock jitter.
        app.resources
            .get_mut::<Time>()
            .unwrap()
            .advance(Duration::from_secs(10));
        app.tick();

        assert_eq!(*counter.borrow(), 8);
    }
}
