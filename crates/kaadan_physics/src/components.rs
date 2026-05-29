use kaadan_math::Vec2;

/// Rigid body type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyType {
    Dynamic,
    Kinematic,
    Static,
}

/// Component: marks an entity as having a physics rigid body.
pub struct RigidBody {
    pub body_type: RigidBodyType,
    /// Rapier handle, set by the physics sync system.
    #[allow(dead_code)]
    pub(crate) handle: Option<rapier2d::dynamics::RigidBodyHandle>,
}

impl RigidBody {
    pub fn dynamic() -> Self {
        Self {
            body_type: RigidBodyType::Dynamic,
            handle: None,
        }
    }

    pub fn kinematic() -> Self {
        Self {
            body_type: RigidBodyType::Kinematic,
            handle: None,
        }
    }

    pub fn fixed() -> Self {
        Self {
            body_type: RigidBodyType::Static,
            handle: None,
        }
    }
}

/// Collider shape.
#[derive(Debug, Clone)]
pub enum ColliderShape {
    Box { half_width: f32, half_height: f32 },
    Circle { radius: f32 },
    Capsule { half_height: f32, radius: f32 },
}

/// Component: attaches a collider to an entity.
pub struct Collider {
    pub shape: ColliderShape,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub is_sensor: bool,
    #[allow(dead_code)]
    pub(crate) handle: Option<rapier2d::geometry::ColliderHandle>,
}

impl Collider {
    pub fn box_shape(half_width: f32, half_height: f32) -> Self {
        Self {
            shape: ColliderShape::Box {
                half_width,
                half_height,
            },
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
            is_sensor: false,
            handle: None,
        }
    }

    pub fn circle(radius: f32) -> Self {
        Self {
            shape: ColliderShape::Circle { radius },
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
            is_sensor: false,
            handle: None,
        }
    }
}

/// Component: linear and angular velocity.
pub struct Velocity {
    pub linear: Vec2,
    pub angular: f32,
}

impl Default for Velocity {
    fn default() -> Self {
        Self {
            linear: Vec2::ZERO,
            angular: 0.0,
        }
    }
}

/// Whether a collision started or stopped this step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionEventType {
    Started,
    Stopped,
}

/// Collision event produced by the physics step, mapped back to ECS entities.
#[derive(Debug, Clone, Copy)]
pub struct CollisionEvent {
    pub entity_a: kaadan_ecs::Entity,
    pub entity_b: kaadan_ecs::Entity,
    pub event_type: CollisionEventType,
}
