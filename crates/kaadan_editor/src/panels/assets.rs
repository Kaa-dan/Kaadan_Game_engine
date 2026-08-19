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
