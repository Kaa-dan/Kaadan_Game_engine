//! Builds the editor's panel layout each frame (immediate-mode egui).

use kaadan_ecs::World;

use crate::panels;
use crate::state::EditorState;

pub fn build(
    ctx: &egui::Context,
    state: &mut EditorState,
    world: &World,
    viewport_tex: Option<egui::TextureId>,
) {
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| panels::toolbar::show(ui, state));

    egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(220.0)
        .show(ctx, |ui| {
            panels::hierarchy::show(ui, world, &mut state.selected);
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
                    ui.image(egui::load::SizedTexture::new(id, size));
                }
                None => {
                    ui.centered_and_justified(|ui| ui.label("initializing viewport…"));
                }
            }
        });
}
