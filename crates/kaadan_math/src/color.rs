use bytemuck::{Pod, Zeroable};

/// Linear RGBA color. GPU-ready. Alpha is NOT premultiplied at storage;
/// premultiply in the shader or batch step.
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Create from linear RGBA values (0.0-1.0)
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create from sRGB values (0.0-1.0). Converts to linear for GPU use.
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

/// sRGB -> Linear conversion (gamma decoding)
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
