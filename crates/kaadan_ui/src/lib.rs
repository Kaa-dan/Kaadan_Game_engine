//! UI framework with text rendering powered by [`fontdue`].
//!
//! Provides layout, widgets, and text rasterization for in-game UI.

mod bitmap_font;
mod interaction;
mod layout;
mod node;
mod screen;
mod text;
mod widgets;

pub use bitmap_font::{
    build_atlas_rgba, glyph_uv, layout_text, GlyphQuad, ATLAS_COLS, ATLAS_H, ATLAS_ROWS, ATLAS_W,
    FIRST_GLYPH, GLYPH_COUNT, GLYPH_H, GLYPH_W, LAST_GLYPH,
};

pub use interaction::ui_interaction_system;
pub use layout::ui_layout_system;
pub use node::{
    AlignItems, FlexDirection, InteractionState, JustifyContent, UiEdges, UiNode, UiStyle,
};
pub use screen::UiScreen;
pub use text::{FontAtlas, RasterizedGlyph};
pub use widgets::{UiButton, UiImage, UiProgressBar, UiText};
