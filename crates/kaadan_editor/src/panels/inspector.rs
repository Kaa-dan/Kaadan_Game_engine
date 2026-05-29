//! Inspector panel: shows the components of the selected entity (read-only in M2).

use kaadan_ecs::{Entity, World};
use kaadan_math::{Color, Transform};
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, PointLight, Sprite};

use crate::components::Name;

pub fn show(ui: &mut egui::Ui, world: &World, selected: Option<Entity>) {
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

    if let Ok(name) = world.get::<Name>(entity) {
        ui.label(egui::RichText::new(&name.0).strong().size(15.0));
    }
    ui.label(format!("Entity id {}", entity.id()));
    ui.separator();

    if let Ok(t) = world.get::<Transform>(entity) {
        section(ui, "Transform", |ui| {
            vec3_row(ui, "Position", t.position.x, t.position.y, t.position.z);
            vec4_row(
                ui,
                "Rotation",
                t.rotation.x,
                t.rotation.y,
                t.rotation.z,
                t.rotation.w,
            );
            vec3_row(ui, "Scale", t.scale.x, t.scale.y, t.scale.z);
        });
    }

    if let Ok(s) = world.get::<Sprite>(entity) {
        section(ui, "Sprite", |ui| {
            color_row(ui, "Color", s.color);
            ui.label(match s.size {
                Some(sz) => format!("Size: {:.1} x {:.1}", sz.x, sz.y),
                None => "Size: (texture)".to_string(),
            });
            ui.label(format!("Z-order: {}", s.z_order));
            ui.label(format!("Texture: #{}", s.texture.index()));
        });
    }

    if let Ok(m) = world.get::<Mesh3D>(entity) {
        section(ui, "Mesh3D", |ui| {
            ui.label(format!("Mesh handle: #{}", m.handle.index()));
        });
    }

    if let Ok(mat) = world.get::<PbrMaterial>(entity) {
        section(ui, "PBR Material", |ui| {
            color_row(ui, "Base color", mat.base_color);
            ui.label(format!("Metallic: {:.2}", mat.metallic));
            ui.label(format!("Roughness: {:.2}", mat.roughness));
            color_row(ui, "Emissive", mat.emissive);
        });
    }

    if let Ok(dl) = world.get::<DirectionalLight>(entity) {
        section(ui, "Directional Light", |ui| {
            vec3_row(
                ui,
                "Direction",
                dl.direction.x,
                dl.direction.y,
                dl.direction.z,
            );
            color_row(ui, "Color", dl.color);
            ui.label(format!("Intensity: {:.2}", dl.intensity));
        });
    }

    if let Ok(pl) = world.get::<PointLight>(entity) {
        section(ui, "Point Light", |ui| {
            color_row(ui, "Color", pl.color);
            ui.label(format!("Intensity: {:.2}", pl.intensity));
            ui.label(format!("Range: {:.2}", pl.range));
        });
    }
}

fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(title)
        .default_open(true)
        .show(ui, add);
}

fn vec3_row(ui: &mut egui::Ui, label: &str, x: f32, y: f32, z: f32) {
    ui.label(format!("{label}: [{x:.3}, {y:.3}, {z:.3}]"));
}

fn vec4_row(ui: &mut egui::Ui, label: &str, x: f32, y: f32, z: f32, w: f32) {
    ui.label(format!("{label}: [{x:.3}, {y:.3}, {z:.3}, {w:.3}]"));
}

fn color_row(ui: &mut egui::Ui, label: &str, c: Color) {
    ui.horizontal(|ui| {
        ui.label(format!("{label}:"));
        let swatch = egui::Color32::from_rgb(
            (c.r.clamp(0.0, 1.0) * 255.0) as u8,
            (c.g.clamp(0.0, 1.0) * 255.0) as u8,
            (c.b.clamp(0.0, 1.0) * 255.0) as u8,
        );
        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, swatch);
        ui.label(format!("{:.2}, {:.2}, {:.2}", c.r, c.g, c.b));
    });
}
