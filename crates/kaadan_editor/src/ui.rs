//! Builds the editor's panel layout each frame (immediate-mode egui).

use kaadan_ecs::World;
use kaadan_renderer::{Camera2D, Camera3D};

use crate::panels;
use crate::state::EditorState;

pub fn build(
    ctx: &egui::Context,
    state: &mut EditorState,
    world: &mut World,
    cam2d: &Camera2D,
    cam3d: &Camera3D,
    viewport_tex: Option<egui::TextureId>,
) {
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| panels::toolbar::show(ui, state, world));

    egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(220.0)
        .show(ctx, |ui| {
            panels::hierarchy::show(ui, world, state);
        });

    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(300.0)
        .show(ctx, |ui| {
            panels::inspector::show(ui, world, state.selected);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
        .show(ctx, |ui| {
            let size = ui.available_size();
            state.viewport_size = size;
            match viewport_tex {
                Some(id) => {
                    let response = ui.add(
                        egui::Image::new(egui::load::SizedTexture::new(id, size))
                            .sense(egui::Sense::click_and_drag()),
                    );
                    let rect = response.rect;
                    crate::gizmo::handle(
                        ui,
                        &response,
                        rect,
                        world,
                        &mut state.selected,
                        state.gizmo_mode,
                        &mut state.gizmo_drag,
                        cam2d,
                        cam3d,
                    );
                }
                None => {
                    ui.centered_and_justified(|ui| ui.label("initializing viewport…"));
                }
            }
        });
}
