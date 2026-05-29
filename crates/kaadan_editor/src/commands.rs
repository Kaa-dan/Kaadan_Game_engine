//! Undoable editor commands: create / delete / duplicate entities.
//!
//! Components carry no reflection, so a command snapshots the *known* component
//! set (see [`EntitySnapshot`]) — the single enumeration shared by duplication,
//! deletion, and (later) serialization. Each command is re-runnable: `apply` and
//! `undo` update the command's own stored entity ids, so redo works even though
//! hecs assigns fresh ids on re-spawn.

use std::collections::HashMap;

use kaadan_ecs::{Entity, World};
use kaadan_math::Transform;
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, PointLight, Sprite};
use kaadan_scene::{set_parent, Children, Parent};

use crate::components::Name;

#[derive(Clone, Default)]
pub struct EntitySnapshot {
    pub name: Option<Name>,
    pub transform: Option<Transform>,
    pub sprite: Option<Sprite>,
    pub mesh: Option<Mesh3D>,
    pub material: Option<PbrMaterial>,
    pub dir_light: Option<DirectionalLight>,
    pub point_light: Option<PointLight>,
}

pub fn snapshot_entity(world: &World, e: Entity) -> EntitySnapshot {
    EntitySnapshot {
        name: world.get::<Name>(e).ok().map(|c| (*c).clone()),
        transform: world.get::<Transform>(e).ok().map(|c| *c),
        sprite: world.get::<Sprite>(e).ok().map(|c| (*c).clone()),
        mesh: world.get::<Mesh3D>(e).ok().map(|c| *c),
        material: world.get::<PbrMaterial>(e).ok().map(|c| (*c).clone()),
        dir_light: world.get::<DirectionalLight>(e).ok().map(|c| (*c).clone()),
        point_light: world.get::<PointLight>(e).ok().map(|c| (*c).clone()),
    }
}

pub fn spawn_snapshot(world: &mut World, snap: &EntitySnapshot) -> Entity {
    let e = world.spawn(());
    let w = world.inner_mut();
    if let Some(n) = &snap.name {
        let _ = w.insert_one(e, n.clone());
    }
    if let Some(t) = snap.transform {
        let _ = w.insert_one(e, t);
    }
    if let Some(s) = &snap.sprite {
        let _ = w.insert_one(e, s.clone());
    }
    if let Some(m) = snap.mesh {
        let _ = w.insert_one(e, m);
    }
    if let Some(m) = &snap.material {
        let _ = w.insert_one(e, m.clone());
    }
    if let Some(l) = &snap.dir_light {
        let _ = w.insert_one(e, l.clone());
    }
    if let Some(l) = &snap.point_light {
        let _ = w.insert_one(e, l.clone());
    }
    e
}

/// Spawns one entity from a template (used by both Create and Duplicate).
pub struct SpawnData {
    template: EntitySnapshot,
    live: Option<Entity>,
}

/// Deletes an entity and its descendants, capturing enough to restore them.
pub struct DeleteData {
    snapshots: Vec<EntitySnapshot>,
    /// For each node, the index of its parent within this subtree (if internal).
    parent_local: Vec<Option<usize>>,
    /// The root's parent, if it lives outside the deleted subtree.
    external_parent: Option<Entity>,
    /// Live ids: the originals on first apply, refreshed to new ids on undo.
    live_ids: Vec<Entity>,
}

pub enum Command {
    Spawn(SpawnData),
    Delete(DeleteData),
}

impl Command {
    pub fn create_entity() -> Self {
        Command::Spawn(SpawnData {
            template: EntitySnapshot {
                name: Some(Name::new("Entity")),
                transform: Some(Transform::IDENTITY),
                ..Default::default()
            },
            live: None,
        })
    }

    pub fn duplicate(world: &World, source: Entity) -> Self {
        let mut template = snapshot_entity(world, source);
        if let Some(name) = &template.name {
            template.name = Some(Name(format!("{} copy", name.0)));
        }
        Command::Spawn(SpawnData {
            template,
            live: None,
        })
    }

    pub fn delete(world: &World, root: Entity) -> Self {
        let ids = gather_subtree(world, root);
        let index_of: HashMap<Entity, usize> =
            ids.iter().enumerate().map(|(i, &e)| (e, i)).collect();
        let snapshots = ids.iter().map(|&e| snapshot_entity(world, e)).collect();
        let parent_local = ids
            .iter()
            .map(|&e| {
                world
                    .get::<Parent>(e)
                    .ok()
                    .and_then(|p| index_of.get(&p.0).copied())
            })
            .collect();
        let external_parent = world
            .get::<Parent>(root)
            .ok()
            .map(|p| p.0)
            .filter(|p| !index_of.contains_key(p));
        Command::Delete(DeleteData {
            snapshots,
            parent_local,
            external_parent,
            live_ids: ids,
        })
    }

    pub fn apply(&mut self, world: &mut World, selection: &mut Option<Entity>) {
        match self {
            Command::Spawn(d) => {
                let e = spawn_snapshot(world, &d.template);
                d.live = Some(e);
                *selection = Some(e);
            }
            Command::Delete(d) => {
                for &e in &d.live_ids {
                    let _ = world.despawn(e);
                }
                if matches!(*selection, Some(s) if d.live_ids.contains(&s)) {
                    *selection = None;
                }
            }
        }
    }

    pub fn undo(&mut self, world: &mut World, selection: &mut Option<Entity>) {
        match self {
            Command::Spawn(d) => {
                if let Some(e) = d.live.take() {
                    d.template = snapshot_entity(world, e);
                    let _ = world.despawn(e);
                    if *selection == Some(e) {
                        *selection = None;
                    }
                }
            }
            Command::Delete(d) => {
                let new_ids: Vec<Entity> = d
                    .snapshots
                    .iter()
                    .map(|snap| spawn_snapshot(world, snap))
                    .collect();
                for (i, parent) in d.parent_local.iter().enumerate() {
                    if let Some(pi) = parent {
                        set_parent(world, new_ids[i], new_ids[*pi]);
                    }
                }
                if let Some(ext) = d.external_parent {
                    if world.is_alive(ext) {
                        set_parent(world, new_ids[0], ext);
                    }
                }
                *selection = new_ids.first().copied();
                d.live_ids = new_ids;
            }
        }
    }
}

fn gather_subtree(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        out.push(e);
        if let Ok(children) = world.get::<Children>(e) {
            for &c in &children.0 {
                if world.is_alive(c) {
                    stack.push(c);
                }
            }
        }
    }
    out
}

#[derive(Default)]
pub struct UndoStack {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl UndoStack {
    pub fn run(&mut self, world: &mut World, selection: &mut Option<Entity>, mut cmd: Command) {
        cmd.apply(world, selection);
        self.undo.push(cmd);
        self.redo.clear();
    }

    pub fn undo(&mut self, world: &mut World, selection: &mut Option<Entity>) {
        if let Some(mut cmd) = self.undo.pop() {
            cmd.undo(world, selection);
            self.redo.push(cmd);
        }
    }

    pub fn redo(&mut self, world: &mut World, selection: &mut Option<Entity>) {
        if let Some(mut cmd) = self.redo.pop() {
            cmd.apply(world, selection);
            self.undo.push(cmd);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
