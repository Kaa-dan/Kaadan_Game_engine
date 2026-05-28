# 02 — Math and Core Types

## Description
Implement `kaadan_math` (wrapping glam with engine types: Transform, Color, Rect, AABB, Handle<T>) and `kaadan_core` (tracing, error types). These are the foundational vocabulary types used by every other crate.

## Phase
1 — Scaffolding & Core Types

## Prerequisites
- Skill 01 (`01-project-foundation`) — workspace must compile cleanly

## Architecture Decisions

### Why wrap glam instead of using it directly?
- Downstream crates import `kaadan_math`, never `glam` directly
- Allows adding engine-specific methods (e.g., `Transform::from_position()`)
- If we ever swap math libraries, only one crate changes
- `glam` is SIMD-optimized and the de facto standard for Rust game engines

### Why generational indices for Handle<T>?
- Prevents dangling references: if a texture is freed and its slot reused, old handles detect the generation mismatch
- Zero-cost abstraction: Handle is just `(u32 index, u32 generation)`
- Type-safe: `Handle<Texture>` and `Handle<Mesh>` are distinct types at compile time

### Color representation
- Store colors in **linear** RGB internally (GPU expects linear)
- Provide `Color::from_srgb()` for authoring convenience (humans think in sRGB)
- Alpha is always premultiplied for correct blending

## Step-by-Step Implementation

### 1. kaadan_math — Re-export glam types

```toml
# crates/kaadan_math/Cargo.toml
[package]
name = "kaadan_math"
version.workspace = true
edition.workspace = true

[dependencies]
glam = { workspace = true }
bytemuck = { workspace = true }
serde = { workspace = true, optional = true }

[features]
default = []
serde = ["dep:serde", "glam/serde"]
```

```rust
// crates/kaadan_math/src/lib.rs
//! KaadanEngine math primitives.

mod color;
mod handle;
mod rect;
mod transform;

pub use color::Color;
pub use handle::{Handle, HandleAllocator};
pub use rect::{AABB, Rect};
pub use transform::Transform;

// Re-export glam types so downstream never imports glam directly
pub use glam::{
    Mat2, Mat3, Mat4,
    Quat,
    Vec2, Vec3, Vec3A, Vec4,
    IVec2, IVec3, IVec4,
    UVec2, UVec3, UVec4,
    Affine2, Affine3A,
    EulerRot,
};
```

### 2. Transform

```rust
// crates/kaadan_math/src/transform.rs
use glam::{Affine3A, Mat4, Quat, Vec3};

/// A 3D transform with position, rotation, and uniform scale.
/// Stored as separate components for easy manipulation;
/// composed into a matrix when needed for rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub fn from_position(position: Vec3) -> Self {
        Self { position, ..Self::IDENTITY }
    }

    pub fn from_position_2d(x: f32, y: f32) -> Self {
        Self::from_position(Vec3::new(x, y, 0.0))
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self { rotation, ..Self::IDENTITY }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self { scale, ..Self::IDENTITY }
    }

    pub fn from_scale_uniform(scale: f32) -> Self {
        Self::from_scale(Vec3::splat(scale))
    }

    /// Compose into a 4x4 matrix: Scale → Rotate → Translate
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Compose into an affine transform (more efficient than full Mat4)
    pub fn to_affine(&self) -> Affine3A {
        Affine3A::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Apply parent transform to this child transform
    pub fn mul_transform(&self, child: &Transform) -> Transform {
        let position = self.rotation * (self.scale * child.position) + self.position;
        let rotation = self.rotation * child.rotation;
        let scale = self.scale * child.scale;
        Transform { position, rotation, scale }
    }

    /// Local forward direction (negative Z in right-handed coords)
    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    /// Local right direction
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    /// Local up direction
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    /// Linearly interpolate between two transforms
    pub fn lerp(&self, other: &Transform, t: f32) -> Transform {
        Transform {
            position: self.position.lerp(other.position, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }
}
```

### 3. Color

```rust
// crates/kaadan_math/src/color.rs
use bytemuck::{Pod, Zeroable};

/// Linear RGBA color. GPU-ready. Alpha is NOT premultiplied at storage;
/// premultiply in the shader or batch step.
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const RED: Self = Self { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const GREEN: Self = Self { r: 0.0, g: 1.0, b: 0.0, a: 1.0 };
    pub const BLUE: Self = Self { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    /// Create from linear RGBA values (0.0–1.0)
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from sRGB values (0.0–1.0). Converts to linear for GPU use.
    pub fn from_srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: srgb_to_linear(r),
            g: srgb_to_linear(g),
            b: srgb_to_linear(b),
            a, // alpha is always linear
        }
    }

    /// Create from a hex color code: 0xRRGGBB or 0xRRGGBBAA
    pub fn from_hex(hex: u32) -> Self {
        if hex > 0xFF_FF_FF {
            // 0xRRGGBBAA
            let r = ((hex >> 24) & 0xFF) as f32 / 255.0;
            let g = ((hex >> 16) & 0xFF) as f32 / 255.0;
            let b = ((hex >> 8) & 0xFF) as f32 / 255.0;
            let a = (hex & 0xFF) as f32 / 255.0;
            Self::from_srgb(r, g, b, a)
        } else {
            // 0xRRGGBB
            let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
            let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
            let b = (hex & 0xFF) as f32 / 255.0;
            Self::from_srgb(r, g, b, 1.0)
        }
    }

    /// Convert to [f32; 4] for passing to wgpu
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

/// sRGB → Linear conversion (gamma decoding)
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear → sRGB conversion (gamma encoding)
#[allow(dead_code)]
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
```

### 4. Rect and AABB

```rust
// crates/kaadan_math/src/rect.rs
use glam::Vec2;

/// A 2D rectangle defined by minimum and maximum corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half = size * 0.5;
        Self { min: center - half, max: center + half }
    }

    pub fn from_position_size(position: Vec2, size: Vec2) -> Self {
        Self { min: position, max: position + size }
    }

    pub fn width(&self) -> f32 { self.max.x - self.min.x }
    pub fn height(&self) -> f32 { self.max.y - self.min.y }
    pub fn size(&self) -> Vec2 { self.max - self.min }
    pub fn center(&self) -> Vec2 { (self.min + self.max) * 0.5 }

    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x && point.x <= self.max.x
            && point.y >= self.min.y && point.y <= self.max.y
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
    }

    pub fn merge(&self, other: &Rect) -> Rect {
        Rect {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}

/// 3D axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}

impl AABB {
    pub fn new(min: glam::Vec3, max: glam::Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_center_half_extents(center: glam::Vec3, half_extents: glam::Vec3) -> Self {
        Self { min: center - half_extents, max: center + half_extents }
    }

    pub fn size(&self) -> glam::Vec3 { self.max - self.min }
    pub fn center(&self) -> glam::Vec3 { (self.min + self.max) * 0.5 }

    pub fn contains_point(&self, point: glam::Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x
            && point.y >= self.min.y && point.y <= self.max.y
            && point.z >= self.min.z && point.z <= self.max.z
    }

    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn merge(&self, other: &AABB) -> AABB {
        AABB {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }
}
```

### 5. Handle<T> with Generational Indices

```rust
// crates/kaadan_math/src/handle.rs
use std::marker::PhantomData;

/// A type-safe handle to a resource with generational index.
/// Prevents use-after-free: if the slot is reused, the generation won't match.
#[derive(Debug)]
pub struct Handle<T> {
    index: u32,
    generation: u32,
    _marker: PhantomData<T>,
}

// Manual impls to avoid requiring T: Clone/Copy/etc.
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self { *self }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

impl<T> Handle<T> {
    pub fn index(&self) -> u32 { self.index }
    pub fn generation(&self) -> u32 { self.generation }
}

/// Entry in the allocator's slot array.
struct Slot {
    generation: u32,
    is_live: bool,
}

/// Allocates and validates Handle<T> instances.
pub struct HandleAllocator<T> {
    slots: Vec<Slot>,
    free_list: Vec<u32>,
    _marker: PhantomData<T>,
}

impl<T> Default for HandleAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HandleAllocator<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_list: Vec::new(),
            _marker: PhantomData,
        }
    }

    /// Allocate a new handle.
    pub fn allocate(&mut self) -> Handle<T> {
        if let Some(index) = self.free_list.pop() {
            let slot = &mut self.slots[index as usize];
            slot.generation += 1;
            slot.is_live = true;
            Handle { index, generation: slot.generation, _marker: PhantomData }
        } else {
            let index = self.slots.len() as u32;
            self.slots.push(Slot { generation: 0, is_live: true });
            Handle { index, generation: 0, _marker: PhantomData }
        }
    }

    /// Free a handle's slot for reuse. Returns true if the handle was valid.
    pub fn free(&mut self, handle: Handle<T>) -> bool {
        if self.is_valid(handle) {
            self.slots[handle.index as usize].is_live = false;
            self.free_list.push(handle.index);
            true
        } else {
            false
        }
    }

    /// Check if a handle still refers to a live resource.
    pub fn is_valid(&self, handle: Handle<T>) -> bool {
        self.slots
            .get(handle.index as usize)
            .map_or(false, |slot| slot.is_live && slot.generation == handle.generation)
    }

    /// Number of currently live handles.
    pub fn live_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_live).count()
    }
}
```

### 6. kaadan_core — Tracing and Error Types

```toml
# crates/kaadan_core/Cargo.toml
[package]
name = "kaadan_core"
version.workspace = true
edition.workspace = true

[dependencies]
kaadan_math = { path = "../kaadan_math" }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
thiserror = { workspace = true }
```

```rust
// crates/kaadan_core/src/lib.rs
//! KaadanEngine core utilities — logging, error types, engine-wide traits.

mod error;
mod logging;

pub use error::KaadanError;
pub use logging::init_logging;
pub use tracing;
```

```rust
// crates/kaadan_core/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KaadanError {
    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Renderer error: {0}")]
    Renderer(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(String),

    #[error("Asset load error: {path}: {reason}")]
    AssetLoad { path: String, reason: String },

    #[error("Invalid handle: index={index}, generation={generation}")]
    InvalidHandle { index: u32, generation: u32 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type KaadanResult<T> = Result<T, KaadanError>;
```

```rust
// crates/kaadan_core/src/logging.rs
use tracing_subscriber::{fmt, EnvFilter};

/// Initialize tracing with env filter. Call once at engine startup.
/// Set `RUST_LOG=kaadan=debug` for engine debug output.
pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("kaadan=info,wgpu=warn"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .init();
}
```

## Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_identity() {
        let t = Transform::IDENTITY;
        assert_eq!(t.to_matrix(), Mat4::IDENTITY);
    }

    #[test]
    fn transform_composition() {
        let parent = Transform::from_position(Vec3::new(10.0, 0.0, 0.0));
        let child = Transform::from_position(Vec3::new(0.0, 5.0, 0.0));
        let result = parent.mul_transform(&child);
        assert_eq!(result.position, Vec3::new(10.0, 5.0, 0.0));
    }

    #[test]
    fn color_srgb_roundtrip() {
        let c = Color::from_srgb(0.5, 0.5, 0.5, 1.0);
        assert!(c.r < 0.5); // linear value is less than sRGB for mid-tones
        assert!(c.r > 0.2);
    }

    #[test]
    fn rect_intersection() {
        let a = Rect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let b = Rect::new(Vec2::new(5.0, 5.0), Vec2::new(15.0, 15.0));
        assert!(a.intersects(&b));
    }

    #[test]
    fn rect_no_intersection() {
        let a = Rect::new(Vec2::ZERO, Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(15.0, 15.0));
        assert!(!a.intersects(&b));
    }

    #[test]
    fn aabb_intersection() {
        let a = AABB::new(Vec3::ZERO, Vec3::splat(10.0));
        let b = AABB::new(Vec3::splat(5.0), Vec3::splat(15.0));
        assert!(a.intersects(&b));
    }

    #[test]
    fn handle_generational_safety() {
        let mut alloc = HandleAllocator::<u32>::new();
        let h1 = alloc.allocate();
        assert!(alloc.is_valid(h1));

        alloc.free(h1);
        assert!(!alloc.is_valid(h1));

        let h2 = alloc.allocate();
        assert!(alloc.is_valid(h2));
        assert!(!alloc.is_valid(h1)); // Old handle still invalid — generation bumped
        assert_eq!(h2.index(), h1.index()); // Same slot reused
        assert_ne!(h2.generation(), h1.generation()); // Different generation
    }

    #[test]
    fn handle_allocator_live_count() {
        let mut alloc = HandleAllocator::<u32>::new();
        let h1 = alloc.allocate();
        let h2 = alloc.allocate();
        assert_eq!(alloc.live_count(), 2);
        alloc.free(h1);
        assert_eq!(alloc.live_count(), 1);
        alloc.free(h2);
        assert_eq!(alloc.live_count(), 0);
    }
}
```

## Deliverables Checklist

- [ ] `kaadan_math` crate with `Transform`, `Color`, `Rect`, `AABB`
- [ ] Type-safe `Handle<T>` with generational indices
- [ ] Re-exports of `glam` types — no downstream crate imports `glam` directly
- [ ] `kaadan_core` with `init_logging()` and `KaadanError` enum
- [ ] Unit tests for Transform composition, AABB intersection, Handle safety
- [ ] `cargo test -p kaadan_math -p kaadan_core` all pass
- [ ] `cargo clippy` reports zero warnings

## Common Pitfalls

1. **Forgetting `#[repr(C)]` on GPU types** — `Color` and any vertex struct must be `#[repr(C)]` and derive `bytemuck::Pod` + `Zeroable` for safe GPU buffer uploads.

2. **sRGB vs Linear confusion** — Always store linear internally. Only convert at boundaries (loading textures, displaying to screen). Getting this wrong makes everything look washed out or too dark.

3. **Transform order matters** — Scale → Rotate → Translate is the standard order. `mul_transform()` must apply parent's rotation to child's scaled position.

4. **Handle<T> must not require T: Clone** — Use manual trait impls with `PhantomData` so handles work with any type, even non-Clone ones.

5. **Don't implement `Deref` on Handle** — Handles are identifiers, not smart pointers. Force explicit lookups through the allocator/storage.

## References

- [glam docs](https://docs.rs/glam/latest/glam/)
- [bytemuck docs](https://docs.rs/bytemuck/latest/bytemuck/)
- [tracing docs](https://docs.rs/tracing/latest/tracing/)
- [Generational indices explained](https://lucassardois.medium.com/generational-indices-guide-8e3c5f7fd594)
- [sRGB ↔ Linear conversion](https://en.wikipedia.org/wiki/SRGB#Transformation)
