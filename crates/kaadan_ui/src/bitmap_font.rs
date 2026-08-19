//! Built-in 8x8 bitmap font — the engine's default, always-available font.
//!
//! Text is a Tier-0 requirement for any shippable game (scores, menus, buttons),
//! but a scalable TrueType path ([`crate::text::FontAtlas`], via `fontdue`)
//! needs the caller to supply a `.ttf`. This module ships a self-contained
//! fallback so text works out of the box with **no external asset**: a compact
//! 8x8 pixel font covering printable ASCII.
//!
//! Glyphs are authored as human-readable pixel art (`.` = empty, any other
//! non-space char = filled) so the source *is* the font, then expanded once at
//! startup into a white-on-transparent RGBA8 atlas that the UI renderer samples
//! with per-glyph UVs (the UI shader multiplies texture × vertex color, so a
//! white glyph tints to any text color).

/// Width of one glyph cell, in pixels.
pub const GLYPH_W: u32 = 8;
/// Height of one glyph cell, in pixels.
pub const GLYPH_H: u32 = 8;

/// First printable ASCII codepoint in the atlas (space).
pub const FIRST_GLYPH: u8 = 0x20;
/// Last printable ASCII codepoint in the atlas (`~`).
pub const LAST_GLYPH: u8 = 0x7E;
/// Number of glyph cells (0x20..=0x7E).
pub const GLYPH_COUNT: usize = (LAST_GLYPH - FIRST_GLYPH + 1) as usize; // 95

/// Atlas grid: 16 columns wide. 95 glyphs → 6 rows (last row partly unused).
pub const ATLAS_COLS: u32 = 16;
/// Atlas grid rows needed to hold every glyph.
pub const ATLAS_ROWS: u32 = 6;
/// Atlas texture width in pixels.
pub const ATLAS_W: u32 = ATLAS_COLS * GLYPH_W; // 128
/// Atlas texture height in pixels.
pub const ATLAS_H: u32 = ATLAS_ROWS * GLYPH_H; // 48

/// Pixel art for the glyphs we draw. Any codepoint in [`FIRST_GLYPH`,
/// [`LAST_GLYPH`]] not listed here renders blank (space). Each entry is 8 rows
/// of up to 8 columns; `.` (and spaces) are empty, anything else is a lit pixel.
#[rustfmt::skip]
const GLYPH_ART: &[(char, [&str; 8])] = &[
    ('!', [
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "........",
        "..##....",
        "........",
    ]),
    ('"', [
        ".##.##..",
        ".##.##..",
        ".##.##..",
        "........",
        "........",
        "........",
        "........",
        "........",
    ]),
    ('#', [
        ".##.##..",
        ".##.##..",
        "#######.",
        ".##.##..",
        "#######.",
        ".##.##..",
        ".##.##..",
        "........",
    ]),
    ('%', [
        "##...#..",
        "##..#...",
        "...#....",
        "..#.....",
        ".#...##.",
        "#...##..",
        "....##..",
        "........",
    ]),
    ('&', [
        ".###....",
        "##.##...",
        "##.##...",
        ".###....",
        "##.##.#.",
        "##..##..",
        ".###.#..",
        "........",
    ]),
    ('\'', [
        "..##....",
        "..##....",
        "..##....",
        "........",
        "........",
        "........",
        "........",
        "........",
    ]),
    ('(', [
        "...##...",
        "..##....",
        ".##.....",
        ".##.....",
        ".##.....",
        "..##....",
        "...##...",
        "........",
    ]),
    (')', [
        "..##....",
        "...##...",
        "....##..",
        "....##..",
        "....##..",
        "...##...",
        "..##....",
        "........",
    ]),
    ('*', [
        "........",
        "..#.#...",
        "..###...",
        ".#####..",
        "..###...",
        "..#.#...",
        "........",
        "........",
    ]),
    ('+', [
        "........",
        "..##....",
        "..##....",
        "######..",
        "..##....",
        "..##....",
        "........",
        "........",
    ]),
    (',', [
        "........",
        "........",
        "........",
        "........",
        "........",
        "..##....",
        "..##....",
        ".##.....",
    ]),
    ('-', [
        "........",
        "........",
        "........",
        "######..",
        "........",
        "........",
        "........",
        "........",
    ]),
    ('.', [
        "........",
        "........",
        "........",
        "........",
        "........",
        "..##....",
        "..##....",
        "........",
    ]),
    ('/', [
        "....##..",
        "....##..",
        "...##...",
        "..##....",
        ".##.....",
        "##......",
        "##......",
        "........",
    ]),
    ('0', [
        ".####...",
        "##..##..",
        "##.###..",
        "##.###..",
        "###.##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('1', [
        "..##....",
        ".###....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "######..",
        "........",
    ]),
    ('2', [
        ".####...",
        "##..##..",
        "....##..",
        "...##...",
        "..##....",
        ".##.....",
        "######..",
        "........",
    ]),
    ('3', [
        "#####...",
        "....##..",
        "...##...",
        "..###...",
        "....##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('4', [
        "...###..",
        "..####..",
        ".##.##..",
        "##..##..",
        "#######.",
        "....##..",
        "....##..",
        "........",
    ]),
    ('5', [
        "######..",
        "##......",
        "#####...",
        "....##..",
        "....##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('6', [
        "..###...",
        ".##.....",
        "##......",
        "#####...",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('7', [
        "######..",
        "....##..",
        "...##...",
        "..##....",
        ".##.....",
        ".##.....",
        ".##.....",
        "........",
    ]),
    ('8', [
        ".####...",
        "##..##..",
        "##..##..",
        ".####...",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('9', [
        ".####...",
        "##..##..",
        "##..##..",
        ".#####..",
        "....##..",
        "...##...",
        ".###....",
        "........",
    ]),
    (':', [
        "........",
        "..##....",
        "..##....",
        "........",
        "..##....",
        "..##....",
        "........",
        "........",
    ]),
    (';', [
        "........",
        "..##....",
        "..##....",
        "........",
        "..##....",
        "..##....",
        ".##.....",
        "........",
    ]),
    ('<', [
        "...##...",
        "..##....",
        ".##.....",
        "##......",
        ".##.....",
        "..##....",
        "...##...",
        "........",
    ]),
    ('=', [
        "........",
        "........",
        "######..",
        "........",
        "######..",
        "........",
        "........",
        "........",
    ]),
    ('>', [
        ".##.....",
        "..##....",
        "...##...",
        "....##..",
        "...##...",
        "..##....",
        ".##.....",
        "........",
    ]),
    ('?', [
        ".####...",
        "##..##..",
        "....##..",
        "...##...",
        "..##....",
        "........",
        "..##....",
        "........",
    ]),
    ('@', [
        ".####...",
        "##..##..",
        "##.###..",
        "##.###..",
        "##.###..",
        "##......",
        ".#####..",
        "........",
    ]),
    ('A', [
        "..##....",
        ".####...",
        "##..##..",
        "##..##..",
        "######..",
        "##..##..",
        "##..##..",
        "........",
    ]),
    ('B', [
        "#####...",
        "##..##..",
        "##..##..",
        "#####...",
        "##..##..",
        "##..##..",
        "#####...",
        "........",
    ]),
    ('C', [
        ".####...",
        "##..##..",
        "##......",
        "##......",
        "##......",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('D', [
        "####....",
        "##.##...",
        "##..##..",
        "##..##..",
        "##..##..",
        "##.##...",
        "####....",
        "........",
    ]),
    ('E', [
        "######..",
        "##......",
        "##......",
        "#####...",
        "##......",
        "##......",
        "######..",
        "........",
    ]),
    ('F', [
        "######..",
        "##......",
        "##......",
        "#####...",
        "##......",
        "##......",
        "##......",
        "........",
    ]),
    ('G', [
        ".####...",
        "##..##..",
        "##......",
        "##.###..",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('H', [
        "##..##..",
        "##..##..",
        "##..##..",
        "######..",
        "##..##..",
        "##..##..",
        "##..##..",
        "........",
    ]),
    ('I', [
        "######..",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "######..",
        "........",
    ]),
    ('J', [
        "..####..",
        "....##..",
        "....##..",
        "....##..",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('K', [
        "##..##..",
        "##.##...",
        "####....",
        "###.....",
        "####....",
        "##.##...",
        "##..##..",
        "........",
    ]),
    ('L', [
        "##......",
        "##......",
        "##......",
        "##......",
        "##......",
        "##......",
        "######..",
        "........",
    ]),
    ('M', [
        "##...##.",
        "###.###.",
        "#######.",
        "##.#.##.",
        "##...##.",
        "##...##.",
        "##...##.",
        "........",
    ]),
    ('N', [
        "##..##..",
        "###.##..",
        "####.#..",
        "##.###..",
        "##..##..",
        "##..##..",
        "##..##..",
        "........",
    ]),
    ('O', [
        ".####...",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('P', [
        "#####...",
        "##..##..",
        "##..##..",
        "#####...",
        "##......",
        "##......",
        "##......",
        "........",
    ]),
    ('Q', [
        ".####...",
        "##..##..",
        "##..##..",
        "##..##..",
        "##.###..",
        "##.##...",
        ".####.#.",
        "........",
    ]),
    ('R', [
        "#####...",
        "##..##..",
        "##..##..",
        "#####...",
        "####....",
        "##.##...",
        "##..##..",
        "........",
    ]),
    ('S', [
        ".#####..",
        "##......",
        "##......",
        ".####...",
        "....##..",
        "....##..",
        "#####...",
        "........",
    ]),
    ('T', [
        "######..",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "........",
    ]),
    ('U', [
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('V', [
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        ".####...",
        "..##....",
        "........",
    ]),
    ('W', [
        "##...##.",
        "##...##.",
        "##...##.",
        "##.#.##.",
        "#######.",
        "###.###.",
        "##...##.",
        "........",
    ]),
    ('X', [
        "##..##..",
        "##..##..",
        ".####...",
        "..##....",
        ".####...",
        "##..##..",
        "##..##..",
        "........",
    ]),
    ('Y', [
        "##..##..",
        "##..##..",
        ".####...",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "........",
    ]),
    ('Z', [
        "######..",
        "....##..",
        "...##...",
        "..##....",
        ".##.....",
        "##......",
        "######..",
        "........",
    ]),
    ('[', [
        ".####...",
        ".##.....",
        ".##.....",
        ".##.....",
        ".##.....",
        ".##.....",
        ".####...",
        "........",
    ]),
    ('\\', [
        "##......",
        "##......",
        ".##.....",
        "..##....",
        "...##...",
        "....##..",
        "....##..",
        "........",
    ]),
    (']', [
        ".####...",
        "...##...",
        "...##...",
        "...##...",
        "...##...",
        "...##...",
        ".####...",
        "........",
    ]),
    ('^', [
        "..##....",
        ".####...",
        "##..##..",
        "........",
        "........",
        "........",
        "........",
        "........",
    ]),
    ('_', [
        "........",
        "........",
        "........",
        "........",
        "........",
        "........",
        "........",
        "######..",
    ]),
    ('a', [
        "........",
        "........",
        ".####...",
        "....##..",
        ".#####..",
        "##..##..",
        ".#####..",
        "........",
    ]),
    ('b', [
        "##......",
        "##......",
        "#####...",
        "##..##..",
        "##..##..",
        "##..##..",
        "#####...",
        "........",
    ]),
    ('c', [
        "........",
        "........",
        ".#####..",
        "##......",
        "##......",
        "##......",
        ".#####..",
        "........",
    ]),
    ('d', [
        "....##..",
        "....##..",
        ".#####..",
        "##..##..",
        "##..##..",
        "##..##..",
        ".#####..",
        "........",
    ]),
    ('e', [
        "........",
        "........",
        ".####...",
        "##..##..",
        "######..",
        "##......",
        ".#####..",
        "........",
    ]),
    ('f', [
        "..###...",
        ".##.##..",
        ".##.....",
        "####....",
        ".##.....",
        ".##.....",
        ".##.....",
        "........",
    ]),
    ('g', [
        "........",
        "........",
        ".#####..",
        "##..##..",
        "##..##..",
        ".#####..",
        "....##..",
        ".####...",
    ]),
    ('h', [
        "##......",
        "##......",
        "#####...",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        "........",
    ]),
    ('i', [
        "..##....",
        "........",
        ".###....",
        "..##....",
        "..##....",
        "..##....",
        "######..",
        "........",
    ]),
    ('j', [
        "....##..",
        "........",
        "..###...",
        "....##..",
        "....##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('k', [
        "##......",
        "##......",
        "##..##..",
        "##.##...",
        "####....",
        "##.##...",
        "##..##..",
        "........",
    ]),
    ('l', [
        ".###....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "######..",
        "........",
    ]),
    ('m', [
        "........",
        "........",
        "###.##..",
        "#######.",
        "##.#.##.",
        "##.#.##.",
        "##...##.",
        "........",
    ]),
    ('n', [
        "........",
        "........",
        "#####...",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        "........",
    ]),
    ('o', [
        "........",
        "........",
        ".####...",
        "##..##..",
        "##..##..",
        "##..##..",
        ".####...",
        "........",
    ]),
    ('p', [
        "........",
        "........",
        "#####...",
        "##..##..",
        "##..##..",
        "#####...",
        "##......",
        "##......",
    ]),
    ('q', [
        "........",
        "........",
        ".#####..",
        "##..##..",
        "##..##..",
        ".#####..",
        "....##..",
        "....##..",
    ]),
    ('r', [
        "........",
        "........",
        "##.###..",
        "###.##..",
        "##......",
        "##......",
        "##......",
        "........",
    ]),
    ('s', [
        "........",
        "........",
        ".#####..",
        "##......",
        ".####...",
        "....##..",
        "#####...",
        "........",
    ]),
    ('t', [
        ".##.....",
        ".##.....",
        "####....",
        ".##.....",
        ".##.....",
        ".##.##..",
        "..###...",
        "........",
    ]),
    ('u', [
        "........",
        "........",
        "##..##..",
        "##..##..",
        "##..##..",
        "##..##..",
        ".#####..",
        "........",
    ]),
    ('v', [
        "........",
        "........",
        "##..##..",
        "##..##..",
        "##..##..",
        ".####...",
        "..##....",
        "........",
    ]),
    ('w', [
        "........",
        "........",
        "##...##.",
        "##.#.##.",
        "##.#.##.",
        "#######.",
        ".##.##..",
        "........",
    ]),
    ('x', [
        "........",
        "........",
        "##..##..",
        ".####...",
        "..##....",
        ".####...",
        "##..##..",
        "........",
    ]),
    ('y', [
        "........",
        "........",
        "##..##..",
        "##..##..",
        "##..##..",
        ".#####..",
        "....##..",
        ".####...",
    ]),
    ('z', [
        "........",
        "........",
        "######..",
        "...##...",
        "..##....",
        ".##.....",
        "######..",
        "........",
    ]),
    ('{', [
        "...###..",
        "..##....",
        "..##....",
        ".##.....",
        "..##....",
        "..##....",
        "...###..",
        "........",
    ]),
    ('|', [
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "..##....",
        "........",
    ]),
    ('}', [
        "###.....",
        "...##...",
        "...##...",
        "....##..",
        "...##...",
        "...##...",
        "###.....",
        "........",
    ]),
    ('~', [
        ".###.##.",
        "##.###..",
        "........",
        "........",
        "........",
        "........",
        "........",
        "........",
    ]),
];

/// Parse one glyph's pixel art into 8 row-bytes (bit `0x80 >> x` = column `x`).
fn art_to_rows(art: &[&str; 8]) -> [u8; 8] {
    let mut rows = [0u8; 8];
    for (y, line) in art.iter().enumerate() {
        let mut byte = 0u8;
        for (x, ch) in line.chars().take(8).enumerate() {
            if ch != '.' && ch != ' ' {
                byte |= 0x80 >> x;
            }
        }
        rows[y] = byte;
    }
    rows
}

/// Row bitmap for a character, or `None` if it isn't a drawable glyph (renders
/// as blank space).
pub fn glyph_rows(c: char) -> Option<[u8; 8]> {
    GLYPH_ART
        .iter()
        .find(|(gc, _)| *gc == c)
        .map(|(_, art)| art_to_rows(art))
}

/// Grid cell index (0-based) of a codepoint within the atlas, or `None` if it
/// is outside the printable range.
pub fn glyph_index(c: char) -> Option<usize> {
    let code = c as u32;
    if code >= FIRST_GLYPH as u32 && code <= LAST_GLYPH as u32 {
        Some((code - FIRST_GLYPH as u32) as usize)
    } else {
        None
    }
}

/// Normalised UV rect `(u0, v0, u1, v1)` of a glyph's cell in the atlas, or
/// `None` if `c` is outside the printable range.
pub fn glyph_uv(c: char) -> Option<(f32, f32, f32, f32)> {
    let idx = glyph_index(c)? as u32;
    let col = idx % ATLAS_COLS;
    let row = idx / ATLAS_COLS;
    let u0 = (col * GLYPH_W) as f32 / ATLAS_W as f32;
    let v0 = (row * GLYPH_H) as f32 / ATLAS_H as f32;
    let u1 = ((col + 1) * GLYPH_W) as f32 / ATLAS_W as f32;
    let v1 = ((row + 1) * GLYPH_H) as f32 / ATLAS_H as f32;
    Some((u0, v0, u1, v1))
}

/// A positioned, textured glyph ready to draw: `rect` in screen pixels (top-left
/// origin, y-down), `uv` the glyph's sub-region of the atlas, `color` the tint.
/// Mirrors the renderer's UI-quad layout so the engine can convert 1:1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphQuad {
    pub rect: kaadan_math::Rect,
    pub uv: kaadan_math::Rect,
    pub color: kaadan_math::Color,
}

/// Lay out `text` as a run of glyph quads starting at `origin` (top-left, pixels).
///
/// The built-in font is 8x8, so `font_size` is the desired glyph height in
/// pixels and the scale is `font_size / 8`. Advances horizontally by one cell
/// per character (monospace); whitespace and non-printable characters advance
/// without emitting a quad. Newlines move to the next line.
pub fn layout_text(
    text: &str,
    origin: kaadan_math::Vec2,
    font_size: f32,
    color: kaadan_math::Color,
) -> Vec<GlyphQuad> {
    use kaadan_math::{Rect, Vec2};
    let scale = font_size / GLYPH_H as f32;
    let advance = GLYPH_W as f32 * scale;
    let line_height = GLYPH_H as f32 * scale;
    let mut quads = Vec::new();
    let mut cursor = origin;
    for c in text.chars() {
        if c == '\n' {
            cursor.x = origin.x;
            cursor.y += line_height;
            continue;
        }
        // Emit a quad only for glyphs that actually have lit pixels; spaces and
        // unknown characters still advance the cursor.
        if glyph_rows(c).is_some() {
            if let Some((u0, v0, u1, v1)) = glyph_uv(c) {
                quads.push(GlyphQuad {
                    rect: Rect::new(
                        cursor,
                        Vec2::new(cursor.x + advance, cursor.y + line_height),
                    ),
                    uv: Rect::new(Vec2::new(u0, v0), Vec2::new(u1, v1)),
                    color,
                });
            }
        }
        cursor.x += advance;
    }
    quads
}

/// Build the RGBA8 glyph atlas: white (opaque) where a glyph pixel is lit,
/// transparent elsewhere. Row-major, `ATLAS_W * ATLAS_H * 4` bytes. Built once
/// at startup and uploaded as a GPU texture.
pub fn build_atlas_rgba() -> Vec<u8> {
    let mut pixels = vec![0u8; (ATLAS_W * ATLAS_H * 4) as usize];
    for code in FIRST_GLYPH..=LAST_GLYPH {
        let c = code as char;
        let Some(rows) = glyph_rows(c) else { continue };
        let idx = (code - FIRST_GLYPH) as u32;
        let cell_x = (idx % ATLAS_COLS) * GLYPH_W;
        let cell_y = (idx / ATLAS_COLS) * GLYPH_H;
        for (y, row) in rows.iter().enumerate() {
            for x in 0..GLYPH_W {
                if row & (0x80 >> x) != 0 {
                    let px = cell_x + x;
                    let py = cell_y + y as u32;
                    let offset = ((py * ATLAS_W + px) * 4) as usize;
                    pixels[offset] = 255;
                    pixels[offset + 1] = 255;
                    pixels[offset + 2] = 255;
                    pixels[offset + 3] = 255;
                }
            }
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_has_expected_size() {
        let atlas = build_atlas_rgba();
        assert_eq!(atlas.len(), (ATLAS_W * ATLAS_H * 4) as usize);
        assert_eq!(GLYPH_COUNT, 95);
    }

    #[test]
    fn printable_ascii_maps_into_grid() {
        assert_eq!(glyph_index(' '), Some(0));
        assert_eq!(glyph_index('A'), Some((b'A' - FIRST_GLYPH) as usize));
        assert_eq!(glyph_index('~'), Some(94));
        assert_eq!(glyph_index('\n'), None);
        // Every glyph's UV stays within [0, 1].
        for code in FIRST_GLYPH..=LAST_GLYPH {
            let (u0, v0, u1, v1) = glyph_uv(code as char).unwrap();
            assert!(u0 >= 0.0 && v0 >= 0.0 && u1 <= 1.0 && v1 <= 1.0);
            assert!(u1 > u0 && v1 > v0);
        }
    }

    #[test]
    fn space_is_blank_letters_are_not() {
        assert_eq!(glyph_rows(' '), None, "space has no art");
        assert!(
            glyph_rows('A').unwrap().iter().any(|&b| b != 0),
            "'A' must have lit pixels"
        );
    }

    #[test]
    fn layout_emits_one_quad_per_visible_glyph() {
        use kaadan_math::{Color, Vec2};
        // "AB C" — 3 visible glyphs (space emits none) but 4 cells of advance.
        let quads = layout_text("AB C", Vec2::ZERO, 16.0, Color::WHITE);
        assert_eq!(quads.len(), 3);
        // font_size 16 over an 8px font => scale 2 => 16px advance per cell.
        assert_eq!(quads[0].rect.min, Vec2::new(0.0, 0.0));
        assert_eq!(quads[1].rect.min.x, 16.0);
        // 'C' is the 4th cell (after the space), so x = 3 * 16.
        assert_eq!(quads[2].rect.min.x, 48.0);
        // Each glyph quad is font_size tall/wide.
        let q = quads[0].rect;
        assert_eq!(q.max.x - q.min.x, 16.0);
        assert_eq!(q.max.y - q.min.y, 16.0);
    }

    #[test]
    fn layout_wraps_on_newline() {
        use kaadan_math::{Color, Vec2};
        let quads = layout_text("A\nB", Vec2::new(10.0, 20.0), 8.0, Color::WHITE);
        assert_eq!(quads.len(), 2);
        // 'A' on line 0, 'B' on line 1 (back to origin.x, down one line height).
        assert_eq!(quads[0].rect.min, Vec2::new(10.0, 20.0));
        assert_eq!(quads[1].rect.min, Vec2::new(10.0, 28.0));
    }

    /// Renders a sample string to ASCII art so the built-in font can be
    /// eyeballed for legibility. Run with `--nocapture` to view.
    #[test]
    fn print_sample_text() {
        let sample = "Kaadan 0123 Hi!";
        let mut out = String::new();
        for y in 0..GLYPH_H as usize {
            for c in sample.chars() {
                match glyph_rows(c) {
                    Some(rows) => {
                        let row = rows[y];
                        for x in 0..GLYPH_W {
                            out.push(if row & (0x80 >> x) != 0 { '#' } else { ' ' });
                        }
                    }
                    None => out.push_str("        "),
                }
            }
            out.push('\n');
        }
        println!("\n{out}");
        assert!(!out.is_empty());
    }
}
