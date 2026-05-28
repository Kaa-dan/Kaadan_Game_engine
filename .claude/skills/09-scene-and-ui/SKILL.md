# 09 — Scene and UI

## Description
Scene graph (parent-child hierarchies with transform propagation), scene serialization via serde/RON, and a retained-mode UI system (buttons, labels, panels, layout). UI renders through the existing sprite/text pipeline.

## Phase
4 — Content Pipeline

## Prerequisites
- Skill 05 (`05-ecs-world`) — World, Entity, components
- Skill 06 (`06-2d-sprite-rendering`) — rendering sprites, camera
- Skill 08 (`08-asset-pipeline`) — loading fonts, textures for UI

## Complexity
High — scene graph, serialization, and UI layout are each substantial

## Architecture Decisions

### Scene graph via ECS components
- `Parent(Entity)` and `Children(Vec<Entity>)` components establish hierarchy
- A system propagates transforms: child world transform = parent world * child local
- This is the Bevy/Unity pattern — hierarchy lives in ECS, not a separate tree
- Allows mixing scene-graph entities with flat entities seamlessly

### Why RON for scene serialization?
- RON (Rusty Object Notation) is human-readable and Rust-native
- Supports enums, tuples, structs — maps naturally to Rust types
- Better for hand-editing scenes than JSON or YAML
- `serde` derive makes serialization nearly automatic

### Retained-mode UI (not immediate mode)
- **Retained:** Build a widget tree, update it when state changes. Framework handles layout and rendering.
- **Immediate:** Rebuild the entire UI every frame (like `egui`).
- Retained is better for mobile: less CPU work per frame, touch interactions need state tracking (pressed/hover), and layout computation is cached.
- However, the implementation is more complex.

### UI rendering
- UI widgets are entities with special components (`UiNode`, `UiStyle`, `UiText`, etc.)
- They render through the same sprite batch pipeline (2D quads with textures)
- Text rendering uses `fontdue` for CPU-side glyph rasterization → texture atlas
- UI layout is a separate pass before rendering, computing screen-space positions

## Step-by-Step Implementation

### 1. Scene Crate Setup

```toml
# crates/kaadan_scene/Cargo.toml
[package]
name = "kaadan_scene"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math", features = ["serde"] }
kaadan_core = { path = "../kaadan_core" }
kaadan_ecs = { path = "../kaadan_ecs" }
serde = { workspace = true }
ron = { workspace = true }
tracing = { workspace = true }
```

### 2. Parent/Children Components

```rust
// crates/kaadan_scene/src/hierarchy.rs

/// Marks an entity as a child of another entity.
#[derive(Debug, Clone, Copy)]
pub struct Parent(pub hecs::Entity);

/// Stores the children of an entity. Managed automatically.
#[derive(Debug, Clone, Default)]
pub struct Children(pub Vec<hecs::Entity>);

/// World transform computed from hierarchy. Read-only (written by the propagation system).
#[derive(Debug, Clone, Copy)]
pub struct GlobalTransform(pub kaadan_math::Transform);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(kaadan_math::Transform::IDENTITY)
    }
}

/// Set an entity's parent. Updates both Parent and Children components.
pub fn set_parent(world: &mut kaadan_ecs::World, child: hecs::Entity, parent: hecs::Entity) {
    // Remove from old parent's Children list if exists
    if let Ok(old_parent) = world.get::<Parent>(child) {
        let old_parent_entity = old_parent.0;
        drop(old_parent);
        if let Ok(mut children) = world.get_mut::<Children>(old_parent_entity) {
            children.0.retain(|&e| e != child);
        }
    }

    // Set new parent
    // Add/update Parent component on child
    // Add child to parent's Children list
    // Ensure parent has Children component
}

/// Transform propagation system.
/// Runs each frame to compute GlobalTransform from hierarchy.
pub fn transform_propagation_system(world: &mut kaadan_ecs::World, _resources: &mut kaadan_ecs::Resources) {
    // 1. Root entities (no Parent): GlobalTransform = local Transform
    // 2. For each entity with Parent: GlobalTransform = parent.GlobalTransform * self.Transform
    // Must process in hierarchy order (parents before children)
    // Use a breadth-first traversal starting from roots
}
```

### 3. Scene Serialization

```rust
// crates/kaadan_scene/src/scene.rs
use serde::{Serialize, Deserialize};

/// A serializable scene — a collection of entity descriptions.
#[derive(Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub entities: Vec<EntityDesc>,
}

/// Describes one entity and its components for serialization.
#[derive(Serialize, Deserialize)]
pub struct EntityDesc {
    pub name: Option<String>,
    pub transform: kaadan_math::Transform,
    #[serde(default)]
    pub children: Vec<EntityDesc>,
    #[serde(default)]
    pub components: Vec<ComponentDesc>,
}

/// Type-tagged component data. Extensible via the component registry.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ComponentDesc {
    Sprite {
        texture_path: String,
        #[serde(default)]
        color: Option<[f32; 4]>,
        #[serde(default)]
        z_order: i32,
    },
    RigidBody {
        body_type: String, // "dynamic", "static", "kinematic"
    },
    Collider {
        shape: String, // "box", "circle"
        #[serde(default)]
        size: Option<[f32; 2]>,
        #[serde(default)]
        radius: Option<f32>,
    },
    Custom {
        type_name: String,
        data: ron::Value,
    },
}

impl Scene {
    /// Load a scene from RON text.
    pub fn from_ron(text: &str) -> Result<Self, kaadan_core::KaadanError> {
        ron::from_str(text).map_err(|e| kaadan_core::KaadanError::Other(format!("Scene parse: {e}")))
    }

    /// Serialize to RON text.
    pub fn to_ron(&self) -> Result<String, kaadan_core::KaadanError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Scene serialize: {e}")))
    }

    /// Spawn all entities from this scene into the world.
    pub fn spawn_into(&self, world: &mut kaadan_ecs::World) -> Vec<hecs::Entity> {
        let mut spawned = Vec::new();
        for entity_desc in &self.entities {
            let entity = self.spawn_entity(world, entity_desc, None);
            spawned.push(entity);
        }
        spawned
    }

    fn spawn_entity(
        &self,
        world: &mut kaadan_ecs::World,
        desc: &EntityDesc,
        parent: Option<hecs::Entity>,
    ) -> hecs::Entity {
        let entity = world.spawn((
            desc.transform,
            GlobalTransform::default(),
        ));

        if let Some(parent) = parent {
            set_parent(world, entity, parent);
        }

        // Spawn children recursively
        for child_desc in &desc.children {
            self.spawn_entity(world, child_desc, Some(entity));
        }

        entity
    }
}
```

### 4. Example RON Scene

```ron
// assets/scenes/main_menu.ron
Scene(
    name: "Main Menu",
    entities: [
        EntityDesc(
            name: Some("Background"),
            transform: Transform(position: (0.0, 0.0, -10.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0)),
            components: [
                Sprite(texture_path: "textures/menu_bg.png", z_order: -10),
            ],
            children: [],
        ),
        EntityDesc(
            name: Some("Title"),
            transform: Transform(position: (0.0, 200.0, 0.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0)),
            components: [
                Sprite(texture_path: "textures/title.png", z_order: 0),
            ],
            children: [],
        ),
    ],
)
```

### 5. UI Crate Setup

```toml
# crates/kaadan_ui/Cargo.toml
[package]
name = "kaadan_ui"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
kaadan_core = { path = "../kaadan_core" }
kaadan_ecs = { path = "../kaadan_ecs" }
kaadan_renderer = { path = "../kaadan_renderer" }
fontdue = { workspace = true }
tracing = { workspace = true }
```

### 6. UI Node and Style

```rust
// crates/kaadan_ui/src/node.rs
use kaadan_math::{Color, Rect, Vec2};

/// Component marking an entity as a UI node.
pub struct UiNode {
    /// Computed screen-space rect (set by layout system)
    pub computed_rect: Rect,
    /// Is this node interactive?
    pub interactive: bool,
    /// Interaction state
    pub state: InteractionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InteractionState {
    #[default]
    None,
    Hovered,
    Pressed,
}

/// Layout style for a UI node (flexbox-inspired).
pub struct UiStyle {
    pub width: Dimension,
    pub height: Dimension,
    pub min_width: Dimension,
    pub min_height: Dimension,
    pub max_width: Dimension,
    pub max_height: Dimension,
    pub padding: Edges,
    pub margin: Edges,
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub position_type: PositionType,
    pub position: Edges,
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_width: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Dimension {
    Auto,
    Pixels(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum FlexDirection {
    #[default]
    Column,
    Row,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum PositionType {
    #[default]
    Relative,
    Absolute,
}
```

### 7. Widget Components

```rust
// crates/kaadan_ui/src/widgets.rs

/// Text label widget.
pub struct UiText {
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    pub alignment: TextAlignment,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Button widget — combines UiNode (interactive) + visual appearance.
pub struct UiButton {
    pub label: String,
    pub on_click: Option<Box<dyn Fn() + Send + Sync>>,
    pub normal_color: Color,
    pub hover_color: Color,
    pub pressed_color: Color,
}

/// Image widget.
pub struct UiImage {
    pub texture: kaadan_math::Handle<kaadan_renderer::Texture>,
    pub tint: Color,
}

/// Progress bar.
pub struct UiProgressBar {
    pub progress: f32, // 0.0–1.0
    pub bar_color: Color,
    pub background_color: Color,
}

/// Panel/container (just UiNode + UiStyle with background).
// No special component needed — just UiNode + UiStyle with background_color set.
```

### 8. Text Rendering with fontdue

```rust
// crates/kaadan_ui/src/text.rs
use fontdue::Font;

/// Manages font loading and glyph rasterization.
pub struct FontAtlas {
    font: Font,
    /// Rasterized glyphs cached as textures
    glyph_cache: HashMap<(char, u32), GlyphInfo>,
    /// Atlas texture containing all rasterized glyphs
    atlas_texture: Option<kaadan_math::Handle<kaadan_renderer::Texture>>,
}

struct GlyphInfo {
    uv_rect: kaadan_math::Rect,
    metrics: fontdue::Metrics,
}

impl FontAtlas {
    pub fn new(font_bytes: &[u8]) -> Result<Self, kaadan_core::KaadanError> {
        let font = Font::from_bytes(font_bytes, fontdue::FontSettings::default())
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Font load: {e}")))?;
        Ok(Self {
            font,
            glyph_cache: HashMap::new(),
            atlas_texture: None,
        })
    }

    /// Rasterize a glyph at a given size. Returns metrics and bitmap.
    pub fn rasterize(&mut self, ch: char, size: f32) -> (&fontdue::Metrics, &[u8]) {
        // fontdue rasterizes to a grayscale bitmap
        let (metrics, bitmap) = self.font.rasterize(ch, size);
        // Cache and pack into atlas texture
        // ...
        todo!()
    }

    /// Measure text width at a given font size.
    pub fn measure_text(&self, text: &str, size: f32) -> kaadan_math::Vec2 {
        let mut width = 0.0f32;
        let mut max_height = 0.0f32;
        for ch in text.chars() {
            let (metrics, _) = self.font.rasterize(ch, size);
            width += metrics.advance_width;
            max_height = max_height.max(metrics.height as f32);
        }
        kaadan_math::Vec2::new(width, max_height)
    }
}
```

### 9. Layout System

```rust
// crates/kaadan_ui/src/layout.rs

/// Computes screen-space rects for all UiNode entities.
/// Flexbox-inspired: processes hierarchy top-down, respects UiStyle.
pub fn ui_layout_system(world: &mut kaadan_ecs::World, resources: &mut kaadan_ecs::Resources) {
    // 1. Find root UI nodes (no Parent or parent is not a UiNode)
    // 2. For each root, set available rect from viewport (minus safe areas)
    // 3. Recursively layout children:
    //    a. Measure children (intrinsic size from text, image, or style)
    //    b. Distribute along flex direction (row or column)
    //    c. Apply justify_content for main axis
    //    d. Apply align_items for cross axis
    //    e. Apply padding and margin
    //    f. Write computed_rect to each UiNode
}

/// UI interaction system — checks touch/pointer against computed_rects.
pub fn ui_interaction_system(world: &mut kaadan_ecs::World, resources: &mut kaadan_ecs::Resources) {
    let input = resources.get::<kaadan_input::InputState>().unwrap();

    for (_entity, node) in world.query::<&mut UiNode>().iter() {
        if !node.interactive { continue; }

        // Check if pointer/touch is inside computed_rect
        // Update InteractionState: None → Hovered → Pressed
        // Fire on_click for buttons on press→release inside rect
    }
}
```

## Deliverables Checklist

- [ ] `Parent`/`Children` components with hierarchy management
- [ ] `GlobalTransform` computed by transform propagation system
- [ ] Scene serialization to/from RON format
- [ ] `Scene::spawn_into()` spawning entity hierarchies
- [ ] `UiNode`, `UiStyle` with flexbox-inspired layout properties
- [ ] Widget components: `UiText`, `UiButton`, `UiImage`, `UiProgressBar`
- [ ] `FontAtlas` using `fontdue` for glyph rasterization
- [ ] Layout system computing screen-space rects from style
- [ ] Interaction system detecting touch/click on interactive nodes
- [ ] Safe area insets support in root layout
- [ ] Demo: main menu with "Play"/"Settings" buttons transitioning to game scene

## Common Pitfalls

1. **Transform propagation order** — Parents must be processed before children. Without ordering, children may use stale parent transforms. Use a topological sort or BFS from roots.

2. **RON requires `#[derive(Serialize, Deserialize)]` on all types** — Including `Transform`, `Vec3`, `Quat`. Enable the `serde` feature on `glam` and `kaadan_math`.

3. **Entity references in serialized scenes** — Entities are runtime IDs. Serialized scenes use indices or names, not `hecs::Entity` values. Map names to entities during spawn.

4. **UI coordinate space** — UI typically uses screen-space (origin top-left, Y-down). World space uses center-origin Y-up. Keep these separate; the layout system operates in screen space.

5. **fontdue rasterizes to grayscale** — You get a `Vec<u8>` bitmap per glyph. Pack these into an RGBA texture atlas (store in alpha channel) for GPU rendering.

6. **Safe areas vary per device** — iPhone notch, Android navigation bar, etc. Always query safe area insets from the platform and pass them to UI layout.

## References

- [serde docs](https://serde.rs/)
- [RON format](https://github.com/ron-rs/ron)
- [fontdue docs](https://docs.rs/fontdue/latest/fontdue/)
- [Flexbox layout algorithm](https://www.w3.org/TR/css-flexbox-1/#layout-algorithm)
- [Bevy UI system](https://bevyengine.org/learn/quick-start/getting-started/ui/) (inspiration)
