//! UI framework with text rendering powered by [`fontdue`].
//!
//! Provides layout, widgets, and text rasterization for in-game UI.

mod node;
mod text;
mod widgets;

pub use node::{AlignItems, FlexDirection, JustifyContent, UiEdges, UiNode, UiStyle};
pub use text::{FontAtlas, RasterizedGlyph};
pub use widgets::{UiButton, UiImage, UiProgressBar, UiText};
