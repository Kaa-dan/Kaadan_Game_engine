//! GPU rendering backend powered by [`wgpu`].
//!
//! Provides 2D and 3D rendering pipelines for KaadanEngine.

mod atlas;
mod camera2d;
mod camera3d;
mod material;
mod mesh;
mod mesh3d;
mod pbr_renderer;
mod pipeline;
mod renderer;
mod sprite;
mod sprite_batch;
mod sprite_renderer;
mod texture;
mod vertex;
mod vertex3d;

#[cfg(feature = "gltf")]
mod gltf_loader;

pub use atlas::{AtlasRegion, TextureAtlas};
pub use camera2d::Camera2D;
pub use camera3d::Camera3D;
pub use material::{DirectionalLight, PbrMaterial, PointLight};
pub use mesh::Mesh;
pub use mesh3d::{create_cube_mesh, Mesh3D, Mesh3DGpu};
pub use pbr_renderer::PbrRenderer;
pub use pipeline::{
    create_basic_pipeline, create_pbr_pipeline, create_sprite_pipeline,
    sprite_camera_bind_group_layout, sprite_texture_bind_group_layout,
};
pub use renderer::{FrameContext, Renderer};
pub use sprite::Sprite;
pub use sprite_batch::{DrawCall, SpriteBatch, SpriteVertex};
pub use sprite_renderer::SpriteRenderer;
pub use texture::Texture;
pub use vertex::Vertex;
pub use vertex3d::Vertex3D;

#[cfg(feature = "gltf")]
pub use gltf_loader::{load_gltf, GltfModel, LoadedMesh};

/// Basic WGSL shader source for colored geometry.
pub const BASIC_SHADER: &str = include_str!("../../../assets/shaders/basic.wgsl");

/// Sprite WGSL shader source with camera + texture sampling.
pub const SPRITE_SHADER: &str = include_str!("../../../assets/shaders/sprite.wgsl");

/// PBR WGSL shader source for lit 3D meshes.
pub const PBR_SHADER: &str = include_str!("../../../assets/shaders/pbr.wgsl");
