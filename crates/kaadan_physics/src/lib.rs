//! 2D physics simulation powered by [`rapier2d`].
//!
//! Provides rigid bodies, colliders, joints, and collision detection.

mod components;
mod plugin;
mod world;

pub use components::{
    Collider, ColliderShape, CollisionEvent, CollisionEventType, RigidBody, RigidBodyType, Velocity,
};
pub use plugin::{
    physics_step_system, physics_sync_system, physics_writeback_system, PhysicsPlugin,
};
pub use world::{PhysicsWorld, RaycastHit};
