# KaadanEngine Editor — User Guide

How to run the editor and build a game: create an environment, place/import
objects and characters, script them in Rust, and drive them with the keyboard in
Play mode.

---

## 1. Running the editor

From the workspace root:

```bash
cargo run -p kaadan_editor
```

- First build compiles the whole graph (wgpu, egui, rapier, …) — expect a few
  minutes cold; later runs are fast.
- The window is titled **"KaadanEngine Editor"** and opens with an **empty scene**.
- Set `RUST_LOG=debug` for verbose logs.
- Want the old demo scene as a reference? `KAADAN_EDITOR_DEMO=1 cargo run -p kaadan_editor`.

---

## 2. The layout

Six dockable panels (drag tab headers to rearrange):

| Panel | Location | Purpose |
|-------|----------|---------|
| **Hierarchy** | left | The scene's entities. Create/select/delete/**reparent** here. |
| **Viewport** | center | The rendered scene with camera navigation + transform gizmos. |
| **Code** | center (tab) | Edit + build the gameplay script crate. |
| **Inspector** | right | Edit the selected entity: name, components, **scripts**. |
| **Assets** | bottom | Loaded textures/meshes. |
| **Build** | bottom (tab) | `cargo build` output for the gameplay crate. |

**Toolbar:** `File`, `GameObject`, and `Edit` menus, gizmo mode toggles
(**Move / Rotate / Scale**), and **▶ Play** / **⏹ Stop**.

---

## 3. Navigating the viewport

The editor camera is now fully controllable:

| Action | Control |
|--------|---------|
| **Orbit** | Right-mouse drag |
| **Pan** | Middle-mouse drag |
| **Zoom** | Scroll wheel |
| **Focus selected** | Press **F** (recenters the camera on the selection) |

---

## 4. Placing objects (GameObject menu)

Use the **GameObject** menu to spawn things at the origin:

- **Empty** — a bare entity (just a Transform); good as a parent/group node.
- **Cube, Sphere, Plane, Cylinder, Capsule** — lit primitives with a default material.
- **Directional Light** — a scene "sun".
- **Point Light** — a local light.

Every spawn is **undoable** (Ctrl+Z). Select an object by clicking it in the
Viewport or the Hierarchy, then move it with the gizmos.

---

## 5. Importing characters & models (glTF)

**File ▸ Import Model…** and pick a `.gltf` or `.glb` file.

- The model is imported as a parent **empty** with one child mesh entity per
  submesh, each with its material and **base-color texture**.
- It's a normal part of the scene: select, move, parent, and script the pieces.
- Imported models **round-trip** through Save/Open — the scene stores a reference
  to the file + submesh, and re-imports geometry and embedded textures on load.

Not yet supported: normal/metallic-roughness textures, skeletal animation, OBJ.

---

## 6. Editing entities (Inspector)

With an entity selected:

- **Name** — the text field at the top.
- **Add component** — dropdown at the bottom (Transform, PbrMaterial,
  DirectionalLight, PointLight). Remove with the **×** on each section.
- **Transform** — drag Position/Scale x/y/z; Rotation is Euler degrees.
- **PbrMaterial** — base color, metallic, roughness, emissive.
- **Scripts** — see §8.

### Transform gizmos (Viewport)
Pick a mode in the toolbar (or press **W** move / **E** rotate / **R** scale) and
drag the on-screen handles. 3D meshes get X/Y/Z; 2D sprites get X/Y. **Gizmo drags
are undoable** (Ctrl+Z).

---

## 7. Building the hierarchy (parenting)

In the **Hierarchy**, **drag one entity onto another** to make it a child — it
then moves/rotates/scales with its parent. **Drag onto empty space** to detach it
back to the root. Dropping an entity onto its own descendant is rejected (no
cycles). Reparenting is undoable.

---

## 8. Scripting (Rust behaviours)

Scripts are Rust **behaviours** (`start`/`update`), compiled and hot-reloaded. The
loop is: **write → Build → attach → Play**.

### Write a behaviour
Open the **Code** panel and edit the gameplay crate (`templates/game_template`).
A behaviour is a struct implementing `Behaviour`; register it by name in `build`:

```rust
struct Player { speed: f32 }
impl Behaviour for Player {
    fn update(&mut self, ctx: &mut BehaviourContext) {
        let mut dir = Vec3::ZERO;
        if let Some(input) = ctx.resources.get::<InputState>() {
            if input.key_pressed(KeyCode::W) { dir.z -= 1.0; }
            // … A/S/D …
        }
        if dir != Vec3::ZERO {
            if let Ok(mut t) = ctx.world.get_mut::<Transform>(ctx.entity) {
                t.position += dir.normalize() * self.speed * ctx.dt;
            }
        }
    }
}

pub fn build(ctx: &mut ScriptContext) {
    ctx.register_behaviour_default::<Player>("Player");
}
```

The template already ships **`Player`** (WASD/arrow-key controller) and **`Spinner`**.

### Build
In the **Code** panel click **Build** (runs `cargo build -p game_template`);
diagnostics appear in the **Build** panel and are click-to-jump.

### Attach to an entity
Select an entity, and in the Inspector's **Scripts** section press **↻** to rescan
the built crate, then pick a behaviour from **"Add behaviour…"**. The assignment is
stored on the entity and **saved with the scene**.

### Play
Press **▶ Play**. Attached behaviours run, and **keyboard input is live** — press
WASD to drive a `Player`. Edit the script and **Build** again to **hot-reload**
without leaving Play. **⏹ Stop** restores the pre-play scene.

> Attachment is **data-driven**: the editor attaches behaviours by name on Play, so
> `build` only *registers* them — it doesn't attach them itself.

---

## 9. Saving & loading scenes

**File ▸ Save Scene** / **Open Scene** — plain-text **RON** (`.ron`), safe to
hand-edit. Persists: name, transform, sprite, mesh (primitives *and* glTF
references), material, lights, **attached script names**, and the parent/child
hierarchy. See `crates/kaadan_editor/src/scene_io.rs` for the schema.

---

## 10. Quick reference

| I want to… | Do this |
|------------|---------|
| Run the editor | `cargo run -p kaadan_editor` |
| Orbit / pan / zoom | Right-drag / middle-drag / scroll |
| Focus selection | **F** |
| Add a primitive/light | **GameObject** menu |
| Import a character/model | **File ▸ Import Model…** (`.gltf`/`.glb`) |
| New / delete / duplicate entity | Hierarchy **➕ / 🗑 / ⧉** (Del also deletes) |
| Move / rotate / scale | **W / E / R** + drag gizmo |
| Parent / unparent | Drag entity onto another / onto empty space |
| Undo / redo | **Ctrl+Z / Ctrl+Shift+Z** (or Edit menu) |
| Write a script | Code panel → edit `game_template` → **Build** |
| Attach a script | Inspector ▸ Scripts ▸ **↻** then **Add behaviour…** |
| Play & control | **▶ Play**, then WASD |
| Save / load | **File ▸ Save / Open Scene** (`.ron`) |

---

## 11. Known limitations

- The Play camera is the editor camera (no in-game ECS cameras yet).
- Scripting is Rust-only (no visual/Lua scripting).
- glTF import covers geometry + base-color textures; no normal/MR maps, no animation.
- One behaviour list per entity; behaviours attach on Play (not previewed in edit mode).
- Create/duplicate/delete snapshots don't yet carry script assignments through undo
  (they persist via Save/Open); gizmo edits are undoable, most inspector-field edits
  are not individually undoable.
