use kaadan_ecs::World;

use crate::gizmo::GizmoMode;
use crate::play::PlayRequest;
use crate::scene_io::IoRequest;
use crate::spawn::SpawnKind;
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
                ui.close_menu();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Kaadan scene (RON)", &["ron"])
                    .set_file_name("scene.ron")
                    .save_file()
                {
                    state.io_request = Some(IoRequest::Save(path));
                }
            }
            if ui.button("Open Scene").clicked() {
                ui.close_menu();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Kaadan scene (RON)", &["ron"])
                    .pick_file()
                {
                    state.io_request = Some(IoRequest::Load(path));
                }
            }
            ui.separator();
            if ui.button("Import Model…").clicked() {
                ui.close_menu();
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("glTF model", &["gltf", "glb"])
                    .pick_file()
                {
                    state.io_request = Some(IoRequest::ImportModel(path));
                }
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                action = Some(Action::Quit);
                ui.close_menu();
            }
        });
        ui.menu_button("GameObject", |ui| {
            let mut spawn = |ui: &mut egui::Ui, label: &str, kind: SpawnKind| {
                if ui.button(label).clicked() {
                    state.spawn_request = Some(kind);
                    ui.close_menu();
                }
            };
            spawn(ui, "Empty", SpawnKind::Empty);
            ui.separator();
            spawn(ui, "Cube", SpawnKind::Cube);
            spawn(ui, "Sphere", SpawnKind::Sphere);
            spawn(ui, "Plane", SpawnKind::Plane);
            spawn(ui, "Cylinder", SpawnKind::Cylinder);
            spawn(ui, "Capsule", SpawnKind::Capsule);
            ui.separator();
            spawn(ui, "Directional Light", SpawnKind::DirectionalLight);
            spawn(ui, "Point Light", SpawnKind::PointLight);
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
