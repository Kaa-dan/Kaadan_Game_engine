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

/// Attach `child` to `parent`, maintaining both [`Parent`] and [`Children`]
/// components. Removes the child from any previous parent's list.
pub fn set_parent(world: &mut kaadan_ecs::World, child: Entity, parent: Entity) {
    // Detach from a previous parent, if any.
    if let Ok(prev) = world.get::<Parent>(child).map(|p| p.0) {
        if let Ok(mut children) = world.get_mut::<Children>(prev) {
            children.0.retain(|&e| e != child);
        }
    }

    let _ = world.inner_mut().insert_one(child, Parent(parent));

    if let Ok(mut children) = world.get_mut::<Children>(parent) {
        if !children.0.contains(&child) {
            children.0.push(child);
        }
        return;
    }
    let _ = world.inner_mut().insert_one(parent, Children(vec![child]));
}

/// System: propagate transforms down the hierarchy to any depth.
///
/// Each root (an entity with a `Transform` and no `Parent`) seeds the walk with
/// its local transform; every descendant's `GlobalTransform` is computed as
/// `parent_global * child_local` by depth-first recursion. `GlobalTransform` is
/// inserted if missing, so callers don't have to pre-add it.
///
/// Assumes the hierarchy is a forest (no cycles), which [`set_parent`] upholds.
pub fn transform_propagation_system(
    world: &mut kaadan_ecs::World,
    _resources: &mut kaadan_ecs::Resources,
) {
    let roots: Vec<(Entity, Transform)> = world
        .query::<&Transform>()
        .without::<&Parent>()
        .iter()
        .map(|(e, t)| (e, *t))
        .collect();

    for (entity, local) in roots {
        propagate(world, entity, local);
    }
}

/// Write `entity`'s `GlobalTransform`, then recurse into its children.
fn propagate(world: &mut kaadan_ecs::World, entity: Entity, global: Transform) {
    set_global(world, entity, global);

    let children = world
        .get::<Children>(entity)
        .map(|c| c.0.clone())
        .unwrap_or_default();

    for child in children {
        let child_local = world
            .get::<Transform>(child)
            .map(|t| *t)
            .unwrap_or(Transform::IDENTITY);
        propagate(world, child, global.mul_transform(&child_local));
    }
}

fn set_global(world: &mut kaadan_ecs::World, entity: Entity, transform: Transform) {
    // Assigning to a bool ends the mutable borrow from get_mut before the
    // inner_mut() insert in the missing-component case.
    let updated = if let Ok(mut global) = world.get_mut::<GlobalTransform>(entity) {
        global.0 = transform;
        true
    } else {
        false
    };
    if !updated {
        let _ = world
            .inner_mut()
            .insert_one(entity, GlobalTransform(transform));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaadan_ecs::{Resources, World};
    use kaadan_math::Vec3;

    #[test]
    fn propagates_through_grandchildren() {
        let mut world = World::new();
        // Spawn without GlobalTransform to also exercise auto-insertion.
        let root = world.spawn((Transform::from_position(Vec3::new(10.0, 0.0, 0.0)),));
        let child = world.spawn((Transform::from_position(Vec3::new(0.0, 5.0, 0.0)),));
        let grandchild = world.spawn((Transform::from_position(Vec3::new(0.0, 0.0, 2.0)),));
        set_parent(&mut world, child, root);
        set_parent(&mut world, grandchild, child);

        let mut resources = Resources::new();
        transform_propagation_system(&mut world, &mut resources);

        let g = world.get::<GlobalTransform>(grandchild).unwrap();
        assert_eq!(g.0.position, Vec3::new(10.0, 5.0, 2.0));
        let c = world.get::<GlobalTransform>(child).unwrap();
        assert_eq!(c.0.position, Vec3::new(10.0, 5.0, 0.0));
    }
}
