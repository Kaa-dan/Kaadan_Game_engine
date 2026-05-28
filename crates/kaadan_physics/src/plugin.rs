use kaadan_ecs::{App, Plugin};

use crate::world::PhysicsWorld;

/// Plugin that registers the physics world resource and systems.
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsWorld::new());
        app.add_system("physics_step", physics_step_system);
    }
}

fn physics_step_system(_world: &mut kaadan_ecs::World, resources: &mut kaadan_ecs::Resources) {
    if let Some(physics) = resources.get_mut::<PhysicsWorld>() {
        physics.step();
    }
}
