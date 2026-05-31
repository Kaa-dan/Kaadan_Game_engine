//! Editor-wide state shared across panels. Grows as milestones land.

use kaadan_ecs::Entity;

use crate::commands::UndoStack;
use crate::gizmo::{DragTarget, GizmoMode};
use crate::play::PlayRequest;
use crate::scene_io::{EditorScene, IoRequest};

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
    /// A pending save/load, performed by the app loop (which owns the GPU device).
    pub io_request: Option<IoRequest>,
    /// Active viewport gizmo mode (move/rotate/scale).
    pub gizmo_mode: GizmoMode,
    /// Which gizmo handle is being dragged this gesture, if any.
    pub gizmo_drag: Option<DragTarget>,
    /// True while Play mode is running game systems.
    pub playing: bool,
    /// A pending Play/Stop transition, performed by the app loop.
    pub play_request: Option<PlayRequest>,
    /// Scene captured when Play started, restored on Stop.
    pub play_snapshot: Option<EditorScene>,
}
