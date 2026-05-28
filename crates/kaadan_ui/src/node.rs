use kaadan_math::{Color, Rect, Vec2};

/// UI positioning and sizing style.
#[derive(Debug, Clone)]
pub struct UiStyle {
    /// Size in logical pixels (0 = auto)
    pub width: f32,
    pub height: f32,
    /// Margin around the element
    pub margin: UiEdges,
    /// Padding inside the element
    pub padding: UiEdges,
    /// Flex direction for children
    pub direction: FlexDirection,
    /// Alignment of children along the main axis
    pub justify: JustifyContent,
    /// Alignment of children along the cross axis
    pub align: AlignItems,
}

impl Default for UiStyle {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            margin: UiEdges::ZERO,
            padding: UiEdges::ZERO,
            direction: FlexDirection::Column,
            justify: JustifyContent::Start,
            align: AlignItems::Start,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UiEdges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl UiEdges {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    Start,
    Center,
    End,
}

/// Component: a UI node with style and computed layout.
pub struct UiNode {
    pub style: UiStyle,
    /// Computed screen-space rect after layout
    pub computed_rect: Rect,
    /// Background color
    pub background: Color,
    /// Whether this node is visible
    pub visible: bool,
}

impl Default for UiNode {
    fn default() -> Self {
        Self {
            style: UiStyle::default(),
            computed_rect: Rect::new(Vec2::ZERO, Vec2::ZERO),
            background: Color::TRANSPARENT,
            visible: true,
        }
    }
}
