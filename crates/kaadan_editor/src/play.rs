//! Play mode: a lightweight runtime preview.
//!
//! The editor has no user scripts yet, so Play runs a built-in preview behaviour
//! (spin 3D meshes) plus hierarchy transform propagation, so pressing Play shows
//! visible motion and Stop demonstrably reverts it from the pre-play snapshot.

use kaadan_ecs::World;
use kaadan_math::{Quat, Transform};
use kaadan_renderer::Mesh3D;

#[derive(Clone, Copy)]
pub enum PlayRequest {
    Start,
    Stop,
}

/// Advance the running scene by `dt` seconds.
pub fn tick(world: &mut World, dt: f32) {
    let spin = Quat::from_rotation_y(0.8 * dt) * Quat::from_rotation_x(0.3 * dt);
    for (_entity, (_mesh, transform)) in world.query::<(&Mesh3D, &mut Transform)>().iter() {
        transform.rotation *= spin;
    }
}
