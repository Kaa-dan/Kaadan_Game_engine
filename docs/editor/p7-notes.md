# Phase 7 — Editor Shell Notes

Phase 7 delivered the dockable editor shell (toolbar + Hierarchy / Inspector /
Viewport / Assets), native file dialogs for Save/Open Scene, and a
`ComponentRegistry`-driven Add/Remove Component flow in the inspector. The
three items below were deliberately left for later phases so that this phase
could land focused, reviewable, and green.

## Deferral 1 — Undoable property + gizmo edits

What ships today:

- Structural edits (Create / Delete / Duplicate) go through `UndoStack` and are
  fully undoable / redoable.
- Inspector property edits and viewport gizmo drags mutate components directly
  through `World::get_mut`. They are **not** captured by the undo stack.

Why deferred:

- An undoable edit needs a *before* snapshot, a *coalesce* policy for
  continuous gestures (drag + slider), and a clean apply/undo path per
  component. Doing this well requires a component-snapshot helper that already
  knows every editor-known type — i.e. the same reflection-ish layer the
  registry hints at but doesn't yet provide.

What to build:

- A `Command::EditComponent { entity, before: EntitySnapshot, after: EntitySnapshot }`
  variant, or a per-field `Command::PatchProperty` with a path string.
- Drag-coalescing: open a command on `drag_started`, mutate live, close
  (push to undo stack) on `drag_stopped`.
- Hook the same path from the inspector (text/drag/slider commits) and
  `gizmo::handle`.

## Deferral 2 — Scene format unification into `kaadan_scene::Scene`

What ships today:

- `crates/kaadan_editor/src/scene_io.rs` defines `EditorScene` /
  `EntityDesc` / `TextureSource` / `MeshSource` as the editor's RON file format.
- `kaadan_scene` exists but only contains the runtime hierarchy components
  (`Parent`, `Children`, `GlobalTransform`) — no serialized scene type.

Why deferred:

- The editor needed *something* to load/save right away to validate the
  toolbar's File menu and Play/Stop's snapshot round-trip. Designing the
  canonical engine-wide scene format is a larger task: it has to cover
  runtime scene loading (gameplay), the asset pipeline's references, and
  prefab/instance overrides.

What to build:

- Move `EditorScene` / `EntityDesc` / `*Desc` / `*Source` into `kaadan_scene`
  as `Scene` + descriptors.
- Have `kaadan_assets` resolve the `TextureSource`/`MeshSource` enums so both
  editor and runtime share one loader path.
- Replace the editor's `to_scene` / `apply_scene` with calls into the shared
  module; delete `scene_io.rs`.

## Deferral 3 — Real play mode with `App` + `ScriptHost`

What ships today:

- `crates/kaadan_editor/src/play.rs` is a placeholder runtime: pressing Play
  snapshots the scene, runs a built-in "spin every Mesh3D" system on each
  frame's `dt`, and Stop restores the snapshot.
- Snapshot/restore is real — the editor's selection, gizmo drag, and Sprite/Mesh
  state correctly revert.

Why deferred:

- A real play mode means standing up a `kaadan_ecs::App` + `kaadan_script::ScriptHost`
  inside the editor process, mounting the same window/renderer, and driving the
  user's gameplay plugin through the full system schedule. That brings in
  hot-reload library-loading concerns, system ordering (editor vs. runtime
  stages), and input routing — all of which deserve their own phase rather
  than being squeezed into the shell milestone.

What to build:

- Replace `play::tick` with a `Runtime` struct that owns an `App` + `ScriptHost`,
  is constructed on Start from the snapshotted scene, and is dropped on Stop.
- Route winit input events into the runtime's `Input` resource when Play is
  active, and back to the editor (gizmo / selection) when it isn't.
- Use the unified `kaadan_scene::Scene` format (deferral 2) for the
  snapshot/restore path so play mode and load/save share one round-trip.
