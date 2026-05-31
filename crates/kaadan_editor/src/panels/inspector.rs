//! Inspector panel: view and edit the components of the selected entity.

use egui::DragValue;
use kaadan_ecs::{Entity, World};
use kaadan_math::{Color, EulerRot, Quat, Transform};
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, PointLight, Sprite};

use crate::components::Name;

pub fn show(ui: &mut egui::Ui, world: &mut World, selected: Option<Entity>) {
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

    if let Ok(mut t_ref) = world.get_mut::<Transform>(entity) {
        let t = &mut *t_ref;
        section(ui, "Transform", |ui| {
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
        section(ui, "Sprite", |ui| {
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
        section(ui, "Mesh3D", |ui| {
            ui.label(format!("Mesh handle: #{handle}"));
        });
    }

    if let Ok(mut mat_ref) = world.get_mut::<PbrMaterial>(entity) {
        let mat = &mut *mat_ref;
        section(ui, "PBR Material", |ui| {
            color_row(ui, "Base color", &mut mat.base_color);
            slider(ui, "Metallic", &mut mat.metallic, 0.0, 1.0);
            slider(ui, "Roughness", &mut mat.roughness, 0.0, 1.0);
            color_row(ui, "Emissive", &mut mat.emissive);
        });
    }

    if let Ok(mut dl_ref) = world.get_mut::<DirectionalLight>(entity) {
        let dl = &mut *dl_ref;
        section(ui, "Directional Light", |ui| {
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
        section(ui, "Point Light", |ui| {
            color_row(ui, "Color", &mut pl.color);
            slider(ui, "Intensity", &mut pl.intensity, 0.0, 10.0);
            slider(ui, "Range", &mut pl.range, 0.0, 100.0);
        });
    }
}

fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(title)
        .default_open(true)
        .show(ui, add);
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
