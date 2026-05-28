use std::collections::HashMap;

use kaadan_math::{Handle, Vec2};

use crate::texture::Texture;

/// A region within a texture atlas, in UV coordinates (0.0-1.0).
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    pub uv_min: Vec2,
    pub uv_max: Vec2,
    pub pixel_size: Vec2,
}

/// Maps named sprite regions to UV coordinates within a texture.
pub struct TextureAtlas {
    pub texture_handle: Handle<Texture>,
    regions: HashMap<String, AtlasRegion>,
}

impl TextureAtlas {
    pub fn new(texture_handle: Handle<Texture>) -> Self {
        Self {
            texture_handle,
            regions: HashMap::new(),
        }
    }

    /// Add a region defined in pixel coordinates.
    pub fn add_region(
        &mut self,
        name: impl Into<String>,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        tex_width: u32,
        tex_height: u32,
    ) {
        let tw = tex_width as f32;
        let th = tex_height as f32;
        self.regions.insert(
            name.into(),
            AtlasRegion {
                uv_min: Vec2::new(x as f32 / tw, y as f32 / th),
                uv_max: Vec2::new((x + w) as f32 / tw, (y + h) as f32 / th),
                pixel_size: Vec2::new(w as f32, h as f32),
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&AtlasRegion> {
        self.regions.get(name)
    }
}
