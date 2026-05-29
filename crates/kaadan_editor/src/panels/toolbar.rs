//! Top toolbar / menu bar.

use crate::state::EditorState;

pub fn show(ui: &mut egui::Ui, state: &mut EditorState) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Quit").clicked() {
                state.should_exit = true;
                ui.close_menu();
            }
        });
        ui.separator();
        ui.add_enabled(false, egui::Button::new("▶ Play"));
        ui.add_enabled(false, egui::Button::new("⏹ Stop"));
    });
}
