//! Editor-wide state shared across panels. Grows as milestones land.

use kaadan_ecs::Entity;

use crate::commands::UndoStack;

#[derive(Default)]
pub struct EditorState {
    /// Set by the File > Quit menu; the app loop checks it after building the UI.
    pub should_exit: bool,
    /// Size (in points) the viewport panel wants its scene texture to fill.
    /// Written each frame while building the central panel.
    pub viewport_size: egui::Vec2,
    /// The currently selected entity, shown in the inspector.
    pub selected: Option<Entity>,
    /// Undo/redo history for structural edits (create/delete/duplicate).
    pub commands: UndoStack,
}
