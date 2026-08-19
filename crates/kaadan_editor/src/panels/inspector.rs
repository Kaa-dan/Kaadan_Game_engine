use egui::DragValue;
use kaadan_ecs::{Entity, World};
use kaadan_math::{Color, EulerRot, Quat, Transform};
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, PointLight, Sprite};
use kaadan_script::ComponentRegistry;

use crate::components::{Name, Scripts};

pub fn show(
    ui: &mut egui::Ui,
    world: &mut World,
    selected: Option<Entity>,
    registry: &ComponentRegistry,
    available_behaviours: &[String],
    rescan: &mut bool,
) {
    ui.heading("Inspector");
    ui.separator();

    let Some(entity) = selected else {
        ui.label("(nothing selected)");
        return;
    };
    if !world.is_alive(entity) {
        ui.label("(selection no longer exists)");
        return;
    }

    if let Ok(mut name) = world.get_mut::<Name>(entity) {
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut name.0);
        });
    }
    ui.label(format!("Entity id {}", entity.id()));
    ui.separator();

    // Each `section_with_remove` queues a removal request through the registry
    // (we can't mutate the world while a `get_mut` borrow is live).
    let mut to_remove: Option<&'static str> = None;

    if let Ok(mut t_ref) = world.get_mut::<Transform>(entity) {
        let t = &mut *t_ref;
        section_with_remove(ui, "Transform", &mut to_remove, |ui| {
            drag3(
                ui,
                "Position",
                &mut t.position.x,
                &mut t.position.y,
                &mut t.position.z,
            );
            rotation_row(ui, &mut t.rotation);
            drag3(ui, "Scale", &mut t.scale.x, &mut t.scale.y, &mut t.scale.z);
        });
    }

    if let Ok(mut s_ref) = world.get_mut::<Sprite>(entity) {
        let s = &mut *s_ref;
        section_with_remove(ui, "Sprite", &mut to_remove, |ui| {
            color_row(ui, "Color", &mut s.color);
            ui.horizontal(|ui| {
                ui.label("Z-order");
                ui.add(DragValue::new(&mut s.z_order));
            });
            ui.label(format!("Texture: #{}", s.texture.index()));
        });
    }

    if let Ok(m) = world.get::<Mesh3D>(entity) {
        let handle = m.handle.index();
        drop(m);
        section_with_remove(ui, "Mesh3D", &mut to_remove, |ui| {
            ui.label(format!("Mesh handle: #{handle}"));
        });
    }

    if let Ok(mut mat_ref) = world.get_mut::<PbrMaterial>(entity) {
        let mat = &mut *mat_ref;
        section_with_remove(ui, "PbrMaterial", &mut to_remove, |ui| {
            color_row(ui, "Base color", &mut mat.base_color);
            slider(ui, "Metallic", &mut mat.metallic, 0.0, 1.0);
            slider(ui, "Roughness", &mut mat.roughness, 0.0, 1.0);
            color_row(ui, "Emissive", &mut mat.emissive);
        });
    }

    if let Ok(mut dl_ref) = world.get_mut::<DirectionalLight>(entity) {
        let dl = &mut *dl_ref;
        section_with_remove(ui, "DirectionalLight", &mut to_remove, |ui| {
            drag3(
                ui,
                "Direction",
                &mut dl.direction.x,
                &mut dl.direction.y,
                &mut dl.direction.z,
            );
            color_row(ui, "Color", &mut dl.color);
            slider(ui, "Intensity", &mut dl.intensity, 0.0, 10.0);
        });
    }

    if let Ok(mut pl_ref) = world.get_mut::<PointLight>(entity) {
        let pl = &mut *pl_ref;
        section_with_remove(ui, "PointLight", &mut to_remove, |ui| {
            color_row(ui, "Color", &mut pl.color);
            slider(ui, "Intensity", &mut pl.intensity, 0.0, 10.0);
            slider(ui, "Range", &mut pl.range, 0.0, 100.0);
        });
    }

    if let Some(name) = to_remove {
        registry.remove(world, entity, name);
    }

    scripts_section(ui, world, entity, available_behaviours, rescan);

    ui.separator();
    add_component_combo(ui, world, entity, registry);
}

fn scripts_section(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    available: &[String],
    rescan: &mut bool,
) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.strong("Scripts");
        if ui
            .small_button("↻")
            .on_hover_text("Rescan the built game crate for behaviours")
            .clicked()
        {
            *rescan = true;
        }
    });

    // Existing attached behaviours, each with a remove button.
    let current: Vec<String> = world
        .get::<Scripts>(entity)
        .map(|s| s.0.clone())
        .unwrap_or_default();
    let mut remove: Option<usize> = None;
    for (i, name) in current.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("• {name}"));
            if ui.small_button("×").on_hover_text("Detach").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        if let Ok(mut s) = world.get_mut::<Scripts>(entity) {
            if i < s.0.len() {
                s.0.remove(i);
            }
        }
    }

    // Add-behaviour dropdown.
    if available.is_empty() {
        ui.label("(Build the game crate, then press ↻)");
    } else {
        let mut pending: Option<String> = None;
        egui::ComboBox::from_id_salt("inspector_add_behaviour")
            .selected_text("Add behaviour…")
            .show_ui(ui, |ui| {
                for name in available {
                    if ui.selectable_label(false, name).clicked() {
                        pending = Some(name.clone());
                    }
                }
            });
        if let Some(name) = pending {
            if world.get::<Scripts>(entity).is_ok() {
                if let Ok(mut s) = world.get_mut::<Scripts>(entity) {
                    if !s.0.contains(&name) {
                        s.0.push(name);
                    }
                }
            } else {
                let _ = world.inner_mut().insert_one(entity, Scripts(vec![name]));
            }
        }
    }
}

fn section_with_remove(
    ui: &mut egui::Ui,
    name: &'static str,
    to_remove: &mut Option<&'static str>,
    add: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        let header = egui::CollapsingHeader::new(name).default_open(true);
        header.show(ui, add);
        // Right-aligned remove button. We can't put it inside the header label,
        // so place it on the same row to the right.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("×")
                .on_hover_text(format!("Remove {name}"))
                .clicked()
            {
                *to_remove = Some(name);
            }
        });
    });
}

fn add_component_combo(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    registry: &ComponentRegistry,
) {
    // Snapshot the choices so we don't borrow `registry` across the closure.
    let choices: Vec<&'static str> = registry
        .names()
        .filter(|name| registry.has(world, entity, name) == Some(false))
        .collect();

    ui.horizontal(|ui| {
        ui.label("Add component:");
        let mut selected_idx: usize = 0;
        let mut pending: Option<&'static str> = None;
        egui::ComboBox::from_id_salt("inspector_add_component")
            .selected_text(choices.first().copied().unwrap_or("(none)"))
            .show_ui(ui, |ui| {
                for (i, name) in choices.iter().enumerate() {
                    ui.selectable_value(&mut selected_idx, i, *name);
                }
            });
        if !choices.is_empty()
            && ui
                .add_enabled(!choices.is_empty(), egui::Button::new("Add"))
                .clicked()
        {
            pending = choices.get(selected_idx).copied();
        }
        if let Some(name) = pending {
            // Only types registered with `register_default` can be added here;
            // others (Parent/Children/Sprite/Mesh3D) need editor-side setup.
            if !registry.insert_default(world, entity, name) {
                tracing::warn!("'{name}' has no default ctor; add it via scene tools");
            }
        }
    });
}

fn drag3(ui: &mut egui::Ui, label: &str, x: &mut f32, y: &mut f32, z: &mut f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(DragValue::new(x).speed(0.05).prefix("x "));
        ui.add(DragValue::new(y).speed(0.05).prefix("y "));
        ui.add(DragValue::new(z).speed(0.05).prefix("z "));
    });
}

fn rotation_row(ui: &mut egui::Ui, rotation: &mut Quat) {
    let (ex, ey, ez) = rotation.to_euler(EulerRot::XYZ);
    let mut deg = [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];
    ui.horizontal(|ui| {
        ui.label("Rotation");
        let mut changed = false;
        for v in &mut deg {
            changed |= ui.add(DragValue::new(v).speed(0.5).suffix("°")).changed();
        }
        if changed {
            *rotation = Quat::from_euler(
                EulerRot::XYZ,
                deg[0].to_radians(),
                deg[1].to_radians(),
                deg[2].to_radians(),
            );
        }
    });
}

fn slider(ui: &mut egui::Ui, label: &str, value: &mut f32, min: f32, max: f32) {
    ui.add(egui::Slider::new(value, min..=max).text(label));
}

fn color_row(ui: &mut egui::Ui, label: &str, c: &mut Color) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut rgb = [c.r, c.g, c.b];
        if ui.color_edit_button_rgb(&mut rgb).changed() {
            c.r = rgb[0];
            c.g = rgb[1];
            c.b = rgb[2];
        }
    });
}
