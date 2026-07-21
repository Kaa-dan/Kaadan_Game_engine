//! Editor-wide state shared across panels. Grows as milestones land.

use egui_dock::{DockState, NodeIndex};
use kaadan_ecs::Entity;
use kaadan_renderer::{DirectionalLight, Mesh3D, PbrMaterial, PointLight, Sprite};
use kaadan_scene::{Children, GlobalTransform, Parent};
use kaadan_script::ComponentRegistry;

use crate::cargo_build::BuildOutput;
use crate::commands::UndoStack;
use crate::components::Name;
use crate::gizmo::{DragTarget, GizmoMode};
use crate::panels::code::CodePanelState;
use crate::play::{PlayRequest, PlaySession};
use crate::scene_io::{EditorScene, IoRequest};

/// Identifier for a dockable editor panel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tab {
    Hierarchy,
    Inspector,
    Viewport,
    Assets,
    Code,
    Build,
}

impl Tab {
    pub fn title(self) -> &'static str {
        match self {
            Tab::Hierarchy => "Hierarchy",
            Tab::Inspector => "Inspector",
            Tab::Viewport => "Viewport",
            Tab::Assets => "Assets",
            Tab::Code => "Code",
            Tab::Build => "Build",
        }
    }
}

pub struct EditorState {
    /// Set by the File > Quit menu; the app loop checks it after building the UI.
    pub should_exit: bool,
    /// Size (in points) the viewport panel wants its scene texture to fill.
    /// Written each frame while building the Viewport tab.
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
    /// The live gameplay runtime while Play mode is active (owns the runtime
    /// `App` + hot-reload `ScriptHost`). `None` when stopped.
    pub play_session: Option<PlaySession>,
    /// Dock layout for the editor's primary panels.
    pub dock: DockState<Tab>,
    /// Registry of editor-visible component types, used by the inspector for
    /// the Add/Remove Component UI.
    pub registry: ComponentRegistry,
    /// State for the Code panel (open file, buffer, file-tree state).
    pub code_panel: CodePanelState,
    /// Output (status + diagnostics) from the most recent / running cargo build.
    pub build_output: BuildOutput,
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            should_exit: false,
            viewport_size: egui::Vec2::ZERO,
            selected: None,
            commands: UndoStack::default(),
            io_request: None,
            gizmo_mode: GizmoMode::default(),
            gizmo_drag: None,
            playing: false,
            play_request: None,
            play_snapshot: None,
            play_session: None,
            dock: default_dock_layout(),
            registry: default_registry(),
            code_panel: CodePanelState::default(),
            build_output: BuildOutput::default(),
        }
    }
}

/// Build the initial dock layout: Hierarchy on the left, Viewport (with a Code
/// sibling tab) in the center, Inspector on the right, Assets and Build docked
/// below the center.
pub fn default_dock_layout() -> DockState<Tab> {
    let mut dock = DockState::new(vec![Tab::Viewport, Tab::Code]);
    let main = dock.main_surface_mut();
    let [center, _left] = main.split_left(NodeIndex::root(), 0.20, vec![Tab::Hierarchy]);
    let [center, _right] = main.split_right(center, 0.75, vec![Tab::Inspector]);
    main.split_below(center, 0.72, vec![Tab::Assets, Tab::Build]);
    dock
}

/// Build the [`ComponentRegistry`] used by the inspector for Add/Remove.
pub fn default_registry() -> ComponentRegistry {
    let mut r = ComponentRegistry::new();
    // Editor-local.
    r.register::<Name>("Name");
    // Math / transforms.
    r.register_default::<kaadan_math::Transform>("Transform");
    // Renderer components — only some have Default.
    r.register::<Sprite>("Sprite");
    r.register::<Mesh3D>("Mesh3D");
    r.register_default::<PbrMaterial>("PbrMaterial");
    r.register_default::<DirectionalLight>("DirectionalLight");
    r.register_default::<PointLight>("PointLight");
    // Hierarchy. Parent/Children are structural and have no Default; the user
    // shouldn't add them through the inspector (use scene tools instead) — they
    // appear here purely so the inspector's "present components" logic knows
    // about them.
    r.register::<Parent>("Parent");
    r.register::<Children>("Children");
    r.register_default::<GlobalTransform>("GlobalTransform");
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_lists_editor_components() {
        let r = default_registry();
        let names: Vec<&'static str> = r.names().collect();
        for required in [
            "Name",
            "Transform",
            "Sprite",
            "Mesh3D",
            "PbrMaterial",
            "DirectionalLight",
            "PointLight",
            "Parent",
            "Children",
            "GlobalTransform",
        ] {
            assert!(
                names.contains(&required),
                "registry missing {required}: {names:?}"
            );
        }
    }

    #[test]
    fn default_dock_layout_contains_all_tabs() {
        let dock = default_dock_layout();
        let tabs: std::collections::HashSet<Tab> =
            dock.iter_all_tabs().map(|(_, tab)| *tab).collect();
        assert!(tabs.contains(&Tab::Hierarchy));
        assert!(tabs.contains(&Tab::Inspector));
        assert!(tabs.contains(&Tab::Viewport));
        assert!(tabs.contains(&Tab::Assets));
        assert!(tabs.contains(&Tab::Code));
        assert!(tabs.contains(&Tab::Build));
    }
}
