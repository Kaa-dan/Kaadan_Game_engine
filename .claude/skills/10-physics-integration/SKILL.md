# 10 — Physics Integration

## Description
Integrate Rapier 2D physics with the ECS. Wraps Rapier's rigid body and collider sets as resources, syncs Transform with physics bodies, exposes collision events and raycasting to game code.

## Phase
5 — Physics & 3D

## Prerequisites
- Skill 02 (`02-math-and-core-types`) — Transform, Vec2/Vec3
- Skill 05 (`05-ecs-world`) — World, Resources, system scheduling

## Complexity
Medium — Rapier does the physics; the work is integration and sync

## Architecture Decisions

### Why Rapier?
- Pure Rust — no C/C++ dependencies, compiles cleanly for all targets including mobile and WASM
- Deterministic simulation (important for replays, networking)
- Actively maintained, well-documented, used by Bevy and other Rust engines
- Separate 2D (`rapier2d`) and 3D (`rapier3d`) crates — start with 2D

### Integration pattern: ECS ↔ Rapier
- Rapier has its own internal storage (`RigidBodySet`, `ColliderSet`)
- Engine provides `RigidBody` and `Collider` ECS components
- A sync system maps between ECS entities and Rapier handles:
  1. Before physics step: write ECS `Transform` → Rapier body positions (for kinematic bodies)
  2. Run `PhysicsPipeline::step()`
  3. After physics step: read Rapier body positions → ECS `Transform` (for dynamic bodies)
- This two-way sync keeps ECS as the source of truth for game code

### Why not embed Rapier types directly as components?
- Rapier types aren't `Send + Sync` or easily cloneable
- Rapier expects its own `RigidBodyHandle`/`ColliderHandle` for internal bookkeeping
- Wrapping provides a clean API: game code sets `RigidBody { body_type: Dynamic }`, the sync system handles the rest

## Step-by-Step Implementation

### 1. Crate Setup

```toml
# crates/kaadan_physics/Cargo.toml
[package]
name = "kaadan_physics"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
kaadan_ecs = { path = "../kaadan_ecs" }
rapier2d = { workspace = true }
tracing = { workspace = true }

[features]
default = ["2d"]
2d = []
3d = ["rapier3d"]

[dependencies.rapier3d]
version = "0.22"
optional = true
```

### 2. Physics Components

```rust
// crates/kaadan_physics/src/components.rs

/// Component: marks an entity as having a rigid body.
#[derive(Debug, Clone)]
pub struct RigidBody {
    pub body_type: RigidBodyType,
    pub gravity_scale: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub can_sleep: bool,
    /// Internal Rapier handle — set by the sync system, not by the user.
    pub(crate) handle: Option<rapier2d::dynamics::RigidBodyHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyType {
    /// Affected by forces and gravity
    Dynamic,
    /// Not affected by forces; can be moved programmatically
    Kinematic,
    /// Immovable (walls, ground)
    Static,
}

impl Default for RigidBody {
    fn default() -> Self {
        Self {
            body_type: RigidBodyType::Dynamic,
            gravity_scale: 1.0,
            linear_damping: 0.0,
            angular_damping: 0.05,
            can_sleep: true,
            handle: None,
        }
    }
}

/// Component: defines collision shape for an entity.
#[derive(Debug, Clone)]
pub struct Collider {
    pub shape: ColliderShape,
    pub restitution: f32,    // Bounciness (0–1)
    pub friction: f32,       // Surface friction
    pub density: f32,        // Affects mass when attached to dynamic body
    pub is_sensor: bool,     // Triggers events but no physical response
    pub(crate) handle: Option<rapier2d::geometry::ColliderHandle>,
}

#[derive(Debug, Clone)]
pub enum ColliderShape {
    Box { half_width: f32, half_height: f32 },
    Circle { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

impl Default for Collider {
    fn default() -> Self {
        Self {
            shape: ColliderShape::Box { half_width: 0.5, half_height: 0.5 },
            restitution: 0.3,
            friction: 0.5,
            density: 1.0,
            is_sensor: false,
            handle: None,
        }
    }
}

/// Component: velocity override. Set this to apply impulses/forces.
#[derive(Debug, Clone, Copy, Default)]
pub struct Velocity {
    pub linear: kaadan_math::Vec2,
    pub angular: f32,
}
```

### 3. PhysicsWorld Resource

```rust
// crates/kaadan_physics/src/world.rs
use rapier2d::prelude::*;
use std::collections::HashMap;

/// ECS Resource containing the Rapier physics simulation.
pub struct PhysicsWorld {
    pub gravity: kaadan_math::Vec2,
    pub(crate) rigid_body_set: RigidBodySet,
    pub(crate) collider_set: ColliderSet,
    pub(crate) integration_parameters: IntegrationParameters,
    pub(crate) physics_pipeline: PhysicsPipeline,
    pub(crate) island_manager: IslandManager,
    pub(crate) broad_phase: DefaultBroadPhase,
    pub(crate) narrow_phase: NarrowPhase,
    pub(crate) impulse_joint_set: ImpulseJointSet,
    pub(crate) multibody_joint_set: MultibodyJointSet,
    pub(crate) ccd_solver: CCDSolver,
    /// Maps ECS Entity → Rapier RigidBodyHandle
    pub(crate) entity_to_body: HashMap<hecs::Entity, RigidBodyHandle>,
    /// Maps Rapier ColliderHandle → ECS Entity (for collision events)
    pub(crate) collider_to_entity: HashMap<ColliderHandle, hecs::Entity>,
    /// Collision events accumulated during the step
    pub collision_events: Vec<CollisionEvent>,
}

/// A collision event exposed to game code.
#[derive(Debug, Clone)]
pub struct CollisionEvent {
    pub entity_a: hecs::Entity,
    pub entity_b: hecs::Entity,
    pub event_type: CollisionEventType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionEventType {
    Started,
    Stopped,
}

impl PhysicsWorld {
    pub fn new(gravity: kaadan_math::Vec2) -> Self {
        Self {
            gravity,
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            entity_to_body: HashMap::new(),
            collider_to_entity: HashMap::new(),
            collision_events: Vec::new(),
        }
    }

    /// Cast a ray and return the first hit.
    pub fn raycast(
        &self,
        origin: kaadan_math::Vec2,
        direction: kaadan_math::Vec2,
        max_distance: f32,
    ) -> Option<RaycastHit> {
        let ray = Ray::new(
            point![origin.x, origin.y],
            vector![direction.x, direction.y],
        );
        self.narrow_phase
            .cast_ray(&self.collider_set, &ray, max_distance, true, QueryFilter::default())
            .and_then(|(handle, toi)| {
                let entity = self.collider_to_entity.get(&handle)?;
                Some(RaycastHit {
                    entity: *entity,
                    distance: toi,
                    point: origin + direction * toi,
                })
            })
    }
}

#[derive(Debug, Clone)]
pub struct RaycastHit {
    pub entity: hecs::Entity,
    pub distance: f32,
    pub point: kaadan_math::Vec2,
}
```

### 4. Physics Systems

```rust
// crates/kaadan_physics/src/systems.rs

/// System: sync new RigidBody/Collider components into Rapier.
pub fn physics_sync_system(world: &mut kaadan_ecs::World, resources: &mut kaadan_ecs::Resources) {
    let physics = resources.get_mut::<PhysicsWorld>().unwrap();

    // Find entities with RigidBody but no Rapier handle → create in Rapier
    for (entity, (rb, transform)) in world.query::<(&mut RigidBody, &kaadan_math::Transform)>().iter() {
        if rb.handle.is_some() { continue; }

        let body = match rb.body_type {
            RigidBodyType::Dynamic => rapier2d::prelude::RigidBodyBuilder::dynamic(),
            RigidBodyType::Kinematic => rapier2d::prelude::RigidBodyBuilder::kinematic_position_based(),
            RigidBodyType::Static => rapier2d::prelude::RigidBodyBuilder::fixed(),
        }
        .translation(vector![transform.position.x, transform.position.y])
        .gravity_scale(rb.gravity_scale)
        .linear_damping(rb.linear_damping)
        .angular_damping(rb.angular_damping)
        .build();

        let handle = physics.rigid_body_set.insert(body);
        rb.handle = Some(handle);
        physics.entity_to_body.insert(entity, handle);
    }

    // Find entities with Collider but no Rapier handle → create and attach
    for (entity, (collider, rb)) in world.query::<(&mut Collider, &RigidBody)>().iter() {
        if collider.handle.is_some() { continue; }
        let Some(body_handle) = rb.handle else { continue; };

        let shape = match &collider.shape {
            ColliderShape::Box { half_width, half_height } =>
                rapier2d::prelude::ColliderBuilder::cuboid(*half_width, *half_height),
            ColliderShape::Circle { radius } =>
                rapier2d::prelude::ColliderBuilder::ball(*radius),
            ColliderShape::Capsule { half_height, radius } =>
                rapier2d::prelude::ColliderBuilder::capsule_y(*half_height, *radius),
        }
        .restitution(collider.restitution)
        .friction(collider.friction)
        .density(collider.density)
        .sensor(collider.is_sensor);

        let handle = physics.collider_set.insert_with_parent(
            shape.build(), body_handle, &mut physics.rigid_body_set,
        );
        collider.handle = Some(handle);
        physics.collider_to_entity.insert(handle, entity);
    }
}

/// System: step the physics simulation.
pub fn physics_step_system(_world: &mut kaadan_ecs::World, resources: &mut kaadan_ecs::Resources) {
    let physics = resources.get_mut::<PhysicsWorld>().unwrap();

    let gravity = vector![physics.gravity.x, physics.gravity.y];
    physics.physics_pipeline.step(
        &gravity,
        &physics.integration_parameters,
        &mut physics.island_manager,
        &mut physics.broad_phase,
        &mut physics.narrow_phase,
        &mut physics.rigid_body_set,
        &mut physics.collider_set,
        &mut physics.impulse_joint_set,
        &mut physics.multibody_joint_set,
        &mut physics.ccd_solver,
        None,
        &(),
        &(),
    );
}

/// System: write Rapier positions back to ECS Transforms (for dynamic bodies).
pub fn physics_writeback_system(world: &mut kaadan_ecs::World, resources: &mut kaadan_ecs::Resources) {
    let physics = resources.get::<PhysicsWorld>().unwrap();

    for (entity, (rb, transform)) in world.query::<(&RigidBody, &mut kaadan_math::Transform)>().iter() {
        if rb.body_type != RigidBodyType::Dynamic { continue; }
        let Some(handle) = rb.handle else { continue; };
        let Some(body) = physics.rigid_body_set.get(handle) else { continue; };

        let pos = body.translation();
        let rot = body.rotation().angle();
        transform.position.x = pos.x;
        transform.position.y = pos.y;
        transform.rotation = kaadan_math::Quat::from_rotation_z(rot);
    }
}
```

### 5. Physics Plugin

```rust
// crates/kaadan_physics/src/plugin.rs
use kaadan_ecs::{App, Plugin};

pub struct PhysicsPlugin {
    pub gravity: kaadan_math::Vec2,
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self { gravity: kaadan_math::Vec2::new(0.0, -9.81) }
    }
}

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsWorld::new(self.gravity));
        app.add_system("physics_sync", physics_sync_system);
        app.add_system("physics_step", physics_step_system);
        app.add_system("physics_writeback", physics_writeback_system);
    }
}
```

## Deliverables Checklist

- [ ] `PhysicsWorld` resource wrapping Rapier sets and pipeline
- [ ] `RigidBody` component (Dynamic, Kinematic, Static)
- [ ] `Collider` component (Box, Circle, Capsule) with material properties
- [ ] `Velocity` component for impulse/force application
- [ ] Sync system: ECS entities ↔ Rapier handles
- [ ] Step system: advance physics simulation each frame
- [ ] Writeback system: Rapier positions → ECS Transform (dynamic bodies)
- [ ] Collision events: `CollisionStarted`/`CollisionStopped`
- [ ] Raycasting API
- [ ] `PhysicsPlugin` for one-line setup
- [ ] Demo: bouncing balls with gravity, static ground, collision detection

## Common Pitfalls

1. **Rapier uses its own vector types** — `rapier2d::na::Vector2`, not `glam::Vec2`. Convert at the boundary: `vector![v.x, v.y]` and `Vec2::new(rv.x, rv.y)`.

2. **Physics timestep should be fixed** — Variable timestep causes non-deterministic physics. Use a fixed timestep (1/60s) with accumulator pattern, not `delta_time`.

3. **Don't move dynamic bodies via Transform** — Dynamic bodies are controlled by physics forces/impulses. Setting Transform directly fights the simulation. Use `Velocity` or `apply_impulse()` instead.

4. **Collider must have a parent RigidBody** — In Rapier, colliders attach to bodies. An entity with `Collider` but no `RigidBody` won't work.

5. **Entity cleanup on despawn** — When an ECS entity is despawned, its Rapier body and colliders must also be removed. Add a cleanup system or hook into despawn events.

6. **Scale and units** — Rapier works best with objects sized 0.1–10 units. If your sprites are 100px, scale down for physics and back up for rendering. 1 physics unit = 1 meter is conventional.

## References

- [Rapier docs](https://rapier.rs/docs/)
- [Rapier 2D Rust API](https://docs.rs/rapier2d/latest/rapier2d/)
- [Fixed timestep explanation](https://gafferongames.com/post/fix_your_timestep/)
- [Bevy Rapier plugin](https://github.com/dimforge/bevy_rapier) (integration inspiration)
