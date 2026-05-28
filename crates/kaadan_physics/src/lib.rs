//! 2D physics simulation powered by [`rapier2d`].
//!
//! Provides rigid bodies, colliders, joints, and collision detection.

mod components;
mod plugin;
mod world;

pub use components::{Collider, ColliderShape, CollisionEvent, RigidBody, RigidBodyType, Velocity};
pub use plugin::PhysicsPlugin;
pub use world::PhysicsWorld;
