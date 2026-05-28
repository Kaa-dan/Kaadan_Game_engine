use kaadan_math::{Mat4, Vec2, Vec3};

/// 2D orthographic camera.
pub struct Camera2D {
    pub position: Vec2,
    pub zoom: f32,
    pub viewport_size: Vec2,
}

impl Camera2D {
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            viewport_size: Vec2::new(viewport_width, viewport_height),
        }
    }

    /// Orthographic projection matrix.
    /// Origin at center, Y-up, zoom affects visible area.
    pub fn projection_matrix(&self) -> Mat4 {
        let half_w = self.viewport_size.x / (2.0 * self.zoom);
        let half_h = self.viewport_size.y / (2.0 * self.zoom);
        Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, -1000.0, 1000.0)
    }

    /// View matrix (inverse camera transform).
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(
            Vec3::new(self.position.x, self.position.y, 1.0),
            Vec3::new(self.position.x, self.position.y, 0.0),
            Vec3::Y,
        )
    }

    /// Combined view-projection matrix.
    pub fn view_projection(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// Convert screen coordinates to world coordinates.
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        let ndc_x = (screen_pos.x / self.viewport_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / self.viewport_size.y) * 2.0;
        Vec2::new(
            ndc_x * self.viewport_size.x / (2.0 * self.zoom) + self.position.x,
            ndc_y * self.viewport_size.y / (2.0 * self.zoom) + self.position.y,
        )
    }
}
