use std::collections::HashMap;

use kaadan_ecs::Entity;
use kaadan_math::Vec2;
use rapier2d::crossbeam::channel::{unbounded, Receiver};
use rapier2d::prelude::*;

use crate::components::{CollisionEvent, CollisionEventType};

/// Result of a successful raycast against the physics world.
#[derive(Debug, Clone, Copy)]
pub struct RaycastHit {
    pub entity: Entity,
    pub distance: f32,
    pub point: Vec2,
}

/// Wraps Rapier's physics pipeline, the rigid-body/collider sets, and the
/// ECS-entity mappings needed to translate simulation results back to the world.
pub struct PhysicsWorld {
    pub gravity: nalgebra::Vector2<f32>,
    pub rigid_body_set: RigidBodySet,
    pub collider_set: ColliderSet,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub query_pipeline: QueryPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joint_set: ImpulseJointSet,
    pub multibody_joint_set: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub entity_to_body: HashMap<Entity, RigidBodyHandle>,
    pub body_to_entity: HashMap<RigidBodyHandle, Entity>,
    pub collider_to_entity: HashMap<ColliderHandle, Entity>,
    /// Collision events produced by the most recent [`PhysicsWorld::step`].
    pub collision_events: Vec<CollisionEvent>,
    event_handler: ChannelEventCollector,
    collision_recv: Receiver<rapier2d::geometry::CollisionEvent>,
}

impl Default for PhysicsWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self::with_gravity(Vec2::new(0.0, -9.81))
    }

    pub fn with_gravity(gravity: Vec2) -> Self {
        let (collision_send, collision_recv) = unbounded();
        let (force_send, _force_recv) = unbounded();
        Self {
            gravity: nalgebra::Vector2::new(gravity.x, gravity.y),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            query_pipeline: QueryPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            entity_to_body: HashMap::new(),
            body_to_entity: HashMap::new(),
            collider_to_entity: HashMap::new(),
            collision_events: Vec::new(),
            event_handler: ChannelEventCollector::new(collision_send, force_send),
            collision_recv,
        }
    }

    /// Advance the simulation by one step, then collect collision events
    /// (mapped to ECS entities) into [`PhysicsWorld::collision_events`].
    pub fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &self.event_handler,
        );

        self.collision_events.clear();
        while let Ok(event) = self.collision_recv.try_recv() {
            let a = self.collider_to_entity.get(&event.collider1()).copied();
            let b = self.collider_to_entity.get(&event.collider2()).copied();
            if let (Some(entity_a), Some(entity_b)) = (a, b) {
                self.collision_events.push(CollisionEvent {
                    entity_a,
                    entity_b,
                    event_type: if event.started() {
                        CollisionEventType::Started
                    } else {
                        CollisionEventType::Stopped
                    },
                });
            }
        }
    }

    /// Cast a ray and return the first entity hit.
    pub fn raycast(&self, origin: Vec2, direction: Vec2, max_distance: f32) -> Option<RaycastHit> {
        let ray = Ray::new(
            nalgebra::Point2::new(origin.x, origin.y),
            nalgebra::Vector2::new(direction.x, direction.y),
        );
        let (collider, toi) = self.query_pipeline.cast_ray(
            &self.rigid_body_set,
            &self.collider_set,
            &ray,
            max_distance,
            true,
            QueryFilter::default(),
        )?;
        let entity = self.collider_to_entity.get(&collider).copied()?;
        let point = origin + direction * toi;
        Some(RaycastHit {
            entity,
            distance: toi,
            point,
        })
    }
}
