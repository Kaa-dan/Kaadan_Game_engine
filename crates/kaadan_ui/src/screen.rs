use kaadan_math::Vec2;

use crate::node::UiEdges;

/// Resource describing the UI viewport in screen pixels, plus safe-area insets
/// (notch / home indicator on mobile). The layout system reads this each frame.
pub struct UiScreen {
    pub size: Vec2,
    pub safe_area: UiEdges,
}

impl UiScreen {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
            safe_area: UiEdges::ZERO,
        }
    }
}

impl Default for UiScreen {
    fn default() -> Self {
        Self::new(800.0, 600.0)
    }
}
