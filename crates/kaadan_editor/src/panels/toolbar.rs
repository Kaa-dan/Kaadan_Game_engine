//! Top toolbar / menu bar.

use kaadan_ecs::World;

use crate::gizmo::GizmoMode;
use crate::play::PlayRequest;
use crate::scene_io::IoRequest;
use crate::state::EditorState;

enum Action {
    Quit,
    Undo,
    Redo,
}

pub fn show(ui: &mut egui::Ui, state: &mut EditorState, world: &mut World) {
    let mut action = None;
    egui::menu::bar(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Save Scene").clicked() {
                state.io_request = Some(IoRequest::Save);
                ui.close_menu();
            }
            if ui.button("Open Scene").clicked() {
                state.io_request = Some(IoRequest::Load);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                action = Some(Action::Quit);
                ui.close_menu();
            }
        });
        ui.menu_button("Edit", |ui| {
            if ui
                .add_enabled(state.commands.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                action = Some(Action::Undo);
                ui.close_menu();
            }
            if ui
                .add_enabled(state.commands.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                action = Some(Action::Redo);
                ui.close_menu();
            }
        });
        ui.separator();
        ui.selectable_value(&mut state.gizmo_mode, GizmoMode::Translate, "Move");
        ui.selectable_value(&mut state.gizmo_mode, GizmoMode::Rotate, "Rotate");
        ui.selectable_value(&mut state.gizmo_mode, GizmoMode::Scale, "Scale");
        ui.separator();
        if ui
            .add_enabled(!state.playing, egui::Button::new("▶ Play"))
            .clicked()
        {
            state.play_request = Some(PlayRequest::Start);
        }
        if ui
            .add_enabled(state.playing, egui::Button::new("⏹ Stop"))
            .clicked()
        {
            state.play_request = Some(PlayRequest::Stop);
        }
    });

    match action {
        Some(Action::Quit) => state.should_exit = true,
        Some(Action::Undo) => state.commands.undo(world, &mut state.selected),
        Some(Action::Redo) => state.commands.redo(world, &mut state.selected),
        None => {}
    }
}
