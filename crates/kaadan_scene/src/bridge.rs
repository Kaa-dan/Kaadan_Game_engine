use kaadan_ecs::{Entity, World};
use kaadan_math::{Quat, Transform, Vec3};

use crate::hierarchy::{set_parent, Children, GlobalTransform, Parent};
use crate::scene::{EntityDesc, Scene, TransformDesc};

/// Canonical scene-level display name for an entity. Round-trips through
/// [`Scene`] via [`EntityDesc::name`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(pub String);

/// Free-form string tags on an entity (e.g. `"player"`, `"enemy"`). Round-trips
/// through [`Scene`] via [`EntityDesc::tags`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags(pub Vec<String>);

impl From<&TransformDesc> for Transform {
    fn from(d: &TransformDesc) -> Self {
        Transform {
            position: Vec3::from_array(d.position),
            rotation: Quat::from_array(d.rotation),
            scale: Vec3::from_array(d.scale),
        }
    }
}

impl From<&Transform> for TransformDesc {
    fn from(t: &Transform) -> Self {
        TransformDesc {
            position: t.position.to_array(),
            rotation: t.rotation.to_array(),
            scale: t.scale.to_array(),
        }
    }
}

impl Scene {
    /// Spawn every entity in this scene into `world`, returning the roots.
    ///
    /// Each entity gets its [`Transform`] (plus a default [`GlobalTransform`] so
    /// [`transform_propagation_system`](crate::transform_propagation_system) has
    /// something to write), a [`Name`] if named, [`Tags`] if any, and is parented
    /// per the descriptor's nesting.
    pub fn spawn_into(&self, world: &mut World) -> Vec<Entity> {
        self.entities
            .iter()
            .map(|desc| spawn_desc(world, desc, None))
            .collect()
    }

    /// Extract `world`'s root entities (those without a [`Parent`]) into a
    /// serializable scene named `name`. Roots are ordered by entity id for a
    /// stable, diff-friendly serialization.
    pub fn from_world(world: &World, name: impl Into<String>) -> Scene {
        let mut roots: Vec<Entity> = world
            .inner()
            .iter()
            .filter(|e| !e.has::<Parent>())
            .map(|e| e.entity())
            .collect();
        roots.sort_by_key(|e| e.id());
        Scene {
            name: name.into(),
            entities: roots.iter().map(|&e| extract_desc(world, e)).collect(),
        }
    }
}

fn spawn_desc(world: &mut World, desc: &EntityDesc, parent: Option<Entity>) -> Entity {
    let entity = world.spawn(());
    {
        let w = world.inner_mut();
        if let Some(name) = &desc.name {
            let _ = w.insert_one(entity, Name(name.clone()));
        }
        if let Some(t) = &desc.transform {
            let _ = w.insert_one(entity, Transform::from(t));
            let _ = w.insert_one(entity, GlobalTransform::default());
        }
        if !desc.tags.is_empty() {
            let _ = w.insert_one(entity, Tags(desc.tags.clone()));
        }
    }
    if let Some(parent) = parent {
        set_parent(world, entity, parent);
    }
    for child in &desc.children {
        spawn_desc(world, child, Some(entity));
    }
    entity
}

fn extract_desc(world: &World, entity: Entity) -> EntityDesc {
    let name = world.get::<Name>(entity).ok().map(|n| n.0.clone());
    let transform = world
        .get::<Transform>(entity)
        .ok()
        .map(|t| TransformDesc::from(&*t));
    let tags = world
        .get::<Tags>(entity)
        .ok()
        .map(|t| t.0.clone())
        .unwrap_or_default();
    let children = world
        .get::<Children>(entity)
        .ok()
        .map(|c| c.0.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|&c| world.is_alive(c))
        .map(|c| extract_desc(world, c))
        .collect();
    EntityDesc {
        name,
        transform,
        children,
        tags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_scene_world_roundtrip() {
        let mut world = World::new();
        let parent = world.spawn((
            Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
            Name("Parent".to_string()),
            Tags(vec!["root".to_string()]),
        ));
        let child = world.spawn((
            Transform::from_position(Vec3::new(0.0, 5.0, 0.0)),
            Name("Child".to_string()),
        ));
        set_parent(&mut world, child, parent);

        // Extract to a scene: one root with one child.
        let scene = Scene::from_world(&world, "level");
        assert_eq!(scene.name, "level");
        assert_eq!(scene.entities.len(), 1, "one root");
        let root_desc = &scene.entities[0];
        assert_eq!(root_desc.name.as_deref(), Some("Parent"));
        assert_eq!(root_desc.tags, vec!["root".to_string()]);
        assert_eq!(root_desc.children.len(), 1);
        assert_eq!(root_desc.children[0].name.as_deref(), Some("Child"));

        // Spawn into a fresh world and confirm the shape survives.
        let mut world2 = World::new();
        let roots = scene.spawn_into(&mut world2);
        assert_eq!(roots.len(), 1);
        let new_parent = roots[0];
        assert_eq!(world2.get::<Name>(new_parent).unwrap().0, "Parent");
        assert_eq!(
            world2.get::<Transform>(new_parent).unwrap().position,
            Vec3::new(1.0, 2.0, 3.0)
        );
        let kids = world2.get::<Children>(new_parent).unwrap().0.clone();
        assert_eq!(kids.len(), 1);
        assert_eq!(world2.get::<Name>(kids[0]).unwrap().0, "Child");
        assert_eq!(world2.get::<Parent>(kids[0]).unwrap().0, new_parent);
    }

    #[test]
    fn ron_scene_spawns_into_world() {
        let scene = Scene {
            name: "s".to_string(),
            entities: vec![EntityDesc {
                name: Some("Hero".to_string()),
                transform: Some(TransformDesc {
                    position: [4.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                }),
                children: vec![],
                tags: vec!["player".to_string()],
            }],
        };
        let ron = scene.to_ron().unwrap();
        let loaded = Scene::from_ron(&ron).unwrap();

        let mut world = World::new();
        let roots = loaded.spawn_into(&mut world);
        assert_eq!(roots.len(), 1);
        assert_eq!(world.get::<Name>(roots[0]).unwrap().0, "Hero");
        assert_eq!(world.get::<Tags>(roots[0]).unwrap().0, vec!["player"]);
        assert_eq!(
            world.get::<Transform>(roots[0]).unwrap().position,
            Vec3::new(4.0, 0.0, 0.0)
        );
    }
}
