use std::collections::HashMap;

/// Rasterized glyph data.
pub struct RasterizedGlyph {
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub advance_x: f32,
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Font atlas for rendering text — caches rasterized glyphs.
pub struct FontAtlas {
    font: fontdue::Font,
    cache: HashMap<(char, u32), RasterizedGlyph>,
}

impl FontAtlas {
    pub fn from_bytes(font_data: &[u8]) -> Result<Self, kaadan_core::KaadanError> {
        let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default())
            .map_err(|e| kaadan_core::KaadanError::Other(e.to_owned()))?;
        Ok(Self {
            font,
            cache: HashMap::new(),
        })
    }

    /// Rasterize a glyph at the given font size. Cached after first call.
    pub fn rasterize(&mut self, ch: char, size: f32) -> &RasterizedGlyph {
        let key = (ch, (size * 10.0) as u32); // quantize size
        self.cache.entry(key).or_insert_with(|| {
            let (metrics, bitmap) = self.font.rasterize(ch, size);
            RasterizedGlyph {
                bitmap,
                width: metrics.width as u32,
                height: metrics.height as u32,
                advance_x: metrics.advance_width,
                offset_x: metrics.xmin as f32,
                offset_y: metrics.ymin as f32,
            }
        })
    }

    /// Measure text width at the given font size.
    pub fn measure_text(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|ch| {
                let (metrics, _) = self.font.rasterize(ch, size);
                metrics.advance_width
            })
            .sum()
    }
}
