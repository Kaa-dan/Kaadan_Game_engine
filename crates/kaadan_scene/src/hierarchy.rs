use kaadan_ecs::Entity;
use kaadan_math::Transform;

/// Component: this entity's parent.
pub struct Parent(pub Entity);

/// Component: this entity's children.
pub struct Children(pub Vec<Entity>);

/// Component: world-space transform computed from the hierarchy.
#[derive(Debug, Clone, Copy)]
pub struct GlobalTransform(pub Transform);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Transform::IDENTITY)
    }
}

/// System: propagate transforms down the hierarchy.
/// Entities with no Parent get GlobalTransform = local Transform.
/// Children get GlobalTransform = parent.GlobalTransform * child.Transform.
pub fn transform_propagation_system(
    world: &mut kaadan_ecs::World,
    _resources: &mut kaadan_ecs::Resources,
) {
    // First pass: update root entities (no Parent)
    let roots: Vec<(Entity, Transform)> = world
        .query::<(&Transform,)>()
        .without::<&Parent>()
        .iter()
        .map(|(e, (t,))| (e, *t))
        .collect();

    for (entity, transform) in &roots {
        if let Ok(mut global) = world.get_mut::<GlobalTransform>(*entity) {
            global.0 = *transform;
        }
    }

    // Second pass: propagate to children
    let parent_list: Vec<(Entity, Vec<Entity>)> = world
        .query::<(&Children,)>()
        .iter()
        .map(|(e, (c,))| (e, c.0.clone()))
        .collect();

    for (parent_entity, children) in &parent_list {
        let parent_global = world
            .get::<GlobalTransform>(*parent_entity)
            .map(|g| g.0)
            .unwrap_or(Transform::IDENTITY);

        for &child in children {
            let child_local = world
                .get::<Transform>(child)
                .map(|t| *t)
                .unwrap_or(Transform::IDENTITY);
            let child_global = parent_global.mul_transform(&child_local);
            if let Ok(mut global) = world.get_mut::<GlobalTransform>(child) {
                global.0 = child_global;
            }
        }
    }
}
