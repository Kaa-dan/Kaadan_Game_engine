use kaadan_ecs::{App, Entity, Plugin, Resources, Stage, World};
use kaadan_math::{Quat, Transform, Vec2};
use rapier2d::prelude::*;

use crate::components::{Collider, ColliderShape, RigidBody, RigidBodyType, Velocity};
use crate::world::PhysicsWorld;

/// Registers the [`PhysicsWorld`] resource and the sync/step/writeback systems.
pub struct PhysicsPlugin {
    pub gravity: Vec2,
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self {
            gravity: Vec2::new(0.0, -9.81),
        }
    }
}

impl PhysicsPlugin {
    pub fn new(gravity: Vec2) -> Self {
        Self { gravity }
    }
}

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsWorld::with_gravity(self.gravity));
        // Physics runs in FixedUpdate for framerate-independent, deterministic
        // simulation. sync (create bodies) -> step -> writeback to ECS.
        app.add_system_to_stage(Stage::FixedUpdate, "physics_sync", physics_sync_system);
        app.add_system_to_stage(Stage::FixedUpdate, "physics_step", physics_step_system);
        app.add_system_to_stage(
            Stage::FixedUpdate,
            "physics_writeback",
            physics_writeback_system,
        );
    }
}

/// Create Rapier bodies/colliders for ECS entities that don't have one yet.
pub fn physics_sync_system(world: &mut World, resources: &mut Resources) {
    let Some(physics) = resources.get_mut::<PhysicsWorld>() else {
        return;
    };

    type SyncQuery<'a> = (
        &'a mut RigidBody,
        Option<&'a Transform>,
        Option<&'a mut Collider>,
        Option<&'a Velocity>,
    );
    for (entity, (rigid_body, transform, collider, velocity)) in world.query::<SyncQuery>().iter() {
        if rigid_body.handle.is_some() {
            continue;
        }

        let position = transform
            .map(|t| nalgebra::Vector2::new(t.position.x, t.position.y))
            .unwrap_or_else(|| nalgebra::Vector2::new(0.0, 0.0));

        let mut builder = match rigid_body.body_type {
            RigidBodyType::Dynamic => RigidBodyBuilder::dynamic(),
            RigidBodyType::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            RigidBodyType::Static => RigidBodyBuilder::fixed(),
        }
        .translation(position);
        if let Some(v) = velocity {
            builder = builder
                .linvel(nalgebra::Vector2::new(v.linear.x, v.linear.y))
                .angvel(v.angular);
        }
        let body_handle = physics.rigid_body_set.insert(builder.build());
        rigid_body.handle = Some(body_handle);
        physics.entity_to_body.insert(entity, body_handle);
        physics.body_to_entity.insert(body_handle, entity);

        if let Some(collider) = collider {
            if collider.handle.is_none() {
                let builder = match &collider.shape {
                    ColliderShape::Box {
                        half_width,
                        half_height,
                    } => ColliderBuilder::cuboid(*half_width, *half_height),
                    ColliderShape::Circle { radius } => ColliderBuilder::ball(*radius),
                    ColliderShape::Capsule {
                        half_height,
                        radius,
                    } => ColliderBuilder::capsule_y(*half_height, *radius),
                }
                .density(collider.density)
                .friction(collider.friction)
                .restitution(collider.restitution)
                .sensor(collider.is_sensor)
                .active_events(ActiveEvents::COLLISION_EVENTS);

                let collider_handle = physics.collider_set.insert_with_parent(
                    builder.build(),
                    body_handle,
                    &mut physics.rigid_body_set,
                );
                collider.handle = Some(collider_handle);
                physics.collider_to_entity.insert(collider_handle, entity);
            }
        }
    }
}

/// Advance the simulation one fixed step.
pub fn physics_step_system(_world: &mut World, resources: &mut Resources) {
    if let Some(physics) = resources.get_mut::<PhysicsWorld>() {
        physics.step();
    }
}

/// Copy simulated transforms/velocities of dynamic bodies back to the ECS.
pub fn physics_writeback_system(world: &mut World, resources: &mut Resources) {
    let Some(physics) = resources.get::<PhysicsWorld>() else {
        return;
    };

    let mut updates: Vec<(Entity, Vec2, f32, Vec2)> = Vec::new();
    for (entity, body_handle) in &physics.entity_to_body {
        if let Some(body) = physics.rigid_body_set.get(*body_handle) {
            if body.is_dynamic() {
                let t = body.translation();
                let lv = body.linvel();
                updates.push((
                    *entity,
                    Vec2::new(t.x, t.y),
                    body.rotation().angle(),
                    Vec2::new(lv.x, lv.y),
                ));
            }
        }
    }

    for (entity, position, angle, linear) in updates {
        if let Ok(mut transform) = world.get_mut::<Transform>(entity) {
            transform.position.x = position.x;
            transform.position.y = position.y;
            transform.rotation = Quat::from_rotation_z(angle);
        }
        if let Ok(mut velocity) = world.get_mut::<Velocity>(entity) {
            velocity.linear = linear;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_body_falls_under_gravity() {
        use kaadan_ecs::Time;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugin(&PhysicsPlugin::default());

        let entity = app.world.spawn((
            RigidBody::dynamic(),
            Collider::circle(0.5),
            Transform::from_position_2d(0.0, 10.0),
            Velocity::default(),
        ));

        let start_y = app.world.get::<Transform>(entity).unwrap().position.y;
        // Physics is in FixedUpdate now, so drive the clock deterministically:
        // advance one fixed step per tick (~120 steps = 2s of simulation).
        let fixed_dt = app.resources.get::<Time>().unwrap().fixed_delta_seconds();
        for _ in 0..120 {
            app.resources
                .get_mut::<Time>()
                .unwrap()
                .advance(Duration::from_secs_f32(fixed_dt));
            app.tick();
        }
        let end_y = app.world.get::<Transform>(entity).unwrap().position.y;

        assert!(
            end_y < start_y - 1.0,
            "body should fall under gravity: {start_y} -> {end_y}"
        );
    }
}
