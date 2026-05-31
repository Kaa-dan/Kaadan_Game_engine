//! Assets panel: lists the GPU assets (textures, meshes) currently registered
//! with the viewport's asset registry.
//!
//! Deferred: a real asset *browser* (filesystem scan, thumbnails, drag-and-drop
//! onto entities, import settings) needs a project/asset-pipeline layer that
//! doesn't exist yet. For Phase 7 the panel is a read-only inventory of what
//! the running editor has loaded — enough to confirm dedup is working and to
//! see what a saved scene will reference. Real browsing lands when the asset
//! pipeline (`kaadan_assets`) is wired through the editor.

use crate::viewport::Viewport;

pub fn show(ui: &mut egui::Ui, viewport: &Viewport) {
    ui.heading("Assets");
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::CollapsingHeader::new("Textures")
            .default_open(true)
            .show(ui, |ui| {
                let keys = viewport.texture_keys();
                if keys.is_empty() {
                    ui.label("(no textures loaded)");
                } else {
                    for key in keys {
                        ui.label(key);
                    }
                }
            });

        egui::CollapsingHeader::new("Meshes")
            .default_open(true)
            .show(ui, |ui| {
                let keys = viewport.mesh_keys();
                if keys.is_empty() {
                    ui.label("(no meshes loaded)");
                } else {
                    for key in keys {
                        ui.label(key);
                    }
                }
            });
    });
}
