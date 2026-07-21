use kaadan_math::Color;

/// Component: text display.
pub struct UiText {
    pub text: String,
    pub font_size: f32,
    pub color: Color,
}

impl UiText {
    pub fn new(text: impl Into<String>, font_size: f32) -> Self {
        Self {
            text: text.into(),
            font_size,
            color: Color::WHITE,
        }
    }
}

/// Component: clickable button.
pub struct UiButton {
    pub label: String,
    pub pressed: bool,
    pub hovered: bool,
    /// True for the single frame the pointer is released over the button.
    pub clicked: bool,
}

impl UiButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            pressed: false,
            hovered: false,
            clicked: false,
        }
    }
}

/// Component: image display.
pub struct UiImage {
    pub texture_handle: kaadan_math::Handle<()>,
}

/// Component: progress bar.
pub struct UiProgressBar {
    pub progress: f32,
    pub fill_color: Color,
    pub background_color: Color,
}

impl UiProgressBar {
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            fill_color: Color::GREEN,
            background_color: Color::new(0.2, 0.2, 0.2, 1.0),
        }
    }
}
