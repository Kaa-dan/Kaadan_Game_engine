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

/// A single shelf in the [`AtlasPacker`]: a horizontal band of fixed height.
struct Shelf {
    /// Y position of the shelf's top edge (pixels).
    y: u32,
    /// Height of the shelf (pixels).
    height: u32,
    /// X cursor: next free x position within the shelf (pixels).
    cursor_x: u32,
}

/// A simple shelf rectangle packer for placing sub-rectangles at runtime.
///
/// Rectangles are placed left-to-right on horizontal shelves; a new shelf is
/// opened on top of the previous one when the current shelves cannot fit a
/// rectangle. This is fast and good enough for runtime atlas building (e.g.
/// glyphs, generated icons), though not optimally tight.
pub struct AtlasPacker {
    width: u32,
    height: u32,
    /// Padding inserted around each inserted rectangle, in pixels.
    padding: u32,
    shelves: Vec<Shelf>,
    /// Y position of the top of the next new shelf (pixels).
    next_shelf_y: u32,
}

impl AtlasPacker {
    /// Create a packer for an atlas of `width` x `height` pixels with 1px
    /// padding around each inserted rectangle.
    pub fn new(width: u32, height: u32) -> Self {
        Self::with_padding(width, height, 1)
    }

    /// Create a packer with an explicit `padding` (in pixels) around each rect.
    pub fn with_padding(width: u32, height: u32, padding: u32) -> Self {
        Self {
            width,
            height,
            padding,
            shelves: Vec::new(),
            next_shelf_y: 0,
        }
    }

    /// Insert a `w` x `h` rectangle, returning its placed region (pixel rect +
    /// normalized UVs), or `None` if it does not fit.
    pub fn insert(&mut self, w: u32, h: u32) -> Option<AtlasRegion> {
        if w == 0 || h == 0 {
            return None;
        }
        let pad = self.padding;
        // Footprint including padding on both sides.
        let need_w = w.checked_add(pad.checked_mul(2)?)?;
        let need_h = h.checked_add(pad.checked_mul(2)?)?;
        if need_w > self.width || need_h > self.height {
            return None;
        }

        // Try to fit on an existing shelf (best-fit by remaining height waste).
        let mut best: Option<usize> = None;
        for (i, shelf) in self.shelves.iter().enumerate() {
            if shelf.height >= need_h && shelf.cursor_x + need_w <= self.width {
                let waste = shelf.height - need_h;
                let better = match best {
                    Some(b) => waste < self.shelves[b].height - need_h,
                    None => true,
                };
                if better {
                    best = Some(i);
                }
            }
        }

        let shelf_idx = match best {
            Some(i) => i,
            None => {
                // Open a new shelf if there is vertical room.
                if self.next_shelf_y + need_h > self.height {
                    return None;
                }
                self.shelves.push(Shelf {
                    y: self.next_shelf_y,
                    height: need_h,
                    cursor_x: 0,
                });
                self.next_shelf_y += need_h;
                self.shelves.len() - 1
            }
        };

        let shelf = &mut self.shelves[shelf_idx];
        let x = shelf.cursor_x + pad;
        let y = shelf.y + pad;
        shelf.cursor_x += need_w;

        Some(self.region(x, y, w, h))
    }

    /// Build an [`AtlasRegion`] for a pixel rect within this atlas.
    fn region(&self, x: u32, y: u32, w: u32, h: u32) -> AtlasRegion {
        let tw = self.width as f32;
        let th = self.height as f32;
        AtlasRegion {
            uv_min: Vec2::new(x as f32 / tw, y as f32 / th),
            uv_max: Vec2::new((x + w) as f32 / tw, (y + h) as f32 / th),
            pixel_size: Vec2::new(w as f32, h as f32),
        }
    }
}

/// Maps named sprite regions to UV coordinates within a texture.
pub struct TextureAtlas {
    pub texture_handle: Handle<Texture>,
    regions: HashMap<String, AtlasRegion>,
    packer: AtlasPacker,
}

impl TextureAtlas {
    /// Create an atlas of `width` x `height` pixels backed by `texture_handle`.
    pub fn new(texture_handle: Handle<Texture>) -> Self {
        Self::with_size(texture_handle, 0, 0)
    }

    /// Create an atlas with explicit dimensions so regions can be packed at
    /// runtime via [`TextureAtlas::pack`].
    pub fn with_size(texture_handle: Handle<Texture>, width: u32, height: u32) -> Self {
        Self {
            texture_handle,
            regions: HashMap::new(),
            packer: AtlasPacker::new(width, height),
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

    /// Pack a `w` x `h` region using the internal [`AtlasPacker`], storing it
    /// under `name`. Returns the placed region, or `None` if it does not fit.
    ///
    /// Requires the atlas to have been created with [`TextureAtlas::with_size`].
    pub fn pack(&mut self, name: impl Into<String>, w: u32, h: u32) -> Option<AtlasRegion> {
        let region = self.packer.insert(w, h)?;
        self.regions.insert(name.into(), region);
        Some(region)
    }

    pub fn get(&self, name: &str) -> Option<&AtlasRegion> {
        self.regions.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pixel rect of a region within an `atlas_size` x `atlas_size` atlas.
    fn pixel_rect(r: &AtlasRegion, atlas_size: f32) -> (f32, f32, f32, f32) {
        let x = r.uv_min.x * atlas_size;
        let y = r.uv_min.y * atlas_size;
        (x, y, r.pixel_size.x, r.pixel_size.y)
    }

    fn overlaps(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
        let (ax, ay, aw, ah) = a;
        let (bx, by, bw, bh) = b;
        ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
    }

    #[test]
    fn insert_places_within_bounds_and_no_overlap() {
        let size = 256u32;
        let mut packer = AtlasPacker::new(size, size);
        let dims = [(20, 30), (40, 10), (16, 16), (50, 50), (10, 60)];

        let mut placed: Vec<(f32, f32, f32, f32)> = Vec::new();
        for &(w, h) in &dims {
            let region = packer.insert(w, h).expect("should fit");
            let rect = pixel_rect(&region, size as f32);
            // Within bounds.
            assert!(rect.0 >= 0.0 && rect.1 >= 0.0);
            assert!(rect.0 + rect.2 <= size as f32);
            assert!(rect.1 + rect.3 <= size as f32);
            // Reported pixel size matches request.
            assert_eq!(rect.2 as u32, w);
            assert_eq!(rect.3 as u32, h);
            placed.push(rect);
        }

        // Pairwise non-overlapping.
        for i in 0..placed.len() {
            for j in (i + 1)..placed.len() {
                assert!(
                    !overlaps(placed[i], placed[j]),
                    "regions {i} and {j} overlap: {:?} vs {:?}",
                    placed[i],
                    placed[j]
                );
            }
        }
    }

    #[test]
    fn oversized_insert_returns_none() {
        let mut packer = AtlasPacker::new(64, 64);
        assert!(packer.insert(128, 8).is_none());
        assert!(packer.insert(8, 128).is_none());
        // Exactly atlas-sized fails because of padding.
        assert!(packer.insert(64, 64).is_none());
        // Zero-sized is rejected.
        assert!(packer.insert(0, 10).is_none());
    }

    #[test]
    fn texture_atlas_pack_round_trips() {
        let mut alloc = kaadan_math::HandleAllocator::<Texture>::new();
        let mut atlas = TextureAtlas::with_size(alloc.allocate(), 256, 256);
        let region = atlas.pack("icon", 32, 32).expect("fits");
        let got = atlas.get("icon").expect("stored");
        assert_eq!(got.pixel_size, region.pixel_size);
    }
}
