//! Builds the editor's panel layout each frame (immediate-mode egui).
//!
//! Phase 7 introduces docking via [`egui_dock`]. The top toolbar stays a fixed
//! [`egui::TopBottomPanel`] (it isn't a dockable surface); everything else —
//! Hierarchy, Inspector, Viewport, Assets — is a tab inside a single
//! [`DockArea`] that fills the central panel.

use egui_dock::{DockArea, DockState, Style};

use crate::panels;
use crate::state::{EditorState, Tab};
use crate::viewport::Viewport;

pub fn build(
    ctx: &egui::Context,
    state: &mut EditorState,
    viewport: &mut Viewport,
    viewport_tex: Option<egui::TextureId>,
) {
    // Drain any pending events from the cargo-build worker before rendering
    // panels — keeps the Build tab live without blocking the UI thread. A
    // repaint is requested if the worker is still active so we keep polling.
    let drained = state.build_output.poll();
    if drained > 0 || state.build_output.is_running() {
        ctx.request_repaint();
    }

    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        panels::toolbar::show(ui, state, &mut viewport.world)
    });

    // Move the dock state out of `state` for the duration of the show call so
    // the tab viewer can hold a `&mut EditorState` without aliasing.
    let mut dock = std::mem::replace(&mut state.dock, DockState::new(Vec::new()));

    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
        .show(ctx, |ui| {
            let mut viewer = EditorTabViewer {
                state,
                viewport,
                viewport_tex,
            };
            DockArea::new(&mut dock)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_inside(ui, &mut viewer);
        });

    state.dock = dock;
}

struct EditorTabViewer<'a> {
    state: &'a mut EditorState,
    viewport: &'a mut Viewport,
    viewport_tex: Option<egui::TextureId>,
}

impl<'a> egui_dock::TabViewer for EditorTabViewer<'a> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(("kaadan_editor_tab", *tab))
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            Tab::Hierarchy => {
                panels::hierarchy::show(ui, &mut self.viewport.world, self.state);
            }
            Tab::Inspector => {
                panels::inspector::show(
                    ui,
                    &mut self.viewport.world,
                    self.state.selected,
                    &self.state.registry,
                );
            }
            Tab::Viewport => {
                show_viewport(ui, self.state, self.viewport, self.viewport_tex);
            }
            Tab::Assets => {
                panels::assets::show(ui, self.viewport);
            }
            Tab::Code => {
                panels::code::show(ui, &mut self.state.code_panel, &mut self.state.build_output);
            }
            Tab::Build => {
                panels::build::show(ui, &mut self.state.build_output, &mut self.state.code_panel);
            }
        }
    }
}

/// Render the offscreen scene texture into this tab and overlay the gizmo.
/// Pointer coordinates are tab-local (the response/rect come from `ui` here),
/// so the gizmo math works whether the Viewport tab is the only one or sharing
/// space with other docked panels.
fn show_viewport(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    viewport: &mut Viewport,
    viewport_tex: Option<egui::TextureId>,
) {
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
                &mut viewport.world,
                &mut state.selected,
                state.gizmo_mode,
                &mut state.gizmo_drag,
                &viewport.camera2d,
                &viewport.camera3d,
            );
        }
        None => {
            ui.centered_and_justified(|ui| ui.label("initializing viewport…"));
        }
    }
}
