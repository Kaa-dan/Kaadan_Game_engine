//! Hierarchy panel: a tree of all entities, plus create/delete/duplicate actions.

use kaadan_ecs::{Entity, World};
use kaadan_renderer::{DirectionalLight, Mesh3D, PointLight, Sprite};
use kaadan_scene::{Children, Parent};

use crate::commands::Command;
use crate::components::Name;
use crate::state::EditorState;

enum Action {
    Create,
    Duplicate,
    Delete,
}

struct Row {
    entity: Entity,
    label: String,
    depth: usize,
}

pub fn show(ui: &mut egui::Ui, world: &mut World, state: &mut EditorState) {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.heading("Hierarchy");
        if ui.small_button("➕").on_hover_text("New entity").clicked() {
            action = Some(Action::Create);
        }
        let has_sel = state.selected.is_some();
        if ui
            .add_enabled(has_sel, egui::Button::new("⧉").small())
            .on_hover_text("Duplicate")
            .clicked()
        {
            action = Some(Action::Duplicate);
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("🗑").small())
            .on_hover_text("Delete")
            .clicked()
        {
            action = Some(Action::Delete);
        }
    });
    ui.separator();

    let rows = collect(world);
    let mut clicked = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        if rows.is_empty() {
            ui.label("(empty scene)");
        }
        for row in &rows {
            ui.horizontal(|ui| {
                ui.add_space(row.depth as f32 * 14.0);
                let is_selected = state.selected == Some(row.entity);
                if ui.selectable_label(is_selected, &row.label).clicked() {
                    clicked = Some(row.entity);
                }
            });
        }
    });
    if let Some(e) = clicked {
        state.selected = Some(e);
    }

    match action {
        Some(Action::Create) => {
            state
                .commands
                .run(world, &mut state.selected, Command::create_entity());
        }
        Some(Action::Duplicate) => {
            if let Some(src) = state.selected {
                let cmd = Command::duplicate(world, src);
                state.commands.run(world, &mut state.selected, cmd);
            }
        }
        Some(Action::Delete) => {
            if let Some(target) = state.selected {
                let cmd = Command::delete(world, target);
                state.commands.run(world, &mut state.selected, cmd);
            }
        }
        None => {}
    }
}

/// Walk roots (entities without a `Parent`) depth-first through `Children`.
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
