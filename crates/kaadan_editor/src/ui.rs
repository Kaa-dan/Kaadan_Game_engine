use egui_dock::{DockArea, DockState, Style};

use crate::commands::Command;
use crate::gizmo::GizmoMode;
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

    handle_shortcuts(ctx, state, viewport);

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
                    &self.state.available_behaviours,
                    &mut self.state.rescan_behaviours,
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

fn handle_shortcuts(ctx: &egui::Context, state: &mut EditorState, viewport: &mut Viewport) {
    if ctx.wants_keyboard_input() {
        return;
    }

    let (set_translate, set_rotate, set_scale, delete, undo, redo, focus) = ctx.input(|i| {
        let cmd = i.modifiers.command;
        let shift = i.modifiers.shift;
        (
            i.key_pressed(egui::Key::W),
            i.key_pressed(egui::Key::E),
            i.key_pressed(egui::Key::R),
            i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            cmd && !shift && i.key_pressed(egui::Key::Z),
            (cmd && shift && i.key_pressed(egui::Key::Z)) || (cmd && i.key_pressed(egui::Key::Y)),
            i.key_pressed(egui::Key::F),
        )
    });

    if set_translate {
        state.gizmo_mode = GizmoMode::Translate;
    }
    if set_rotate {
        state.gizmo_mode = GizmoMode::Rotate;
    }
    if set_scale {
        state.gizmo_mode = GizmoMode::Scale;
    }
    if delete {
        if let Some(sel) = state.selected.filter(|&e| viewport.world.is_alive(e)) {
            let cmd = Command::delete(&viewport.world, sel);
            state
                .commands
                .run(&mut viewport.world, &mut state.selected, cmd);
        }
    }
    if undo {
        state
            .commands
            .undo(&mut viewport.world, &mut state.selected);
    }
    if redo {
        state
            .commands
            .redo(&mut viewport.world, &mut state.selected);
    }
    if focus {
        if let Some(sel) = state.selected.filter(|&e| viewport.world.is_alive(e)) {
            if let Ok(t) = viewport.world.get::<kaadan_math::Transform>(sel) {
                viewport.camera3d.focus_on(t.position);
            }
        }
    }
}

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

            // Capture the selected entity's transform at the start of a drag so
            // the whole gesture commits as a single undoable edit on release.
            if response.drag_started() {
                state.xform_edit = state
                    .selected
                    .filter(|&e| viewport.world.is_alive(e))
                    .and_then(|e| {
                        viewport
                            .world
                            .get::<kaadan_math::Transform>(e)
                            .ok()
                            .map(|t| (e, *t))
                    });
            }

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

            // Orbit/pan/zoom the editor camera with the non-primary buttons, but
            // not while a gizmo handle is grabbed (that uses the primary button).
            if state.gizmo_drag.is_none() {
                crate::camera_controller::handle(ui, &response, &mut viewport.camera3d);
            }

            // Commit the transform edit (if the value actually changed) once the
            // drag ends.
            if response.drag_stopped() {
                if let Some((e, before)) = state.xform_edit.take() {
                    if let Ok(after) = viewport.world.get::<kaadan_math::Transform>(e).map(|t| *t) {
                        if after != before {
                            state.commands.run(
                                &mut viewport.world,
                                &mut state.selected,
                                Command::edit_transform(e, before, after),
                            );
                        }
                    }
                }
            }
        }
        None => {
            ui.centered_and_justified(|ui| ui.label("initializing viewport…"));
        }
    }
}
