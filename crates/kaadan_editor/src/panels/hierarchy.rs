//! Hierarchy panel: a tree of all entities in the scene world.

use kaadan_ecs::{Entity, World};
use kaadan_renderer::{DirectionalLight, Mesh3D, PointLight, Sprite};
use kaadan_scene::{Children, Parent};

use crate::components::Name;

struct Row {
    entity: Entity,
    label: String,
    depth: usize,
}

pub fn show(ui: &mut egui::Ui, world: &World, selected: &mut Option<Entity>) {
    ui.heading("Hierarchy");
    ui.separator();

    let rows = collect(world);
    if rows.is_empty() {
        ui.label("(empty scene)");
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for row in rows {
            ui.horizontal(|ui| {
                ui.add_space(row.depth as f32 * 14.0);
                let is_selected = *selected == Some(row.entity);
                if ui.selectable_label(is_selected, row.label).clicked() {
                    *selected = Some(row.entity);
                }
            });
        }
    });
}

/// Walk roots (entities without a `Parent`) depth-first through `Children`,
/// producing a flat list annotated with indentation depth.
fn collect(world: &World) -> Vec<Row> {
    let mut roots: Vec<Entity> = world
        .inner()
        .iter()
        .filter(|e| !e.has::<Parent>())
        .map(|e| e.entity())
        .collect();
    roots.sort_by_key(|e| e.id());

    let mut rows = Vec::new();
    for root in roots {
        walk(world, root, 0, &mut rows);
    }
    rows
}

fn walk(world: &World, entity: Entity, depth: usize, rows: &mut Vec<Row>) {
    rows.push(Row {
        entity,
        label: label_for(world, entity),
        depth,
    });
    if let Ok(children) = world.get::<Children>(entity) {
        let kids = children.0.clone();
        drop(children);
        for child in kids {
            if world.is_alive(child) {
                walk(world, child, depth + 1, rows);
            }
        }
    }
}

fn label_for(world: &World, entity: Entity) -> String {
    if let Ok(name) = world.get::<Name>(entity) {
        return name.0.clone();
    }
    let kind = if world.get::<Mesh3D>(entity).is_ok() {
        "Mesh"
    } else if world.get::<Sprite>(entity).is_ok() {
        "Sprite"
    } else if world.get::<DirectionalLight>(entity).is_ok() {
        "Directional Light"
    } else if world.get::<PointLight>(entity).is_ok() {
        "Point Light"
    } else {
        "Entity"
    };
    format!("{kind} {}", entity.id())
}
