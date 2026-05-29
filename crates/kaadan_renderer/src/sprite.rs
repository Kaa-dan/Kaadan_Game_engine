use kaadan_math::{Color, Handle, Vec2};

use crate::atlas::AtlasRegion;
use crate::texture::Texture;

/// Component: a 2D sprite attached to an entity.
#[derive(Clone)]
pub struct Sprite {
    /// Handle to the texture (or atlas texture)
    pub texture: Handle<Texture>,
    /// UV region within the texture (full texture if None)
    pub region: Option<AtlasRegion>,
    /// Tint color (multiplied with texture)
    pub color: Color,
    /// Sprite size in world units (if None, uses texture pixel size)
    pub size: Option<Vec2>,
    /// Anchor/pivot point (0,0 = bottom-left, 0.5,0.5 = center)
    pub anchor: Vec2,
    /// Draw order — higher values render on top
    pub z_order: i32,
    /// Flip horizontally
    pub flip_x: bool,
    /// Flip vertically
    pub flip_y: bool,
}

impl Sprite {
    pub fn new(texture: Handle<Texture>) -> Self {
        Self {
            texture,
            region: None,
            color: Color::WHITE,
            size: None,
            anchor: Vec2::new(0.5, 0.5),
            z_order: 0,
            flip_x: false,
            flip_y: false,
        }
    }
}
