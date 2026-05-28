//! GPU rendering backend powered by [`wgpu`].
//!
//! Provides 2D and 3D rendering pipelines for KaadanEngine.

mod atlas;
mod camera2d;
mod camera3d;
mod material;
mod mesh;
mod pipeline;
mod renderer;
mod sprite;
mod sprite_batch;
mod texture;
mod vertex;
mod vertex3d;

pub use atlas::{AtlasRegion, TextureAtlas};
pub use camera2d::Camera2D;
pub use camera3d::Camera3D;
pub use material::{DirectionalLight, PbrMaterial, PointLight};
pub use mesh::Mesh;
pub use pipeline::create_basic_pipeline;
pub use renderer::{FrameContext, Renderer};
pub use sprite::Sprite;
pub use sprite_batch::{DrawCall, SpriteBatch, SpriteVertex};
pub use texture::Texture;
pub use vertex::Vertex;
pub use vertex3d::Vertex3D;

/// Basic WGSL shader source for colored geometry.
pub const BASIC_SHADER: &str = include_str!("../../../assets/shaders/basic.wgsl");

/// Sprite WGSL shader source with camera + texture sampling.
pub const SPRITE_SHADER: &str = include_str!("../../../assets/shaders/sprite.wgsl");
